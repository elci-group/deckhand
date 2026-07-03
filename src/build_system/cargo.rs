use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::build_system::{run_native, BuildSystem, CleanContext, CleanResult, Partition};
use crate::fmt;

#[derive(Debug)]
pub struct Cargo;

impl BuildSystem for Cargo {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("Cargo.toml").exists()
    }

    fn artifacts(&self, root: &Path) -> Vec<PathBuf> {
        let target = root.join("target");
        if target.exists() {
            vec![target]
        } else {
            vec![]
        }
    }

    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult> {
        let manifest = root.join("Cargo.toml");
        let target = ctx.target_dir.clone().unwrap_or_else(|| root.join("target"));

        if ctx.keep_days > 0 {
            let bytes = super::remove_older_than(&target, ctx.keep_days, ctx.dry_run)?;
            return Ok(CleanResult {
                removed_dirs: vec![],
                bytes_freed: bytes,
            });
        }

        let mut removed = Vec::new();
        let mut bytes_freed = 0u64;

        if ctx.allow_native_commands && manifest.exists() {
            let mut cmd = Command::new("cargo");
            cmd.arg("clean")
                .arg("--manifest-path")
                .arg(&manifest)
                .current_dir(root);
            if let Some(profile) = &ctx.profile {
                if profile != "all" {
                    cmd.arg("--profile").arg(profile);
                }
            }
            if let Some(td) = &ctx.target_dir {
                cmd.arg("--target-dir").arg(td);
            }

            let output = run_native(&mut cmd, 300).with_context(|| {
                format!("cargo clean failed for {}", root.display())
            })?;
            if !output.status.success() {
                anyhow::bail!(
                    "cargo clean failed for {}: {}",
                    root.display(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            if target.exists() {
                bytes_freed += fmt::dir_size(&target).unwrap_or(0);
                if target.read_dir()?.next().is_none() {
                    if !ctx.dry_run {
                        fs::remove_dir(&target)?;
                    }
                    removed.push(target);
                }
            }
        } else {
            // Safe fallback: delete the target directory ourselves.
            for artifact in self.artifacts(root) {
                if artifact.exists() {
                    bytes_freed += fmt::dir_size(&artifact).unwrap_or(0);
                    if !ctx.dry_run {
                        fs::remove_dir_all(&artifact)?;
                    }
                    removed.push(artifact);
                }
            }
        }

        Ok(CleanResult {
            removed_dirs: removed,
            bytes_freed,
        })
    }

    fn status_partitions(&self, root: &Path) -> Result<Vec<Partition>> {
        let mut partitions = Vec::new();
        let target = root.join("target");
        if target.exists() {
            let name = if root.join("Cargo.toml").exists() {
                format!(
                    "cargo target ({})",
                    root.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                )
            } else {
                "cargo target".to_string()
            };
            partitions.push(Partition {
                name,
                path: target.clone(),
                size_bytes: fmt::dir_size(&target)?,
            });
        }
        Ok(partitions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cargo_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert!(Cargo.detect(dir.path()));
    }

    #[test]
    fn lists_target_artifact() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        let arts = Cargo.artifacts(dir.path());
        assert_eq!(arts, vec![dir.path().join("target")]);
    }
}
