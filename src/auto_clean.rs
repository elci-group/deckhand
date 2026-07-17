use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use crate::color::*;
use serde::{Deserialize, Serialize};

use crate::clean;
use crate::config::{Config, ProjectOverride};
use crate::fmt;
use crate::workspace::{Project, Workspace};

const STATE_DIR: &str = ".deckhand";
const STATE_FILE: &str = "auto-clean.toml";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoCleanState {
    #[serde(default)]
    pub queue: Vec<QueuedProject>,
    #[serde(default)]
    pub last_cleaned: HashMap<PathBuf, DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedProject {
    pub path: PathBuf,
    pub matched_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct BinaryMeta {
    path: PathBuf,
    size: u64,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

pub fn run(cfg: &Config, dry_run: bool) -> Result<String> {
    let ws = crate::workspace::discover(&cfg.workspace.path, &cfg.clean.languages)?;
    let ac = &cfg.auto_clean;

    if !ac.enabled {
        println!(
            "{} auto-clean is disabled; set [auto_clean].enabled = true in deckhand.toml",
            "info:".blue().bold()
        );
        return Ok("auto-clean is disabled".to_string());
    }

    if ac.clutter_tolerance.is_none() && ac.min_free_space.is_none() {
        println!(
            "{} auto-clean: no activation thresholds configured (clutter_tolerance / min_free_space); nothing to do",
            "info:".blue().bold()
        );
        return Ok("auto-clean has no activation thresholds; nothing to do".to_string());
    }

    fmt::banner("Deckhand: auto-clean");
    println!("Workspace root: {}", ws.root.display());
    if dry_run {
        println!(
            "{}",
            "[dry-run] no files will be removed and state will not be updated"
                .yellow()
        );
    }
    println!();

    let scan_paths = ac.resolved_scan_paths();
    let matched = find_matching_projects(&ws, &scan_paths)?;

    if matched.is_empty() {
        println!("No installed binaries matched current target outputs.");
        return Ok("auto-clean found no matching installed binaries".to_string());
    }

    let mut state = load_state(&ws.root)?;
    let now = Utc::now();
    state.queue = update_queue(state.queue, matched, now);

    print_queue(&ws, &state.queue);

    if !should_activate(&ws, &state.queue, ac)? {
        if !dry_run {
            save_state(&ws.root, &state)?;
        }
        println!(
            "Activation thresholds not met; {} project(s) queued.",
            state.queue.len()
        );
        return Ok(format!(
            "auto-clean queued {} project(s); activation thresholds not met",
            state.queue.len()
        ));
    }

    println!(
        "{} activation thresholds met; processing {} candidate(s) in FIFO order",
        "→".green().bold(),
        state.queue.len()
    );
    println!();

    let cleaned = execute_clean(&ws, &state.queue, &state.last_cleaned, cfg, dry_run)?;

    let summary = if dry_run {
        println!();
        println!(
            "{} {} project(s) would be cleaned if not run with --dry-run",
            "[dry-run]".yellow(),
            cleaned.len()
        );
        format!(
            "auto-clean dry run would clean {} project(s); {} queued",
            cleaned.len(),
            state.queue.len()
        )
    } else {
        let cleaned_set: HashSet<&Path> = cleaned.iter().map(|p| p.as_path()).collect();
        state.queue.retain(|q| !cleaned_set.contains(q.path.as_path()));
        for path in &cleaned {
            state.last_cleaned.insert(path.clone(), now);
        }
        save_state(&ws.root, &state)?;

        if cleaned.is_empty() {
            println!("No projects cleaned (all candidates on cooldown).");
            "auto-clean met thresholds but cleaned no projects; all candidates on cooldown"
                .to_string()
        } else {
            println!();
            println!(
                "Cleaned {} project(s); {} still queued.",
                cleaned.len(),
                state.queue.len()
            );
            format!(
                "auto-clean cleaned {} project(s); {} still queued",
                cleaned.len(),
                state.queue.len()
            )
        }
    };

    Ok(summary)
}

fn state_path(root: &Path) -> PathBuf {
    root.join(STATE_DIR).join(STATE_FILE)
}

fn load_state(root: &Path) -> Result<AutoCleanState> {
    let path = state_path(root);
    if !path.exists() {
        return Ok(AutoCleanState::default());
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read auto-clean state {}", path.display()))?;
    let state: AutoCleanState = toml::from_str(&text)
        .with_context(|| format!("failed to parse auto-clean state {}", path.display()))?;
    Ok(state)
}

fn save_state(root: &Path, state: &AutoCleanState) -> Result<()> {
    let dir = root.join(STATE_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
    }
    let path = state_path(root);
    let text =
        toml::to_string_pretty(state).context("failed to serialize auto-clean state")?;
    fs::write(&path, text)
        .with_context(|| format!("failed to write auto-clean state {}", path.display()))?;
    Ok(())
}

fn find_matching_projects(ws: &Workspace, scan_paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let installed: Vec<BinaryMeta> = scan_paths
        .iter()
        .filter(|p| p.is_dir())
        .flat_map(|p| collect_installed_binaries(p))
        .collect();

    if installed.is_empty() {
        return Ok(Vec::new());
    }

    let mut matched = Vec::new();
    let mut seen = HashSet::new();
    for project in &ws.projects {
        let targets = project_target_binaries(ws, project);
        for target in &targets {
            if installed.iter().any(|inst| matches_installed(inst, target)) {
                if seen.insert(project.path.clone()) {
                    matched.push(project.path.clone());
                }
                break;
            }
        }
    }
    Ok(matched)
}

fn collect_installed_binaries(dir: &Path) -> Vec<BinaryMeta> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_executable(&path) {
            if let Some(meta) = binary_meta(&path) {
                out.push(meta);
            }
        }
    }
    out
}

fn project_target_binaries(ws: &Workspace, project: &Project) -> Vec<BinaryMeta> {
    let mut bins = Vec::new();
    let target_dirs = [
        project.path.join("target/debug"),
        project.path.join("target/release"),
        ws.root.join("target/debug"),
        ws.root.join("target/release"),
    ];
    for dir in target_dirs {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_executable(&path) {
                if let Some(meta) = binary_meta(&path) {
                    bins.push(meta);
                }
            }
        }
    }
    bins
}

fn binary_meta(path: &Path) -> Option<BinaryMeta> {
    let meta = fs::metadata(path).ok()?;
    Some(BinaryMeta {
        path: path.to_path_buf(),
        size: meta.len(),
        #[cfg(unix)]
        dev: std::os::unix::fs::MetadataExt::dev(&meta),
        #[cfg(unix)]
        ino: std::os::unix::fs::MetadataExt::ino(&meta),
    })
}

fn matches_installed(installed: &BinaryMeta, target: &BinaryMeta) -> bool {
    if file_name(&installed.path) != file_name(&target.path) {
        return false;
    }
    if installed.size == target.size {
        return true;
    }
    #[cfg(unix)]
    if installed.dev == target.dev && installed.ino == target.ino {
        return true;
    }
    false
}

fn file_name(path: &Path) -> Option<String> {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(path) {
        Ok(meta) => meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    // Non-Unix scan paths are treated as plain file lists.
    true
}

fn update_queue(
    mut queue: Vec<QueuedProject>,
    matched: Vec<PathBuf>,
    now: DateTime<Utc>,
) -> Vec<QueuedProject> {
    let existing: HashSet<PathBuf> = queue.iter().map(|q| q.path.clone()).collect();
    for path in matched {
        if !existing.contains(&path) {
            queue.push(QueuedProject { path, matched_at: now });
        }
    }
    queue
}

fn print_queue(ws: &Workspace, queue: &[QueuedProject]) {
    println!("{} matched project queue (FIFO):", "●".cyan());
    for (i, queued) in queue.iter().enumerate() {
        let name = ws
            .projects
            .iter()
            .find(|p| p.path == queued.path)
            .map(|p| p.name.as_str())
            .unwrap_or("unknown");
        println!(
            "  {:2}. {} ({}) matched {}",
            i + 1,
            name.cyan(),
            queued.path.display(),
            queued.matched_at.format("%Y-%m-%d %H:%M:%S UTC")
        );
    }
    println!();
}

fn should_activate(
    ws: &Workspace,
    queue: &[QueuedProject],
    cfg: &crate::config::AutoCleanConfig,
) -> Result<bool> {
    if let Some(tolerance) = cfg.clutter_tolerance {
        let clutter: u64 = queue
            .iter()
            .filter_map(|q| ws.projects.iter().find(|p| p.path == q.path))
            .map(total_artifact_size)
            .sum();
        if clutter > tolerance {
            println!(
                "Clutter tolerance exceeded: {} > {}",
                fmt::human_size(clutter).bold(),
                fmt::human_size(tolerance).bold()
            );
            return Ok(true);
        }
    }

    if let Some(min_free) = cfg.min_free_space {
        let free = free_space(&ws.root)?;
        if free < min_free {
            println!(
                "Free space below limit: {} < {}",
                fmt::human_size(free).bold(),
                fmt::human_size(min_free).bold()
            );
            return Ok(true);
        }
    }

    Ok(false)
}

fn total_artifact_size(project: &Project) -> u64 {
    project
        .system
        .artifacts(&project.path)
        .iter()
        .map(|p| fmt::dir_size(p).unwrap_or(0))
        .sum()
}

fn free_space(path: &Path) -> Result<u64> {
    crate::fs::available_space(path)
        .with_context(|| format!("failed to query free space for {}", path.display()))
}

fn execute_clean(
    ws: &Workspace,
    queue: &[QueuedProject],
    last_cleaned: &HashMap<PathBuf, DateTime<Utc>>,
    cfg: &Config,
    dry_run: bool,
) -> Result<Vec<PathBuf>> {
    let mut cleaned = Vec::new();
    let now = Utc::now();

    for queued in queue {
        let project = match ws.projects.iter().find(|p| p.path == queued.path) {
            Some(p) => p,
            None => continue,
        };

        let cooldown = cooldown_for(&project.path, &project.name, &cfg.auto_clean);
        if let Some(&last) = last_cleaned.get(&project.path) {
            let elapsed = now.signed_duration_since(last);
            if elapsed < cooldown {
                let remaining = cooldown - elapsed;
                println!(
                    "  {} {} on cooldown ({} remaining)",
                    project.name.cyan(),
                    "skipped".yellow(),
                    format_duration(remaining)
                );
                continue;
            }
        }

        match clean::clean_project(project, cfg, "all", dry_run, None, None) {
            Ok(result) => {
                cleaned.push(project.path.clone());
                print_clean_result(project, &result, dry_run);
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

    Ok(cleaned)
}

fn cooldown_for(
    _path: &Path,
    project_name: &str,
    cfg: &crate::config::AutoCleanConfig,
) -> ChronoDuration {
    let secs = cfg
        .projects
        .get(project_name)
        .and_then(|p: &ProjectOverride| p.cooldown)
        .or(cfg.cooldown)
        .unwrap_or(0);
    ChronoDuration::seconds(secs as i64)
}

fn format_duration(d: ChronoDuration) -> String {
    let secs = d.num_seconds();
    if secs < 60 {
        return format!("{}s", secs);
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{}m", mins);
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{}h", hours);
    }
    format!("{}d", hours / 24)
}

fn print_clean_result(project: &Project, result: &crate::build_system::CleanResult, dry_run: bool) {
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
            let size = fmt::dir_size(dir).unwrap_or(0);
            println!(
                "  {} {} {} {}",
                project.name.cyan(),
                action,
                dir.display(),
                format!("({})", fmt::human_size(size)).dimmed()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_queue_appends_new_matches_fifo() {
        let p1 = PathBuf::from("/a");
        let p2 = PathBuf::from("/b");
        let p3 = PathBuf::from("/c");
        let now = Utc::now();
        let queue = vec![QueuedProject {
            path: p1.clone(),
            matched_at: now,
        }];
        let matched = vec![p2.clone(), p1.clone(), p3.clone()];
        let updated = update_queue(queue, matched, now);
        assert_eq!(updated.len(), 3);
        assert_eq!(updated[0].path, p1);
        assert_eq!(updated[1].path, p2);
        assert_eq!(updated[2].path, p3);
    }

    #[test]
    fn cooldown_for_uses_project_override() {
        let mut cfg = crate::config::AutoCleanConfig {
            cooldown: Some(3600),
            ..Default::default()
        };
        cfg.projects.insert(
            "special".to_string(),
            ProjectOverride { cooldown: Some(60) },
        );
        assert_eq!(cooldown_for(Path::new("/x"), "special", &cfg).num_seconds(), 60);
        assert_eq!(cooldown_for(Path::new("/x"), "other", &cfg).num_seconds(), 3600);
    }

    #[test]
    fn cooldown_for_falls_back_to_global_then_zero() {
        let cfg = crate::config::AutoCleanConfig::default();
        assert_eq!(cooldown_for(Path::new("/x"), "any", &cfg).num_seconds(), 0);
        let cfg = crate::config::AutoCleanConfig {
            cooldown: Some(120),
            ..Default::default()
        };
        assert_eq!(cooldown_for(Path::new("/x"), "any", &cfg).num_seconds(), 120);
    }

    #[test]
    fn matches_installed_by_name_and_size() {
        let dir = crate::test_util::tempdir().unwrap();
        let installed = dir.path().join("mybin");
        let target = dir.path().join("target").join("release").join("mybin");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&installed, b"payload").unwrap();
        fs::write(&target, b"payload").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let inst = binary_meta(&installed).unwrap();
        let tgt = binary_meta(&target).unwrap();
        assert!(matches_installed(&inst, &tgt));
    }

    #[test]
    fn matches_installed_by_name_and_size_rejects_different_size() {
        let dir = crate::test_util::tempdir().unwrap();
        let installed = dir.path().join("mybin");
        let target = dir.path().join("target").join("release").join("mybin");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&installed, b"payload").unwrap();
        fs::write(&target, b"different").unwrap();

        let inst = binary_meta(&installed).unwrap();
        let tgt = binary_meta(&target).unwrap();
        assert!(!matches_installed(&inst, &tgt));
    }
}
