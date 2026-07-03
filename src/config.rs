use anyhow::{Context, Result};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub clean: CleanConfig,
    #[serde(default)]
    pub sweep: SweepConfig,
    #[serde(default)]
    pub status: StatusConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(default = "default_path")]
    pub path: PathBuf,
    #[serde(default)]
    pub members: MemberSpec,
}

#[derive(Debug, Clone, Default, Serialize)]
pub enum MemberSpec {
    #[default]
    Auto,
    List(Vec<String>),
}

impl MemberSpec {
    pub fn is_auto(&self) -> bool {
        matches!(self, MemberSpec::Auto)
    }
}

impl<'de> Deserialize<'de> for MemberSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MemberSpecVisitor;
        impl<'de> Visitor<'de> for MemberSpecVisitor {
            type Value = MemberSpec;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("\"auto\", \"all\", or a list of crate names")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    "auto" | "all" => Ok(MemberSpec::Auto),
                    _ => Err(de::Error::custom(format!(
                        "unknown member spec \"{}\", expected \"auto\", \"all\", or a list",
                        value
                    ))),
                }
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let list = Vec::<String>::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
                Ok(MemberSpec::List(list))
            }
        }

        deserializer.deserialize_any(MemberSpecVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanConfig {
    #[serde(default = "default_profiles")]
    pub profiles: Vec<String>,
    #[serde(default = "default_false")]
    pub keep_incremental: bool,
    #[serde(default)]
    pub keep_days: u64,
    /// Enabled language drivers. Missing key defaults to Cargo-only for backward compatibility.
    #[serde(default = "default_clean_languages")]
    pub languages: Vec<String>,
    #[serde(default = "default_true")]
    pub allow_native_commands: bool,
    #[serde(default = "default_false")]
    pub remove_node_modules: bool,
    #[serde(default = "default_false")]
    pub remove_venvs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepConfig {
    #[serde(default = "default_true")]
    pub registry_cache: bool,
    #[serde(default = "default_true")]
    pub git_checkouts: bool,
    #[serde(default = "default_thirty")]
    pub keep_registry_days: u64,
    #[serde(default = "default_false")]
    pub node_modules: bool,
    #[serde(default = "default_true")]
    pub python_bytecode: bool,
    #[serde(default = "default_false")]
    pub go_build_cache: bool,
    #[serde(default = "default_false")]
    pub swift_derived_data: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusConfig {
    #[serde(default = "default_ten")]
    pub warn_free_percent: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            workspace: WorkspaceConfig::default(),
            clean: CleanConfig::default(),
            sweep: SweepConfig::default(),
            status: StatusConfig::default(),
        }
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        WorkspaceConfig {
            path: default_path(),
            members: MemberSpec::Auto,
        }
    }
}

impl Default for CleanConfig {
    fn default() -> Self {
        CleanConfig {
            profiles: default_profiles(),
            keep_incremental: false,
            keep_days: 0,
            languages: default_clean_languages_all(),
            allow_native_commands: true,
            remove_node_modules: false,
            remove_venvs: false,
        }
    }
}

impl Default for SweepConfig {
    fn default() -> Self {
        SweepConfig {
            registry_cache: true,
            git_checkouts: true,
            keep_registry_days: 30,
            node_modules: false,
            python_bytecode: true,
            go_build_cache: false,
            swift_derived_data: false,
        }
    }
}

impl Default for StatusConfig {
    fn default() -> Self {
        StatusConfig {
            warn_free_percent: 10,
        }
    }
}

fn default_path() -> PathBuf {
    PathBuf::from(".")
}

fn default_profiles() -> Vec<String> {
    vec!["debug".to_string(), "release".to_string()]
}

fn default_false() -> bool {
    false
}

fn default_true() -> bool {
    true
}

fn default_thirty() -> u64 {
    30
}

fn default_ten() -> u64 {
    10
}

fn default_clean_languages() -> Vec<String> {
    // Backward-compatible default when the key is missing from an existing config file.
    vec!["cargo".to_string()]
}

fn default_clean_languages_all() -> Vec<String> {
    vec![
        "cargo".to_string(),
        "node".to_string(),
        "python".to_string(),
        "go".to_string(),
        "swift".to_string(),
        "gradle".to_string(),
    ]
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        let cfg: Config = toml::from_str(&text)
            .with_context(|| format!("failed to parse config {}", path.display()))?;
        Ok(cfg)
    }

    pub fn load_or_default(path: Option<PathBuf>) -> Result<Self> {
        match path {
            Some(p) => Self::load(&p),
            None => {
                let default = default_config_path();
                if default.exists() {
                    Self::load(&default)
                } else {
                    Ok(Config::default())
                }
            }
        }
    }
}

pub fn default_config_path() -> PathBuf {
    PathBuf::from("deckhand.toml")
}
