use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::build_system::{BuildSystem, CleanContext, CleanResult, Partition};
use crate::fmt;

#[derive(Debug)]
pub struct Python;

impl Python {
    fn venv_dirs(&self, root: &Path) -> Vec<PathBuf> {
        [".venv", "venv", "env", ".env", "virtualenv"]
            .iter()
            .map(|n| root.join(n))
            .filter(|p| p.is_dir())
            .collect()
    }

    fn egg_info_dirs(&self, root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.ends_with(".egg-info") {
                            out.push(path);
                        }
                    }
                }
            }
        }
        out
    }
}

impl BuildSystem for Python {
    fn name(&self) -> &'static str {
        "python"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("pyproject.toml").exists()
            || root.join("setup.py").exists()
            || root.join("setup.cfg").exists()
    }

    fn artifacts(&self, root: &Path) -> Vec<PathBuf> {
        let mut dirs = super::collect_artifact_dirs(
            root,
            &["dist", "build", ".pytest_cache", ".mypy_cache", ".ruff_cache", "htmlcov", ".tox"],
            &["__pycache__"],
        );
        dirs.extend(self.egg_info_dirs(root));
        dirs.sort();
        dirs.dedup();
        dirs
    }

    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult> {
        let mut to_remove = self.artifacts(root);

        if ctx.remove_venvs {
            to_remove.extend(self.venv_dirs(root));
        }

        to_remove.sort();
        to_remove.dedup();

        if ctx.keep_days > 0 {
            let mut freed = 0u64;
            for dir in &to_remove {
                freed += super::remove_older_than(dir, ctx.keep_days, ctx.dry_run)?;
            }
            return Ok(CleanResult {
                removed_dirs: vec![],
                bytes_freed: freed,
            });
        }

        let bytes_freed = super::remove_dirs(&to_remove, ctx.dry_run)?;
        Ok(CleanResult {
            removed_dirs: to_remove,
            bytes_freed,
        })
    }

    fn status_partitions(&self, root: &Path) -> Result<Vec<Partition>> {
        let mut partitions = Vec::new();
        for artifact in self.artifacts(root) {
            partitions.push(Partition {
                name: format!(
                    "python {}",
                    artifact.file_name().unwrap_or_default().to_string_lossy()
                ),
                path: artifact.clone(),
                size_bytes: fmt::dir_size(&artifact)?,
            });
        }
        Ok(partitions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_python_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        assert!(Python.detect(dir.path()));
    }

    #[test]
    fn finds_pycache_and_dist() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("setup.py"), "").unwrap();
        let pycache = dir.path().join("src").join("__pycache__");
        fs::create_dir_all(&pycache).unwrap();
        fs::create_dir(dir.path().join("dist")).unwrap();
        let arts = Python.artifacts(dir.path());
        assert!(arts.contains(&pycache));
        assert!(arts.contains(&dir.path().join("dist")));
    }

    #[test]
    fn does_not_delete_venv_by_default() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        fs::create_dir(dir.path().join(".venv")).unwrap();
        let ctx = CleanContext::default();
        let res = Python.clean(dir.path(), &ctx).unwrap();
        assert!(dir.path().join(".venv").exists());
        assert!(res.removed_dirs.is_empty());
    }

    #[test]
    fn deletes_venv_when_opted_in() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        fs::create_dir(dir.path().join(".venv")).unwrap();
        let ctx = CleanContext {
            remove_venvs: true,
            ..Default::default()
        };
        Python.clean(dir.path(), &ctx).unwrap();
        assert!(!dir.path().join(".venv").exists());
    }
}
