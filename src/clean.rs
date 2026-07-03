use std::path::Path;

use anyhow::Result;
use colored::*;

use crate::build_system::{CleanContext, CleanResult};
use crate::config::Config;
use crate::fmt;
use crate::workspace;

pub fn run(
    cfg: &Config,
    profile: &str,
    dry_run: bool,
    older_than: Option<u64>,
    target_dir: Option<&Path>,
) -> Result<()> {
    let ws = workspace::discover(&cfg.workspace.path, &cfg.clean.languages)?;

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

    let ctx = CleanContext {
        dry_run,
        keep_days: older_than.unwrap_or(cfg.clean.keep_days),
        profile: Some(profile.to_string()),
        target_dir: target_dir.map(Path::to_path_buf),
        allow_native_commands: cfg.clean.allow_native_commands,
        remove_node_modules: cfg.clean.remove_node_modules,
        remove_venvs: cfg.clean.remove_venvs,
        remove_go_build_cache: cfg.sweep.go_build_cache,
        remove_swift_derived_data: cfg.sweep.swift_derived_data,
    };

    let mut total_freed = 0u64;
    for project in &ws.projects {
        match project.system.clean(&project.path, &ctx) {
            Ok(result) => {
                total_freed += result.bytes_freed;
                print_result(project, &result, dry_run);
            }
            Err(e) => {
                eprintln!(
                    "  {} {}: {}",
                    "error".red().bold(),
                    project.name,
                    e
                );
            }
        }
    }

    if ws.projects.len() > 1 || total_freed > 0 {
        println!();
        println!(
            "Total freed: {}",
            fmt::human_size(total_freed).green().bold()
        );
    }

    Ok(())
}

fn print_result(project: &workspace::Project, result: &CleanResult, dry_run: bool) {
    let action = if dry_run { "would clean" } else { "cleaned" };
    if result.removed_dirs.is_empty() && result.bytes_freed == 0 {
        println!("  {} nothing to clean", project.name.cyan());
        return;
    }

    if result.removed_dirs.is_empty() {
        println!(
            "  {} {} ({})",
            project.name.cyan(),
            action,
            fmt::human_size(result.bytes_freed).green()
        );
    } else {
        for dir in &result.removed_dirs {
            println!(
                "  {} {} {} {}",
                project.name.cyan(),
                action,
                dir.display(),
                format!("({})", fmt::human_size(result.bytes_freed)).dimmed()
            );
        }
    }
}
