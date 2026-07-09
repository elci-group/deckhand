//! Minimal, zero-dependency directory walker.
//!
//! Replaces `walkdir` for Deckhand's simple traversal needs.  Supports depth
//! limits, entry filtering, and basic file-type/metadata access.  Symlinks are
//! followed to their targets; loops are avoided by tracking visited inodes on
//! Unix.

use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DirEntry {
    path: PathBuf,
    file_type: fs::FileType,
    depth: usize,
}

impl DirEntry {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file_type(&self) -> fs::FileType {
        self.file_type
    }

    pub fn file_name(&self) -> std::ffi::OsString {
        self.path.file_name().unwrap_or_default().to_os_string()
    }

    pub fn metadata(&self) -> io::Result<Metadata> {
        fs::metadata(&self.path)
    }

    pub fn depth(&self) -> usize {
        self.depth
    }
}

pub struct WalkDir {
    root: PathBuf,
    max_depth: Option<usize>,
}

impl WalkDir {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            root: path.as_ref().to_path_buf(),
            max_depth: None,
        }
    }

    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

}

impl IntoIterator for WalkDir {
    type Item = io::Result<DirEntry>;
    type IntoIter = WalkDirIterator;

    fn into_iter(self) -> Self::IntoIter {
        WalkDirIterator {
            stack: vec![(self.root, 0)],
            max_depth: self.max_depth,
            seen_inodes: HashSet::new(),
        }
    }
}

pub struct WalkDirIterator {
    stack: Vec<(PathBuf, usize)>,
    max_depth: Option<usize>,
    seen_inodes: HashSet<(u64, u64)>,
}

impl Iterator for WalkDirIterator {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        let (path, depth) = self.stack.pop()?;

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => return Some(Err(e)),
        };

        let file_type = metadata.file_type();

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let key = (metadata.dev(), metadata.ino());
            if file_type.is_symlink() {
                // Follow the symlink and track the target inode.
                match fs::metadata(&path) {
                    Ok(target_meta) => {
                        let target_key = (target_meta.dev(), target_meta.ino());
                        if !self.seen_inodes.insert(target_key) {
                            return self.next();
                        }
                    }
                    Err(e) => return Some(Err(e)),
                }
            } else if file_type.is_dir() && !self.seen_inodes.insert(key) {
                return self.next();
            }
        }

        if file_type.is_dir() {
            let descend = match self.max_depth {
                Some(max) => depth < max,
                None => true,
            };
            if descend {
                match fs::read_dir(&path) {
                    Ok(entries) => {
                        for entry in entries.flatten() {
                            self.stack.push((entry.path(), depth + 1));
                        }
                    }
                    Err(e) => return Some(Err(e)),
                }
            }
        }

        Some(Ok(DirEntry {
            path,
            file_type,
            depth,
        }))
    }
}

/// Collect directories matching a set of top-level names and recursive
/// directory-name searches, excluding directories by name.
pub fn collect_artifact_dirs(
    root: &Path,
    top_level: &[&str],
    recursive: &[&str],
    exclude: &[&str],
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }

    for name in top_level {
        let p = root.join(name);
        if p.exists() && !exclude.contains(name) {
            out.push(p);
        }
    }

    if !recursive.is_empty() {
        let mut stack = vec![(root.to_path_buf(), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            if depth > 0 {
                let name = dir.file_name().unwrap_or_default();
                let name_lossy = name.to_string_lossy();
                if exclude.contains(&name_lossy.as_ref()) {
                    continue;
                }
                if recursive.iter().any(|r| *r == name_lossy.as_ref()) {
                    out.push(dir.clone());
                }
            }

            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push((path, depth + 1));
                }
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{}-{}", prefix, std::process::id()))
    }

    #[test]
    fn walkdir_finds_files_and_dirs() {
        let tmp = temp_dir("deckhand-walk-test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("a/b")).unwrap();
        fs::write(tmp.join("a/file.txt"), "hello").unwrap();

        let paths: Vec<_> = WalkDir::new(&tmp)
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| e.path().strip_prefix(&tmp).unwrap().to_path_buf())
            .collect();

        assert!(paths.contains(&PathBuf::from("")));
        assert!(paths.contains(&PathBuf::from("a")));
        assert!(paths.contains(&PathBuf::from("a/b")));
        assert!(paths.contains(&PathBuf::from("a/file.txt")));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn walkdir_respects_max_depth() {
        let tmp = temp_dir("deckhand-walk-depth");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("a/b/c")).unwrap();

        let max_depth = WalkDir::new(&tmp)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| e.depth())
            .max()
            .unwrap_or(0);

        assert_eq!(max_depth, 2);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn collect_dirs_respects_excludes() {
        let tmp = temp_dir("deckhand-collect");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("target/debug")).unwrap();
        fs::create_dir_all(tmp.join("src/__pycache__")).unwrap();

        let dirs = collect_artifact_dirs(&tmp, &[], &["__pycache__"], &["target"]);
        assert!(dirs.iter().any(|p| p.ends_with("__pycache__")));
        assert!(!dirs.iter().any(|p| p.ends_with("debug")));

        let _ = fs::remove_dir_all(&tmp);
    }
}
