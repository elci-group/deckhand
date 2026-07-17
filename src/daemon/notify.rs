//! Desktop-notification backends for daemon cleanup suggestions.
//!
//! The daemon talks to a [`Notifier`] trait so the confirmation flow works
//! everywhere:
//! - [`LogNotifier`] — always available; prints the suggestion and relies on
//!   `deckhand daemon confirm` (also the CI/headless path).
//! - [`NotifySendNotifier`] — zero new crates; drives the `notify-send` binary
//!   with `--wait --action=…` and parses the clicked action from its stdout.
//! - `ZbusNotifier` (cargo feature `dbus`) — native `org.freedesktop.Notifications`
//!   client via `notify-rust`; real action callbacks without external binaries.
//!
//! Selection: `notify_backend = "auto"` prefers the compiled-in D-Bus backend
//! when a session bus is reachable, then `notify-send`, then the log.

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::config::DaemonConfig;
use crate::fmt;

use super::state::Suggestion;

pub type NotificationId = u32;

/// What the user did with a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyAction {
    Review,
    CleanNow,
    Snooze,
    Cancel,
    /// Closed without choosing an action.
    Dismissed,
    /// Wait expired without any interaction.
    Timeout,
}

pub trait Notifier: Send + Sync {
    fn name(&self) -> &'static str;
    /// Show the suggestion notification (click 1 of the double-click flow).
    fn suggest(&self, suggestion: &Suggestion, fast_path: bool) -> Result<NotificationId>;
    /// Show the per-project breakdown (click 2 of the double-click flow).
    fn review(&self, suggestion: &Suggestion) -> Result<NotificationId>;
    /// Fire-and-forget informational notice (cleanup results, errors).
    fn inform(&self, summary: &str, body: &str) -> Result<()>;
    /// Block until the user acts on the notification or `timeout` elapses.
    fn wait_action(&self, id: NotificationId, timeout: Duration) -> NotifyAction;
}

fn next_id(counter: &AtomicU32) -> NotificationId {
    counter.fetch_add(1, Ordering::Relaxed).max(1)
}

fn suggest_text(s: &Suggestion) -> (String, String) {
    (
        "Deckhand: cleanup suggested".to_string(),
        format!(
            "{} reclaimable across {} project(s)",
            fmt::human_size(s.total_bytes),
            s.findings.len()
        ),
    )
}

fn review_text(s: &Suggestion) -> (String, String) {
    let mut lines: Vec<String> = s
        .findings
        .iter()
        .take(5)
        .map(|f| format!("{} — {}", f.name, fmt::human_size(f.bytes)))
        .collect();
    if s.findings.len() > 5 {
        lines.push(format!("…and {} more", s.findings.len() - 5));
    }
    (
        format!("Deckhand: {} reclaimable", fmt::human_size(s.total_bytes)),
        lines.join("\n"),
    )
}

/// Map a backend-specific action token to a [`NotifyAction`].
fn map_action(token: &str) -> NotifyAction {
    match token.trim() {
        "review" => NotifyAction::Review,
        "clean" => NotifyAction::CleanNow,
        "snooze" => NotifyAction::Snooze,
        "cancel" => NotifyAction::Cancel,
        // "default", "close", "__closed", empty, unknown → treated as dismissed.
        _ => NotifyAction::Dismissed,
    }
}

// ---------------------------------------------------------------------------
// LogNotifier
// ---------------------------------------------------------------------------

/// Always-available backend: prints notifications and immediately reports the
/// wait as timed out, leaving the pending suggestion to `daemon confirm`.
pub struct LogNotifier {
    counter: AtomicU32,
}

impl LogNotifier {
    pub fn new() -> Self {
        Self {
            counter: AtomicU32::new(1),
        }
    }
}

impl Default for LogNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for LogNotifier {
    fn name(&self) -> &'static str {
        "log"
    }

    fn suggest(&self, suggestion: &Suggestion, _fast_path: bool) -> Result<NotificationId> {
        let (summary, body) = suggest_text(suggestion);
        println!(
            "[deckhandd] {}: {} (id {}). Confirm with: deckhand daemon confirm",
            summary, body, suggestion.id
        );
        Ok(next_id(&self.counter))
    }

    fn review(&self, suggestion: &Suggestion) -> Result<NotificationId> {
        let (summary, body) = review_text(suggestion);
        println!("[deckhandd] {}:\n{}", summary, body);
        Ok(next_id(&self.counter))
    }

    fn inform(&self, summary: &str, body: &str) -> Result<()> {
        println!("[deckhandd] {}: {}", summary, body);
        Ok(())
    }

    fn wait_action(&self, _id: NotificationId, _timeout: Duration) -> NotifyAction {
        NotifyAction::Timeout
    }
}

// ---------------------------------------------------------------------------
// NotifySendNotifier
// ---------------------------------------------------------------------------

/// Drives the `notify-send` binary. Each pending notification is a blocking
/// `notify-send --wait` child process whose stdout yields the clicked action.
pub struct NotifySendNotifier {
    counter: AtomicU32,
    children: Mutex<HashMap<NotificationId, Child>>,
}

impl NotifySendNotifier {
    pub fn new() -> Self {
        Self {
            counter: AtomicU32::new(1),
            children: Mutex::new(HashMap::new()),
        }
    }

    /// Locate `notify-send` in `$PATH`.
    pub fn available() -> bool {
        find_in_path("notify-send")
    }

    fn spawn(&self, summary: &str, body: &str, actions: &[(&str, &str)]) -> Result<NotificationId> {
        let mut cmd = Command::new("notify-send");
        cmd.arg("--wait")
            .arg("--app-name=Deckhand")
            .arg("--urgency=normal");
        for (token, label) in actions {
            cmd.arg(format!("--action={}={}", token, label));
        }
        cmd.arg(summary).arg(body);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let child = cmd.spawn().context("failed to spawn notify-send")?;
        let id = next_id(&self.counter);
        self.children.lock().unwrap().insert(id, child);
        Ok(id)
    }
}

impl Default for NotifySendNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for NotifySendNotifier {
    fn name(&self) -> &'static str {
        "notify-send"
    }

    fn suggest(&self, suggestion: &Suggestion, fast_path: bool) -> Result<NotificationId> {
        let (summary, body) = suggest_text(suggestion);
        let mut actions = vec![("review", "Review"), ("snooze", "Snooze 1d")];
        if fast_path {
            actions.push(("clean", "Clean now"));
        }
        self.spawn(&summary, &body, &actions)
    }

    fn review(&self, suggestion: &Suggestion) -> Result<NotificationId> {
        let (summary, body) = review_text(suggestion);
        self.spawn(&summary, &body, &[("clean", "Clean now"), ("cancel", "Cancel")])
    }

    fn inform(&self, summary: &str, body: &str) -> Result<()> {
        let status = Command::new("notify-send")
            .arg("--app-name=Deckhand")
            .arg(summary)
            .arg(body)
            .status();
        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => bail!("notify-send exited with {}", s),
            Err(e) => Err(e).context("failed to run notify-send"),
        }
    }

    fn wait_action(&self, id: NotificationId, timeout: Duration) -> NotifyAction {
        let child = self.children.lock().unwrap().remove(&id);
        let mut child = match child {
            Some(c) => c,
            None => return NotifyAction::Dismissed,
        };

        let pid = child.id();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let r = child.wait_with_output();
            let _ = tx.send(r);
        });

        match rx.recv_timeout(timeout) {
            Ok(Ok(output)) => {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!(
                        "[deckhandd] notify-send failed ({}): {}",
                        output.status,
                        stderr.trim()
                    );
                }
                map_action(&String::from_utf8_lossy(&output.stdout))
            }
            Ok(Err(_)) => NotifyAction::Dismissed,
            Err(_) => {
                // Best-effort kill of the timed-out notification process.
                let _ = Command::new("kill").arg(pid.to_string()).status();
                NotifyAction::Timeout
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ZbusNotifier (feature "dbus")
// ---------------------------------------------------------------------------

/// Native D-Bus backend via `notify-rust`. Notification handles are stashed by
/// id; `wait_action` blocks on the server's action signal.
#[cfg(feature = "dbus")]
pub struct ZbusNotifier {
    counter: AtomicU32,
    handles: Mutex<HashMap<NotificationId, notify_rust::NotificationHandle>>,
}

#[cfg(feature = "dbus")]
impl ZbusNotifier {
    pub fn new() -> Self {
        Self {
            counter: AtomicU32::new(1),
            handles: Mutex::new(HashMap::new()),
        }
    }

    fn show(
        &self,
        summary: &str,
        body: &str,
        actions: &[(&str, &str)],
    ) -> Result<NotificationId> {
        let mut n = notify_rust::Notification::new();
        n.appname("Deckhand")
            .summary(summary)
            .body(body)
            .timeout(notify_rust::Timeout::Never);
        for (token, label) in actions {
            n.action(token, label);
        }
        let handle = n.show().context("failed to show notification over D-Bus")?;
        let id = next_id(&self.counter);
        self.handles.lock().unwrap().insert(id, handle);
        Ok(id)
    }
}

#[cfg(feature = "dbus")]
impl Notifier for ZbusNotifier {
    fn name(&self) -> &'static str {
        "dbus"
    }

    fn suggest(&self, suggestion: &Suggestion, fast_path: bool) -> Result<NotificationId> {
        let (summary, body) = suggest_text(suggestion);
        let mut actions = vec![("review", "Review"), ("snooze", "Snooze 1d")];
        if fast_path {
            actions.push(("clean", "Clean now"));
        }
        self.show(&summary, &body, &actions)
    }

    fn review(&self, suggestion: &Suggestion) -> Result<NotificationId> {
        let (summary, body) = review_text(suggestion);
        self.show(&summary, &body, &[("clean", "Clean now"), ("cancel", "Cancel")])
    }

    fn inform(&self, summary: &str, body: &str) -> Result<()> {
        notify_rust::Notification::new()
            .appname("Deckhand")
            .summary(summary)
            .body(body)
            .show()
            .context("failed to show notification over D-Bus")?;
        Ok(())
    }

    fn wait_action(&self, id: NotificationId, _timeout: Duration) -> NotifyAction {
        let handle = self.handles.lock().unwrap().remove(&id);
        let handle = match handle {
            Some(h) => h,
            None => return NotifyAction::Dismissed,
        };
        // Blocks until the user acts or the notification is closed. The
        // timeout is delegated to the notification server (we keep
        // notifications resident, so an ignored suggestion simply stays in
        // the tray until the daemon replaces or clears it).
        let mut action = NotifyAction::Dismissed;
        handle.wait_for_action(|token| {
            action = map_action(token);
        });
        action
    }
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

fn find_in_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(program);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

/// Whether a D-Bus session bus looks reachable.
fn session_bus_available() -> bool {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        return true;
    }
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|dir| std::path::Path::new(&dir).join("bus").exists())
        .unwrap_or(false)
}

/// Pick the notifier for this environment. `auto` order: compiled-in D-Bus
/// (when a session bus is reachable) → `notify-send` → log.
pub fn select(cfg: &DaemonConfig) -> std::sync::Arc<dyn Notifier> {
    match cfg.notify_backend.as_str() {
        "log" => std::sync::Arc::new(LogNotifier::new()),
        "notify-send" => {
            if NotifySendNotifier::available() {
                std::sync::Arc::new(NotifySendNotifier::new())
            } else {
                eprintln!("[deckhandd] notify-send not found in PATH; falling back to log backend");
                std::sync::Arc::new(LogNotifier::new())
            }
        }
        #[cfg(feature = "dbus")]
        "dbus" => std::sync::Arc::new(ZbusNotifier::new()),
        #[cfg(not(feature = "dbus"))]
        "dbus" => {
            eprintln!("[deckhandd] built without the dbus feature; falling back to log backend");
            std::sync::Arc::new(LogNotifier::new())
        }
        _ => {
            #[cfg(feature = "dbus")]
            if session_bus_available() {
                return std::sync::Arc::new(ZbusNotifier::new());
            }
            if NotifySendNotifier::available() {
                std::sync::Arc::new(NotifySendNotifier::new())
            } else {
                std::sync::Arc::new(LogNotifier::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample() -> Suggestion {
        Suggestion {
            id: "abc123".to_string(),
            created: Utc::now(),
            hash: "deadbeef".to_string(),
            total_bytes: 3 * 1024 * 1024 * 1024,
            findings: (0..7)
                .map(|i| super::super::state::SuggestedProject {
                    name: format!("crate-{}", i),
                    path: std::path::PathBuf::from(format!("/tmp/crate-{}", i)),
                    bytes: 512 * 1024 * 1024,
                })
                .collect(),
        }
    }

    #[test]
    fn action_tokens_map() {
        assert_eq!(map_action("review"), NotifyAction::Review);
        assert_eq!(map_action("clean"), NotifyAction::CleanNow);
        assert_eq!(map_action("snooze\n"), NotifyAction::Snooze);
        assert_eq!(map_action("cancel"), NotifyAction::Cancel);
        assert_eq!(map_action("default"), NotifyAction::Dismissed);
        assert_eq!(map_action(""), NotifyAction::Dismissed);
        assert_eq!(map_action("__closed"), NotifyAction::Dismissed);
    }

    #[test]
    fn review_body_truncates_to_five() {
        let (_, body) = review_text(&sample());
        assert!(body.contains("crate-0"));
        assert!(body.contains("crate-4"));
        assert!(!body.contains("crate-5"));
        assert!(body.contains("…and 2 more"));
    }

    #[test]
    fn log_notifier_times_out_and_increments() {
        let n = LogNotifier::new();
        let id1 = n.suggest(&sample(), false).unwrap();
        let id2 = n.review(&sample()).unwrap();
        assert_ne!(id1, id2);
        assert_eq!(
            n.wait_action(id1, Duration::from_millis(1)),
            NotifyAction::Timeout
        );
        n.inform("done", "freed 1 GB").unwrap();
    }

    #[test]
    fn auto_selection_never_panics() {
        let cfg = DaemonConfig::default();
        let backend = select(&cfg);
        let name = backend.name();
        assert!(matches!(name, "log" | "notify-send" | "dbus"));
    }
}
