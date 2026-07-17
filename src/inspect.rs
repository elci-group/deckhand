//! Filesystem inspection for Rust projects.
//!
//! `deckhand inspect` walks a filesystem subtree (the user's home by default,
//! the whole disk from `/`, or an explicit path) and finds every Rust project —
//! a directory containing a `Cargo.toml`. For each project it decides whether it
//! is a candidate for cleaning (it has a non-empty `target/` build directory)
//! and, if so, how much space could be reclaimed, with a debug/release split.
//!
//! The command is strictly read-only: it never removes or modifies anything.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::color::*;
use crate::fmt;

#[derive(Debug, Clone)]
pub struct InspectOptions {
    /// Root directory to scan.
    pub scan_root: PathBuf,
    /// Hide candidates smaller than this from the text output (bytes).
    pub min_size: u64,
    /// Emit JSON instead of human-readable text.
    pub json: bool,
    /// Stay on the scan root's filesystem (used for `--scope root`).
    pub same_file_system: bool,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub name: String,
    pub path: PathBuf,
    pub target: PathBuf,
    pub candidate: bool,
    pub cleanable_bytes: u64,
    pub debug_bytes: u64,
    pub release_bytes: u64,
}

pub fn run(opts: &InspectOptions) -> Result<()> {
    let root = &opts.scan_root;
    if !root.is_dir() {
        anyhow::bail!("scan root {} is not a directory", root.display());
    }

    let mut findings = scan(root, opts.same_file_system);

    // Largest reclaimable first, then by path for stable output.
    findings.sort_by(|a, b| {
        b.cleanable_bytes
            .cmp(&a.cleanable_bytes)
            .then_with(|| a.path.cmp(&b.path))
    });

    let total_cleanable: u64 = findings.iter().map(|f| f.cleanable_bytes).sum();
    let candidates = findings.iter().filter(|f| f.candidate).count();

    if opts.json {
        return print_json(root, &findings, candidates, total_cleanable, opts.min_size);
    }

    print_text(root, &findings, candidates, total_cleanable, opts.min_size);
    Ok(())
}

/// Walk `root` and record every directory that contains a `Cargo.toml`.
///
/// Descent is iterative (a stack) so deep trees cannot overflow the call stack
/// and so individual directories can be pruned. Directory reads and metadata
/// lookups that fail (permissions, races) are skipped rather than aborting the
/// whole scan, mirroring the tolerant traversal used by `status` and `fmt`.
fn scan(root: &Path, same_file_system: bool) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

    #[cfg(unix)]
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    #[cfg(unix)]
    let root_dev = fs::metadata(root).ok().map(|m| {
        use std::os::unix::fs::MetadataExt;
        m.dev()
    });

    while let Some(dir) = stack.pop() {
        let meta = match fs::metadata(&dir) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_dir() {
            continue;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // Guard against symlink / hardlink loops.
            if !seen.insert((meta.dev(), meta.ino())) {
                continue;
            }
            // Confine `--scope root` to the filesystem it started on so we do
            // not wander into other mounts (or pseudo-filesystems).
            if same_file_system {
                if let Some(rd) = root_dev {
                    if meta.dev() != rd {
                        continue;
                    }
                }
            }
        }

        if dir.join("Cargo.toml").is_file() {
            findings.push(build_finding(&dir));
        }

        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if is_pruned(name) {
                continue;
            }
            if same_file_system && is_pseudo(name) {
                continue;
            }
            // `is_dir` follows symlinks; the inode guard above breaks loops.
            if path.is_dir() {
                stack.push(path);
            }
        }
    }

    findings
}

fn build_finding(dir: &Path) -> Finding {
    let target = dir.join("target");
    let (total, debug, release) = if target.is_dir() {
        target_sizes(&target)
    } else {
        (0, 0, 0)
    };
    let name = read_package_name(&dir.join("Cargo.toml"))
        .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "unknown".to_string());

    Finding {
        name,
        path: dir.to_path_buf(),
        target,
        candidate: total > 0,
        cleanable_bytes: total,
        debug_bytes: debug,
        release_bytes: release,
    }
}

/// Walk `target` once and return `(total, debug, release)` byte sizes, bucketing
/// each file by the first path component beneath `target`.
fn target_sizes(target: &Path) -> (u64, u64, u64) {
    let mut total = 0u64;
    let mut debug = 0u64;
    let mut release = 0u64;

    for entry in crate::walk::WalkDir::new(target)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        total += size;

        if let Ok(rel) = path.strip_prefix(target) {
            if let Some(first) = rel.components().next() {
                let s = first.as_os_str();
                if s == "debug" {
                    debug += size;
                } else if s == "release" {
                    release += size;
                }
            }
        }
    }

    (total, debug, release)
}

fn read_package_name(manifest: &Path) -> Option<String> {
    let text = fs::read_to_string(manifest).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    value
        .get("package")?
        .get("name")?
        .as_str()
        .map(|s| s.to_string())
}

/// Directory names that never hold a Rust project root worth reporting and that
/// are expensive to walk. Mirrors `.deckhandignore` plus cargo's `target/`.
fn is_pruned(name: &str) -> bool {
    matches!(name, ".git" | "node_modules" | "target")
}

/// Pseudo-/virtual filesystem top-levels that must never be descended when
/// scanning from `/`.
fn is_pseudo(name: &str) -> bool {
    matches!(name, "proc" | "sys" | "dev" | "run")
}

/// Whether a finding is surfaced in the text output given the size threshold.
fn is_reported(f: &Finding, min_size: u64) -> bool {
    f.candidate && f.cleanable_bytes >= min_size
}

fn print_text(
    root: &Path,
    findings: &[Finding],
    candidates: usize,
    total_cleanable: u64,
    min_size: u64,
) {
    fmt::banner("Deckhand: inspect");
    println!("Scan root: {}", root.display());
    println!(
        "Found {} Rust project(s); {} candidate(s) for cleaning.",
        findings.len(),
        candidates
    );
    if min_size > 0 {
        println!(
            "Showing candidates >= {} (use --min-size 0 to show all).",
            fmt::human_size(min_size)
        );
    }
    println!();

    let reported: Vec<&Finding> = findings
        .iter()
        .filter(|f| is_reported(f, min_size))
        .collect();

    if candidates == 0 {
        println!("{}", "No cleaning candidates found.".dimmed());
    } else if reported.is_empty() {
        println!(
            "{}",
            "No candidates meet the --min-size threshold.".dimmed()
        );
    } else {
        println!("{}", "Candidates (reclaimable):".bold());
        for (i, f) in reported.iter().enumerate() {
            println!(
                "  {:2}. {:>10}  {}  {}{}",
                i + 1,
                fmt::human_size(f.cleanable_bytes).green().bold(),
                f.name.cyan(),
                f.target.display().to_string().dimmed(),
                profile_breakdown(f),
            );
        }
    }

    let non_candidates = findings.len() - candidates;
    if non_candidates > 0 {
        println!();
        println!(
            "{}",
            format!(
                "{} project(s) have no build artifacts (not candidates).",
                non_candidates
            )
            .dimmed()
        );
    }

    if total_cleanable > 0 {
        println!();
        println!(
            "Total reclaimable: {}",
            fmt::human_size(total_cleanable).green().bold()
        );
    }
}

fn profile_breakdown(f: &Finding) -> String {
    let mut parts = Vec::new();
    if f.debug_bytes > 0 {
        parts.push(format!("debug {}", fmt::human_size(f.debug_bytes)));
    }
    if f.release_bytes > 0 {
        parts.push(format!("release {}", fmt::human_size(f.release_bytes)));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("  {}", format!("({})", parts.join(", ")).dimmed())
}

#[derive(Serialize)]
struct InspectReport {
    scan_root: PathBuf,
    total_projects: usize,
    candidates: usize,
    total_cleanable_bytes: u64,
    total_cleanable_human: String,
    min_size_bytes: u64,
    projects: Vec<ProjectReport>,
}

#[derive(Serialize)]
struct ProjectReport {
    name: String,
    path: PathBuf,
    target: PathBuf,
    candidate: bool,
    cleanable_bytes: u64,
    cleanable_human: String,
    debug_bytes: u64,
    release_bytes: u64,
}

fn print_json(
    root: &Path,
    findings: &[Finding],
    candidates: usize,
    total_cleanable: u64,
    min_size: u64,
) -> Result<()> {
    let report = InspectReport {
        scan_root: root.to_path_buf(),
        total_projects: findings.len(),
        candidates,
        total_cleanable_bytes: total_cleanable,
        total_cleanable_human: fmt::human_size(total_cleanable),
        min_size_bytes: min_size,
        projects: findings
            .iter()
            .map(|f| ProjectReport {
                name: f.name.clone(),
                path: f.path.clone(),
                target: f.target.clone(),
                candidate: f.candidate,
                cleanable_bytes: f.cleanable_bytes,
                cleanable_human: fmt::human_size(f.cleanable_bytes),
                debug_bytes: f.debug_bytes,
                release_bytes: f.release_bytes,
            })
            .collect(),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_manifest(dir: &Path, name: &str) {
        fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\n", name),
        )
        .unwrap();
    }

    #[test]
    fn scan_finds_projects_and_sizes() {
        let dir = crate::test_util::tempdir().unwrap();
        let root = dir.path();

        // proj-a has a populated target/ with both profiles.
        let a = root.join("proj-a");
        fs::create_dir_all(a.join("target/debug")).unwrap();
        fs::create_dir_all(a.join("target/release")).unwrap();
        fs::create_dir_all(a.join("src")).unwrap();
        write_manifest(&a, "proj-a");
        fs::write(a.join("target/debug/a"), vec![0u8; 10]).unwrap();
        fs::write(a.join("target/release/a"), vec![0u8; 20]).unwrap();

        // proj-b has a manifest but no build artifacts.
        let b = root.join("proj-b");
        fs::create_dir_all(b.join("src")).unwrap();
        write_manifest(&b, "proj-b");

        let findings = scan(root, false);
        assert_eq!(findings.len(), 2, "expected two cargo projects");

        let fa = findings.iter().find(|f| f.name == "proj-a").unwrap();
        assert!(fa.candidate);
        assert_eq!(fa.cleanable_bytes, 30);
        assert_eq!(fa.debug_bytes, 10);
        assert_eq!(fa.release_bytes, 20);
        assert_eq!(fa.target, a.join("target"));

        let fb = findings.iter().find(|f| f.name == "proj-b").unwrap();
        assert!(!fb.candidate);
        assert_eq!(fb.cleanable_bytes, 0);
    }

    #[test]
    fn scan_prunes_noise_dirs() {
        let dir = crate::test_util::tempdir().unwrap();
        let root = dir.path();

        // A real project at the top level.
        let real = root.join("real");
        fs::create_dir_all(&real).unwrap();
        write_manifest(&real, "real");

        // Phantom manifests hidden inside directories we must prune.
        let git_hidden = root.join(".git/some/crate");
        fs::create_dir_all(&git_hidden).unwrap();
        write_manifest(&git_hidden, "git-hidden");

        let nm_hidden = root.join("node_modules/dep");
        fs::create_dir_all(&nm_hidden).unwrap();
        write_manifest(&nm_hidden, "nm-hidden");

        let target_hidden = root.join("target/nested");
        fs::create_dir_all(&target_hidden).unwrap();
        write_manifest(&target_hidden, "target-hidden");

        let findings = scan(root, false);
        assert_eq!(findings.len(), 1, "only the real project should be found");
        assert_eq!(findings[0].name, "real");
    }

    #[test]
    fn is_reported_respects_min_size() {
        let mk = |bytes: u64, candidate: bool| Finding {
            name: "x".to_string(),
            path: PathBuf::from("/x"),
            target: PathBuf::from("/x/target"),
            candidate,
            cleanable_bytes: bytes,
            debug_bytes: 0,
            release_bytes: 0,
        };
        assert!(is_reported(&mk(100, true), 50));
        assert!(is_reported(&mk(100, true), 100));
        assert!(!is_reported(&mk(40, true), 50));
        // Non-candidates are never reported regardless of size.
        assert!(!is_reported(&mk(0, false), 0));
    }
}
