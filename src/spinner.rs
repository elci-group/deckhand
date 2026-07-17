//! Minimal blocking spinner for long-running operations.
//!
//! Goes to stderr so stdout stays machine-parseable. Automatically skipped when
//! emojis are disabled, keeping non-fancy terminals clean.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::emoji;

/// Run `f` while showing a moon-phase spinner on stderr.
pub fn spin<F, T>(message: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    if !emoji::enabled() {
        return f();
    }

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let msg = message.to_string();

    let handle = thread::spawn(move || {
        let frames = ["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘"];
        let mut i = 0;
        while running_clone.load(Ordering::Relaxed) {
            eprint!("\r{} {} ... ", frames[i % frames.len()], msg);
            let _ = io::stderr().flush();
            i += 1;
            thread::sleep(Duration::from_millis(120));
        }
        // Clear the spinner line.
        eprint!("\r\x1b[K");
        let _ = io::stderr().flush();
    });

    let result = f();
    running.store(false, Ordering::Relaxed);
    let _ = handle.join();
    result
}
