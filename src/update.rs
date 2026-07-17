//! Self-update via git release tags.
//!
//! `deckhand update` checks the configured git remote for the highest semver
//! tag, compares it to the running binary's version, and — with user consent
//! or `auto_install = true` — rebuilds and reinstalls from that tag.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::build_system::run_native;
use crate::color::*;
use crate::emoji;
use crate::spinner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Whether update checks are enabled at all.
    #[serde(default = "default_false")]
    pub enabled: bool,
    /// If true, install newer tags without prompting. If false, only notify.
    #[serde(default = "default_false")]
    pub auto_install: bool,
    /// Git remote to query for tags.
    #[serde(default = "default_update_remote")]
    pub remote: String,
    /// Local clone of the deckhand source repository.
    #[serde(default = "default_update_source_dir")]
    pub source_dir: PathBuf,
    /// Directory where the `deckhand` binary should be installed.
    #[serde(default = "default_update_install_dir")]
    pub install_dir: PathBuf,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_install: false,
            remote: default_update_remote(),
            source_dir: default_update_source_dir(),
            install_dir: default_update_install_dir(),
        }
    }
}

fn default_false() -> bool {
    false
}

fn default_update_remote() -> String {
    "origin".to_string()
}

fn default_update_source_dir() -> PathBuf {
    PathBuf::from("~/deckhand")
}

fn default_update_install_dir() -> PathBuf {
    PathBuf::from("~/.local/bin")
}

#[derive(Debug, Clone, Copy)]
pub struct UpdateOptions {
    pub dry_run: bool,
    pub force: bool,
    pub yes: bool,
}

pub fn run(cfg: &UpdateConfig, opts: UpdateOptions) -> Result<String> {
    if !cfg.enabled && !opts.yes && !opts.force {
        println!(
            "{} Self-updates are disabled. Enable them with [update].enabled = true in deckhand.toml.",
            emoji::e(emoji::INFO)
        );
        return Ok("update disabled".to_string());
    }

    let current = env!("CARGO_PKG_VERSION").to_string();
    let source_dir = expand_tilde(&cfg.source_dir);
    let install_dir = expand_tilde(&cfg.install_dir);

    if !source_dir.join(".git").is_dir() {
        bail!(
            "source_dir {} is not a git repository; set [update].source_dir to a clone of deckhand",
            source_dir.display()
        );
    }

    let latest = spinner::spin("Checking for deckhand updates", || {
        latest_tag(&source_dir, &cfg.remote)
    })?;

    let latest = match latest {
        Some(t) => t,
        None => {
            println!(
                "{} No semver tags found on remote '{}'.",
                emoji::e(emoji::INFO),
                cfg.remote
            );
            return Ok("no release tags found".to_string());
        }
    };

    let tag = latest.clone();
    let newer = is_newer(&current, &latest);

    if !newer && !opts.force {
        println!(
            "{} Already up to date: {} (latest {})",
            emoji::e(emoji::SUCCESS),
            current.bold(),
            latest.bold()
        );
        return Ok(format!("up to date at {}", current));
    }

    let action = if opts.force && !newer {
        format!(
            "reinstall {} (current {})",
            latest.bold(),
            current.bold()
        )
    } else {
        format!("update {} → {}", current.bold(), latest.bold())
    };

    if opts.dry_run {
        println!(
            "{} Would {} from tag {} into {}",
            emoji::e(emoji::INFO),
            action,
            tag.cyan(),
            install_dir.display().to_string().dimmed()
        );
        return Ok(format!("dry run: would install {}", latest));
    }

    if !cfg.auto_install && !opts.yes {
        println!(
            "{} A newer deckhand release is available: {} (current {})",
            emoji::e(emoji::INFO),
            latest.green().bold(),
            current.bold()
        );
        println!(
            "   Run {} to install it.",
            "deckhand update --yes".cyan()
        );
        return Ok(format!("newer version {} available", latest));
    }

    install_tag(&source_dir, &install_dir, &tag)?;

    println!(
        "{} Installed deckhand {} into {}",
        emoji::e(emoji::SUCCESS),
        latest.green().bold(),
        install_dir.display().to_string().dimmed()
    );
    Ok(format!("updated to {}", latest))
}

/// Find the highest semver tag on the given remote.
fn latest_tag(source_dir: &Path, remote: &str) -> Result<Option<String>> {
    let mut cmd = Command::new("git");
    cmd.args(["ls-remote", "--tags", remote])
        .current_dir(source_dir);

    let output = run_native(&mut cmd, 30)
        .with_context(|| {
            format!(
                "failed to run git ls-remote --tags {} in {} (timed out or SSH auth issue)",
                remote,
                source_dir.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git ls-remote failed for remote '{}': {}",
            remote,
            stderr.trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut best: Option<(u64, u64, u64, String)> = None;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 2 {
            continue;
        }
        let ref_name = parts[1];
        let tag = ref_name.strip_prefix("refs/tags/").unwrap_or(ref_name);
        // Skip peeled refs (the ^{} lines point to the underlying commit).
        if tag.ends_with("^{}") {
            continue;
        }
        let version = tag.strip_prefix('v').unwrap_or(tag);
        if let Some(parsed) = parse_version(version) {
            if best.as_ref().map(|b| parsed > (b.0, b.1, b.2)).unwrap_or(true) {
                best = Some((parsed.0, parsed.1, parsed.2, tag.to_string()));
            }
        }
    }

    Ok(best.map(|b| b.3))
}

fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let mut nums = s.split('.');
    let major = nums.next()?.parse().ok()?;
    let minor = nums.next()?.parse().ok()?;
    let patch = nums.next().unwrap_or("0").parse().ok()?;
    // Reject anything with extra numeric components or pre-release noise.
    if nums.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => latest != current,
    }
}

/// Fetch a tag, build from it, install, then return to the original branch.
fn install_tag(source_dir: &Path, install_dir: &Path, tag: &str) -> Result<()> {
    let original = current_ref(source_dir)?;

    let _guard = RestoreBranchGuard {
        source_dir: source_dir.to_path_buf(),
        original: original.clone(),
    };

    run_git(source_dir, &["fetch", "origin", "tag", tag], "fetch tag")?;
    run_git(source_dir, &["checkout", tag], "checkout tag")?;

    let mut cmd = std::process::Command::new("./install.sh");
    cmd.current_dir(source_dir)
        .env("INSTALL_DIR", install_dir);

    let output = run_native(&mut cmd, 600)
        .with_context(|| "failed to run ./install.sh".to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("install.sh failed: {}", stderr.trim());
    }

    // Guard restores original branch on drop.
    drop(_guard);
    Ok(())
}

fn current_ref(source_dir: &Path) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(source_dir);
    let output = run_native(&mut cmd, 30)
        .with_context(|| format!("failed to get current branch in {}", source_dir.display()))?;
    if !output.status.success() {
        bail!("git rev-parse failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_git(source_dir: &Path, args: &[&str], context: &str) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(source_dir);
    let timeout = if args.iter().any(|a| a.contains("fetch")) {
        120
    } else {
        30
    };
    let output = run_native(&mut cmd, timeout)
        .with_context(|| format!("failed to run git {:?} in {}", args, source_dir.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", context, stderr.trim());
    }
    Ok(())
}

struct RestoreBranchGuard {
    source_dir: PathBuf,
    original: String,
}

impl Drop for RestoreBranchGuard {
    fn drop(&mut self) {
        let target = if self.original == "HEAD" {
            // Detached originally; best effort back to the commit hash.
            let _ = current_ref(&self.source_dir).map(|r| {
                let _ = run_git(&self.source_dir, &["checkout", &r], "restore original ref");
            });
            return;
        } else {
            self.original.clone()
        };
        let _ = run_git(&self.source_dir, &["checkout", &target], "restore original branch");
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Some(s) = path.to_str() {
        if s == "~" {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home);
            }
        } else if let Some(rest) = s.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(rest);
            }
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_semver_tags() {
        assert_eq!(parse_version("0.11.5"), Some((0, 11, 5)));
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("2.0"), Some((2, 0, 0)));
    }

    #[test]
    fn rejects_prerelease_and_extra_components() {
        assert_eq!(parse_version("0.11.5-alpha"), None);
        assert_eq!(parse_version("0.11.5.1"), None);
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.11.4", "0.11.5"));
        assert!(!is_newer("0.11.5", "0.11.5"));
        assert!(!is_newer("0.11.5", "0.11.4"));
        assert!(is_newer("0.9.15", "0.11.5"));
    }

    #[test]
    fn expands_tilde() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_tilde(Path::new("~/deckhand")), PathBuf::from(&home).join("deckhand"));
        assert_eq!(expand_tilde(Path::new("~")), PathBuf::from(&home));
    }
}
