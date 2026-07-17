use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use crate::color::*;

use crate::build_system::{run_native, BuildSystem, CleanContext, CleanResult, Partition};
use crate::fmt;

#[derive(Debug)]
pub struct Maven;

impl Maven {
    fn find_target_dirs(&self, root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let root_target = root.join("target");
        if root_target.exists() {
            out.push(root_target);
        }
        // Common Maven multi-module layout: submodule/target
        for entry in crate::walk::WalkDir::new(root)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
        {
            let path = entry.path();
            if path.join("pom.xml").exists() {
                let target = path.join("target");
                if target.exists() {
                    out.push(target);
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

impl BuildSystem for Maven {
    fn name(&self) -> &'static str {
        "maven"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("pom.xml").exists()
    }

    fn artifacts(&self, root: &Path) -> Vec<PathBuf> {
        self.find_target_dirs(root)
    }

    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult> {
        let targets = self.find_target_dirs(root);

        if ctx.keep_days > 0 {
            let mut freed = 0u64;
            for dir in &targets {
                freed += super::remove_older_than(dir, ctx.keep_days, ctx.dry_run)?;
            }
            return Ok(CleanResult {
                removed_dirs: vec![],
                bytes_freed: freed,
            });
        }

        if ctx.allow_native_commands && root.join("pom.xml").exists() {
            let mut cmd = Command::new("mvn");
            cmd.arg("-q").arg("clean").current_dir(root);
            match run_native(&mut cmd, 300) {
                Ok(output) if output.status.success() => {}
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    eprintln!(
                        "  {} mvn clean failed for {}: {} (falling back to manual removal)",
                        "warning:".yellow().bold(),
                        root.display(),
                        stderr.lines().next().unwrap_or("unknown error")
                    );
                }
                Err(e) => {
                    eprintln!(
                        "  {} could not run mvn clean for {}: {} (falling back to manual removal)",
                        "warning:".yellow().bold(),
                        root.display(),
                        e
                    );
                }
            }
        }

        let mut removed = Vec::new();
        let mut bytes_freed = 0u64;
        for dir in targets {
            bytes_freed += fmt::dir_size(&dir).unwrap_or(0);
            if !ctx.dry_run {
                fs::remove_dir_all(&dir)?;
            }
            removed.push(dir);
        }

        Ok(CleanResult {
            removed_dirs: removed,
            bytes_freed,
        })
    }

    fn status_partitions(&self, root: &Path) -> Result<Vec<Partition>> {
        let mut parts = Vec::new();
        for artifact in self.artifacts(root) {
            parts.push(Partition {
                name: format!(
                    "maven {}",
                    artifact.strip_prefix(root).unwrap_or(&artifact).display()
                ),
                path: artifact.clone(),
                size_bytes: fmt::dir_size(&artifact)?,
            });
        }
        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_maven_project() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("pom.xml"), "<project></project>").unwrap();
        assert!(Maven.detect(dir.path()));
    }

    #[test]
    fn finds_target_dirs() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("pom.xml"), "<project></project>").unwrap();
        fs::create_dir_all(dir.path().join("app").join("target")).unwrap();
        fs::write(dir.path().join("app").join("pom.xml"), "<project></project>").unwrap();
        let arts = Maven.artifacts(dir.path());
        assert!(arts.contains(&dir.path().join("app").join("target")));
    }

    #[test]
    fn removes_target_without_native_command() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("pom.xml"), "<project></project>").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target").join("x.class"), "x").unwrap();

        let ctx = CleanContext {
            dry_run: false,
            allow_native_commands: false,
            ..Default::default()
        };
        let res = Maven.clean(dir.path(), &ctx).unwrap();
        assert!(!dir.path().join("target").exists());
        assert!(res.removed_dirs.contains(&dir.path().join("target")));
    }

    #[test]
    fn dry_run_keeps_target() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("pom.xml"), "").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();

        let ctx = CleanContext {
            dry_run: true,
            allow_native_commands: false,
            ..Default::default()
        };
        let res = Maven.clean(dir.path(), &ctx).unwrap();
        assert!(dir.path().join("target").exists());
        assert!(res.removed_dirs.contains(&dir.path().join("target")));
    }
}
