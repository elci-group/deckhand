# Supported languages and build systems

Deckhand uses a small trait-based plugin system. Each driver detects itself from a manifest file, lists cleanable artifact directories, runs a native clean command when available, and falls back to safe pattern deletion when native commands are disabled or unavailable.

## Driver overview

| Driver   | Manifest files                                       | Native clean command            | Default in `[clean].languages` |
|----------|------------------------------------------------------|----------------------------------|--------------------------------|
| cargo    | `Cargo.toml`                                         | `cargo clean`                    | yes                            |
| node     | `package.json`                                       | `npm/pnpm/yarn/bun run clean`    | yes                            |
| python   | `pyproject.toml`, `setup.py`, `setup.cfg`            | pattern deletion                 | yes                            |
| go       | `go.mod`, `go.work`                                  | `go clean`                       | yes                            |
| swift    | `Package.swift`                                      | `swift package clean`            | yes                            |
| gradle   | `build.gradle`, `build.gradle.kts`, `settings.gradle[.kts]` | `./gradlew clean` / `gradle clean` | yes                     |

## Rust / Cargo

- **Detection:** `Cargo.toml` in the workspace root.
- **Workspace expansion:** `[workspace].members` globs are expanded; each member is treated as its own project.
- **Artifacts:** `target/` (or the `--target-dir` override).
- **Native command:** `cargo clean --manifest-path <path>/Cargo.toml [--profile <p>] [--target-dir <dir>]`.
- **Age filter:** when `--older-than` or `keep_days` is set, Deckhand removes individual files older than the cutoff instead of running `cargo clean`.

## Node.js / TypeScript

- **Detection:** `package.json`.
- **Package manager:** detected from lock files (priority: `pnpm-lock.yaml`, `yarn.lock`, `bun.lockb`/`bun.lock`, `package-lock.json`; default `npm`).
- **Framework output directories** (only when the corresponding dependency is declared):
  - Next.js → `.next/`, `out/`
  - Nuxt → `.nuxt/`, `.output/`, `dist/`
  - Vite → `dist/`
  - SvelteKit → `.svelte-kit/`, `build/`, `.vercel/`, `.netlify/`
  - Astro → `dist/`
  - Vue CLI → `dist/`
  - Gatsby → `public/`, `.cache/`
  - Docusaurus → `build/`, `.docusaurus/`
  - SolidStart → `.output/`, `dist/`
  - Remix → `build/`, `public/build/`
  - Expo → `.expo/`, `dist/`
- **Common artifacts:** `node_modules/`, `dist/`, `build/`, `coverage/`, `storybook-static/`, `playwright-report/`, `.cache/`, `.parcel-cache/`, `.nyc_output/`.
- **Native command:** if `package.json` declares a `clean` script, Deckhand runs `<pm> run clean` before pattern deletion.
- **Opt-in removals:** `node_modules/` is only removed when `remove_node_modules = true` (clean) or `node_modules = true` (sweep).

## Python

- **Detection:** `pyproject.toml`, `setup.py`, or `setup.cfg`.
- **Artifacts:**
  - `__pycache__/` (recursive)
  - `dist/`, `build/`
  - `*.egg-info/`
  - `.pytest_cache/`, `.mypy_cache/`, `.ruff_cache/`
  - `htmlcov/`, `.tox/`
- **Native command:** none by default; pattern deletion is used.
- **Opt-in removals:** virtual environments (`.venv/`, `venv/`, `env/`, `.env/`, `virtualenv/`) are only removed when `remove_venvs = true`.

## Go

- **Detection:** `go.mod` or `go.work`.
- **Artifacts:** Go has no fixed project-local build directory; `go clean` handles object files and test caches.
- **Native command:** `go clean` from the project root. When `go_build_cache = true` in `[sweep]`, Deckhand also runs `go clean -cache` (global cache).
- **Fallback:** when native commands are disabled, Deckhand removes local `bin/` and `build/` directories if present.

## Swift

- **Detection:** `Package.swift`.
- **Artifacts:** `.build/`.
- **Native command:** `swift package clean` from the project root.
- **Opt-in removals:** Xcode `DerivedData` is only removed when `swift_derived_data = true` in `[sweep]`.

## Gradle / Kotlin / Java

- **Detection:** `build.gradle`, `build.gradle.kts`, `settings.gradle`, or `settings.gradle.kts`.
- **Artifacts:** `build/` in the root and in detected subprojects.
- **Native command:** `./gradlew clean` if `gradlew` exists, otherwise `gradle clean`.
- **Fallback:** when native commands are disabled, Deckhand deletes discovered `build/` directories directly.

## Safety defaults

- Native commands are preferred but can be disabled with `allow_native_commands = false`.
- Destructive removals of `node_modules/` and Python virtual environments are opt-in.
- All destructive commands support `--dry-run`.
- Native commands run with a timeout to avoid hangs.
