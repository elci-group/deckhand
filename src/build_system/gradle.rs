use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::build_system::{run_native, BuildSystem, CleanContext, CleanResult, Partition};
use crate::fmt;

#[derive(Debug)]
pub struct Gradle;

impl Gradle {
    fn gradle_cmd(&self, root: &Path) -> (String, Vec<String>) {
        if root.join("gradlew").exists() {
            ("./gradlew".to_string(), vec![])
        } else {
            ("gradle".to_string(), vec![])
        }
    }

    fn find_build_dirs(&self, root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let root_build = root.join("build");
        if root_build.exists() {
            out.push(root_build);
        }
        // Common Gradle multi-project layout: subproject/build
        for entry in walkdir::WalkDir::new(root)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
        {
            let path = entry.path();
            if path.join("build.gradle").exists() || path.join("build.gradle.kts").exists() {
                let sub_build = path.join("build");
                if sub_build.exists() {
                    out.push(sub_build);
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

impl BuildSystem for Gradle {
    fn name(&self) -> &'static str {
        "gradle"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("build.gradle").exists()
            || root.join("build.gradle.kts").exists()
            || root.join("settings.gradle").exists()
            || root.join("settings.gradle.kts").exists()
    }

    fn artifacts(&self, root: &Path) -> Vec<PathBuf> {
        self.find_build_dirs(root)
    }

    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult> {
        if ctx.allow_native_commands {
            let (program, _) = self.gradle_cmd(root);
            let mut cmd = Command::new(&program);
            cmd.arg("clean").current_dir(root);
            let output = run_native(&mut cmd, 300)
                .with_context(|| format!("gradle clean failed for {}", root.display()))?;
            if !output.status.success() {
                anyhow::bail!(
                    "gradle clean failed for {}: {}",
                    root.display(),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        let mut removed = Vec::new();
        let mut bytes_freed = 0u64;
        for build_dir in self.find_build_dirs(root) {
            bytes_freed += fmt::dir_size(&build_dir).unwrap_or(0);
            if !ctx.dry_run {
                fs::remove_dir_all(&build_dir)?;
            }
            removed.push(build_dir);
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
                    "gradle {}",
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
    fn detects_gradle_kts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
        assert!(Gradle.detect(dir.path()));
    }

    #[test]
    fn finds_build_dirs() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.gradle"), "").unwrap();
        fs::create_dir_all(dir.path().join("app").join("build")).unwrap();
        fs::write(dir.path().join("app").join("build.gradle"), "").unwrap();
        let arts = Gradle.artifacts(dir.path());
        assert!(arts.contains(&dir.path().join("app").join("build")));
    }
}
