use crate::config::Config;
use crate::fmt;
use crate::workspace;
use anyhow::{Context, Result};
use chrono::{Local, TimeDelta};
use colored::*;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run(cfg: &Config, path: &Path, dry_run: bool, keep_days: u64) -> Result<()> {
    fmt::banner("Deckhand: sweep");
    println!("Sweep path: {}", path.display());
    if dry_run {
        println!("{}", "[dry-run] no files will be removed".yellow());
    }
    println!();

    // Workspace target directories
    let ws = workspace::discover(path).ok();
    if let Some(ref ws) = ws {
        for member in &ws.members {
            let target = member.path.join("target");
            if target.exists() {
                let before = fmt::dir_size(&target)?;
                sweep_dir(&target, keep_days, dry_run)?;
                let after = if dry_run { before } else { fmt::dir_size(&target)? };
                let freed = before.saturating_sub(after);
                println!(
                    "  {} {} → {} (freed {})",
                    member.name.cyan(),
                    fmt::human_size(before),
                    fmt::human_size(after),
                    fmt::human_size(freed).green()
                );
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
            println!(
                "  registry cache {} removed, {} → {} (freed {})",
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
            println!(
                "  git checkouts {} removed, {} → {} (freed {})",
                removed,
                fmt::human_size(before),
                fmt::human_size(after),
                fmt::human_size(before.saturating_sub(after)).green()
            );
        }
    }

    Ok(())
}

fn sweep_dir(dir: &Path, keep_days: u64, dry_run: bool) -> Result<()> {
    let cutoff = Local::now() - TimeDelta::days(keep_days as i64);
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let meta = entry.metadata()?;
        let mtime = meta.modified()?;
        let dt = chrono::DateTime::<Local>::from(mtime);
        if dt < cutoff {
            if !dry_run {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}

fn sweep_cache(dir: &Path, keep_days: u64, dry_run: bool) -> Result<usize> {
    let cutoff = Local::now() - TimeDelta::days(keep_days as i64);
    let mut removed = 0;
    for entry in walkdir::WalkDir::new(dir)
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
