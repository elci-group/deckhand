use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use crate::color::*;
use crate::emoji;

const SERVICE_NAME: &str = "deckhand-auto-clean.service";

#[derive(Debug, Clone)]
pub struct InstallOptions<'a> {
    pub config_path: Option<&'a Path>,
    pub force: bool,
    pub dry_run: bool,
}

pub fn install(opts: InstallOptions<'_>) -> Result<()> {
    let unit_dir = unit_dir()?;
    let unit_path = unit_dir.join(SERVICE_NAME);

    if unit_path.exists() && !opts.force {
        bail!(
            "{} already exists. Use --force to overwrite.",
            unit_path.display()
        );
    }

    let deckhand_exe = std::env::current_exe()
        .context("failed to determine path to current deckhand executable")?;
    let deckhand_exe = deckhand_exe.canonicalize().unwrap_or(deckhand_exe);

    let (config_path, working_dir) = resolve_config_and_workdir(opts.config_path)?;
    let config_arg = format!("--config={}", config_path.display());

    let unit = format!(
        r#"[Unit]
Description=Deckhand auto-clean for {}
After=default.target

[Service]
Type=oneshot
WorkingDirectory={}
ExecStart={} {} auto-clean

[Install]
WantedBy=default.target
"#,
        working_dir.display(),
        working_dir.display(),
        deckhand_exe.display(),
        config_arg
    );

    if opts.dry_run {
        println!(
            "{} {} would write {}:\n{}",
            emoji::e(emoji::INFO),
            "[dry-run]".yellow(),
            unit_path.display(),
            unit
        );
        return Ok(());
    }

    if !unit_dir.exists() {
        fs::create_dir_all(&unit_dir)
            .with_context(|| format!("failed to create {}", unit_dir.display()))?;
    }

    fs::write(&unit_path, unit)
        .with_context(|| format!("failed to write {}", unit_path.display()))?;

    systemctl(&["--user", "daemon-reload"])?;
    systemctl(&["--user", "enable", SERVICE_NAME])?;

    println!(
        "{} Installed and enabled {}",
        emoji::e(emoji::SUCCESS),
        SERVICE_NAME
    );
    println!("  {} unit: {}", emoji::e(emoji::LOCK), unit_path.display());
    println!(
        "  {} run: systemctl --user start {}",
        emoji::e(emoji::ROCKET),
        SERVICE_NAME
    );

    Ok(())
}

pub fn uninstall(dry_run: bool) -> Result<()> {
    let unit_dir = unit_dir()?;
    let unit_path = unit_dir.join(SERVICE_NAME);

    if !unit_path.exists() {
        println!(
            "{} {} is not installed",
            emoji::e(emoji::INFO),
            SERVICE_NAME
        );
        return Ok(());
    }

    if dry_run {
        println!(
            "{} {} would disable and remove {}",
            emoji::e(emoji::INFO),
            "[dry-run]".yellow(),
            unit_path.display()
        );
        return Ok(());
    }

    // Best-effort disable; ignore failure if it was not enabled.
    let _ = systemctl(&["--user", "disable", SERVICE_NAME]);

    fs::remove_file(&unit_path)
        .with_context(|| format!("failed to remove {}", unit_path.display()))?;

    systemctl(&["--user", "daemon-reload"])?;

    println!(
        "{} Disabled and removed {}",
        emoji::e(emoji::SUCCESS),
        SERVICE_NAME
    );
    Ok(())
}

pub fn status() -> Result<()> {
    let unit_dir = unit_dir()?;
    let unit_path = unit_dir.join(SERVICE_NAME);

    let installed = unit_path.exists();
    let enabled = if installed {
        match systemctl_output(&["--user", "is-enabled", SERVICE_NAME]) {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                text.trim() == "enabled"
            }
            Err(_) => false,
        }
    } else {
        false
    };

    println!(
        "{} {}: {}",
        emoji::e(emoji::AUTO_START),
        SERVICE_NAME,
        if installed { "installed".green() } else { "not installed".red() }
    );
    println!(
        "{} enabled: {}",
        emoji::e(emoji::INFO),
        if enabled { "yes".green() } else { "no".red() }
    );
    if installed {
        println!("{} unit path: {}", emoji::e(emoji::LOCK), unit_path.display());
        println!(
            "{} next login will run: systemctl --user start {}",
            emoji::e(emoji::ROCKET),
            SERVICE_NAME
        );
    }
    Ok(())
}

fn unit_dir() -> Result<PathBuf> {
    let config_dir = if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        bail!("could not determine user config directory; set HOME or XDG_CONFIG_HOME");
    };
    Ok(config_dir.join("systemd/user"))
}

fn resolve_config_and_workdir(config_path: Option<&Path>) -> Result<(PathBuf, PathBuf)> {
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

fn systemctl(args: &[&str]) -> Result<()> {
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

fn systemctl_output(args: &[&str]) -> Result<std::process::Output> {
    let output = Command::new("systemctl")
        .args(args)
        .output()
        .context("failed to run systemctl; is systemd installed?")?;
    Ok(output)
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
        fs::write(&config, "").unwrap();
        let (resolved, workdir) = resolve_config_and_workdir(Some(&config)).unwrap();
        assert_eq!(resolved.file_name().unwrap(), "deckhand.toml");
        assert_eq!(workdir, dir.path());
    }
}
