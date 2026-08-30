//! Ownership detection and repair for build artifact cleanup.
//!
//! This module handles permission-related errors during cleanup operations
//! by providing deterministic sudo chown commands to fix ownership issues.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a permission error during cleanup with repair information.
#[derive(Debug, Clone)]
pub struct OwnershipError {
    /// The path that couldn't be removed due to permissions
    pub path: PathBuf,
    /// The specific error that occurred
    pub error: String,
    /// The deterministic chown command to fix the issue
    pub repair_command: String,
    /// The current owner (if detectable)
    pub current_owner: Option<String>,
    /// The target owner (typically the current user)
    pub target_owner: String,
}

/// .dhnotes file format for human-readable context about why files should not be removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhNotes {
    /// Why this file/directory should be preserved
    pub reason: String,
    /// When this note was created
    pub created_at: String,
    /// Who/what created this note (e.g., "developer", "agent", "deckhand")
    pub created_by: String,
    /// Optional additional context
    pub context: Option<String>,
}

impl DhNotes {
    /// Create a new .dhnotes entry
    pub fn new(reason: String, created_by: String, context: Option<String>) -> Self {
        let created_at = chrono::Utc::now().to_rfc3339();
        Self {
            reason,
            created_at,
            created_by,
            context,
        }
    }

    /// Write .dhnotes to a directory
    pub fn write_to_dir(&self, dir: &Path) -> Result<()> {
        let notes_path = dir.join(".dhnotes");
        let json = serde_json::to_string_pretty(self)
            .context("failed to serialize .dhnotes")?;
        fs::write(&notes_path, json)
            .with_context(|| format!("failed to write .dhnotes to {}", notes_path.display()))?;
        Ok(())
    }

    /// Read .dhnotes from a directory if it exists
    pub fn read_from_dir(dir: &Path) -> Result<Option<Self>> {
        let notes_path = dir.join(".dhnotes");
        if !notes_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&notes_path)
            .with_context(|| format!("failed to read .dhnotes from {}", notes_path.display()))?;
        let notes: DhNotes = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse .dhnotes from {}", notes_path.display()))?;
        Ok(Some(notes))
    }
}

/// Check if an error is a permission-related error
pub fn is_permission_error(err: &anyhow::Error) -> bool {
    if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
        return io_err.kind() == std::io::ErrorKind::PermissionDenied;
    }
    // Check for common permission error patterns in error messages
    let err_msg = err.to_string().to_lowercase();
    err_msg.contains("permission denied") || 
    err_msg.contains("operation not permitted") ||
    err_msg.contains("access denied")
}

/// Generate a deterministic chown command for a path
pub fn generate_chown_command(path: &Path, target_user: &str) -> String {
    format!("sudo chown -R {} {}", target_user, path.display())
}

/// Try to get the current owner of a path using stat
#[cfg(unix)]
pub fn get_path_owner(path: &Path) -> Result<Option<String>> {
    use std::os::unix::fs::MetadataExt;
    
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to get metadata for {}", path.display()))?;
    
    let uid = metadata.uid();
    
    // Try to convert UID to username
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    
    let mut pw = MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    
    let uid_c = uid as libc::uid_t;
    
    unsafe {
        let mut buf = [0u8; 1024];
        if libc::getpwuid_r(uid_c, pw.as_mut_ptr(), &mut buf as *mut _ as *mut libc::c_char, buf.len(), &mut result) == 0 
            && !result.is_null() {
            let pw = pw.assume_init();
            if pw.pw_name.is_null() {
                return Ok(None);
            }
            let username = CString::from_raw(pw.pw_name);
            return Ok(Some(username.into_string().unwrap_or_else(|_| format!("uid:{}", uid))));
        }
    }
    
    Ok(Some(format!("uid:{}", uid)))
}

#[cfg(not(unix))]
pub fn get_path_owner(_path: &Path) -> Result<Option<String>> {
    Ok(None)
}

/// Create an ownership error with repair information
pub fn create_ownership_error(path: PathBuf, error: anyhow::Error) -> OwnershipError {
    let target_owner = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "current_user".to_string());
    
    let current_owner = get_path_owner(&path).ok().flatten();
    
    let repair_command = generate_chown_command(&path, &target_owner);
    
    OwnershipError {
        path,
        error: error.to_string(),
        repair_command,
        current_owner,
        target_owner,
    }
}

/// Check if a directory is protected by .dhnotes
pub fn is_protected_by_dhnotes(dir: &Path) -> Result<Option<DhNotes>> {
    DhNotes::read_from_dir(dir)
}

/// Check for potential ownership issues in a directory
pub fn check_ownership_issues(dir: &Path) -> Option<OwnershipError> {
    if !dir.exists() {
        return None;
    }
    
    // Check for .dhnotes first
    if let Ok(Some(notes)) = DhNotes::read_from_dir(dir) {
        return Some(OwnershipError {
            path: dir.to_path_buf(),
            error: format!("Preserved due to .dhnotes: {}", notes.reason),
            repair_command: String::new(),
            current_owner: None,
            target_owner: String::new(),
        });
    }
    
    // Try to detect ownership issues
    match get_path_owner(dir) {
        Ok(Some(owner)) => {
            let target_user = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "current_user".to_string());
            
            if owner != target_user {
                let repair_command = generate_chown_command(dir, &target_user);
                return Some(OwnershipError {
                    path: dir.to_path_buf(),
                    error: format!("Ownership mismatch: current owner is {}", owner),
                    repair_command,
                    current_owner: Some(owner),
                    target_owner: target_user,
                });
            }
        }
        Ok(None) => {
            // Could not determine owner, might be OK
        }
        Err(_) => {
            // Error checking owner, might indicate permission issue
            let target_user = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "current_user".to_string());
            let repair_command = generate_chown_command(dir, &target_user);
            return Some(OwnershipError {
                path: dir.to_path_buf(),
                error: "Permission check failed - possible ownership issue".to_string(),
                repair_command,
                current_owner: None,
                target_owner: target_user,
            });
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dhnotes_serialization() {
        let notes = DhNotes::new(
            "Test reason".to_string(),
            "test_agent".to_string(),
            Some("Additional context".to_string()),
        );
        
        let json = serde_json::to_string_pretty(&notes).unwrap();
        let parsed: DhNotes = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.reason, "Test reason");
        assert_eq!(parsed.created_by, "test_agent");
        assert_eq!(parsed.context, Some("Additional context".to_string()));
    }

    #[test]
    fn generates_deterministic_chown() {
        let path = PathBuf::from("/tmp/test");
        let cmd = generate_chown_command(&path, "testuser");
        assert_eq!(cmd, "sudo chown -R testuser /tmp/test");
    }

    #[test]
    fn writes_and_reads_dhnotes() {
        let dir = crate::test_util::tempdir().unwrap();
        let notes = DhNotes::new(
            "Preserve for testing".to_string(),
            "test".to_string(),
            None,
        );
        
        notes.write_to_dir(dir.path()).unwrap();
        let read = DhNotes::read_from_dir(dir.path()).unwrap();
        
        assert!(read.is_some());
        let read = read.unwrap();
        assert_eq!(read.reason, "Preserve for testing");
        assert_eq!(read.created_by, "test");
    }

    #[test]
    fn missing_dhnotes_returns_none() {
        let dir = crate::test_util::tempdir().unwrap();
        let read = DhNotes::read_from_dir(dir.path()).unwrap();
        assert!(read.is_none());
    }

    #[test]
    fn check_ownership_on_protected_dir() {
        let dir = crate::test_util::tempdir().unwrap();
        let notes = DhNotes::new(
            "Protected directory".to_string(),
            "test".to_string(),
            None,
        );
        
        notes.write_to_dir(dir.path()).unwrap();
        let issue = check_ownership_issues(dir.path());
        
        assert!(issue.is_some());
        let issue = issue.unwrap();
        assert!(issue.error.contains("Preserved due to .dhnotes"));
        assert!(issue.repair_command.is_empty());
    }

    #[test]
    fn check_ownership_on_missing_dir() {
        let dir = crate::test_util::tempdir().unwrap();
        let nonexistent = dir.path().join("nonexistent");
        let issue = check_ownership_issues(&nonexistent);
        assert!(issue.is_none());
    }
}
