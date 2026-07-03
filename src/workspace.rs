use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::build_system::{self, BuildSystem};

#[derive(Debug, Default, Deserialize)]
struct CargoManifest {
    workspace: Option<WorkspaceTable>,
    package: Option<PackageTable>,
}

#[derive(Debug, Default, Deserialize)]
struct WorkspaceTable {
    members: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageTable {
    name: Option<String>,
}

pub struct Workspace {
    pub root: PathBuf,
    pub projects: Vec<Project>,
}

pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub system: Arc<dyn BuildSystem>,
}

impl Workspace {
    /// True if the workspace contains more than one distinct project path.
    pub fn is_multi_project(&self) -> bool {
        let mut paths = HashSet::new();
        for p in &self.projects {
            paths.insert(&p.path);
        }
        paths.len() > 1
    }
}

/// Discover build systems in `root`. Cargo workspace members are expanded;
/// other build systems are reported at the root level.
pub fn discover(root: &Path, language_names: &[String]) -> Result<Workspace> {
    let mut projects = Vec::new();

    for system in build_system::enabled_systems(language_names) {
        if !system.detect(root) {
            continue;
        }

        if system.name() == "cargo" {
            let members = discover_cargo_members(root)?;
            for member in members {
                projects.push(Project {
                    name: member.name,
                    path: member.path,
                    system: Arc::new(build_system::cargo::Cargo),
                });
            }
        } else {
            let name = root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            projects.push(Project {
                name,
                path: root.to_path_buf(),
                system: Arc::from(system),
            });
        }
    }

    if projects.is_empty() {
        anyhow::bail!(
            "no supported build system detected in {} (enabled languages: {:?})",
            root.display(),
            language_names
        );
    }

    Ok(Workspace {
        root: root.to_path_buf(),
        projects,
    })
}

#[derive(Debug)]
struct CargoMember {
    name: String,
    path: PathBuf,
}

fn discover_cargo_members(root: &Path) -> Result<Vec<CargoMember>> {
    let manifest_path = root.join("Cargo.toml");
    if !manifest_path.exists() {
        return Ok(vec![]);
    }
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: CargoManifest = toml::from_str(&text)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

    let mut members = Vec::new();

    if let Some(workspace) = manifest.workspace {
        if let Some(member_globs) = workspace.members {
            let mut seen = HashSet::new();
            for glob in member_globs {
                let pattern = root.join(&glob);
                for entry in glob_dirs(&pattern)? {
                    let canonical = entry.canonicalize().unwrap_or(entry.clone());
                    if seen.insert(canonical.clone()) {
                        let name = package_name(&entry).unwrap_or_else(|| {
                            entry.file_name().unwrap_or_default().to_string_lossy().into()
                        });
                        members.push(CargoMember { name, path: entry });
                    }
                }
            }
        }
    }

    // If no workspace members found, treat the root as a single package.
    if members.is_empty() {
        let name = package_name(root).unwrap_or_else(|| "root".to_string());
        members.push(CargoMember {
            name,
            path: root.to_path_buf(),
        });
    }

    Ok(members)
}

fn glob_dirs(pattern: &Path) -> Result<Vec<PathBuf>> {
    let pattern_str = pattern.to_string_lossy();
    let mut dirs = Vec::new();
    // Very small glob support: handle trailing /** and *
    if pattern_str.ends_with("/**") {
        let base = PathBuf::from(&pattern_str[..pattern_str.len() - 3]);
        if base.is_dir() {
            collect_crate_dirs(&base, &mut dirs)?;
        }
    } else if pattern_str.ends_with('*') {
        let base = PathBuf::from(&pattern_str[..pattern_str.len() - 1]);
        if base.is_dir() {
            for entry in fs::read_dir(base)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() && path.join("Cargo.toml").exists() {
                    dirs.push(path);
                }
            }
        }
    } else if pattern.join("Cargo.toml").exists() {
        dirs.push(pattern.to_path_buf());
    }
    Ok(dirs)
}

fn collect_crate_dirs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if dir.join("Cargo.toml").exists() {
        out.push(dir.to_path_buf());
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_crate_dirs(&path, out)?;
        }
    }
    Ok(())
}

fn package_name(path: &Path) -> Option<String> {
    let manifest = path.join("Cargo.toml");
    let text = fs::read_to_string(manifest).ok()?;
    let manifest: CargoManifest = toml::from_str(&text).ok()?;
    manifest.package?.name
}

pub fn target_dir(member_path: &Path) -> PathBuf {
    member_path.join("target")
}
