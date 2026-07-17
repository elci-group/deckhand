//! Persisted daemon state: deep-scan cache, pending suggestion, and
//! snooze/dismiss markers.
//!
//! Lives in `$XDG_STATE_HOME/deckhand/daemon.toml` (default
//! `~/.local/state/deckhand/`), mirroring the TOML state pattern used by
//! `auto_clean`. Lock and pid files go to `$XDG_RUNTIME_DIR/deckhand/` when
//! available, falling back to the state directory.

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonState {
    #[serde(default)]
    pub last_scan: Option<DateTime<Utc>>,
    #[serde(default)]
    pub snoozed_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub dismissed_hash: Option<String>,
    #[serde(default)]
    pub dismissed_total: u64,
    #[serde(default)]
    pub suggestion: Option<Suggestion>,
    /// Artifact directories seen by the last deep scan, with their mtimes.
    /// Tier-0 watches compare against these to avoid re-walking unchanged trees.
    #[serde(default)]
    pub watches: Vec<ArtifactWatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: String,
    pub created: DateTime<Utc>,
    pub hash: String,
    pub total_bytes: u64,
    #[serde(default)]
    pub findings: Vec<SuggestedProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedProject {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactWatch {
    pub dir: PathBuf,
    pub project: PathBuf,
    #[serde(default)]
    pub mtime: Option<DateTime<Utc>>,
}

/// `true` when the suggestion is snoozed at `now`.
pub fn snoozed(state: &DaemonState, now: DateTime<Utc>) -> bool {
    state.snoozed_until.map(|until| now < until).unwrap_or(false)
}

pub fn state_dir() -> Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        return Ok(PathBuf::from(xdg).join("deckhand"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".local/state/deckhand"));
    }
    bail!("could not determine state directory; set HOME or XDG_STATE_HOME");
}

/// Directory for the lock and pid files. Prefers the (tmpfs) runtime dir.
pub fn runtime_dir() -> Result<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_RUNTIME_DIR") {
        let dir = PathBuf::from(xdg).join("deckhand");
        if dir.parent().map(|p| p.is_dir()).unwrap_or(false) {
            return Ok(dir);
        }
    }
    state_dir()
}

pub fn state_path() -> Result<PathBuf> {
    Ok(state_dir()?.join("daemon.toml"))
}

pub fn lock_path() -> Result<PathBuf> {
    Ok(runtime_dir()?.join("daemon.lock"))
}

pub fn pid_path() -> Result<PathBuf> {
    Ok(runtime_dir()?.join("daemon.pid"))
}

pub fn load() -> Result<DaemonState> {
    let path = state_path()?;
    load_from(&path)
}

pub fn load_from(path: &std::path::Path) -> Result<DaemonState> {
    if !path.exists() {
        return Ok(DaemonState::default());
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read daemon state {}", path.display()))?;
    let state: DaemonState = toml::from_str(&text)
        .with_context(|| format!("failed to parse daemon state {}", path.display()))?;
    Ok(state)
}

pub fn save(state: &DaemonState) -> Result<()> {
    let path = state_path()?;
    save_to(&path, state)
}

pub fn save_to(path: &std::path::Path, state: &DaemonState) -> Result<()> {
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create {}", dir.display()))?;
    let text =
        toml::to_string_pretty(state).context("failed to serialize daemon state")?;
    fs::write(path, text)
        .with_context(|| format!("failed to write daemon state {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_suggestion() -> Suggestion {
        Suggestion {
            id: "abc123".to_string(),
            created: Utc::now(),
            hash: "deadbeef".to_string(),
            total_bytes: 3_000_000_000,
            findings: vec![SuggestedProject {
                name: "deckhand".to_string(),
                path: PathBuf::from("/home/sal/deckhand"),
                bytes: 3_000_000_000,
            }],
        }
    }

    #[test]
    fn state_round_trip() {
        let dir = crate::test_util::tempdir().unwrap();
        let path = dir.path().join("nested").join("daemon.toml");

        let mut state = DaemonState::default();
        state.last_scan = Some(Utc::now());
        state.snoozed_until = Some(Utc::now());
        state.dismissed_hash = Some("cafe".to_string());
        state.dismissed_total = 1_000_000_000;
        state.suggestion = Some(sample_suggestion());
        state.watches.push(ArtifactWatch {
            dir: PathBuf::from("/x/target"),
            project: PathBuf::from("/x"),
            mtime: Some(Utc::now()),
        });

        save_to(&path, &state).unwrap();
        let loaded = load_from(&path).unwrap();

        assert!(loaded.last_scan.is_some());
        assert!(loaded.snoozed_until.is_some());
        assert_eq!(loaded.dismissed_hash.as_deref(), Some("cafe"));
        assert_eq!(loaded.dismissed_total, 1_000_000_000);
        let s = loaded.suggestion.unwrap();
        assert_eq!(s.id, "abc123");
        assert_eq!(s.total_bytes, 3_000_000_000);
        assert_eq!(s.findings.len(), 1);
        assert_eq!(loaded.watches.len(), 1);
        assert_eq!(loaded.watches[0].dir, PathBuf::from("/x/target"));
    }

    #[test]
    fn missing_state_file_yields_default() {
        let dir = crate::test_util::tempdir().unwrap();
        let path = dir.path().join("nope.toml");
        let state = load_from(&path).unwrap();
        assert!(state.suggestion.is_none());
        assert!(state.watches.is_empty());
    }

    #[test]
    fn snoozed_only_until_deadline() {
        let mut state = DaemonState::default();
        let now = Utc::now();
        assert!(!snoozed(&state, now));
        state.snoozed_until = Some(now + chrono::Duration::hours(1));
        assert!(snoozed(&state, now));
        state.snoozed_until = Some(now - chrono::Duration::hours(1));
        assert!(!snoozed(&state, now));
    }
}
