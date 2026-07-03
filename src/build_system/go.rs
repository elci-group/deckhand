use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;

use crate::build_system::{run_native, BuildSystem, CleanContext, CleanResult, Partition};
use crate::fmt;

#[derive(Debug)]
pub struct Go;

impl BuildSystem for Go {
    fn name(&self) -> &'static str {
        "go"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("go.mod").exists() || root.join("go.work").exists()
    }

    fn artifacts(&self, root: &Path) -> Vec<PathBuf> {
        // Go has no fixed project-local build directory. `go clean` takes care of
        // object files and test caches inside the module. We report nothing here
        // so that dry-run/status do not invent paths.
        let _ = root;
        vec![]
    }

    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult> {
        let mut bytes_freed = 0u64;

        if ctx.allow_native_commands {
            let mut cmd = Command::new("go");
            cmd.arg("clean").current_dir(root);
            let output = run_native(&mut cmd, 120)
                .with_context(|| format!("go clean failed for {}", root.display()))?;
            if !output.status.success() {
                anyhow::bail!(
                    "go clean failed for {}: {}",
                    root.display(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            if ctx.remove_go_build_cache {
                let mut cmd = Command::new("go");
                cmd.arg("clean").arg("-cache").current_dir(root);
                let _ = run_native(&mut cmd, 120);
            }
        } else {
            // Fallback: remove known local artifact names.
            for name in ["bin", "build"] {
                let p = root.join(name);
                if p.exists() {
                    bytes_freed += fmt::dir_size(&p).unwrap_or(0);
                    if !ctx.dry_run {
                        fs::remove_dir_all(&p)?;
                    }
                }
            }
        }

        Ok(CleanResult {
            removed_dirs: vec![],
            bytes_freed,
        })
    }

    fn status_partitions(&self, root: &Path) -> Result<Vec<Partition>> {
        let mut parts = Vec::new();
        for name in ["bin", "build"] {
            let p = root.join(name);
            if p.exists() {
                parts.push(Partition {
                    name: format!("go {}", name),
                    path: p.clone(),
                    size_bytes: fmt::dir_size(&p)?,
                });
            }
        }
        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_go_mod() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("go.mod"), "module example\n").unwrap();
        assert!(Go.detect(dir.path()));
    }

    #[test]
    fn detects_go_work() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("go.work"), "go 1.21\n").unwrap();
        assert!(Go.detect(dir.path()));
    }
}
