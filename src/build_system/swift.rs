use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::build_system::{run_native, BuildSystem, CleanContext, CleanResult, Partition};
use crate::fmt;

#[derive(Debug)]
pub struct Swift;

impl Swift {
    fn derived_data_dir(&self) -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/Developer/Xcode/DerivedData"))
    }
}

impl BuildSystem for Swift {
    fn name(&self) -> &'static str {
        "swift"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("Package.swift").exists()
    }

    fn artifacts(&self, root: &Path) -> Vec<PathBuf> {
        let build = root.join(".build");
        if build.exists() {
            vec![build]
        } else {
            vec![]
        }
    }

    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult> {
        if ctx.allow_native_commands {
            let mut cmd = Command::new("swift");
            cmd.arg("package").arg("clean").current_dir(root);
            let output = run_native(&mut cmd, 120)
                .with_context(|| format!("swift package clean failed for {}", root.display()))?;
            if !output.status.success() {
                anyhow::bail!(
                    "swift package clean failed for {}: {}",
                    root.display(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        let mut removed = Vec::new();
        let mut bytes_freed = 0u64;
        for artifact in self.artifacts(root) {
            bytes_freed += fmt::dir_size(&artifact).unwrap_or(0);
            if !ctx.dry_run {
                fs::remove_dir_all(&artifact)?;
            }
            removed.push(artifact);
        }

        if ctx.remove_swift_derived_data {
            if let Some(dd) = self.derived_data_dir() {
                if dd.exists() {
                    bytes_freed += fmt::dir_size(&dd).unwrap_or(0);
                    if !ctx.dry_run {
                        fs::remove_dir_all(&dd)?;
                    }
                    removed.push(dd);
                }
            }
        }

        Ok(CleanResult {
            removed_dirs: removed,
            bytes_freed,
        })
    }

    fn status_partitions(&self, root: &Path) -> Result<Vec<Partition>> {
        let mut parts = Vec::new();
        let build = root.join(".build");
        if build.exists() {
            parts.push(Partition {
                name: "swift .build".to_string(),
                path: build.clone(),
                size_bytes: fmt::dir_size(&build)?,
            });
        }
        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_swift_package() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Package.swift"), "// swift-tools-version:5.5\n").unwrap();
        assert!(Swift.detect(dir.path()));
    }

    #[test]
    fn lists_build_artifact() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Package.swift"), "").unwrap();
        fs::create_dir(dir.path().join(".build")).unwrap();
        let arts = Swift.artifacts(dir.path());
        assert_eq!(arts, vec![dir.path().join(".build")]);
    }
}
