# DumDum Project Context

Project: deckhand

Languages observed:
- JSON: 32 files
- Markdown: 3 files
- Python: 1 files
- Rust: 24 files
- Shell: 1 files
- TOML: 4 files

Directory shape:
- .: 6 files
- .kaptaind: 4 files
- .kaptaind/analysis: 28 files
- docs: 2 files
- scripts: 1 files
- src: 17 files
- src/build_system: 7 files

Important file signals:
- .kaptaind/analysis/0268c650-8583-46f4-85c0-14f00f4a92f6.json (JSON, 797 bytes): {
- .kaptaind/analysis/04e91774-658e-46b8-99ca-c322b9e2101b.json (JSON, 1520 bytes): {
- .kaptaind/analysis/0d023bc5-3ebe-43fc-8954-4932360062df.json (JSON, 8331 bytes): {
- .kaptaind/analysis/0d9f3a2d-698e-43e3-bf9f-99c7f8af3d7a.json (JSON, 1820 bytes): {
- .kaptaind/analysis/10620d48-ac48-431c-b4f6-939daef24f7c.json (JSON, 1505 bytes): {
- .kaptaind/analysis/192f97b0-e0ac-4593-ac81-7f8bb53fb594.json (JSON, 1528 bytes): {
- .kaptaind/analysis/1e1db6dc-6fa4-4bd6-afa2-54220ad5692f.json (JSON, 1481 bytes): {
- .kaptaind/analysis/1ffb8775-6540-4dda-9175-1106352fa200.json (JSON, 1513 bytes): {
- .kaptaind/analysis/2fca2e77-26bb-4df7-8c52-0379ca6e6f04.json (JSON, 1531 bytes): {
- .kaptaind/analysis/336c7488-0125-46e5-a8b7-85b77345658f.json (JSON, 1513 bytes): {
- .kaptaind/analysis/370d2428-735a-48ec-a032-e28d9249a831.json (JSON, 1572 bytes): {
- .kaptaind/analysis/53fbb809-685d-4a29-923c-9d05d9c02ad9.json (JSON, 1482 bytes): {
- .kaptaind/analysis/6a06033c-b75b-41cb-b716-9f5a24b2cf3c.json (JSON, 4351 bytes): {
- .kaptaind/analysis/6ed22b99-c533-442b-9e55-64f82c6b95f0.json (JSON, 1485 bytes): {
- .kaptaind/analysis/765a95f7-ac58-499e-8c95-e79f78b5b000.json (JSON, 1484 bytes): {
- .kaptaind/analysis/7f1cd0ce-6459-40e1-90fd-0d6903c4e35a.json (JSON, 796 bytes): {
- .kaptaind/analysis/80c51718-90de-4241-9b6b-fa1f873dfc81.json (JSON, 796 bytes): {
- .kaptaind/analysis/8c539084-cb29-4132-932a-5760182bad07.json (JSON, 1523 bytes): {
- .kaptaind/analysis/8ef4d0c6-9361-4051-a944-3570d2cffeac.json (JSON, 1519 bytes): {
- .kaptaind/analysis/909833e3-397b-44b0-a6d4-0df582948043.json (JSON, 1522 bytes): {
- .kaptaind/analysis/91bfe245-c74d-47ac-aeb8-7c2d53424377.json (JSON, 1522 bytes): {
- .kaptaind/analysis/92c42ab7-4028-48e8-a7f3-28bd7bd74266.json (JSON, 797 bytes): {
- .kaptaind/analysis/a53b2864-9acc-495a-bfa4-ccc8394bc1e2.json (JSON, 1529 bytes): {
- .kaptaind/analysis/b1941d26-fa87-4126-8738-8da40d49cd32.json (JSON, 797 bytes): {
- .kaptaind/analysis/c9008b86-f71d-420b-9e22-cadc1c05c23f.json (JSON, 1525 bytes): {
- .kaptaind/analysis/c93f6a29-28f0-4909-8704-79aa4bef79fc.json (JSON, 1525 bytes): {
- .kaptaind/analysis/d069a331-1a98-4701-bdf9-f22b57f02623.json (JSON, 1522 bytes): {
- .kaptaind/analysis/e7c133c5-0b9f-4c1a-a22a-59c68e032d00.json (JSON, 1564 bytes): {
- .kaptaind/ast_cache.json (JSON, 14354 bytes): {"entries":{"src/walk.rs":{"hash":"486cae576e3a536b654134f229d591ccad1e5e22cd581dbd9207e874132fac55","symbols":[{"name":"DirEntry","kind":"struct"},{"name":"Dir
[trimmed]
- .kaptaind/status.json (JSON, 128 bytes): {
- .kaptaind/telemetry.json (JSON, 246 bytes): {
- .kaptaind/version_cache.json (JSON, 134 bytes): {
- Cargo.toml (TOML, 649 bytes): [package]
- README.md (Markdown, 6199 bytes): <div align="center">
- deckhand.toml (TOML, 384 bytes): [workspace]
- deliver.toml (TOML, 959 bytes): [[file]]
- docs/LANGUAGES.md (Markdown, 4850 bytes): Deckhand uses a small trait-based plugin system. Each driver detects itself from a manifest file, lists cleanable artifact directories, runs a native clean comm
[trimmed]
- docs/branding.md (Markdown, 767 bytes): The Deckhand logo combines nautical maintenance imagery with a Popeye-inspired sailor palette:
- install.sh (Shell, 549 bytes): set -euo pipefail
- kaptaind.toml (TOML, 647 bytes): [watch]
- scripts/generate-logo.py (Python, 8527 bytes): import os
- src/auto_clean.rs (Rust, 17068 bytes): use std::collections::{HashMap, HashSet};
- src/auto_start.rs (Rust, 6259 bytes): use std::fs;
- src/build_system/cargo.rs (Rust, 4700 bytes): use std::fs;
- src/build_system/go.rs (Rust, 3078 bytes): use std::fs;
- src/build_system/gradle.rs (Rust, 4850 bytes): use std::fs;
- src/build_system/mod.rs (Rust, 6450 bytes): use anyhow::{Context, Result};
- src/build_system/node.rs (Rust, 7935 bytes): use std::collections::HashMap;
- src/build_system/python.rs (Rust, 4675 bytes): use std::fs;
- src/build_system/swift.rs (Rust, 3337 bytes): use std::fs;
- src/clean.rs (Rust, 3729 bytes): use std::path::Path;
- src/color.rs (Rust, 1962 bytes): use std::sync::atomic::{AtomicBool, Ordering};
- src/config.rs (Rust, 18563 bytes): use anyhow::{Context, Result};
- src/fmt.rs (Rust, 865 bytes): use crate::color::*;
- src/fs.rs (Rust, 1372 bytes): use std::io;
- src/init.rs (Rust, 3793 bytes): use std::fs;
- src/inspect.rs (Rust, 13935 bytes): use std::collections::HashSet;
- src/lib.rs (Rust, 4136 bytes): pub mod auto_clean;
- src/main.rs (Rust, 8855 bytes): use anyhow::Result;
- src/status.rs (Rust, 5424 bytes): use std::path::{Path, PathBuf};
- src/sweep.rs (Rust, 7637 bytes): use std::env;
- src/test_util.rs (Rust, 1375 bytes): use std::fs;
- src/tts.rs (Rust, 13276 bytes): use anyhow::{anyhow, Context, Result};
- src/walk.rs (Rust, 6953 bytes): use std::collections::HashSet;
- src/workspace.rs (Rust, 5637 bytes): use std::collections::HashSet;


Recent documented file:
## `.kaptaind/analysis/0268c650-8583-46f4-85c0-14f00f4a92f6.json`

**Documentation depth:** brief explanation, target 260-380 words.

**Planned coverage:**
- What it is and where it sits in the project.
- Why it matters to users or maintainers.
- User-visible behavior or operational effect.
- Maintainer notes and review checklist.

**What it is:** This is a JSON file in `deckhand`. It configures tooling or runtime behavior rather than directly serving end-user screens.

**Why it matters:** It configures tooling or runtime behavior rather than directly serving end-user screens. DumDum treats this file as part of the project's working contract, so the explanation should connect the file to behavior, operations, or future maintenance rather than only restating its filename.

**What users should know:** Users may not touch this file directly, but its behavior can still affect reliability, output, or workflow.

**How it works:** The first meaningful line and surrounding directory are the strongest signals for this file. If that signal is weak, inspect imports, callers, or links before treating the explanation as complete.

**Media and demos:** No inline GIF, image, or VHS recording references were detected in this snapshot.

**Maintainer notes:** Keep the generated explanation aligned when this file changes. Current snapshot: 34 lines, 0 detected function-like definitions, hash 8688772064654143620.

**Review checklist:**
- Confirm the explanation still matches the file after major edits.
- Check whether linked commands, images, GIFs, or VHS tapes still exist.
- Re-run DumDum after the file has rested so generated sections stay aligned.


Recent documented file:
## `.kaptaind/analysis/04e91774-658e-46b8-99ca-c322b9e2101b.json`

**Documentation depth:** brief explanation, target 260-380 words.

**Planned coverage:**
- What it is and where it sits in the project.
- Why it matters to users or maintainers.
- User-visible behavior or operational effect.
- Maintainer notes and review checklist.

**What it is:** This is a JSON file in `deckhand`. It configures tooling or runtime behavior rather than directly serving end-user screens.

**Why it matters:** It configures tooling or runtime behavior rather than directly serving end-user screens. DumDum treats this file as part of the project's working contract, so the explanation should connect the file to behavior, operations, or future maintenance rather than only restating its filename.

**What users should know:** Users may not touch this file directly, but its behavior can still affect reliability, output, or workflow.

**How it works:** The first meaningful line and surrounding directory are the strongest signals for this file. If that signal is weak, inspect imports, callers, or links before treating the explanation as complete.

**Media and demos:** No inline GIF, image, or VHS recording references were detected in this snapshot.

**Maintainer notes:** Keep the generated explanation aligned when this file changes. Current snapshot: 62 lines, 0 detected function-like definitions, hash 6001917836916064137.

**Review checklist:**
- Confirm the explanation still matches the file after major edits.
- Check whether linked commands, images, GIFs, or VHS tapes still exist.
- Re-run DumDum after the file has rested so generated sections stay aligned.


Recent documented file:
## `.kaptaind/analysis/0d023bc5-3ebe-43fc-8954-4932360062df.json`

**Documentation depth:** deep explanation, target 850-1200 words.

**Planned coverage:**
- What it is and where it sits in the project.
- Why it matters to users or maintainers.
- User-visible behavior or operational effect.
- How the important functions, settings, or document sections work together.
- Failure modes, security concerns, and testing guidance.
- Maintainer notes and review checklist.

**What it is:** This is a JSON file in `deckhand`. It configures tooling or runtime behavior rather than directly serving end-user screens.

**Why it matters:** It configures tooling or runtime behavior rather than directly serving end-user screens. DumDum treats this file as part of the project's working contract, so the explanation should connect the file to behavior, operations, or future maintenance rather than only restating its filename.

**What users should know:** Users may not touch this file directly, but its behavior can still affect reliability, output, or workflow.

**How it works:** The first meaningful line and surrounding directory are the strongest signals for this file. If that signal is weak, inspect imports, callers, or links before treating the explanation as complete.

**Media and demos:** No inline GIF, image, or VHS recording references were detected in this snapshot.

**Maintainer notes:** Keep the generated explanation aligned when this file changes. Current snapshot: 305 lines, 0 detected function-like definitions, hash 3949753601146996753.

**Review checklist:**
- Confirm the explanation still matches the file after major edits.
- Check whether linked commands, images, GIFs, or VHS tapes still exist.
- Re-run DumDum after the file has rested so generated sections stay aligned.


Recent documented file:
## `.kaptaind/analysis/0d9f3a2d-698e-43e3-bf9f-99c7f8af3d7a.json`

**Documentation depth:** standard explanation, target 520-760 words.

**Planned coverage:**
- What it is and where it sits in the project.
- Why it matters to users or maintainers.
- User-visible behavior or operational effect.
- How the important functions, settings, or document sections work together.
- Maintainer notes and review checklist.

**What it is:** This is a JSON file in `deckhand`. It configures tooling or runtime behavior rather than directly serving end-user screens.

**Why it matters:** It configures tooling or runtime behavior rather than directly serving end-user screens. DumDum treats this file as part of the project's working contract, so the explanation should connect the file to behavior, operations, or future maintenance rather than only restating its filename.

**What users should know:** Users may not touch this file directly, but its behavior can still affect reliability, output, or workflow.

**How it works:** The first meaningful line and surrounding directory are the strongest signals for this file. If that signal is weak, inspect imports, callers, or links before treating the explanation as complete.

**Media and demos:** No inline GIF, image, or VHS recording references were detected in this snapshot.

**Maintainer notes:** Keep the generated explanation aligned when this file changes. Current snapshot: 71 lines, 0 detected function-like definitions, hash 4101754937365594559.

**Review checklist:**
- Confirm the explanation still matches the file after major edits.
- Check whether linked commands, images, GIFs, or VHS tapes still exist.
- Re-run DumDum after the file has rested so generated sections stay aligned.


Recent documented file:
## `.kaptaind/analysis/10620d48-ac48-431c-b4f6-939daef24f7c.json`

**Documentation depth:** brief explanation, target 260-380 words.

**Planned coverage:**
- What it is and where it sits in the project.
- Why it matters to users or maintainers.
- User-visible behavior or operational effect.
- Maintainer notes and review checklist.

**What it is:** This is a JSON file in `deckhand`. It configures tooling or runtime behavior rather than directly serving end-user screens.

**Why it matters:** It configures tooling or runtime behavior rather than directly serving end-user screens. DumDum treats this file as part of the project's working contract, so the explanation should connect the file to behavior, operations, or future maintenance rather than only restating its filename.

**What users should know:** Users may not touch this file directly, but its behavior can still affect reliability, output, or workflow.

**How it works:** The first meaningful line and surrounding directory are the strongest signals for this file. If that signal is weak, inspect imports, callers, or links before treating the explanation as complete.

**Media and demos:** No inline GIF, image, or VHS recording references were detected in this snapshot.

**Maintainer notes:** Keep the generated explanation aligned when this file changes. Current snapshot: 62 lines, 0 detected function-like definitions, hash 7930943650135896109.

**Review checklist:**
- Confirm the explanation still matches the file after major edits.
- Check whether linked commands, images, GIFs, or VHS tapes still exist.
- Re-run DumDum after the file has rested so generated sections stay aligned.


Recent documented file:
## `.kaptaind/analysis/192f97b0-e0ac-4593-ac81-7f8bb53fb594.json`

**Documentation depth:** brief explanation, target 260-380 words.

**Planned coverage:**
- What it is and where it sits in the project.
- Why it matters to users or maintainers.
- User-visible behavior or operational effect.
- Maintainer notes and review checklist.

**What it is:** This is a JSON file in `deckhand`. It configures tooling or runtime behavior rather than directly serving end-user screens.

**Why it matters:** It configures tooling or runtime behavior rather than directly serving end-user screens. DumDum treats this file as part of the project's working contract, so the explanation should connect the file to behavior, operations, or future maintenance rather than only restating its filename.

**What users should know:** Users may not touch this file directly, but its behavior can still affect reliability, output, or workflow.

**How it works:** The first meaningful line and surrounding directory are the strongest signals for this file. If that signal is weak, inspect imports, callers, or links before treating the explanation as complete.

**Media and demos:** No inline GIF, image, or VHS recording references were detected in this snapshot.

**Maintainer notes:** Keep the generated explanation aligned when this file changes. Current snapshot: 62 lines, 0 detected function-like definitions, hash 1824143980363348904.

**Review checklist:**
- Confirm the explanation still matches the file after major edits.
- Check whether linked commands, images, GIFs, or VHS tapes still exist.
- Re-run DumDum after the file has rested so generated sections stay aligned.


Recent documented file:
## `.kaptaind/analysis/1e1db6dc-6fa4-4bd6-afa2-54220ad5692f.json`

**Documentation depth:** brief explanation, target 260-380 words.

**Planned coverage:**
- What it is and where it sits in the project.
- Why it matters to users or maintainers.
- User-visible behavior or operational effect.
- Maintainer notes and review checklist.

**What it is:** This is a
[trimmed]