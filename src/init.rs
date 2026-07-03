use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::PathBuf;

const DEFAULT_CONFIG: &str = r#"# Deckhand configuration
# See https://github.com/elci-group/deckhand

[workspace]
path = "."
members = "auto"          # "auto", "all", or ["crate-a", "crate-b"]

[clean]
profiles = ["debug", "release"]
keep_incremental = false
keep_days = 0             # 0 = no age filter

[sweep]
registry_cache = true
git_checkouts = true
keep_registry_days = 30

[status]
warn_free_percent = 10
"#;

const DEFAULT_IGNORE: &str = r#"# Files and directories Deckhand should ignore during discovery
node_modules
.git
target
*.log
"#;

pub fn run(force: bool) -> Result<()> {
    let config_path = PathBuf::from("deckhand.toml");
    if config_path.exists() && !force {
        println!(
            "{} deckhand.toml already exists. Use --force to overwrite.",
            "info:".blue().bold()
        );
        return Ok(());
    }

    fs::write(&config_path, DEFAULT_CONFIG)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    println!(
        "{} created {}",
        "✓".green().bold(),
        config_path.display()
    );

    let ignore_path = PathBuf::from(".deckhandignore");
    if !ignore_path.exists() || force {
        fs::write(&ignore_path, DEFAULT_IGNORE)
            .with_context(|| format!("failed to write {}", ignore_path.display()))?;
        println!(
            "{} created {}",
            "✓".green().bold(),
            ignore_path.display()
        );
    }

    Ok(())
}
