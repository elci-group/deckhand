## `src/color.rs`

**What it is and where it sits in the project.**
The file `src/color.rs` is a Rust source file located in the `src` directory of the project. It has a size of 1962 bytes and contains 71 lines of code.

**Why it matters to users or maintainers.**
This file provides a set of minimal, zero-dependency terminal color helpers. It replaces the `colored` crate with a small internal implementation, supporting ANSI color/style methods, global no-color override, and method chaining.

**User-visible behavior or operational effect.**
The color helpers in this file allow users to easily add color to their terminal output. For example, they can use the `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `bold`, `dimmed`, and `underline` methods to add color to a string.

**How the important functions, settings, or document sections work together.**
The file uses a trait-based approach to provide color helpers. The `Colorize` trait defines the methods that can be used to add color to a string. The `impl Colorize for u0026str` block implements the `Colorize` trait for the `u0026str` type, providing the actual implementation of the color helpers.

The `set_override` function allows users to globally disable or re-enable color output. The `color_enabled` function checks whether color output is currently enabled.

The `wrap` function is used to wrap a string in ANSI escape codes to add color. It takes a string and an ANSI code as arguments and returns a new string with the ANSI code applied.

**Failure modes, security concerns, and testing guidance.**
The file contains two test functions: `colorize_adds_ansi_codes` and `no_color_strips_ansi_codes`. These tests ensure that the color helpers work correctly and that the `set_override` function behaves as expected.

There are no known security concerns in this file.

**Maintainer notes and review checklist.**
Maintainers should review this file to ensure that the color helpers work correctly and that the `set_override` function behaves as expected. They should also ensure that the tests are comprehensive and cover all possible scenarios.

Review checklist:

* Confirm that the color helpers work correctly.
* Ensure that the `set_override` function behaves as expected.
* Review the tests to ensure they are comprehensive and cover all possible scenarios.
* Re-run DumDum after the file has rested so generated sections stay aligned.

**Media and demos.**
No inline GIF, image, or VHS recording references were detected in this snapshot.

**Code Snippets.**
```rust
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

fn color_enabled() -u003e bool {
    !NO_COLOR.load(Ordering::Relaxed)
}

fn wrap(s: u0026str, code: u0026str) -u003e String {
    if color_enabled() {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

pub trait Colorize {
    fn red(self) -u003e String;
    fn green(self) -u003e String;
    fn yellow(self) -u003e String;
    fn blue(self) -u003e String;
    fn magenta(self) -u003e String;
    fn cyan(self) -u003e String;
    fn bold(self) -u003e String;
    fn dimmed(self) -u003e String;
    fn underline(self) -u003e String;
}

impl Colorize for u0026str {
    fn red(self) -u003e String { wrap(self, "31") }
    fn green(self) -u003e String { wrap(self, "32") }
    fn yellow(self) -u003e String { wrap(self, "33") }
    fn blue(self) -u003e String { wrap(self, "34") }
    fn magenta(self) -u003e String { wrap(self, "35") }
    fn cyan(self) -u003e String { wrap(self, "36") }
    fn bold(self) -u003e String { wrap(self, "1") }
    fn dimmed(self) -u003e String { wrap(self, "2") }
    fn underline(self) -u003e String { wrap(self, "4") }
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
```
