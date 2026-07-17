use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::build_system::{run_native, BuildSystem, CleanContext, CleanResult, Partition};
use crate::fmt;

#[derive(Debug)]
pub struct Dotnet;

impl Dotnet {
    /// Locate the best manifest to pass to `dotnet clean`.
    /// Prefer a solution file; otherwise use the first project file.
    fn manifest(&self, root: &Path) -> Option<PathBuf> {
        let mut files: Vec<PathBuf> = fs::read_dir(root)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();

        if let Some(sln) = files.iter().find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("sln"))
                .unwrap_or(false)
        }) {
            return Some(sln.clone());
        }

        files
            .into_iter()
            .find(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| matches!(e.to_ascii_lowercase().as_str(), "csproj" | "fsproj" | "vbproj"))
                    .unwrap_or(false)
            })
    }

    fn artifact_dirs(&self, root: &Path) -> Vec<PathBuf> {
        ["bin", "obj"]
            .iter()
            .map(|n| root.join(n))
            .filter(|p| p.exists())
            .collect()
    }
}

impl BuildSystem for Dotnet {
    fn name(&self) -> &'static str {
        "dotnet"
    }

    fn detect(&self, root: &Path) -> bool {
        if !root.is_dir() {
            return false;
        }
        fs::read_dir(root)
            .ok()
            .map(|mut rd| {
                rd.any(|e| {
                    if let Ok(e) = e {
                        let p = e.path();
                        if p.is_file() {
                            let ext = p
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_ascii_lowercase());
                            return matches!(
                                ext.as_deref(),
                                Some("sln") | Some("csproj") | Some("fsproj") | Some("vbproj")
                            );
                        }
                    }
                    false
                })
            })
            .unwrap_or(false)
    }

    fn artifacts(&self, root: &Path) -> Vec<PathBuf> {
        self.artifact_dirs(root)
    }

    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult> {
        let artifacts = self.artifact_dirs(root);

        if ctx.keep_days > 0 {
            let mut freed = 0u64;
            for dir in &artifacts {
                freed += super::remove_older_than(dir, ctx.keep_days, ctx.dry_run)?;
            }
            return Ok(CleanResult {
                removed_dirs: vec![],
                bytes_freed: freed,
            });
        }

        let mut removed = Vec::new();
        let mut bytes_freed = 0u64;

        if ctx.allow_native_commands {
            if let Some(manifest) = self.manifest(root) {
                // Record sizes before any clean operation so dry-run reports
                // what would be freed without mutating the filesystem.
                for dir in &artifacts {
                    bytes_freed += fmt::dir_size(dir).unwrap_or(0);
                }

                if ctx.dry_run {
                    removed.extend(artifacts);
                } else {
                    let mut cmd = Command::new("dotnet");
                    cmd.arg("clean").arg(&manifest).current_dir(root);
                    let output = run_native(&mut cmd, 300).with_context(|| {
                        format!("dotnet clean failed for {}", root.display())
                    })?;
                    if !output.status.success() {
                        anyhow::bail!(
                            "dotnet clean failed for {}: {}",
                            root.display(),
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }

                    // `dotnet clean` leaves empty bin/obj dirs behind; remove them.
                    for dir in self.artifact_dirs(root) {
                        if dir.exists() && dir.read_dir()?.next().is_none() {
                            fs::remove_dir(&dir)?;
                        }
                        removed.push(dir);
                    }
                }
            } else {
                // Manifest disappeared between detect and clean; fall back.
                bytes_freed += super::remove_dirs(&artifacts, ctx.dry_run)?;
                removed.extend(artifacts);
            }
        } else {
            bytes_freed += super::remove_dirs(&artifacts, ctx.dry_run)?;
            removed.extend(artifacts);
        }

        Ok(CleanResult {
            removed_dirs: removed,
            bytes_freed,
        })
    }

    fn status_partitions(&self, root: &Path) -> Result<Vec<Partition>> {
        let mut parts = Vec::new();
        for dir in self.artifact_dirs(root) {
            parts.push(Partition {
                name: format!(
                    "dotnet {}",
                    dir.file_name().unwrap_or_default().to_string_lossy()
                ),
                path: dir.clone(),
                size_bytes: fmt::dir_size(&dir)?,
            });
        }
        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_csproj() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("Foo.csproj"), "").unwrap();
        assert!(Dotnet.detect(dir.path()));
    }

    #[test]
    fn detects_solution() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("App.sln"), "").unwrap();
        assert!(Dotnet.detect(dir.path()));
    }

    #[test]
    fn prefers_solution_as_manifest() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("App.sln"), "").unwrap();
        fs::write(dir.path().join("App.csproj"), "").unwrap();
        let manifest = Dotnet.manifest(dir.path()).unwrap();
        assert_eq!(manifest.file_name().unwrap(), "App.sln");
    }

    #[test]
    fn lists_bin_and_obj() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("App.csproj"), "").unwrap();
        fs::create_dir(dir.path().join("bin")).unwrap();
        fs::create_dir(dir.path().join("obj")).unwrap();
        let arts = Dotnet.artifacts(dir.path());
        assert!(arts.contains(&dir.path().join("bin")));
        assert!(arts.contains(&dir.path().join("obj")));
    }

    #[test]
    fn removes_bin_obj_without_native_command() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("App.csproj"), "").unwrap();
        fs::create_dir(dir.path().join("bin")).unwrap();
        fs::create_dir(dir.path().join("obj")).unwrap();
        fs::write(dir.path().join("bin").join("app.dll"), "x").unwrap();

        let ctx = CleanContext {
            dry_run: false,
            allow_native_commands: false,
            ..Default::default()
        };
        let res = Dotnet.clean(dir.path(), &ctx).unwrap();
        assert!(!dir.path().join("bin").exists());
        assert!(res.removed_dirs.contains(&dir.path().join("bin")));
    }

    #[test]
    fn dry_run_keeps_bin_obj() {
        let dir = crate::test_util::tempdir().unwrap();
        fs::write(dir.path().join("App.csproj"), "").unwrap();
        fs::create_dir(dir.path().join("bin")).unwrap();

        let ctx = CleanContext {
            dry_run: true,
            allow_native_commands: false,
            ..Default::default()
        };
        let res = Dotnet.clean(dir.path(), &ctx).unwrap();
        assert!(dir.path().join("bin").exists());
        assert!(res.removed_dirs.contains(&dir.path().join("bin")));
    }
}
