//! Long-running monitoring daemon.
//!
//! The daemon watches build surfaces (Tier 0: statvfs + artifact mtimes),
//! deep-scans on cadence, debounced changes, or demand (Tier 1), and posts a
//! cleanup suggestion when thresholds are crossed (Tier 2). Deletion happens
//! only after user confirmation — the "double click" flow (notification
//! [Review] → [Clean now]) or `deckhand daemon confirm`. With
//! `[daemon] auto_clean = true` it cleans immediately after notifying
//! (unattended mode; ships disabled).
//!
//! Single instance is enforced with an `flock` on the runtime dir; `SIGUSR1`
//! requests a rescan, `SIGHUP` reloads the config, `SIGTERM`/`SIGINT` stop.

pub mod monitor;
pub mod notify;
pub mod service;
pub mod state;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chrono::Utc;

use crate::clean;
use crate::config::Config;
use crate::emoji;
use crate::fmt;
use crate::workspace;

use monitor::Decision;
use notify::{NotifyAction, Notifier};
use state::{ArtifactWatch, DaemonState, Suggestion};

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static RELOAD: AtomicBool = AtomicBool::new(false);
static RESCAN: AtomicBool = AtomicBool::new(false);

/// How often the loop wakes to check flags and timers. Sub-second granularity
/// is pointless; 1s keeps shutdown responsive at zero measurable cost.
const LOOP_TICK: Duration = Duration::from_secs(1);

#[cfg(unix)]
extern "C" fn on_signal(sig: i32) {
    match sig {
        libc::SIGTERM | libc::SIGINT => SHUTDOWN.store(true, Ordering::SeqCst),
        libc::SIGHUP => RELOAD.store(true, Ordering::SeqCst),
        libc::SIGUSR1 => RESCAN.store(true, Ordering::SeqCst),
        _ => {}
    }
}

#[cfg(unix)]
fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGUSR1, on_signal as *const () as libc::sighandler_t);
    }
}

/// flock-held single-instance guard; removes the pid file on drop.
struct InstanceLock {
    _file: std::fs::File,
    pid_path: PathBuf,
}

impl InstanceLock {
    #[cfg(unix)]
    fn acquire() -> Result<Self> {
        use std::os::unix::io::AsRawFd;
        let dir = state::runtime_dir()?;
        std::fs::create_dir_all(&dir)?;
        let lock_path = state::lock_path()?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)?;
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            bail!(
                "another deckhand daemon is already running (lock: {})",
                lock_path.display()
            );
        }
        let pid_path = state::pid_path()?;
        std::fs::write(&pid_path, std::process::id().to_string())?;
        Ok(Self {
            _file: file,
            pid_path,
        })
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.pid_path);
    }
}

// ---------------------------------------------------------------------------
// daemon run
// ---------------------------------------------------------------------------

struct Daemon {
    cfg: Config,
    cfg_path: Option<PathBuf>,
    notifier: Arc<dyn Notifier>,
    state: DaemonState,
    watches: Vec<ArtifactWatch>,
    /// Last free-space percent seen by Tier 0, for edge-triggered low-space
    /// scans (fires on crossing the floor and on each further 5-point drop,
    /// never continuously while stuck below the floor).
    last_free_pct: Option<u64>,
    tx: mpsc::Sender<(String, NotifyAction)>,
    rx: mpsc::Receiver<(String, NotifyAction)>,
}

/// Run the daemon in the foreground. This is what the systemd unit executes.
#[cfg(unix)]
pub fn run(cfg_path: Option<PathBuf>) -> Result<()> {
    let cfg = Config::load_or_default(cfg_path.clone())?;
    if !cfg.daemon.enabled {
        bail!("daemon is disabled; set [daemon].enabled = true in deckhand.toml");
    }

    let _lock = InstanceLock::acquire()?;
    install_signal_handlers();

    let notifier = notify::select(&cfg.daemon);
    let st = state::load().unwrap_or_else(|e| {
        eprintln!("[deckhandd] warning: starting with fresh state ({})", e);
        DaemonState::default()
    });
    let watches = st.watches.clone();
    let (tx, rx) = mpsc::channel();

    let mut d = Daemon {
        cfg,
        cfg_path,
        notifier,
        state: st,
        watches,
        last_free_pct: None,
        tx,
        rx,
    };
    d.startup_banner();
    d.main_loop();

    println!("[deckhandd] shutting down");
    d.save_state();
    Ok(())
}

#[cfg(not(unix))]
pub fn run(_cfg_path: Option<PathBuf>) -> Result<()> {
    bail!("the deckhand daemon is only supported on Unix platforms")
}

impl Daemon {
    fn startup_banner(&self) {
        println!(
            "[deckhandd] deckhand daemon started (pid {}, backend {}, workspace {})",
            std::process::id(),
            self.notifier.name(),
            self.cfg.workspace.path.display()
        );
        println!(
            "[deckhandd] watch {}s / scan {}s / debounce {}s; threshold {}, free floor {}%",
            self.cfg.daemon.watch_interval_secs(),
            self.cfg.daemon.scan_interval_secs(),
            self.cfg.daemon.debounce_secs(),
            fmt::human_size(self.cfg.daemon.notify_threshold_bytes()),
            self.cfg.daemon.free_percent_floor(&self.cfg.status)
        );
    }

    fn main_loop(&mut self) {
        let mut last_tier0 = Instant::now() - Duration::from_secs(60);
        let mut dirty_since: Option<Instant> = None;
        let mut next_scan = self.initial_scan_schedule();

        while !SHUTDOWN.load(Ordering::SeqCst) {
            if RELOAD.swap(false, Ordering::SeqCst) {
                self.reload();
            }
            if RESCAN.swap(false, Ordering::SeqCst) {
                println!("[deckhandd] rescan requested");
                self.deep_scan_now();
                next_scan = self.schedule_next_scan();
            }

            for (sid, action) in self.rx.try_iter().collect::<Vec<_>>() {
                self.handle_event(&sid, action);
            }

            let now = Instant::now();
            if now.duration_since(last_tier0)
                >= Duration::from_secs(self.cfg.daemon.watch_interval_secs())
            {
                last_tier0 = now;
                self.tier0_tick(&mut dirty_since);
            }

            if let Some(since) = dirty_since {
                if since.elapsed() >= Duration::from_secs(self.cfg.daemon.debounce_secs()) {
                    dirty_since = None;
                    println!("[deckhandd] artifact changes settled; rescanning");
                    self.deep_scan_now();
                    next_scan = self.schedule_next_scan();
                }
            }

            if Instant::now() >= next_scan {
                self.deep_scan_now();
                next_scan = self.schedule_next_scan();
            }

            std::thread::sleep(LOOP_TICK);
        }
    }

    /// First deep scan: 10s after start when the cache is stale or absent,
    /// otherwise aligned to the regular cadence.
    fn initial_scan_schedule(&self) -> Instant {
        let interval = Duration::from_secs(self.cfg.daemon.scan_interval_secs());
        let due = match self.state.last_scan {
            Some(t) => {
                let age = Utc::now()
                    .signed_duration_since(t)
                    .to_std()
                    .unwrap_or(Duration::ZERO);
                age >= interval
            }
            None => true,
        };
        if due {
            Instant::now() + Duration::from_secs(10)
        } else {
            Instant::now() + interval
        }
    }

    fn schedule_next_scan(&self) -> Instant {
        Instant::now() + Duration::from_secs(self.cfg.daemon.scan_interval_secs())
    }

    fn reload(&mut self) {
        match Config::load_or_default(self.cfg_path.clone()) {
            Ok(cfg) => {
                println!("[deckhandd] configuration reloaded");
                self.notifier = notify::select(&cfg.daemon);
                self.cfg = cfg;
            }
            Err(e) => eprintln!("[deckhandd] config reload failed: {}", e),
        }
    }

    /// Tier 0: cheap statvfs + mtime probes. Any change or low free space
    /// schedules a debounced deep scan; nothing is walked here.
    fn tier0_tick(&mut self, dirty_since: &mut Option<Instant>) {
        if monitor::watches_changed(&self.watches) {
            monitor::refresh_watches(&mut self.watches);
            if dirty_since.is_none() {
                println!("[deckhandd] artifact change detected; debouncing");
            }
            *dirty_since = Some(Instant::now());
        }
        if let Ok(pct) = monitor::free_percent(&self.cfg.workspace.path) {
            let floor = self.cfg.daemon.free_percent_floor(&self.cfg.status);
            let below = pct < floor;
            // Edge-triggered: fire on entering the floor band and on each
            // further 5-point drop, so a chronically-full disk does not
            // schedule a rescan every debounce window.
            let newly_below = below && self.last_free_pct.map_or(true, |prev| prev >= floor);
            let dropped_further =
                below && self.last_free_pct.map_or(false, |prev| pct.saturating_add(5) <= prev);
            if (newly_below || dropped_further) && dirty_since.is_none() {
                println!("[deckhandd] free space low ({}%); scheduling rescan", pct);
                *dirty_since = Some(Instant::now());
            }
            self.last_free_pct = Some(pct);
        }
    }

    /// Tier 1 + 2: deep scan, then evaluate and act on the decision.
    fn deep_scan_now(&mut self) {
        if !self.cfg.daemon.enabled {
            return; // disabled via SIGHUP reload; stay alive but idle
        }
        let scan = match monitor::deep_scan(&self.cfg) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[deckhandd] scan failed: {}", e);
                return;
            }
        };
        // Pick up CLI confirm/decline changes made since the last scan.
        if let Ok(fresh) = state::load() {
            self.state = fresh;
            self.watches = self.state.watches.clone();
        }
        let decision = monitor::evaluate(&self.state, &scan, &self.cfg, Utc::now());
        println!(
            "[deckhandd] scan: {} reclaimable across {} project(s), free {}% → {:?}",
            fmt::human_size(scan.total_bytes),
            scan.findings.len(),
            scan.free_percent.map(|p| p.to_string()).unwrap_or_else(|| "?".into()),
            decision
        );

        self.state.last_scan = Some(scan.scanned_at);
        self.state.watches = scan.watches.clone();
        self.watches = scan.watches.clone();

        match decision {
            Decision::Clear => {
                if self.state.suggestion.take().is_some() {
                    println!("[deckhandd] thresholds no longer met; suggestion cleared");
                }
            }
            Decision::Keep | Decision::Suppress => {}
            Decision::Notify => {
                let suggestion = monitor::build_suggestion(&scan);
                if self.cfg.daemon.auto_clean {
                    println!(
                        "[deckhandd] auto-clean: cleaning {} project(s) without confirmation",
                        suggestion.findings.len()
                    );
                    let (bytes, count) = execute_suggestion(&self.cfg, &suggestion, false);
                    self.state.suggestion = None;
                    self.inform(
                        "Deckhand: auto-clean complete",
                        &format!(
                            "Freed {} across {} project(s)",
                            fmt::human_size(bytes),
                            count
                        ),
                    );
                } else {
                    println!(
                        "[deckhandd] suggesting cleanup of {} (id {})",
                        fmt::human_size(suggestion.total_bytes),
                        suggestion.id
                    );
                    self.state.suggestion = Some(suggestion.clone());
                    self.notify_suggestion(&suggestion);
                }
            }
        }
        self.save_state();
    }

    fn notify_suggestion(&self, suggestion: &Suggestion) {
        match self.notifier.suggest(suggestion, self.cfg.daemon.fast_path) {
            Ok(nid) => self.spawn_wait(suggestion.id.clone(), nid),
            Err(e) => eprintln!("[deckhandd] notification failed: {}", e),
        }
    }

    /// Block a worker thread on the notification's action and forward it to
    /// the main loop, tagged with the suggestion id to reject stale events.
    fn spawn_wait(&self, sid: String, nid: u32) {
        let notifier = Arc::clone(&self.notifier);
        let tx = self.tx.clone();
        let timeout = Duration::from_secs(self.cfg.daemon.scan_interval_secs());
        std::thread::spawn(move || {
            let action = notifier.wait_action(nid, timeout);
            let _ = tx.send((sid, action));
        });
    }

    fn handle_event(&mut self, sid: &str, action: NotifyAction) {
        // Pick up state changes made by CLI confirm/decline since the scan.
        if let Ok(fresh) = state::load() {
            self.state = fresh;
            self.state.watches = self.watches.clone();
        }
        let suggestion = match &self.state.suggestion {
            Some(s) if s.id == sid => s.clone(),
            Some(_) => {
                println!("[deckhandd] ignoring stale notification event");
                return;
            }
            None => {
                println!("[deckhandd] suggestion already handled");
                return;
            }
        };

        match action {
            NotifyAction::Review => {
                println!("[deckhandd] review requested for suggestion {}", sid);
                match self.notifier.review(&suggestion) {
                    Ok(nid) => self.spawn_wait(sid.to_string(), nid),
                    Err(e) => eprintln!("[deckhandd] review notification failed: {}", e),
                }
            }
            NotifyAction::CleanNow => {
                println!("[deckhandd] cleanup confirmed for suggestion {}", sid);
                let (bytes, count) = execute_suggestion(&self.cfg, &suggestion, false);
                self.state.suggestion = None;
                self.save_state();
                self.inform(
                    "Deckhand: cleanup complete",
                    &format!(
                        "Freed {} across {} project(s)",
                        fmt::human_size(bytes),
                        count
                    ),
                );
                self.deep_scan_now(); // refresh watches and sizes after deletion
            }
            NotifyAction::Snooze => {
                let until = Utc::now()
                    + chrono::Duration::seconds(self.cfg.daemon.snooze_duration_secs() as i64);
                println!("[deckhandd] snoozed until {}", until);
                self.state.snoozed_until = Some(until);
                self.state.suggestion = None;
                self.save_state();
            }
            NotifyAction::Cancel | NotifyAction::Dismissed => {
                println!("[deckhandd] suggestion {} dismissed", sid);
                self.state.dismissed_hash = Some(suggestion.hash.clone());
                self.state.dismissed_total = suggestion.total_bytes;
                self.state.suggestion = None;
                self.save_state();
            }
            // Keep the pending suggestion: it stays confirmable via CLI and
            // the identical-hash rule prevents re-notification.
            NotifyAction::Timeout => {}
        }
    }

    fn inform(&self, summary: &str, body: &str) {
        if let Err(e) = self.notifier.inform(summary, body) {
            eprintln!("[deckhandd] inform notification failed: {}", e);
        }
    }

    fn save_state(&self) {
        if let Err(e) = state::save(&self.state) {
            eprintln!("[deckhandd] failed to save state: {}", e);
        }
    }
}

// ---------------------------------------------------------------------------
// Shared clean execution (daemon confirm path + CLI confirm)
// ---------------------------------------------------------------------------

/// Clean every project in a suggestion, returning (bytes freed, projects
/// cleaned). Projects are matched to freshly discovered workspaces by path;
/// each path is cleaned at most once even if watch roots overlap.
pub fn execute_suggestion(cfg: &Config, suggestion: &Suggestion, dry_run: bool) -> (u64, usize) {
    let mut roots = vec![cfg.workspace.path.clone()];
    for extra in cfg.daemon.resolved_watch_paths() {
        if !roots.contains(&extra) {
            roots.push(extra);
        }
    }

    let mut done: HashSet<PathBuf> = HashSet::new();
    let mut total = 0u64;
    let mut count = 0usize;
    for root in &roots {
        let ws = match workspace::discover(root, &cfg.clean.languages) {
            Ok(ws) => ws,
            Err(_) => continue,
        };
        for finding in &suggestion.findings {
            if done.contains(&finding.path) {
                continue;
            }
            if let Some(project) = ws.projects.iter().find(|p| p.path == finding.path) {
                match clean::clean_project(project, cfg, "all", dry_run, None, None) {
                    Ok(result) => {
                        total += result.bytes_freed;
                        count += 1;
                        done.insert(finding.path.clone());
                    }
                    Err(e) => eprintln!("[deckhandd] clean failed for {}: {}", finding.name, e),
                }
            }
        }
    }
    (total, count)
}

// ---------------------------------------------------------------------------
// CLI handlers: status / scan / confirm / decline
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn live_pid() -> Option<i32> {
    let path = state::pid_path().ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    let pid: i32 = text.trim().parse().ok()?;
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    alive.then_some(pid)
}

/// Show whether the daemon is running and what (if anything) it suggests.
pub fn status() -> Result<()> {
    let st = state::load()?;

    #[cfg(unix)]
    let running = live_pid();
    #[cfg(not(unix))]
    let running: Option<i32> = None;

    println!(
        "{} Deckhand daemon: {}",
        emoji::e(emoji::AUTO_START),
        match running {
            Some(pid) => format!("running (pid {})", pid),
            None => "not running".to_string(),
        }
    );
    match state::state_path() {
        Ok(p) => println!("{} state: {}", emoji::e(emoji::FOLDER), p.display()),
        Err(_) => {}
    }
    println!(
        "{} last scan: {}",
        emoji::e(emoji::CLOCK),
        st.last_scan
            .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "never".to_string())
    );

    if let Some(until) = st.snoozed_until {
        if until > Utc::now() {
            println!(
                "{} snoozed until {}",
                emoji::e(emoji::CLOCK),
                until.format("%Y-%m-%d %H:%M:%S UTC")
            );
        }
    }

    if let Some(s) = &st.suggestion {
        println!(
            "{} pending suggestion {}: {} across {} project(s)",
            emoji::e(emoji::WARNING),
            s.id,
            fmt::human_size(s.total_bytes),
            s.findings.len()
        );
        for f in s.findings.iter().take(5) {
            println!("    {} ({}) — {}", f.name, f.path.display(), fmt::human_size(f.bytes));
        }
        if s.findings.len() > 5 {
            println!("    …and {} more", s.findings.len() - 5);
        }
        println!(
            "{} confirm with: deckhand daemon confirm{}",
            emoji::e(emoji::INFO),
            if running.is_some() { " (or click the notification)" } else { "" }
        );
    } else {
        println!("{} no pending suggestion", emoji::e(emoji::SUCCESS));
    }
    Ok(())
}

/// Ask the running daemon to deep-scan now (SIGUSR1).
#[cfg(unix)]
pub fn scan_now() -> Result<()> {
    match live_pid() {
        Some(pid) => {
            let rc = unsafe { libc::kill(pid, libc::SIGUSR1) };
            if rc != 0 {
                bail!("failed to signal daemon (pid {})", pid);
            }
            println!(
                "{} Rescan requested from daemon (pid {})",
                emoji::e(emoji::SUCCESS),
                pid
            );
            Ok(())
        }
        None => bail!("deckhand daemon is not running"),
    }
}

#[cfg(not(unix))]
pub fn scan_now() -> Result<()> {
    bail!("deckhand daemon is not supported on this platform")
}

/// Confirm the pending suggestion and clean (the CLI half of the double-click
/// flow; equivalent to clicking [Clean now]).
pub fn confirm(cfg: &Config, id: Option<&str>, dry_run: bool) -> Result<()> {
    let mut st = state::load()?;
    let suggestion = match &st.suggestion {
        Some(s) => {
            if let Some(want) = id {
                if s.id != want {
                    bail!("pending suggestion id is {} (not {})", s.id, want);
                }
            }
            s.clone()
        }
        None => bail!("no pending suggestion; the daemon posts one when thresholds are crossed"),
    };

    println!(
        "{} Suggestion {}: {} across {} project(s)",
        emoji::e(emoji::INSPECT),
        suggestion.id,
        fmt::human_size(suggestion.total_bytes),
        suggestion.findings.len()
    );
    for f in &suggestion.findings {
        println!("    {} ({}) — {}", f.name, f.path.display(), fmt::human_size(f.bytes));
    }
    if dry_run {
        println!("{} [dry-run] nothing will be removed", emoji::e(emoji::INFO));
    }

    let (bytes, count) = execute_suggestion(cfg, &suggestion, dry_run);
    if dry_run {
        println!(
            "{} would free {} across {} project(s)",
            emoji::e(emoji::INFO),
            fmt::human_size(bytes),
            count
        );
    } else {
        println!(
            "{} freed {} across {} project(s)",
            emoji::e(emoji::SUCCESS),
            fmt::human_size(bytes),
            count
        );
        st.suggestion = None;
        state::save(&st)?;
    }
    Ok(())
}

/// Dismiss the pending suggestion, or snooze it for the configured duration.
pub fn decline(cfg: &Config, snooze: bool) -> Result<()> {
    let mut st = state::load()?;
    let suggestion = st
        .suggestion
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no pending suggestion"))?;

    if snooze {
        let until =
            Utc::now() + chrono::Duration::seconds(cfg.daemon.snooze_duration_secs() as i64);
        st.snoozed_until = Some(until);
        println!(
            "{} snoozed until {}",
            emoji::e(emoji::CLOCK),
            until.format("%Y-%m-%d %H:%M:%S UTC")
        );
    } else {
        st.dismissed_hash = Some(suggestion.hash);
        st.dismissed_total = suggestion.total_bytes;
        println!(
            "{} suggestion dismissed; it will not reappear unless it grows by 25% or changes",
            emoji::e(emoji::INFO)
        );
    }
    st.suggestion = None;
    state::save(&st)?;
    Ok(())
}
