//! System-wide cleaning of Cargo projects.
//!
//! `deckhand deep-clean` walks a filesystem subtree (the user's home by
//! default, the whole disk from `/`, or an explicit path) to find every Rust
//! project — reusing the same scan as `deckhand inspect` — and runs
//! `cargo clean` on each project with a non-empty `target/` directory.
//!
//! Unlike `deckhand clean`, which is scoped to the current workspace, this
//! command reaches across the whole scan root, so actual deletion is gated
//! behind `--yes`; `--dry-run` previews what would be removed.

use std::path::PathBuf;

use anyhow::Result;

use crate::build_system::{BuildSystem, CleanContext};
use crate::color::*;
use crate::config::Config;
use crate::emoji;
use crate::fmt;
use crate::inspect::{self, Finding};
use crate::spinner;

#[derive(Debug, Clone)]
pub struct DeepCleanOptions {
    /// Root directory to scan.
    pub scan_root: PathBuf,
    /// Only report what would be removed.
    pub dry_run: bool,
    /// Actually remove build artifacts. Without this the command refuses to
    /// delete anything, mirroring `update`'s confirmation pattern.
    pub yes: bool,
    /// Stay on the scan root's filesystem (used for `--scope root`).
    pub same_file_system: bool,
}

pub fn run(cfg: &Config, opts: &DeepCleanOptions) -> Result<String> {
    let root = &opts.scan_root;
    if !root.is_dir() {
        anyhow::bail!("scan root {} is not a directory", root.display());
    }

    let mut findings = spinner::spin("Scanning for Rust projects", || {
        inspect::scan(root, opts.same_file_system)
    });
    findings.retain(|f| f.candidate);
    // Largest reclaimable first, then by path for stable output.
    findings.sort_by(|a, b| {
        b.cleanable_bytes
            .cmp(&a.cleanable_bytes)
            .then_with(|| a.path.cmp(&b.path))
    });

    fmt::banner(&emoji::label(emoji::CLEAN, "Deckhand: deep-clean"));
    println!("{} Scan root: {}", emoji::e(emoji::FOLDER), root.display());
    println!(
        "{} Found {} Cargo project(s) with build artifacts.",
        emoji::e(emoji::INFO),
        findings.len()
    );
    if opts.dry_run {
        println!(
            "{} {}",
            emoji::e(emoji::INFO),
            "[dry-run] no files will be removed".yellow()
        );
    }
    println!();

    if findings.is_empty() {
        println!("{} {}", emoji::e(emoji::INFO), "Nothing to clean.".dimmed());
        return Ok("deep-clean found no cargo projects to clean".to_string());
    }

    if opts.dry_run {
        return Ok(dry_run_report(&findings));
    }

    if !opts.yes {
        println!(
            "{} Refusing to clean without --yes ({} at stake across {} project(s)).",
            emoji::e(emoji::WARNING),
            fmt::human_size(total_cleanable(&findings)).yellow().bold(),
            findings.len()
        );
        println!(
            "   Run {} to preview, or {} to clean.",
            "deckhand deep-clean --dry-run".cyan(),
            "deckhand deep-clean --yes".cyan()
        );
        return Ok(format!(
            "deep-clean refused without confirmation; {} reclaimable across {} project(s)",
            fmt::human_size(total_cleanable(&findings)),
            findings.len()
        ));
    }

    clean_all(cfg, &findings)
}

fn total_cleanable(findings: &[Finding]) -> u64 {
    findings.iter().map(|f| f.cleanable_bytes).sum()
}

fn dry_run_report(findings: &[Finding]) -> String {
    for f in findings {
        println!(
            "  {}{} {} {}",
            emoji::s(emoji::INFO),
            f.name.cyan(),
            "would clean".dimmed(),
            f.target.display(),
        );
    }
    println!();
    println!(
        "{} Total reclaimable: {} across {} project(s)",
        emoji::e(emoji::DISK),
        fmt::human_size(total_cleanable(findings)).green().bold(),
        findings.len()
    );
    format!(
        "deep-clean dry run would clean {} project(s); total reclaimable {}",
        findings.len(),
        fmt::human_size(total_cleanable(findings))
    )
}

fn clean_all(cfg: &Config, findings: &[Finding]) -> Result<String> {
    let ctx = CleanContext {
        dry_run: false,
        keep_days: 0,
        profile: Some("all".to_string()),
        target_dir: None,
        allow_native_commands: cfg.clean.allow_native_commands,
        remove_node_modules: false,
        remove_venvs: false,
        remove_go_build_cache: false,
        remove_swift_derived_data: false,
    };
    let cargo = crate::build_system::cargo::Cargo;

    let mut total_freed = 0u64;
    let mut failed = 0usize;
    for f in findings {
        match cargo.clean(&f.path, &ctx) {
            Ok(result) => {
                total_freed += result.bytes_freed;
                println!(
                    "  {}{} {} ({})",
                    emoji::s(emoji::TRASH),
                    "cleaned".dimmed(),
                    f.name.cyan(),
                    fmt::human_size(result.bytes_freed).green()
                );
            }
            Err(e) => {
                failed += 1;
                eprintln!(
                    "  {}{} {}: {}",
                    emoji::s(emoji::ERROR),
                    "error".red().bold(),
                    f.name,
                    e
                );
            }
        }
    }

    println!();
    println!(
        "{} Total freed: {}",
        emoji::e(emoji::DISK),
        fmt::human_size(total_freed).green().bold()
    );

    if failed > 0 {
        anyhow::bail!("{} of {} project(s) failed to clean", failed, findings.len());
    }
    Ok(format!(
        "deep-clean cleaned {} project(s); total freed {}",
        findings.len(),
        fmt::human_size(total_freed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn write_project(dir: &Path, name: &str, target_bytes: usize) {
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\n", name),
        )
        .unwrap();
        if target_bytes > 0 {
            fs::create_dir_all(dir.join("target/debug")).unwrap();
            fs::write(dir.join("target/debug/out"), vec![0u8; target_bytes]).unwrap();
        }
    }

    fn test_config() -> Config {
        let mut cfg = Config::default();
        // Tests must not shell out to a real `cargo` binary; force the
        // remove-dir fallback path.
        cfg.clean.allow_native_commands = false;
        cfg
    }

    #[test]
    fn dry_run_removes_nothing() {
        let dir = crate::test_util::tempdir().unwrap();
        write_project(&dir.path().join("proj-a"), "proj-a", 64);

        let summary = run(
            &test_config(),
            &DeepCleanOptions {
                scan_root: dir.path().to_path_buf(),
                dry_run: true,
                yes: false,
                same_file_system: false,
            },
        )
        .unwrap();

        assert!(summary.contains("would clean 1 project(s)"));
        assert!(dir.path().join("proj-a/target/debug/out").exists());
    }

    #[test]
    fn refuses_without_yes() {
        let dir = crate::test_util::tempdir().unwrap();
        write_project(&dir.path().join("proj-a"), "proj-a", 64);

        let summary = run(
            &test_config(),
            &DeepCleanOptions {
                scan_root: dir.path().to_path_buf(),
                dry_run: false,
                yes: false,
                same_file_system: false,
            },
        )
        .unwrap();

        assert!(summary.contains("refused"));
        assert!(dir.path().join("proj-a/target/debug/out").exists());
    }

    #[test]
    fn yes_cleans_all_projects() {
        let dir = crate::test_util::tempdir().unwrap();
        write_project(&dir.path().join("proj-a"), "proj-a", 64);
        write_project(&dir.path().join("nested/proj-b"), "proj-b", 128);
        // A project with no build artifacts is skipped entirely.
        write_project(&dir.path().join("proj-c"), "proj-c", 0);

        let summary = run(
            &test_config(),
            &DeepCleanOptions {
                scan_root: dir.path().to_path_buf(),
                dry_run: false,
                yes: true,
                same_file_system: false,
            },
        )
        .unwrap();

        assert!(summary.contains("cleaned 2 project(s)"), "{}", summary);
        assert!(!dir.path().join("proj-a/target").exists());
        assert!(!dir.path().join("nested/proj-b/target").exists());
    }

    #[test]
    fn empty_scan_root_is_not_an_error() {
        let dir = crate::test_util::tempdir().unwrap();
        let summary = run(
            &test_config(),
            &DeepCleanOptions {
                scan_root: dir.path().to_path_buf(),
                dry_run: false,
                yes: true,
                same_file_system: false,
            },
        )
        .unwrap();
        assert!(summary.contains("no cargo projects"));
    }
}
