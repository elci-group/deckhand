//! Minimal, zero-dependency terminal color helpers.
//!
//! Delegates ANSI rendering and the global no-color override to `form3`, the
//! shared dependency-free styling crate, while keeping Deckhand's own call
//! sites (`"text".green().bold()`, method chaining included) unchanged.

/// Disable or re-enable color output globally.
pub use form3::term::set_override;

// Each method below calls `form3::compat::Colorize` by full path (UFCS)
// rather than importing it, so its trait methods never come into scope here
// and collide with this module's own `Colorize`.

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
    fn red(self) -> String { form3::compat::Colorize::red(self).to_string() }
    fn green(self) -> String { form3::compat::Colorize::green(self).to_string() }
    fn yellow(self) -> String { form3::compat::Colorize::yellow(self).to_string() }
    fn blue(self) -> String { form3::compat::Colorize::blue(self).to_string() }
    fn magenta(self) -> String { form3::compat::Colorize::magenta(self).to_string() }
    fn cyan(self) -> String { form3::compat::Colorize::cyan(self).to_string() }
    fn bold(self) -> String { form3::compat::Colorize::bold(self).to_string() }
    fn dimmed(self) -> String { form3::compat::Colorize::dimmed(self).to_string() }
    fn underline(self) -> String { form3::compat::Colorize::underline(self).to_string() }
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
