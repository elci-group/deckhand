//! Internal filesystem helpers that replace small single-purpose crates.
//!
//! `available_space` replaces `fs2::available_space` using a direct `statvfs`
//! call on Unix.  This removes the `fs2` dependency while keeping the same POSIX
//! API.

use std::io;
use std::path::Path;

/// Return the available bytes on the filesystem containing `path`.
#[cfg(unix)]
pub fn available_space<P: AsRef<Path>>(path: P) -> io::Result<u64> {
    let c_path = std::ffi::CString::new(path.as_ref().as_os_str().as_encoded_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains null"))?;

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(stat.f_bavail * stat.f_frsize)
}

/// Stub for non-Unix platforms where `statvfs` is unavailable.
#[cfg(not(unix))]
pub fn available_space<P: AsRef<Path>>(_path: P) -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "available_space is only implemented on Unix",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn available_space_reports_nonzero() {
        let free = available_space(".").unwrap();
        assert!(free > 0, "available_space should be positive");
    }
}
