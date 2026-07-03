use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde::Deserialize;

use crate::build_system::{run_native, BuildSystem, CleanContext, CleanResult, Partition};
use crate::fmt;

#[derive(Debug, Default, Deserialize)]
struct PackageJson {
    name: Option<String>,
    scripts: Option<HashMap<String, String>>,
    dependencies: Option<HashMap<String, serde_json::Value>>,
    dev_dependencies: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "workspaces")]
    _workspaces: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct Node;

impl Node {
    fn package_json(&self, root: &Path) -> Option<PackageJson> {
        let path = root.join("package.json");
        let text = fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn has_dep(&self, pkg: &PackageJson, name: &str) -> bool {
        pkg.dependencies
            .as_ref()
            .map(|d| d.contains_key(name))
            .unwrap_or(false)
            || pkg
                .dev_dependencies
                .as_ref()
                .map(|d| d.contains_key(name))
                .unwrap_or(false)
    }

    fn package_manager(&self, root: &Path) -> Option<&'static str> {
        if root.join("pnpm-lock.yaml").exists() {
            Some("pnpm")
        } else if root.join("yarn.lock").exists() {
            Some("yarn")
        } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
            Some("bun")
        } else if root.join("package-lock.json").exists() {
            Some("npm")
        } else {
            Some("npm") // default
        }
    }

    fn framework_output_dirs(&self, pkg: &PackageJson) -> Vec<&'static str> {
        let mut dirs = Vec::new();
        if self.has_dep(pkg, "next") {
            dirs.extend([".next", "out"]);
        }
        if self.has_dep(pkg, "nuxt") || self.has_dep(pkg, "nuxt3") {
            dirs.extend([".nuxt", ".output", "dist"]);
        }
        if self.has_dep(pkg, "vite") || self.has_dep(pkg, "@vitejs/plugin-vue") {
            dirs.push("dist");
        }
        if self.has_dep(pkg, "@sveltejs/kit") {
            dirs.extend([".svelte-kit", "build", ".vercel", ".netlify"]);
        }
        if self.has_dep(pkg, "astro") {
            dirs.push("dist");
        }
        if self.has_dep(pkg, "@vue/cli-service") {
            dirs.push("dist");
        }
        if self.has_dep(pkg, "gatsby") {
            dirs.extend(["public", ".cache"]);
        }
        if self.has_dep(pkg, "@docusaurus/core") {
            dirs.extend(["build", ".docusaurus"]);
        }
        if self.has_dep(pkg, "solid-start") {
            dirs.extend([".output", "dist"]);
        }
        if self.has_dep(pkg, "@remix-run/dev") {
            dirs.extend(["build", "public/build"]);
        }
        if self.has_dep(pkg, "expo") {
            dirs.extend([".expo", "dist"]);
        }
        dirs
    }

    fn has_clean_script(&self, pkg: &PackageJson) -> bool {
        pkg.scripts
            .as_ref()
            .map(|s| s.contains_key("clean"))
            .unwrap_or(false)
    }
}

impl BuildSystem for Node {
    fn name(&self) -> &'static str {
        "node"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("package.json").exists()
    }

    fn artifacts(&self, root: &Path) -> Vec<PathBuf> {
        let mut dirs = vec![
            root.join("node_modules"),
            root.join("dist"),
            root.join("build"),
            root.join("coverage"),
            root.join("storybook-static"),
            root.join("playwright-report"),
            root.join(".cache"),
            root.join(".parcel-cache"),
            root.join(".nyc_output"),
        ];

        if let Some(pkg) = self.package_json(root) {
            for d in self.framework_output_dirs(&pkg) {
                dirs.push(root.join(d));
            }
        }

        dirs.into_iter().filter(|p| p.exists()).collect()
    }

    fn clean(&self, root: &Path, ctx: &CleanContext) -> Result<CleanResult> {
        let pkg = self.package_json(root);

        // Optionally invoke the project's own clean script.
        if ctx.allow_native_commands {
            if let Some(ref pkg) = pkg {
                if self.has_clean_script(pkg) {
                    if let Some(pm) = self.package_manager(root) {
                        let mut cmd = Command::new(pm);
                        cmd.arg("run").arg("clean").current_dir(root);
                        let _ = run_native(&mut cmd, 120);
                    }
                }
            }
        }

        let mut to_remove = Vec::new();
        for artifact in self.artifacts(root) {
            let name = artifact.file_name().and_then(|n| n.to_str());
            if name == Some("node_modules") && !ctx.remove_node_modules {
                continue;
            }
            to_remove.push(artifact);
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
                    "node {}",
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

    fn write_package(root: &Path, deps: &[(&str, &str)]) {
        let mut lines = vec!["{".to_string()];
        lines.push("  \"dependencies\": {".to_string());
        for (i, (k, v)) in deps.iter().enumerate() {
            lines.push(format!("    \"{}\": \"{}\"{}", k, v, if i + 1 < deps.len() { "," } else { "" }));
        }
        lines.push("  }".to_string());
        lines.push("}".to_string());
        fs::write(root.join("package.json"), lines.join("\n")).unwrap();
    }

    #[test]
    fn detects_node_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert!(Node.detect(dir.path()));
    }

    #[test]
    fn detects_nextjs_output() {
        let dir = tempfile::tempdir().unwrap();
        write_package(dir.path(), &[("next", "14.0.0")]);
        fs::create_dir(dir.path().join(".next")).unwrap();
        let arts = Node.artifacts(dir.path());
        assert!(arts.contains(&dir.path().join(".next")));
    }

    #[test]
    fn dry_run_keeps_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules").join("x"), "x").unwrap();

        let ctx = CleanContext {
            dry_run: true,
            remove_node_modules: false,
            ..Default::default()
        };
        let res = Node.clean(dir.path(), &ctx).unwrap();
        assert!(dir.path().join("node_modules").exists());
        assert!(res.removed_dirs.is_empty());
    }

    #[test]
    fn removes_node_modules_when_opted_in() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();

        let ctx = CleanContext {
            dry_run: false,
            remove_node_modules: true,
            ..Default::default()
        };
        let res = Node.clean(dir.path(), &ctx).unwrap();
        assert!(!dir.path().join("node_modules").exists());
        assert_eq!(res.removed_dirs, vec![dir.path().join("node_modules")]);
    }
}
