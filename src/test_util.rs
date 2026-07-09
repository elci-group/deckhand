//! Lightweight test-only temporary directory helper.
//!
//! Replaces the `tempfile` dev-dependency for Deckhand's unit tests.  The
//! generated names include the process id and a monotonic counter to make
//! collisions extremely unlikely in test runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn new() -> std::io::Result<Self> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("deckhand-test-{}-{}", std::process::id(), n);
        let path = std::env::temp_dir().join(name);
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Create a temporary directory that is removed when the returned value is
/// dropped.
pub fn tempdir() -> std::io::Result<TempDir> {
    TempDir::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempdir_creates_and_cleans() {
        let path = {
            let dir = tempdir().unwrap();
            let p = dir.path().to_path_buf();
            assert!(p.exists());
            p
        };
        assert!(!path.exists());
    }
}
