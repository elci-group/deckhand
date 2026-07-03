use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::*;

use crate::build_system;

const DEFAULT_IGNORE: &str = r"# Files and directories Deckhand should ignore during discovery
node_modules
.git
target
*.log
";

pub fn run(force: bool) -> Result<()> {
    let config_path = PathBuf::from("deckhand.toml");
    if config_path.exists() && !force {
        println!(
            "{} deckhand.toml already exists. Use --force to overwrite.",
            "info:".blue().bold()
        );
        return Ok(());
    }

    let languages = detect_languages(".");
    let languages_toml = languages
        .iter()
        .map(|l| format!("  \"{}\"", l))
        .collect::<Vec<_>>()
        .join(",\n");

    let config = format!(
        r#"# Deckhand configuration
# See https://github.com/elci-group/deckhand

[workspace]
path = "."
members = "auto"          # "auto", "all", or ["crate-a", "crate-b"]

[clean]
profiles = ["debug", "release"]
keep_incremental = false
keep_days = 0             # 0 = no age filter
languages = [
{languages_toml}
]
allow_native_commands = true
remove_node_modules = false
remove_venvs = false

[sweep]
registry_cache = true
git_checkouts = true
keep_registry_days = 30
node_modules = false
python_bytecode = true
go_build_cache = false
swift_derived_data = false

[status]
warn_free_percent = 10
"#
    );

    fs::write(&config_path, config)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    println!("{} created {}", "✓".green().bold(), config_path.display());

    let ignore_path = PathBuf::from(".deckhandignore");
    if !ignore_path.exists() || force {
        fs::write(&ignore_path, DEFAULT_IGNORE)
            .with_context(|| format!("failed to write {}", ignore_path.display()))?;
        println!("{} created {}", "✓".green().bold(), ignore_path.display());
    }

    Ok(())
}

fn detect_languages(path: &str) -> Vec<String> {
    let root = PathBuf::from(path);
    let mut detected = Vec::new();
    for system in build_system::registry() {
        if system.detect(&root) {
            detected.push(system.name().to_string());
        }
    }
    if detected.is_empty() {
        // Safe fallback for existing Cargo-only projects.
        detected.push("cargo".to_string());
    }
    detected
}
