<div align="center">
  <img src="assets/logo-wide.png" alt="Deckhand logo" width="640">
  <br><br>
  <strong>Deterministic build-surface maintenance and hygiene agent for Cargo workspaces.</strong>
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

## Quick start

```bash
# Create deckhand.toml for the current Cargo workspace
deckhand init

# Show disk usage of targets and caches
deckhand status

# Clean every workspace member target
deckhand clean

# Sweep stale artifacts older than 30 days
deckhand sweep

# Dry-run any destructive command
deckhand clean --dry-run
deckhand sweep --dry-run
```

## Commands

| Command | Purpose |
|---------|---------|
| `deckhand init` | Generate `deckhand.toml` and `.deckhandignore` |
| `deckhand status` | Report workspace target/cache disk usage |
| `deckhand clean` | Run `cargo clean` across workspace members |
| `deckhand sweep` | Prune stale build artifacts and caches |

## Configuration

See `deckhand.toml` created by `deckhand init`.

```toml
[workspace]
path = "."
members = "auto"

[clean]
profiles = ["debug", "release"]
keep_incremental = false
keep_days = 0

[sweep]
registry_cache = true
git_checkouts = true
keep_registry_days = 30

[status]
warn_free_percent = 10
```

## License

MIT
