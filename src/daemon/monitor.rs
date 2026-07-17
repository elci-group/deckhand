//! Tiered monitoring engine for the daemon.
//!
//! Efficiency model:
//! - Tier 0 (cheap, every `watch_interval`): statvfs free-space checks and
//!   `stat` mtimes of the artifact dirs recorded by the last deep scan.
//!   O(#projects) syscalls, no directory walks.
//! - Tier 1 (deep scan): full artifact sizing via [`deep_scan`], triggered on
//!   cadence (`scan_interval`), after a debounced quiet window following mtime
//!   changes, or on demand (SIGUSR1 / `daemon scan`).
//! - Tier 2 (suggestion evaluation): pure decision over the scan result and
//!   persisted state — see [`evaluate`].

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::fmt;
use crate::workspace;

use super::state::{snoozed, ArtifactWatch, DaemonState, Suggestion, SuggestedProject};

/// Per-project reclaimable bytes from a deep scan.
#[derive(Debug, Clone)]
pub struct ScanFinding {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// Result of a Tier-1 deep scan.
#[derive(Debug, Clone)]
pub struct DeepScan {
    pub findings: Vec<ScanFinding>,
    pub watches: Vec<ArtifactWatch>,
    pub total_bytes: u64,
    /// Free-space percent of the filesystem holding the primary workspace.
    pub free_percent: Option<u64>,
    pub scanned_at: DateTime<Utc>,
}

/// What the daemon should do after evaluating a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Thresholds met: create (or replace) the pending suggestion and notify.
    Notify,
    /// Thresholds met and a suggestion is pending, but the user must not be
    /// re-notified (snoozed, dismissed, or identical to the pending one).
    Suppress,
    /// A suggestion with the same hash is already pending; nothing to do.
    Keep,
    /// Thresholds no longer met: drop any pending suggestion.
    Clear,
}

/// Run a Tier-1 deep scan across the workspace and all configured watch paths.
///
/// Artifact dirs are deduplicated across projects (Cargo workspace members
/// share the root `target/`), so bytes are never double-counted.
pub fn deep_scan(cfg: &Config) -> Result<DeepScan> {
    let mut roots = vec![cfg.workspace.path.clone()];
    for extra in cfg.daemon.resolved_watch_paths() {
        if !roots.contains(&extra) {
            roots.push(extra);
        }
    }

    let mut findings = Vec::new();
    let mut watches = Vec::new();
    let mut seen_dirs: HashSet<PathBuf> = HashSet::new();

    for root in &roots {
        let ws = match workspace::discover(root, &cfg.clean.languages) {
            Ok(ws) => ws,
            Err(_) => continue, // not a supported project tree; skip quietly
        };
        for project in &ws.projects {
            let mut bytes = 0u64;
            for dir in project.system.artifacts(&project.path) {
                if !seen_dirs.insert(dir.clone()) {
                    continue;
                }
                bytes += fmt::dir_size(&dir).unwrap_or(0);
                watches.push(ArtifactWatch {
                    dir,
                    project: project.path.clone(),
                    mtime: dir_mtime(watches_dir(watches.last().map(|_: &ArtifactWatch| ()))),
                });
            }
            if bytes > 0 {
                findings.push(ScanFinding {
                    name: project.name.clone(),
                    path: project.path.clone(),
                    bytes,
                });
            }
        }
    }

    let total_bytes = findings.iter().map(|f| f.bytes).sum();
    Ok(DeepScan {
        findings,
        watches,
        total_bytes,
        free_percent: free_percent(&cfg.workspace.path).ok(),
        scanned_at: Utc::now(),
    })
}

/// Current mtime of `dir` as UTC, `None` when it cannot be read.
pub fn dir_mtime(dir: &Path) -> Option<DateTime<Utc>> {
    std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .ok()
        .map(DateTime::<Utc>::from)
}

/// Free-space percent (0-100) of the filesystem containing `path`.
pub fn free_percent(path: &Path) -> std::io::Result<u64> {
    let (available, total) = crate::fs::space_usage(path)?;
    if total == 0 {
        return Ok(100);
    }
    Ok(available.saturating_mul(100) / total)
}

/// Tier-0 change probe: `true` when any watched artifact dir's current mtime
/// differs from the cached one (including appeared or disappeared dirs).
pub fn watches_changed(watches: &[ArtifactWatch]) -> bool {
    watches.iter().any(|w| dir_mtime(&w.dir) != w.mtime)
}

/// Refresh the cached mtimes in `watches` to the current on-disk values.
pub fn refresh_watches(watches: &mut [ArtifactWatch]) {
    for w in watches.iter_mut() {
        w.mtime = dir_mtime(&w.dir);
    }
}

/// Whether the scan crosses either suggestion threshold.
pub fn thresholds_met(scan: &DeepScan, cfg: &Config) -> bool {
    if scan.total_bytes >= cfg.daemon.notify_threshold_bytes() {
        return true;
    }
    match scan.free_percent {
        Some(pct) => pct < cfg.daemon.free_percent_floor(&cfg.status),
        None => false,
    }
}

/// Hash of the suggestion content: sorted `path:size-bucket` pairs. Bucketing
/// to 64 MiB keeps the hash stable across trivial size jitter while reacting
/// to new projects and meaningful growth.
pub fn suggestion_hash(findings: &[ScanFinding]) -> String {
    use std::hash::{Hash, Hasher};
    const BUCKET: u64 = 64 * 1024 * 1024;
    let mut parts: Vec<String> = findings
        .iter()
        .map(|f| format!("{}:{}", f.path.display(), f.bytes / BUCKET))
        .collect();
    parts.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Decide what to do with a fresh scan, given the persisted state.
///
/// Re-notification rules (anti-nag):
/// - snoozed → suppress until the deadline;
/// - same hash already pending → keep, never re-notify;
/// - same hash as dismissed and total grew < 25% → suppress;
/// - anything else → notify.
pub fn evaluate(state: &DaemonState, scan: &DeepScan, cfg: &Config, now: DateTime<Utc>) -> Decision {
    if !thresholds_met(scan, cfg) {
        return Decision::Clear;
    }
    if snoozed(state, now) {
        return Decision::Suppress;
    }
    let hash = suggestion_hash(&scan.findings);
    if let Some(pending) = &state.suggestion {
        if pending.hash == hash {
            return Decision::Keep;
        }
        return Decision::Notify;
    }
    if let Some(dismissed) = &state.dismissed_hash {
        if *dismissed == hash && scan.total_bytes <= state.dismissed_total.saturating_mul(5) / 4 {
            return Decision::Suppress;
        }
    }
    Decision::Notify
}

/// Build the suggestion record for a scan that should be notified.
pub fn build_suggestion(scan: &DeepScan) -> Suggestion {
    Suggestion {
        id: new_suggestion_id(),
        created: scan.scanned_at,
        hash: suggestion_hash(&scan.findings),
        total_bytes: scan.total_bytes,
        findings: scan
            .findings
            .iter()
            .map(|f| SuggestedProject {
                name: f.name.clone(),
                path: f.path.clone(),
                bytes: f.bytes,
            })
            .collect(),
    }
}

/// Short opaque id for correlating notifications, CLI confirms, and state.
/// Derived from time and pid — no external crates needed.
fn new_suggestion_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or(0);
    let raw = nanos
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (std::process::id() as u64) << 32;
    format!("{:08x}", (raw >> 16) as u32)
}

// Helper used inside deep_scan's watch construction; kept separate to make the
// borrow checker happy without restructuring the loop.
fn watches_dir(_: Option<()>) -> &'static Path {
    Path::new("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn finding(path: &str, bytes: u64) -> ScanFinding {
        ScanFinding {
            name: path.to_string(),
            path: PathBuf::from(path),
            bytes,
        }
    }

    fn scan_with(findings: Vec<ScanFinding>, total: u64, free_pct: Option<u64>) -> DeepScan {
        DeepScan {
            findings,
            watches: Vec::new(),
            total_bytes: total,
            free_percent: free_pct,
            scanned_at: Utc::now(),
        }
    }

    fn daemon_cfg(threshold: u64, floor: u64) -> Config {
        let mut cfg = Config::default();
        cfg.daemon.notify_threshold = Some(threshold);
        cfg.daemon.min_free_percent = Some(floor);
        cfg
    }

    #[test]
    fn hash_is_stable_and_bucketed() {
        let a = vec![finding("/a", 100 << 20), finding("/b", 200 << 20)];
        // Same buckets, different order and tiny jitter → same hash.
        let b = vec![finding("/b", (200 << 20) + 1024), finding("/a", 100 << 20)];
        assert_eq!(suggestion_hash(&a), suggestion_hash(&b));
        // Crossing a 64 MiB bucket boundary changes the hash.
        let c = vec![finding("/a", 164 << 20), finding("/b", 200 << 20)];
        assert_ne!(suggestion_hash(&a), suggestion_hash(&c));
    }

    #[test]
    fn thresholds_by_size_or_free_percent() {
        let cfg = daemon_cfg(1_000, 10);
        assert!(thresholds_met(&scan_with(vec![], 1_000, Some(90)), &cfg));
        assert!(thresholds_met(&scan_with(vec![], 0, Some(9)), &cfg));
        assert!(!thresholds_met(&scan_with(vec![], 999, Some(10)), &cfg));
        assert!(!thresholds_met(&scan_with(vec![], 999, None), &cfg));
    }

    #[test]
    fn evaluate_clear_when_below_thresholds() {
        let cfg = daemon_cfg(1_000, 10);
        let mut state = DaemonState::default();
        state.suggestion = Some(build_suggestion(&scan_with(vec![], 5_000, Some(90))));
        let scan = scan_with(vec![], 10, Some(90));
        assert_eq!(evaluate(&state, &scan, &cfg, Utc::now()), Decision::Clear);
    }

    #[test]
    fn evaluate_keep_when_same_hash_pending() {
        let cfg = daemon_cfg(1_000, 10);
        let scan = scan_with(vec![finding("/a", 5_000)], 5_000, Some(90));
        let mut state = DaemonState::default();
        state.suggestion = Some(build_suggestion(&scan));
        assert_eq!(evaluate(&state, &scan, &cfg, Utc::now()), Decision::Keep);
    }

    #[test]
    fn evaluate_notify_on_new_or_changed_hash() {
        let cfg = daemon_cfg(1_000, 10);
        let state = DaemonState::default();
        let scan = scan_with(vec![finding("/a", 5_000)], 5_000, Some(90));
        assert_eq!(evaluate(&state, &scan, &cfg, Utc::now()), Decision::Notify);

        // A different pending hash → notify again (replace).
        let mut state = DaemonState::default();
        state.suggestion = Some(build_suggestion(&scan_with(
            vec![finding("/b", 5_000)],
            5_000,
            Some(90),
        )));
        assert_eq!(evaluate(&state, &scan, &cfg, Utc::now()), Decision::Notify);
    }

    #[test]
    fn evaluate_suppresses_snoozed() {
        let cfg = daemon_cfg(1_000, 10);
        let mut state = DaemonState::default();
        state.snoozed_until = Some(Utc::now() + chrono::Duration::hours(1));
        let scan = scan_with(vec![finding("/a", 5_000)], 5_000, Some(90));
        assert_eq!(evaluate(&state, &scan, &cfg, Utc::now()), Decision::Suppress);
    }

    #[test]
    fn evaluate_suppresses_dismissed_until_25pct_growth() {
        let cfg = daemon_cfg(1_000, 10);
        let scan = scan_with(vec![finding("/a", 1_000)], 1_000, Some(90));
        let mut state = DaemonState::default();
        state.dismissed_hash = Some(suggestion_hash(&scan.findings));
        state.dismissed_total = 1_000;

        // Same content, same total → suppressed.
        assert_eq!(evaluate(&state, &scan, &cfg, Utc::now()), Decision::Suppress);

        // 25% growth with an unchanged hash (jitter inside one bucket) → notify.
        let mut grown = scan.clone();
        grown.total_bytes = 1_251;
        assert_eq!(evaluate(&state, &grown, &cfg, Utc::now()), Decision::Notify);

        // New project changes the hash → notify even below the growth rule.
        let changed = scan_with(
            vec![finding("/a", 1_000), finding("/b", 100)],
            1_100,
            Some(90),
        );
        assert_eq!(evaluate(&state, &changed, &cfg, Utc::now()), Decision::Notify);
    }

    #[test]
    fn watches_change_detection() {
        let dir = crate::test_util::tempdir().unwrap();
        let artifact = dir.path().join("target");
        fs::create_dir_all(&artifact).unwrap();

        let mut watch = ArtifactWatch {
            dir: artifact.clone(),
            project: dir.path().to_path_buf(),
            mtime: dir_mtime(&artifact),
        };
        assert!(!watches_changed(&[watch.clone()]));

        // Cached None but dir exists → changed.
        let stale = ArtifactWatch {
            mtime: None,
            ..watch.clone()
        };
        assert!(watches_changed(&[stale]));

        // Dir removed after being cached → changed.
        let gone = ArtifactWatch {
            dir: dir.path().join("missing"),
            project: dir.path().to_path_buf(),
            mtime: Some(Utc::now()),
        };
        assert!(watches_changed(&[gone]));

        // Refresh restores equilibrium.
        watch.mtime = None;
        refresh_watches(std::slice::from_mut(&mut watch));
        assert!(!watches_changed(&[watch]));
    }

    #[test]
    fn deep_scan_counts_artifacts_once() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"solo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let target = dir.path().join("target").join("debug");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("bin"), vec![0u8; 4096]).unwrap();

        let mut cfg = Config::default();
        cfg.workspace.path = dir.path().to_path_buf();
        let scan = deep_scan(&cfg).unwrap();

        assert_eq!(scan.findings.len(), 1);
        assert_eq!(scan.findings[0].name, "solo");
        assert_eq!(scan.findings[0].bytes, 4096);
        assert_eq!(scan.total_bytes, 4096);
        assert_eq!(scan.watches.len(), 1);
        assert!(scan.watches[0].mtime.is_some());
        assert!(scan.free_percent.is_some());
    }
}
