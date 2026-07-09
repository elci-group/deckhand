pub mod auto_clean;
pub mod auto_start;
pub mod build_system;
pub mod clean;
pub mod color;
pub mod config;
pub mod fmt;
pub mod fs;
pub mod init;
pub mod status;
pub mod sweep;
pub mod walk;
pub mod workspace;

#[cfg(test)]
pub mod test_util;

use anyhow::Result;
use std::path::PathBuf;

pub fn default_config_path() -> PathBuf {
    PathBuf::from("deckhand.toml")
}

pub fn load_config(path: Option<PathBuf>) -> Result<config::Config> {
    let path = path.unwrap_or_else(default_config_path);
    config::Config::load(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_default_config() {
        let cfg = config::Config::default();
        assert!(cfg.clean.profiles.contains(&"debug".to_string()));
        assert!(cfg.sweep.registry_cache);
        assert_eq!(cfg.status.warn_free_percent, 10);
        assert!(cfg.clean.languages.contains(&"node".to_string()));
    }

    #[test]
    fn parses_config_with_member_list() {
        let dir = crate::test_util::tempdir().unwrap();
        let path = dir.path().join("deckhand.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(
            br#"
[workspace]
members = ["crate-a", "crate-b"]

[clean]
profiles = ["release"]
"#,
        )
        .unwrap();

        let cfg = config::Config::load(&path).unwrap();
        match cfg.workspace.members {
            config::MemberSpec::List(list) => {
                assert_eq!(list, vec!["crate-a".to_string(), "crate-b".to_string()]);
            }
            _ => panic!("expected member list"),
        }
        assert_eq!(cfg.clean.profiles, vec!["release".to_string()]);
        // Missing languages key should default to cargo-only.
        assert_eq!(cfg.clean.languages, vec!["cargo".to_string()]);
    }

    #[test]
    fn discovers_single_package_workspace() {
        let dir = crate::test_util::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[package]
name = "solo"
version = "0.1.0"
"#,
        )
        .unwrap();

        let ws = workspace::discover(dir.path(), &config::CleanConfig::default().languages).unwrap();
        assert_eq!(ws.projects.len(), 1);
        assert_eq!(ws.projects[0].name, "solo");
    }

    #[test]
    fn discovers_workspace_members() {
        let dir = crate::test_util::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/*"]
"#,
        )
        .unwrap();

        let crates = dir.path().join("crates");
        std::fs::create_dir_all(crates.join("a").join("src")).unwrap();
        std::fs::create_dir_all(crates.join("b").join("src")).unwrap();
        std::fs::write(
            crates.join("a").join("Cargo.toml"),
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            crates.join("b").join("Cargo.toml"),
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let ws = workspace::discover(dir.path(), &config::CleanConfig::default().languages).unwrap();
        let names: Vec<_> = ws.projects.iter().map(|m| m.name.clone()).collect();
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
    }

    #[test]
    fn detects_mixed_build_systems() {
        let dir = crate::test_util::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"mixed\"\n").unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(dir.path().join("pyproject.toml"), "").unwrap();

        let ws = workspace::discover(dir.path(), &config::CleanConfig::default().languages).unwrap();
        let names: Vec<_> = ws.projects.iter().map(|p| p.system.name()).collect();
        assert!(names.contains(&"cargo"));
        assert!(names.contains(&"node"));
        assert!(names.contains(&"python"));
    }
}
