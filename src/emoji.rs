//! Global emoji toggle and icon helpers.
//!
//! JSON output and TTS summaries stay plain-text; emojis are only used in
//! human-facing terminal prints.

use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable or disable emoji output globally.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Whether emoji output is currently enabled.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Return the emoji when enabled, otherwise an empty string.
pub fn e(emoji: &str) -> &str {
    if enabled() {
        emoji
    } else {
        ""
    }
}

/// Build a label like "🧹 Clean" when emojis are enabled, else "Clean".
pub fn label(emoji: &str, text: &str) -> String {
    if enabled() {
        format!("{} {}", emoji, text)
    } else {
        text.to_string()
    }
}

pub const STATUS: &str = "📊";
pub const CLEAN: &str = "🧹";
pub const SWEEP: &str = "🌊";
pub const INSPECT: &str = "🔍";
pub const AUTO_CLEAN: &str = "🤖";
pub const AUTO_START: &str = "🚀";
pub const INIT: &str = "📝";

pub const ERROR: &str = "❌";
pub const WARNING: &str = "⚠️";
pub const SUCCESS: &str = "✅";
pub const INFO: &str = "ℹ️";

pub const DISK: &str = "💾";
pub const TRASH: &str = "🗑️";
pub const PACKAGE: &str = "📦";
pub const FOLDER: &str = "📁";
pub const SPARKLES: &str = "✨";
pub const CLOCK: &str = "⏱️";
pub const LOCK: &str = "🔒";
