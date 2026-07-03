use crate::config::Config;
use crate::fmt;
use crate::workspace;
use anyhow::Result;
use colored::*;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct StatusReport {
    workspace_root: PathBuf,
    partitions: Vec<PartitionReport>,
    largest_artifacts: Vec<ArtifactReport>,
}

#[derive(Serialize, Clone)]
struct PartitionReport {
    name: String,
    path: PathBuf,
    size_bytes: u64,
    size_human: String,
}

#[derive(Serialize)]
struct ArtifactReport {
    path: PathBuf,
    size_bytes: u64,
    size_human: String,
}

pub fn run(cfg: &Config, json: bool, limit: Option<usize>) -> Result<()> {
    let ws = workspace::discover(&cfg.workspace.path)?;
    let mut partitions = Vec::new();

    // Workspace root target (only for real workspaces, not single-package roots)
    let is_multi_member = ws.members.len() > 1 || (ws.members.len() == 1 && ws.members[0].path != ws.root);
    if is_multi_member {
        let root_target = ws.root.join("target");
        if root_target.exists() {
            partitions.push(report_partition("workspace target", &root_target)?);
        }
    }

    // Member targets
    for member in &ws.members {
        let target = member.path.join("target");
        if target.exists() {
            partitions.push(report_partition(&format!("{} target", member.name), &target)?);
        }
    }

    // Cargo caches
    if let Ok(cargo_home) = cargo_home() {
        let registry = cargo_home.join("registry/cache");
        if registry.exists() {
            partitions.push(report_partition("registry cache", &registry)?);
        }
        let git = cargo_home.join("git/checkouts");
        if git.exists() {
            partitions.push(report_partition("git checkouts", &git)?);
        }
    }

    let mut largest = find_largest_artifacts(&ws, limit.unwrap_or(10));
    largest.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

    if json {
        let report = StatusReport {
            workspace_root: ws.root,
            partitions: partitions.clone(),
            largest_artifacts: largest
                .into_iter()
                .map(|a| ArtifactReport {
                    path: a.path,
                    size_bytes: a.size_bytes,
                    size_human: fmt::human_size(a.size_bytes),
                })
                .collect(),
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    fmt::banner("Deckhand: status");
    println!("Workspace root: {}", ws.root.display());
    println!();
    println!("{}", "Partitions:".bold());
    for p in &partitions {
        println!("  {:30} {:>10}", format!("{}", p.name.cyan()), p.size_human.bold());
    }

    println!();
    println!("{}", "Largest artifacts:".bold());
    for (i, a) in largest.iter().enumerate() {
        println!(
            "  {:2}. {:10} {}",
            i + 1,
            fmt::human_size(a.size_bytes).bold(),
            a.path.display().to_string().dimmed()
        );
    }

    Ok(())
}

fn report_partition(name: &str, path: &Path) -> Result<PartitionReport> {
    let size = fmt::dir_size(path)?;
    Ok(PartitionReport {
        name: name.to_string(),
        path: path.to_path_buf(),
        size_bytes: size,
        size_human: fmt::human_size(size),
    })
}

struct Artifact {
    path: PathBuf,
    size_bytes: u64,
}

fn find_largest_artifacts(ws: &workspace::Workspace, limit: usize) -> Vec<Artifact> {
    let mut artifacts: Vec<Artifact> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for member in &ws.members {
        let target = member.path.join("target");
        if !target.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&target)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path().to_path_buf();
            if let Ok(meta) = entry.metadata() {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen.insert(canonical) {
                    artifacts.push(Artifact {
                        path,
                        size_bytes: meta.len(),
                    });
                }
            }
        }
    }

    artifacts.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    artifacts.truncate(limit);
    artifacts
}

fn cargo_home() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        Ok(PathBuf::from(home))
    } else {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(home).join(".cargo"))
    }
}
