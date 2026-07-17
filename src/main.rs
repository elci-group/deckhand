use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use deckhand::{auto_clean, auto_start, clean, color, config, daemon, emoji, init, inspect, status, sweep, tts, update};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "deckhand",
    version,
    about = "Deterministic multi-language build-surface maintenance",
    long_about = "Deckhand keeps build artifacts clean across Cargo, Node/Bun, Python, Go, Swift, \
Gradle, .NET, and Maven projects. It is the operational-hygiene counterpart to version-management tools like kaptaind."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to deckhand.toml
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Suppress colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Suppress emoji and progress spinners
    #[arg(long, global = true)]
    no_emoji: bool,

    /// Speak command summaries via ElevenLabs TTS
    #[arg(long, global = true, conflicts_with = "no_tts")]
    tts: bool,

    /// Disable TTS even when [tts].enabled = true
    #[arg(long, global = true)]
    no_tts: bool,

    /// ElevenLabs voice id for TTS
    #[arg(long, global = true, value_name = "VOICE_ID")]
    tts_voice: Option<String>,

    /// ElevenLabs model id for TTS
    #[arg(long, global = true, value_name = "MODEL_ID")]
    tts_model: Option<String>,

    /// ElevenLabs API key for TTS (prefer env/config; visible only to this process)
    #[arg(long, global = true, value_name = "KEY")]
    tts_api_key: Option<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum InspectScope {
    /// Scan the user's home directory ($HOME)
    Home,
    /// Scan the whole filesystem from / (confined to the root filesystem)
    Root,
}

#[derive(Subcommand)]
enum Commands {
    /// Run cargo clean across the workspace
    Clean {
        /// Profile to clean: debug, release, or all
        #[arg(short, long, default_value = "all")]
        profile: String,

        /// Only print what would be removed
        #[arg(long)]
        dry_run: bool,

        /// Only remove artifacts older than N days
        #[arg(short, long)]
        older_than: Option<u64>,

        /// Override target directory
        #[arg(short, long)]
        target_dir: Option<PathBuf>,
    },

    /// Sweep stale artifacts and caches
    Sweep {
        /// Root path to sweep
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Only print what would be removed
        #[arg(long)]
        dry_run: bool,

        /// Keep registry cache entries newer than N days
        #[arg(short, long, default_value_t = 30)]
        keep_days: u64,
    },

    /// Report workspace sea-state (disk usage)
    Status {
        /// Output JSON instead of text
        #[arg(short, long)]
        json: bool,

        /// Show only the top N largest artifacts
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Scan the filesystem for Rust projects and report cleaning candidates
    Inspect {
        /// Scan scope: home (~) or the whole filesystem (root)
        #[arg(long, value_enum, default_value = "home")]
        scope: InspectScope,

        /// Explicit scan root; overrides --scope and expands a leading ~
        #[arg(long)]
        path: Option<PathBuf>,

        /// Only show candidates with at least this much reclaimable (e.g. 100MB)
        #[arg(long, default_value = "0")]
        min_size: String,

        /// Output JSON instead of text
        #[arg(short, long)]
        json: bool,
    },

    /// Initialize deckhand.toml for the current project
    Init {
        /// Overwrite existing config
        #[arg(short, long)]
        force: bool,
    },

    /// Match installed binaries to target outputs and clean when thresholds are met
    AutoClean {
        /// Only print what would be removed or queued
        #[arg(long)]
        dry_run: bool,
    },

    /// Install or manage a systemd user service that runs deckhand at login
    AutoStart {
        #[command(subcommand)]
        command: AutoStartCommands,
    },

    /// Run or manage the monitoring daemon (cleanup suggestions + confirmed cleans)
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },

    /// Check for and install a newer deckhand release
    Update {
        /// Only print what would be installed
        #[arg(long)]
        dry_run: bool,

        /// Reinstall even if already on the latest version
        #[arg(long)]
        force: bool,

        /// Skip the manual-confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum AutoStartCommands {
    /// Install a systemd user service that runs deckhand on login
    Install {
        /// Path to deckhand.toml used by the service
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Overwrite an existing service file
        #[arg(short, long)]
        force: bool,

        /// Only print what would be installed
        #[arg(long)]
        dry_run: bool,
    },

    /// Remove the systemd user service
    Uninstall {
        /// Only print what would be removed
        #[arg(long)]
        dry_run: bool,
    },

    /// Show whether the service is installed and enabled
    Status,
}

#[derive(Subcommand)]
enum DaemonCommands {
    /// Run the daemon in the foreground (this is what the systemd unit starts)
    Run,

    /// Install a systemd user service that runs the daemon at login
    Install {
        /// Path to deckhand.toml used by the service
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Overwrite an existing service file
        #[arg(short, long)]
        force: bool,

        /// Only print what would be installed
        #[arg(long)]
        dry_run: bool,
    },

    /// Remove the daemon's systemd user service
    Uninstall {
        /// Only print what would be removed
        #[arg(long)]
        dry_run: bool,
    },

    /// Show whether the daemon is running and any pending suggestion
    Status,

    /// Ask the running daemon to deep-scan now
    Scan,

    /// Confirm the pending suggestion and clean (equivalent to clicking "Clean now")
    Confirm {
        /// Suggestion id to confirm (defaults to the pending one)
        id: Option<String>,

        /// Only print what would be removed
        #[arg(long)]
        dry_run: bool,
    },

    /// Dismiss the pending suggestion (or snooze it)
    Decline {
        /// Snooze for [daemon].snooze_duration instead of dismissing
        #[arg(long)]
        snooze: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_color {
        color::set_override(false);
    }
    emoji::set_enabled(!cli.no_emoji);

    let tts_overrides = tts::TtsOverrides {
        enabled: if cli.tts {
            Some(true)
        } else if cli.no_tts {
            Some(false)
        } else {
            None
        },
        voice_id: cli.tts_voice,
        model_id: cli.tts_model,
        api_key: cli.tts_api_key,
    };

    match cli.command {
        Commands::Clean {
            profile,
            dry_run,
            older_than,
            target_dir,
        } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            let summary = clean::run(&cfg, &profile, dry_run, older_than, target_dir.as_deref())?;
            tts::announce(&cfg, &tts_overrides, "clean", &summary);
        }
        Commands::Sweep {
            path,
            dry_run,
            keep_days,
        } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            let summary = sweep::run(&cfg, &path, dry_run, keep_days)?;
            tts::announce(&cfg, &tts_overrides, "sweep", &summary);
        }
        Commands::Status { json, limit } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            let summary = status::run(&cfg, json, limit)?;
            tts::announce(&cfg, &tts_overrides, "status", &summary);
        }
        Commands::Inspect {
            scope,
            path,
            min_size,
            json,
        } => {
            let min_size_bytes =
                config::parse_human_size(&min_size).map_err(|e| anyhow::anyhow!(e))?;
            let (scan_root, same_file_system) = resolve_scan_root(scope, path)?;
            inspect::run(&inspect::InspectOptions {
                scan_root,
                min_size: min_size_bytes,
                json,
                same_file_system,
            })?;
        }
        Commands::Init { force } => {
            init::run(force)?;
        }
        Commands::AutoClean { dry_run } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            let summary = auto_clean::run(&cfg, dry_run)?;
            tts::announce(&cfg, &tts_overrides, "auto_clean", &summary);
        }
        Commands::AutoStart { command } => match command {
            AutoStartCommands::Install {
                config,
                force,
                dry_run,
            } => {
                auto_start::install(auto_start::InstallOptions {
                    config_path: config.as_deref(),
                    force,
                    dry_run,
                })?;
            }
            AutoStartCommands::Uninstall { dry_run } => {
                auto_start::uninstall(dry_run)?;
            }
            AutoStartCommands::Status => {
                auto_start::status()?;
            }
        },
        Commands::Daemon { command } => match command {
            DaemonCommands::Run => {
                daemon::run(cli.config)?;
            }
            DaemonCommands::Install {
                config,
                force,
                dry_run,
            } => {
                daemon::service::install(daemon::service::InstallOptions {
                    config_path: config.as_deref(),
                    force,
                    dry_run,
                })?;
            }
            DaemonCommands::Uninstall { dry_run } => {
                daemon::service::uninstall(dry_run)?;
            }
            DaemonCommands::Status => {
                daemon::status()?;
            }
            DaemonCommands::Scan => {
                daemon::scan_now()?;
            }
            DaemonCommands::Confirm { id, dry_run } => {
                let cfg = config::Config::load_or_default(cli.config)?;
                daemon::confirm(&cfg, id.as_deref(), dry_run)?;
            }
            DaemonCommands::Decline { snooze } => {
                let cfg = config::Config::load_or_default(cli.config)?;
                daemon::decline(&cfg, snooze)?;
            }
        },
        Commands::Update { dry_run, force, yes } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            update::run(&cfg.update, update::UpdateOptions { dry_run, force, yes })?;
        }
    }

    Ok(())
}

/// Resolve the directory `inspect` should scan and whether to stay on one
/// filesystem. `--path` takes precedence over `--scope` and never confines to a
/// single filesystem; `--scope root` scans `/` confined to the root filesystem.
fn resolve_scan_root(scope: InspectScope, path: Option<PathBuf>) -> Result<(PathBuf, bool)> {
    if let Some(p) = path {
        return Ok((expand_tilde(&p), false));
    }
    match scope {
        InspectScope::Root => Ok((PathBuf::from("/"), true)),
        InspectScope::Home => {
            let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
                anyhow::anyhow!("HOME is not set; use --path to specify a scan root")
            })?;
            Ok((home, false))
        }
    }
}

/// Expand a leading `~` or `~/` in a path to `$HOME`, matching config semantics.
fn expand_tilde(path: &std::path::Path) -> PathBuf {
    if let Some(s) = path.to_str() {
        if s == "~" {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home);
            }
        } else if let Some(rest) = s.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home).join(rest);
            }
        }
    }
    path.to_path_buf()
}
