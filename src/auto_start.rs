use std::fs;

use anyhow::{bail, Context, Result};
use crate::color::*;
use crate::emoji;
use crate::systemd::{self, config_arg};

const SERVICE_NAME: &str = "deckhand-auto-clean.service";

#[derive(Debug, Clone)]
pub struct InstallOptions<'a> {
    pub config_path: Option<&'a std::path::Path>,
    pub force: bool,
    pub dry_run: bool,
}

pub fn install(opts: InstallOptions<'_>) -> Result<()> {
    let unit_dir = systemd::unit_dir()?;
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

    let (config_path, working_dir) = systemd::resolve_config_and_workdir(opts.config_path)?;

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
        config_arg(&config_path)
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

    systemd::systemctl(&["--user", "daemon-reload"])?;
    systemd::systemctl(&["--user", "enable", SERVICE_NAME])?;

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
    let unit_dir = systemd::unit_dir()?;
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
    let _ = systemd::systemctl(&["--user", "disable", SERVICE_NAME]);

    fs::remove_file(&unit_path)
        .with_context(|| format!("failed to remove {}", unit_path.display()))?;

    systemd::systemctl(&["--user", "daemon-reload"])?;

    println!(
        "{} Disabled and removed {}",
        emoji::e(emoji::SUCCESS),
        SERVICE_NAME
    );
    Ok(())
}

pub fn status() -> Result<()> {
    let unit_dir = systemd::unit_dir()?;
    let unit_path = unit_dir.join(SERVICE_NAME);

    let installed = unit_path.exists();
    let enabled = if installed {
        match systemd::systemctl_output(&["--user", "is-enabled", SERVICE_NAME]) {
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
