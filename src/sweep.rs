use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{Local, TimeDelta};
use crate::color::*;
use crate::emoji;

use crate::build_system::CleanContext;
use crate::config::Config;
use crate::fmt;
use crate::workspace;

pub fn run(cfg: &Config, path: &Path, dry_run: bool, keep_days: u64) -> Result<String> {
    fmt::banner(&emoji::label(emoji::SWEEP, "Deckhand: sweep"));
    println!("{} Sweep path: {}", emoji::e(emoji::FOLDER), path.display());
    if dry_run {
        println!(
            "{} {}",
            emoji::e(emoji::INFO),
            "[dry-run] no files will be removed".yellow()
        );
    }
    println!();

    let ctx = CleanContext {
        dry_run,
        keep_days,
        allow_native_commands: cfg.clean.allow_native_commands,
        remove_node_modules: cfg.sweep.node_modules,
        remove_venvs: false,
        remove_go_build_cache: cfg.sweep.go_build_cache,
        remove_swift_derived_data: cfg.sweep.swift_derived_data,
        ..Default::default()
    };

    // Discover projects using the configured languages.
    let ws = workspace::discover(path, &cfg.clean.languages).ok();
    let project_count = ws.as_ref().map(|ws| ws.projects.len()).unwrap_or(0);
    let mut total_freed = 0u64;
    let mut artifacts_seen = 0usize;
    let mut cache_entries_removed = 0usize;
    let mut failed = 0usize;

    if let Some(ref ws) = ws {
        for project in &ws.projects {
            let artifacts = project.system.artifacts(&project.path);
            for artifact in artifacts {
                let name = artifact
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Cargo caches are handled separately below.
                if project.system.name() == "cargo" {
                    let before = fmt::dir_size(&artifact)?;
                    sweep_dir(&artifact, keep_days, dry_run)?;
                    let after = if dry_run { before } else { fmt::dir_size(&artifact)? };
                    let freed = before.saturating_sub(after);
                    total_freed += freed;
                    artifacts_seen += 1;
                    println!(
                        "  {}{} {} → {} (freed {})",
                        emoji::s(emoji::SWEEP),
                        project.name.cyan(),
                        fmt::human_size(before),
                        fmt::human_size(after),
                        fmt::human_size(freed).green()
                    );
                    continue;
                }

                // Python bytecode sweeping respects the config flag.
                if project.system.name() == "python" && !cfg.sweep.python_bytecode {
                    continue;
                }

                // Node modules sweeping respects the config flag.
                if name == "node_modules" && !cfg.sweep.node_modules {
                    continue;
                }

                if artifact.exists() {
                    let before = fmt::dir_size(&artifact)?;
                    if let Err(e) = project.system.clean(&project.path, &ctx) {
                        failed += 1;
                        eprintln!(
                            "  {}{} cleaning {}: {}",
                            emoji::s(emoji::ERROR),
                            "error".red().bold(),
                            project.name,
                            e
                        );
                    }
                    let after = if dry_run { before } else { fmt::dir_size(&artifact).unwrap_or(0) };
                    let freed = before.saturating_sub(after);
                    total_freed += freed;
                    artifacts_seen += 1;
                    println!(
                        "  {}{} {} → {} (freed {})",
                        emoji::s(emoji::SWEEP),
                        format!("{} {}", project.name, name).cyan(),
                        fmt::human_size(before),
                        fmt::human_size(after),
                        fmt::human_size(freed).green()
                    );
                }
            }
        }
    }

    // Cargo registry cache
    if cfg.sweep.registry_cache {
        let cache = cargo_home()?.join("registry/cache");
        if cache.exists() {
            let before = fmt::dir_size(&cache)?;
            let removed = sweep_cache(&cache, keep_days, dry_run)?;
            let after = if dry_run { before } else { fmt::dir_size(&cache)? };
            cache_entries_removed += removed;
            total_freed += before.saturating_sub(after);
            println!(
                "  {} registry cache {} removed, {} → {} (freed {})",
                emoji::s(emoji::PACKAGE),
                removed,
                fmt::human_size(before),
                fmt::human_size(after),
                fmt::human_size(before.saturating_sub(after)).green()
            );
        }
    }

    // Git checkouts
    if cfg.sweep.git_checkouts {
        let git = cargo_home()?.join("git/checkouts");
        if git.exists() {
            let before = fmt::dir_size(&git)?;
            let removed = sweep_cache(&git, keep_days, dry_run)?;
            let after = if dry_run { before } else { fmt::dir_size(&git)? };
            cache_entries_removed += removed;
            total_freed += before.saturating_sub(after);
            println!(
                "  {} git checkouts {} removed, {} → {} (freed {})",
                emoji::s(emoji::PACKAGE),
                removed,
                fmt::human_size(before),
                fmt::human_size(after),
                fmt::human_size(before.saturating_sub(after)).green()
            );
        }
    }

    // NuGet cache
    if cfg.sweep.nuget_cache {
        let cache = nuget_packages()?;
        if cache.exists() {
            let before = fmt::dir_size(&cache)?;
            let removed = sweep_cache(&cache, keep_days, dry_run)?;
            let after = if dry_run { before } else { fmt::dir_size(&cache)? };
            cache_entries_removed += removed;
            total_freed += before.saturating_sub(after);
            println!(
                "  {} nuget cache {} removed, {} → {} (freed {})",
                emoji::s(emoji::PACKAGE),
                removed,
                fmt::human_size(before),
                fmt::human_size(after),
                fmt::human_size(before.saturating_sub(after)).green()
            );
        }
    }

    // Bun install cache
    if cfg.sweep.bun_cache {
        let cache = bun_install_cache()?;
        if cache.exists() {
            let before = fmt::dir_size(&cache)?;
            let removed = sweep_cache(&cache, keep_days, dry_run)?;
            let after = if dry_run { before } else { fmt::dir_size(&cache)? };
            cache_entries_removed += removed;
            total_freed += before.saturating_sub(after);
            println!(
                "  {} bun cache {} removed, {} → {} (freed {})",
                emoji::s(emoji::PACKAGE),
                removed,
                fmt::human_size(before),
                fmt::human_size(after),
                fmt::human_size(before.saturating_sub(after)).green()
            );
        }
    }

    // Maven repository cache
    if cfg.sweep.maven_repository {
        let cache = maven_repository()?;
        if cache.exists() {
            let before = fmt::dir_size(&cache)?;
            let removed = sweep_cache(&cache, keep_days, dry_run)?;
            let after = if dry_run { before } else { fmt::dir_size(&cache)? };
            cache_entries_removed += removed;
            total_freed += before.saturating_sub(after);
            println!(
                "  {} maven repository {} removed, {} → {} (freed {})",
                emoji::s(emoji::PACKAGE),
                removed,
                fmt::human_size(before),
                fmt::human_size(after),
                fmt::human_size(before.saturating_sub(after)).green()
            );
        }
    }

    let summary = if dry_run {
        format!(
            "sweep dry run inspected {} project(s) and {} artifact(s); caches older than {} day(s) would be pruned; no files removed",
            project_count, artifacts_seen, keep_days
        )
    } else {
        format!(
            "sweep inspected {} project(s) and {} artifact(s); freed {}; removed {} cache entr{} older than {} day(s)",
            project_count,
            artifacts_seen,
            fmt::human_size(total_freed),
            cache_entries_removed,
            if cache_entries_removed == 1 { "y" } else { "ies" },
            keep_days
        )
    };
    if failed > 0 {
        anyhow::bail!("{} project artifact(s) failed to sweep", failed);
    }
    Ok(summary)
}

fn sweep_dir(dir: &Path, keep_days: u64, dry_run: bool) -> Result<()> {
    let cutoff = Local::now() - TimeDelta::days(keep_days as i64);
    for entry in crate::walk::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let meta = entry.metadata()?;
        let mtime = meta.modified()?;
        let dt = chrono::DateTime::<Local>::from(mtime);
        if dt < cutoff && !dry_run {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

fn sweep_cache(dir: &Path, keep_days: u64, dry_run: bool) -> Result<usize> {
    let cutoff = Local::now() - TimeDelta::days(keep_days as i64);
    let mut removed = 0;
    for entry in crate::walk::WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() || e.file_type().is_dir())
    {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = match meta.modified() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let dt = chrono::DateTime::<Local>::from(mtime);
        if dt < cutoff {
            if !dry_run {
                if entry.file_type().is_dir() {
                    let _ = fs::remove_dir_all(entry.path());
                } else {
                    let _ = fs::remove_file(entry.path());
                }
            }
            removed += 1;
        }
    }
    Ok(removed)
}

fn cargo_home() -> Result<PathBuf> {
    if let Some(home) = env::var_os("CARGO_HOME") {
        Ok(PathBuf::from(home))
    } else {
        let home = env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(".cargo"))
    }
}

fn nuget_packages() -> Result<PathBuf> {
    if let Some(path) = env::var_os("NUGET_PACKAGES") {
        Ok(PathBuf::from(path))
    } else {
        let home = env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(".nuget/packages"))
    }
}

fn bun_install_cache() -> Result<PathBuf> {
    if let Some(path) = env::var_os("BUN_INSTALL") {
        Ok(PathBuf::from(path).join("install/cache"))
    } else {
        let home = env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(".bun/install/cache"))
    }
}

fn maven_repository() -> Result<PathBuf> {
    if let Some(path) = env::var_os("M2_REPO") {
        Ok(PathBuf::from(path))
    } else {
        let home = env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(".m2/repository"))
    }
}
