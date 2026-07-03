use crate::config::Config;
use crate::fmt;
use crate::workspace::{self, Member};
use anyhow::{Context, Result};
use chrono::{Local, TimeDelta};
use colored::*;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run(
    cfg: &Config,
    profile: &str,
    dry_run: bool,
    older_than: Option<u64>,
    target_dir: Option<&Path>,
) -> Result<()> {
    let ws = workspace::discover(&cfg.workspace.path)?;
    let targets = build_target_list(&ws, target_dir)?;

    fmt::banner("Deckhand: clean");
    println!("Workspace root: {}", ws.root.display());
    println!("Profile: {}", profile.bold());
    if let Some(days) = older_than {
        println!("Age filter: older than {} days", days);
    }
    if dry_run {
        println!("{}", "[dry-run] no files will be removed".yellow());
    }
    println!();

    for member in &ws.members {
        clean_member(member, profile, dry_run, older_than, target_dir)?;
    }

    if targets.iter().any(|t| t.exists()) {
        println!();
    }

    Ok(())
}

fn clean_member(
    member: &Member,
    profile: &str,
    dry_run: bool,
    older_than: Option<u64>,
    target_dir: Option<&Path>,
) -> Result<()> {
    let dir = target_dir.map(|p| member.path.join(p)).unwrap_or_else(|| member.path.join("target"));

    if !dir.exists() {
        return Ok(());
    }

    if let Some(days) = older_than {
        let cutoff = Local::now() - TimeDelta::days(days as i64);
        let removed = remove_older_than(&dir, cutoff, dry_run)?;
        let action = if dry_run { "would remove" } else { "removed" };
        println!(
            "  {} {} old artifact(s) from {}",
            action,
            removed.to_string().bold(),
            dir.display()
        );
        return Ok(());
    }

    if dry_run {
        let size = fmt::human_size(fmt::dir_size(&dir)?);
        println!(
            "  {} would clean {} {}",
            member.name.cyan(),
            dir.display(),
            format!("({})", size).dimmed()
        );
        return Ok(());
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("clean");
    cmd.arg("--manifest-path").arg(member.path.join("Cargo.toml"));
    if profile != "all" {
        cmd.arg("--profile").arg(profile);
    }
    if let Some(td) = target_dir {
        cmd.arg("--target-dir").arg(td);
    }

    let output = cmd.output().with_context(|| "failed to run cargo clean")?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo clean failed for {}:\n{}",
            member.name,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("  {} cleaned {}", member.name.green(), dir.display());
    Ok(())
}

fn build_target_list(
    ws: &workspace::Workspace,
    target_dir: Option<&Path>,
) -> Result<Vec<std::path::PathBuf>> {
    let mut targets = Vec::new();
    for member in &ws.members {
        let t = target_dir
            .map(|p| member.path.join(p))
            .unwrap_or_else(|| member.path.join("target"));
        targets.push(t);
    }
    Ok(targets)
}

fn remove_older_than(dir: &Path, cutoff: chrono::DateTime<Local>, dry_run: bool) -> Result<usize> {
    let mut removed = 0;
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let meta = entry.metadata()?;
        let mtime = meta.modified()?;
        let dt = chrono::DateTime::<Local>::from(mtime);
        if dt < cutoff {
            if dry_run {
                removed += 1;
            } else {
                fs::remove_file(entry.path())?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}
