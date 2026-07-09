use anyhow::Result;
use clap::{Parser, Subcommand};
use deckhand::{auto_clean, auto_start, clean, color, config, init, status, sweep};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "deckhand",
    version,
    about = "Deterministic multi-language build-surface maintenance",
    long_about = "Deckhand keeps build artifacts clean across Cargo, Node, Python, Go, Swift, \
and Gradle projects. It is the operational-hygiene counterpart to version-management tools like kaptaind."
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_color {
        color::set_override(false);
    }

    match cli.command {
        Commands::Clean {
            profile,
            dry_run,
            older_than,
            target_dir,
        } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            clean::run(&cfg, &profile, dry_run, older_than, target_dir.as_deref())?;
        }
        Commands::Sweep {
            path,
            dry_run,
            keep_days,
        } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            sweep::run(&cfg, &path, dry_run, keep_days)?;
        }
        Commands::Status { json, limit } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            status::run(&cfg, json, limit)?;
        }
        Commands::Init { force } => {
            init::run(force)?;
        }
        Commands::AutoClean { dry_run } => {
            let cfg = config::Config::load_or_default(cli.config)?;
            auto_clean::run(&cfg, dry_run)?;
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
    }

    Ok(())
}
