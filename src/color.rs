//! Minimal, zero-dependency terminal color helpers.
//!
//! Replaces the `colored` crate with a small internal implementation.  Supports
//! the ANSI color/style methods used by Deckhand, global no-color override, and
//! method chaining (e.g. `"text".green().bold()`).

use std::sync::atomic::{AtomicBool, Ordering};

static NO_COLOR: AtomicBool = AtomicBool::new(false);

/// Disable or re-enable color output globally.
pub fn set_override(enabled: bool) {
    NO_COLOR.store(!enabled, Ordering::Relaxed);
}

fn color_enabled() -> bool {
    !NO_COLOR.load(Ordering::Relaxed)
}

fn wrap(s: &str, code: &str) -> String {
    if color_enabled() {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

pub trait Colorize {
    fn red(self) -> String;
    fn green(self) -> String;
    fn yellow(self) -> String;
    fn blue(self) -> String;
    fn magenta(self) -> String;
    fn cyan(self) -> String;
    fn bold(self) -> String;
    fn dimmed(self) -> String;
    fn underline(self) -> String;
}

impl Colorize for &str {
    fn red(self) -> String { wrap(self, "31") }
    fn green(self) -> String { wrap(self, "32") }
    fn yellow(self) -> String { wrap(self, "33") }
    fn blue(self) -> String { wrap(self, "34") }
    fn magenta(self) -> String { wrap(self, "35") }
    fn cyan(self) -> String { wrap(self, "36") }
    fn bold(self) -> String { wrap(self, "1") }
    fn dimmed(self) -> String { wrap(self, "2") }
    fn underline(self) -> String { wrap(self, "4") }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorize_adds_ansi_codes() {
        set_override(true);
        assert_eq!("x".red(), "\x1b[31mx\x1b[0m");
        assert_eq!("x".green().bold(), "\x1b[1m\x1b[32mx\x1b[0m\x1b[0m");
    }

    #[test]
    fn no_color_strips_ansi_codes() {
        set_override(false);
        assert_eq!("x".red(), "x");
        assert_eq!("x".green().bold(), "x");
        set_override(true);
    }
}
