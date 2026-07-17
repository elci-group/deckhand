use anyhow::{Context, Result};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub clean: CleanConfig,
    #[serde(default)]
    pub sweep: SweepConfig,
    #[serde(default)]
    pub status: StatusConfig,
    #[serde(default)]
    pub tts: TtsConfig,
    #[serde(default)]
    pub auto_clean: AutoCleanConfig,
    #[serde(default)]
    pub update: UpdateConfig,
}

pub use crate::update::UpdateConfig;

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
    #[serde(default = "default_false")]
    pub nuget_cache: bool,
    #[serde(default = "default_false")]
    pub bun_cache: bool,
    #[serde(default = "default_false")]
    pub maven_repository: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusConfig {
    #[serde(default = "default_ten")]
    pub warn_free_percent: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_tts_provider")]
    pub provider: String,
    #[serde(default = "default_tts_voice_id")]
    pub voice_id: String,
    #[serde(default = "default_tts_model_id")]
    pub model_id: String,
    #[serde(default = "default_tts_output_format")]
    pub output_format: String,
    #[serde(default = "default_tts_base_url")]
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default = "default_tts_announce")]
    pub announce: Vec<String>,
    #[serde(default = "default_tts_timeout_secs")]
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCleanConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_scan_paths")]
    pub scan_paths: Vec<PathBuf>,
    #[serde(default, deserialize_with = "deserialize_human_size_opt")]
    pub clutter_tolerance: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_human_size_opt")]
    pub min_free_space: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_duration_opt")]
    pub cooldown: Option<u64>,
    #[serde(default)]
    pub projects: HashMap<String, ProjectOverride>,
}

impl AutoCleanConfig {
    /// Return scan paths with a leading `~` expanded to `$HOME`.
    pub fn resolved_scan_paths(&self) -> Vec<PathBuf> {
        self.scan_paths.iter().map(|p| expand_tilde(p)).collect()
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Some(s) = path.to_str() {
        if s == "~" {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home);
            }
        } else if let Some(rest) = s.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(rest);
            }
        }
    }
    path.to_path_buf()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectOverride {
    #[serde(default, deserialize_with = "deserialize_duration_opt")]
    pub cooldown: Option<u64>,
}

impl Default for AutoCleanConfig {
    fn default() -> Self {
        AutoCleanConfig {
            enabled: false,
            scan_paths: default_scan_paths(),
            clutter_tolerance: None,
            min_free_space: None,
            cooldown: None,
            projects: HashMap::new(),
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
            nuget_cache: false,
            bun_cache: false,
            maven_repository: false,
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

impl Default for TtsConfig {
    fn default() -> Self {
        TtsConfig {
            enabled: false,
            provider: default_tts_provider(),
            voice_id: default_tts_voice_id(),
            model_id: default_tts_model_id(),
            output_format: default_tts_output_format(),
            base_url: default_tts_base_url(),
            api_key: None,
            api_key_env: None,
            announce: default_tts_announce(),
            timeout_secs: default_tts_timeout_secs(),
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

fn default_tts_provider() -> String {
    "elevenlabs".to_string()
}

fn default_tts_voice_id() -> String {
    "21m00Tcm4TlvDq8ikWAM".to_string()
}

fn default_tts_model_id() -> String {
    "eleven_multilingual_v2".to_string()
}

fn default_tts_output_format() -> String {
    "mp3_44100_128".to_string()
}

fn default_tts_base_url() -> String {
    "https://api.elevenlabs.io".to_string()
}

fn default_tts_announce() -> Vec<String> {
    vec![
        "clean".to_string(),
        "sweep".to_string(),
        "auto_clean".to_string(),
    ]
}

fn default_tts_timeout_secs() -> u64 {
    30
}

fn default_scan_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("~/.local/bin"),
    ]
}

/// Parse a human-readable byte size such as "5GB", "1.5MB", or "1024".
pub fn parse_human_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let mut num_str = String::new();
    let mut unit_start = s.len();
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() || c == '.' {
            num_str.push(c);
        } else {
            unit_start = i;
            break;
        }
    }
    if num_str.is_empty() {
        return Err(format!("no numeric value in size \"{}\"", s));
    }
    let value: f64 = num_str
        .parse()
        .map_err(|e| format!("invalid size number: {}", e))?;
    let unit = s[unit_start..].trim().to_lowercase();
    let multiplier: u64 = match unit.as_str() {
        "b" | "" => 1,
        "kb" | "k" => 1024,
        "mb" | "m" => 1024 * 1024,
        "gb" | "g" => 1024 * 1024 * 1024,
        "tb" | "t" => 1024u64 * 1024 * 1024 * 1024,
        "pb" | "p" => 1024u64 * 1024 * 1024 * 1024 * 1024,
        _ => return Err(format!("unknown size unit \"{}\"", unit)),
    };
    Ok((value * multiplier as f64) as u64)
}

/// Parse a human-readable duration such as "1h30m" or an integer number of seconds.
pub fn parse_duration(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if let Ok(secs) = s.parse::<u64>() {
        return Ok(secs);
    }

    let mut total = 0u64;
    let mut chars = s.chars().peekable();
    while chars.peek().is_some() {
        let mut num_str = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                num_str.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if num_str.is_empty() {
            return Err(format!("expected number in duration \"{}\"", s));
        }
        let n: u64 = num_str.parse().unwrap();

        let mut unit = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_alphabetic() {
                unit.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if unit.is_empty() {
            return Err(format!("missing unit after {} in duration \"{}\"", n, s));
        }
        let secs = match unit.to_lowercase().as_str() {
            "s" | "sec" | "secs" | "second" | "seconds" => n,
            "m" | "min" | "mins" | "minute" | "minutes" => n * 60,
            "h" | "hr" | "hrs" | "hour" | "hours" => n * 3600,
            "d" | "day" | "days" => n * 86400,
            "w" | "wk" | "week" | "weeks" => n * 604800,
            _ => return Err(format!("unknown duration unit \"{}\"", unit)),
        };
        total += secs;
    }
    Ok(total)
}

fn deserialize_human_size_opt<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    struct HumanSizeVisitor;
    impl<'de> Visitor<'de> for HumanSizeVisitor {
        type Value = Option<u64>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an integer number of bytes or a human-size string like \"5GB\"")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value < 0 {
                return Err(de::Error::custom("size cannot be negative"));
            }
            Ok(Some(value as u64))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            parse_human_size(value).map(Some).map_err(de::Error::custom)
        }
    }
    deserializer.deserialize_any(HumanSizeVisitor)
}

fn deserialize_duration_opt<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    struct DurationVisitor;
    impl<'de> Visitor<'de> for DurationVisitor {
        type Value = Option<u64>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an integer number of seconds or a duration string like \"1h30m\"")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value < 0 {
                return Err(de::Error::custom("duration cannot be negative"));
            }
            Ok(Some(value as u64))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            parse_duration(value).map(Some).map_err(de::Error::custom)
        }
    }
    deserializer.deserialize_any(DurationVisitor)
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
        "dotnet".to_string(),
        "maven".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_human_sizes() {
        assert_eq!(parse_human_size("1024").unwrap(), 1024);
        assert_eq!(parse_human_size("1KB").unwrap(), 1024);
        assert_eq!(parse_human_size("2MB").unwrap(), 2 * 1024 * 1024);
        assert_eq!(
            parse_human_size("1.5GB").unwrap(),
            (1.5 * (1024.0f64).powi(3)) as u64
        );
        assert_eq!(
            parse_human_size("1tb").unwrap(),
            1024u64 * 1024 * 1024 * 1024
        );
        assert_eq!(
            parse_human_size("1pb").unwrap(),
            1024u64 * 1024 * 1024 * 1024 * 1024
        );
    }

    #[test]
    fn rejects_invalid_size() {
        assert!(parse_human_size("abc").is_err());
        assert!(parse_human_size("5XB").is_err());
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("60").unwrap(), 60);
        assert_eq!(parse_duration("30s").unwrap(), 30);
        assert_eq!(parse_duration("5m").unwrap(), 300);
        assert_eq!(parse_duration("1h30m").unwrap(), 5400);
        assert_eq!(parse_duration("1d").unwrap(), 86400);
    }

    #[test]
    fn rejects_invalid_duration() {
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("1x").is_err());
    }

    #[test]
    fn parses_auto_clean_config() {
        let dir = crate::test_util::tempdir().unwrap();
        let path = dir.path().join("deckhand.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(
            br#"
[auto_clean]
enabled = true
clutter_tolerance = "5GB"
min_free_space = "10GB"
cooldown = "1h30m"

[auto_clean.projects."my-crate"]
cooldown = "30m"
"#,
        )
        .unwrap();

        let cfg = Config::load(&path).unwrap();
        assert!(cfg.auto_clean.enabled);
        assert_eq!(
            cfg.auto_clean.clutter_tolerance,
            Some(5 * 1024 * 1024 * 1024)
        );
        assert_eq!(cfg.auto_clean.min_free_space, Some(10 * 1024 * 1024 * 1024));
        assert_eq!(cfg.auto_clean.cooldown, Some(5400));
        assert_eq!(
            cfg.auto_clean.projects.get("my-crate").unwrap().cooldown,
            Some(1800)
        );
    }

    #[test]
    fn auto_clean_defaults_when_missing() {
        let cfg = Config::default();
        assert!(!cfg.auto_clean.enabled);
        assert_eq!(cfg.auto_clean.clutter_tolerance, None);
        assert_eq!(cfg.auto_clean.min_free_space, None);
        assert_eq!(cfg.auto_clean.cooldown, None);
        assert!(cfg.auto_clean.projects.is_empty());
        assert_eq!(cfg.auto_clean.scan_paths.len(), 4);
    }

    #[test]
    fn tts_defaults_are_safe() {
        let cfg = Config::default();
        assert!(!cfg.tts.enabled);
        assert_eq!(cfg.tts.provider, "elevenlabs");
        assert!(cfg.tts.api_key.is_none());
        assert!(cfg.tts.api_key_env.is_none());
        assert!(cfg.tts.announce.contains(&"clean".to_string()));
        assert_eq!(cfg.tts.timeout_secs, 30);
    }

    #[test]
    fn resolves_tilde_in_scan_paths() {
        std::env::set_var("HOME", "/home/sailor");
        let mut cfg = Config::default();
        cfg.auto_clean.scan_paths = vec![PathBuf::from("~/.local/bin")];
        let resolved = cfg.auto_clean.resolved_scan_paths();
        assert_eq!(resolved, vec![PathBuf::from("/home/sailor/.local/bin")]);
    }
}
