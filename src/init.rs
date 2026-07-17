use std::fs;
use std::path::PathBuf;

use crate::color::*;
use anyhow::{Context, Result};

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
nuget_cache = false
bun_cache = false
maven_repository = false

[status]
warn_free_percent = 10

# Optional spoken summaries via ElevenLabs. Disabled by default.
# Key resolution order: --tts-api-key / DECKHAND_TTS_API_KEY, this file,
# project .env, ~/.config/deckhand/deckhand.toml, then ELEVENLABS_API_KEY from
# the environment or ~/.bashrc, ~/.zshrc, or ~/.profile.
# [tts]
# enabled = true
# provider = "elevenlabs"
# voice_id = "21m00Tcm4TlvDq8ikWAM"
# model_id = "eleven_multilingual_v2"
# output_format = "mp3_44100_128"
# base_url = "https://api.elevenlabs.io"
# announce = ["clean", "sweep", "auto_clean"]
# timeout_secs = 30
# api_key = "xi-..."                 # local project key; avoid committing it
# api_key_env = "ELEVENLABS_API_KEY" # or read a named environment variable

# Match-and-patch auto-clean strategy.
# When enabled, deckhand scans the paths below for installed binaries that match
# the current contents of each project's target/ directory. Matched projects are
# queued FIFO and cleaned only when clutter or free-space thresholds are met,
# and only after any configured cooldown has elapsed.
[auto_clean]
enabled = false
scan_paths = ["/bin", "/usr/bin", "/usr/local/bin", "~/.local/bin"]
# clutter_tolerance = "5GB"     # activate when queued artifacts exceed this
# min_free_space = "10GB"       # activate when free space drops below this
# cooldown = "1h"               # global cooldown between automated cleans

# Per-project overrides take precedence over the global cooldown.
# [auto_clean.projects."my-crate"]
# cooldown = "30m"
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
