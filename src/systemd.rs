//! Shared helpers for managing systemd user units.
//!
//! Used by `auto_start` (login one-shot) and `daemon::service` (long-running
//! daemon unit). All functions shell out to `systemctl --user`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

pub fn unit_dir() -> Result<PathBuf> {
    let config_dir = if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        bail!("could not determine user config directory; set HOME or XDG_CONFIG_HOME");
    };
    Ok(config_dir.join("systemd/user"))
}

pub fn resolve_config_and_workdir(config_path: Option<&Path>) -> Result<(PathBuf, PathBuf)> {
    let config_path = match config_path {
        Some(p) => p.to_path_buf(),
        None => {
            let cwd = std::env::current_dir().context("failed to get current directory")?;
            cwd.join("deckhand.toml")
        }
    };

    let config_path = config_path
        .canonicalize()
        .with_context(|| format!("config path does not exist: {}", config_path.display()))?;

    let working_dir = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"));

    Ok((config_path, working_dir))
}

pub fn systemctl(args: &[&str]) -> Result<()> {
    let output = Command::new("systemctl")
        .args(args)
        .output()
        .context("failed to run systemctl; is systemd installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("systemctl {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(())
}

pub fn systemctl_output(args: &[&str]) -> Result<std::process::Output> {
    let output = Command::new("systemctl")
        .args(args)
        .output()
        .context("failed to run systemctl; is systemd installed?")?;
    Ok(output)
}

/// Render a `--config=<path>` argument for unit `ExecStart` lines.
pub fn config_arg(config_path: &Path) -> String {
    format!("--config={}", config_path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_dir_uses_xdg_config_home() {
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-config");
        let dir = unit_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/xdg-config/systemd/user"));
    }

    #[test]
    fn resolve_config_uses_supplied_path() {
        let dir = crate::test_util::tempdir().unwrap();
        let config = dir.path().join("deckhand.toml");
        std::fs::write(&config, "").unwrap();
        let (resolved, workdir) = resolve_config_and_workdir(Some(&config)).unwrap();
        assert_eq!(resolved.file_name().unwrap(), "deckhand.toml");
        assert_eq!(workdir, dir.path());
    }
}
