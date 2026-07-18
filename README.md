<div align="center">
  <img src="assets/logo-wide.png" alt="Deckhand logo" width="640">
  <br><br>
  <strong>Deterministic multi-language build-surface maintenance and hygiene agent.</strong>
  <br><br>
</div>

If [kaptaind] is who decides what gets shipped, **Deckhand** is what makes sure the ship is clean enough to sail.

![Deckhand icon](assets/logo.png)

[kaptaind]: https://github.com/elci-group/kaptaind

## Install

```bash
git clone https://github.com/elci-group/deckhand.git
cd deckhand
./install.sh
```

Deckhand keeps its dependency surface small: only a handful of foundational
crates (`anyhow`, `chrono`, `clap`, `libc`, `serde`, `serde_json`, `toml`) are
used. Utility features such as terminal colors, directory walking, free-space
queries, and test temporary directories are implemented internally with no
additional dependencies.

## Quick start

```bash
# Create deckhand.toml for the current project
deckhand init

# Show disk usage of build artifacts and caches
deckhand status

# Find Rust projects and how much space their target/ dirs use
deckhand inspect

# Clean build artifacts across all detected languages
deckhand clean

# Sweep stale artifacts older than 30 days
deckhand sweep

# Dry-run any destructive command
deckhand clean --dry-run
deckhand sweep --dry-run

# Run auto-clean automatically on user login
deckhand auto-start install
```

## Supported languages

Deckhand detects and cleans build artifacts for:

- **Rust** (`Cargo.toml`) — `cargo clean`
- **Node.js** (`package.json`) — framework-aware output dirs, `npm/pnpm/yarn/bun run clean`
- **Python** (`pyproject.toml`, `setup.py`, `setup.cfg`) — bytecode caches, dist/build dirs
- **Go** (`go.mod`, `go.work`) — `go clean`
- **Swift** (`Package.swift`) — `swift package clean`
- **Gradle** (`build.gradle[.kts]`) — `./gradlew clean` / `gradle clean`
- **.NET** (`*.csproj`/`*.fsproj`/`*.vbproj`/`*.sln`) — `dotnet clean`
- **Maven** (`pom.xml`) — `mvn clean`

See [docs/LANGUAGES.md](docs/LANGUAGES.md) for the full manifest/artifact matrix.

## Commands

| Command | Purpose |
|---------|---------|
| `deckhand init` | Generate `deckhand.toml` and `.deckhandignore` |
| `deckhand status` | Report build artifact/cache disk usage |
| `deckhand inspect` | Scan for Rust projects and report cleaning candidates |
| `deckhand deep-clean` | Clean every Cargo project found on the system (requires `--yes`) |
| `deckhand clean` | Run native clean commands across detected build systems |
| `deckhand sweep` | Prune stale build artifacts and caches |
| `deckhand auto-clean` | Clean matched projects when clutter/free-space thresholds are met |
| `deckhand auto-start` | Install or manage a systemd user service that runs deckhand at login |
| `deckhand daemon` | Monitor disk usage and suggest cleanups via desktop notifications |

## Configuration

`deckhand init` creates a `deckhand.toml` tailored to the project it detects.

```toml
[workspace]
path = "."
members = "auto"

[clean]
profiles = ["debug", "release"]
keep_incremental = false
keep_days = 0
languages = ["cargo", "node", "python", "go", "swift", "gradle"]
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

# [tts]
# enabled = true
# provider = "elevenlabs"
# voice_id = "21m00Tcm4TlvDq8ikWAM"
# model_id = "eleven_multilingual_v2"
# output_format = "mp3_44100_128"
# announce = ["clean", "sweep", "auto_clean"]
# api_key_env = "ELEVENLABS_API_KEY"

[auto_clean]
enabled = false
scan_paths = ["/bin", "/usr/bin", "/usr/local/bin", "~/.local/bin"]
# clutter_tolerance = "5GB"
# min_free_space = "10GB"
# cooldown = "1h"

# [auto_clean.projects."my-crate"]
# cooldown = "30m"

# [daemon]
# enabled = false
# scan_interval = "6h"        # deep-scan cadence
# watch_interval = "60s"      # free-space + artifact-mtime checks
# debounce = "5m"             # quiet window after build activity
# notify_threshold = "2GB"    # suggest when this much is reclaimable
# min_free_percent = 10       # or when free space drops below this
# fast_path = false           # also offer [Clean now] on the first notification
# auto_clean = false          # unattended mode: clean without confirmation
# notify_backend = "auto"     # auto | dbus | notify-send | log
# snooze_duration = "1d"
# watch_paths = []            # extra roots besides [workspace].path
```

### Backward compatibility

Existing `deckhand.toml` files that do not specify `[clean].languages` continue to run only the Cargo driver, preserving previous Cargo-only behavior. New projects or configs without a `deckhand.toml` file enable all language drivers by default.

## ElevenLabs TTS

Deckhand can speak short completion summaries with ElevenLabs. It is disabled by
default and never fails a command if synthesis or playback is unavailable.

Enable it in `deckhand.toml`:

```toml
[tts]
enabled = true
provider = "elevenlabs"
announce = ["clean", "sweep", "auto_clean"]
api_key_env = "ELEVENLABS_API_KEY"
```

Or force it for one run:

```bash
deckhand --tts clean
deckhand --tts --tts-voice 21m00Tcm4TlvDq8ikWAM sweep --dry-run
```

API keys are resolved in this order:

1. `--tts-api-key` / `DECKHAND_TTS_API_KEY`
2. Project `deckhand.toml` `[tts].api_key` or `[tts].api_key_env`
3. Project `.env` (`DECKHAND_TTS_API_KEY` or `ELEVENLABS_API_KEY`)
4. Top-level `~/.config/deckhand/deckhand.toml` `[tts]`
5. User environment / shell files: `ELEVENLABS_API_KEY` from the environment,
   `~/.bashrc`, `~/.zshrc`, or `~/.profile`

The integration shells out to `curl` and a local audio player (`ffplay`,
`mpg123`, `mplayer`, `play`, `paplay`, or `aplay`), so it adds no Rust
dependencies. Keep API keys out of version control; prefer `.env`,
`api_key_env`, or the top-level deckhand config for shared machines.

## Documentation

- `deckhand --help` and `deckhand <command> --help` for command-line reference.
- `docs/deckhand.1` for the full man page.
- `docs/LANGUAGES.md` for the supported-language manifest/artifact matrix.
- `docs/branding.md` for project branding assets and guidelines.

## Monitoring with kaptaind

Deckhand is designed to work alongside [kaptaind]. When kaptaind monitors a
repository that uses deckhand, it can analyze the working tree, gate commits on
the `cargo test` hook, and propose semantic version bumps based on the actual
changes. Run `kaptaind-cli analyze` to preview the impact of local changes
before committing.

## Auto-start on login

`deckhand auto-start install` creates a systemd user service that runs `deckhand auto-clean` each time you log in. The service uses the `deckhand.toml` from the directory where you ran the install command (or the path passed with `--config`).

```bash
# Install the login service for the current project
deckhand auto-start install

# Install using a specific config
deckhand auto-start install --config /path/to/deckhand.toml

# Check status or remove
deckhand auto-start status
deckhand auto-start uninstall
```

## Daemon: monitoring and cleanup notifications

`deckhand daemon run` starts a long-running monitor (this is what the systemd unit launches). It watches cheap signals — filesystem free space (`statvfs`) and artifact-directory mtimes — every `watch_interval`, and only walks directories for sizes when something actually changed (after a `debounce` quiet window), when `scan_interval` elapses, or when you run `deckhand daemon scan`. When reclaimable space crosses `notify_threshold` (or free space drops below the floor), it posts a desktop notification suggesting a cleanup.

Nothing is ever deleted without confirmation. The default flow is a "double click":

1. Suggestion notification: "3.2 GB reclaimable across 5 projects" — `[Review] [Snooze 1d]`
2. Click **Review** → per-project breakdown — `[Clean now] [Cancel]`
3. Click **Clean now** → deckhand cleans and posts the result ("Freed 3.1 GB")

Headless fallback: if no notification backend is available, the suggestion is logged and held; confirm with `deckhand daemon confirm` (or dismiss with `deckhand daemon decline`). Snoozed or dismissed suggestions stay quiet until they grow by 25% or their project set changes.

Notification backends (`notify_backend = "auto"`): a compiled-in D-Bus backend (build with `--features dbus`, uses `notify-rust`/zbus — no external binaries needed), the `notify-send` CLI when present in `$PATH`, or plain logging. The default build adds zero dependencies.

Set `[daemon] auto_clean = true` for unattended mode: crossed thresholds trigger cleaning immediately (a result notification is still posted). It ships disabled.

```bash
# Run in the foreground (journal-friendly logs)
deckhand daemon run

# Install as a systemd user service
deckhand daemon install
systemctl --user start deckhand-daemon.service

# Interact with the running daemon
deckhand daemon status      # pending suggestion, last scan
deckhand daemon scan        # force a deep scan now (SIGUSR1)
deckhand daemon confirm     # clean the pending suggestion
deckhand daemon decline     # dismiss (or --snooze)
```

## License

MIT
