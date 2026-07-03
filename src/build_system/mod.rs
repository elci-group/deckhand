use anyhow::{Context, Result};
use chrono::{Local, TimeDelta};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::fmt;

pub mod cargo;
pub mod go;
pub mod gradle;
pub mod node;
pub mod python;
pub mod swift;

/// Context shared with every `BuildSystem::clean` call.
#[derive(Debug, Clone)]
pub struct CleanContext {
    pub dry_run: bool,
    pub keep_days: u64,
    pub profile: Option<String>,
    pub target_dir: Option<PathBuf>,
    pub allow_native_commands: bool,
    pub remove_node_modules: bool,
    pub remove_venvs: bool,
    pub remove_go_build_cache: bool,
    pub remove_swift_derived_data: bool,
}

impl Default for CleanContext {
    fn default() -> Self {
        Self {
            dry_run: false,
            keep_days: 0,
            profile: None,
            target_dir: None,
            allow_native_commands: true,
            remove_node_modules: false,
            remove_venvs: false,
            remove_go_build_cache: false,
            remove_swift_derived_data: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CleanResult {
    pub removed_dirs: Vec<PathBuf>,
    pub bytes_freed: u64,
}

#[derive(Debug, Clone)]
pub struct Partition {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
}

/// Every language/build-system that Deckhand can clean implements this trait.
pub trait BuildSystem: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    fn detect(&self, root: &Path) -> bool;
    /// Return absolute artifact paths that exist for this project root.
    fn artifacts(&self, root: &Path) -> Vec<PathBuf>;
    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult>;
    fn status_partitions(&self, root: &Path) -> Result<Vec<Partition>>;
}

/// All supported build systems, ordered by detection priority.
pub fn registry() -> Vec<Box<dyn BuildSystem>> {
    vec![
        Box::new(cargo::Cargo),
        Box::new(node::Node),
        Box::new(python::Python),
        Box::new(go::Go),
        Box::new(swift::Swift),
        Box::new(gradle::Gradle),
    ]
}

/// Return only the build systems whose names appear in `language_names`.
pub fn enabled_systems(language_names: &[String]) -> Vec<Box<dyn BuildSystem>> {
    registry()
        .into_iter()
        .filter(|s| language_names.iter().any(|n| n == s.name()))
        .collect()
}

/// Run a native command with a timeout, returning its output.
pub fn run_native(cmd: &mut Command, timeout_secs: u64) -> Result<std::process::Output> {
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn {:?}", cmd.get_program()))?;

    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let r = child.wait_with_output();
        let _ = tx.send(r);
    });

    match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => {
            // Best-effort kill of the timed-out process.
            let _ = Command::new("kill").arg(pid.to_string()).status();
            Err(anyhow::anyhow!(
                "command timed out after {} seconds",
                timeout_secs
            ))
        }
    }
}

/// Remove files older than `days` under `dir`. Returns bytes that would be/were freed.
pub fn remove_older_than(dir: &Path, days: u64, dry_run: bool) -> Result<u64> {
    if days == 0 || !dir.exists() {
        return Ok(0);
    }
    let cutoff = Local::now() - TimeDelta::days(days as i64);
    let mut freed = 0u64;
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
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
            freed += meta.len();
            if !dry_run {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(freed)
}

/// Remove a list of directories, returning the total bytes freed.
pub fn remove_dirs(dirs: &[PathBuf], dry_run: bool) -> Result<u64> {
    let mut freed = 0u64;
    for dir in dirs {
        if dir.exists() {
            freed += fmt::dir_size(dir).unwrap_or(0);
            if !dry_run {
                fs::remove_dir_all(dir)
                    .with_context(|| format!("failed to remove {}", dir.display()))?;
            }
        }
    }
    Ok(freed)
}

fn is_excluded_dir(name: &str, exclude: &[&str]) -> bool {
    exclude.contains(&name)
}

/// Collect directories matching a set of top-level names and recursive directory-name searches.
/// Directories listed in `exclude` are pruned from recursion and never returned.
pub fn collect_artifact_dirs(
    root: &Path,
    top_level: &[&str],
    recursive: &[&str],
    exclude: &[&str],
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }

    for name in top_level {
        let p = root.join(name);
        if p.exists() && !is_excluded_dir(name, exclude) {
            out.push(p);
        }
    }

    if !recursive.is_empty() {
        let walker = walkdir::WalkDir::new(root).into_iter();
        for entry in walker.filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            !is_excluded_dir(
                &e.file_name().to_string_lossy(),
                exclude,
            )
        }) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy();
            if recursive.iter().any(|r| *r == name.as_ref()) {
                out.push(entry.path().to_path_buf());
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_dirs_finds_pycache() {
        let dir = tempfile::tempdir().unwrap();
        let pycache = dir.path().join("src").join("__pycache__");
        fs::create_dir_all(&pycache).unwrap();
        let found = collect_artifact_dirs(dir.path(), &[], &["__pycache__"], &[]);
        assert!(found.contains(&pycache));
    }

    #[test]
    fn collect_dirs_skips_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let venv_pycache = dir
            .path()
            .join(".venv")
            .join("lib")
            .join("site-packages")
            .join("__pycache__");
        fs::create_dir_all(&venv_pycache).unwrap();
        let found = collect_artifact_dirs(dir.path(), &[], &["__pycache__"], &[".venv"]);
        assert!(!found.contains(&venv_pycache));
    }
}
