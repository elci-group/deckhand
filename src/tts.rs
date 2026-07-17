use anyhow::{anyhow, Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;

/// CLI overrides for TTS. `enabled = None` defers to `[tts].enabled`.
#[derive(Debug, Clone, Default)]
pub struct TtsOverrides {
    pub enabled: Option<bool>,
    pub voice_id: Option<String>,
    pub model_id: Option<String>,
    pub api_key: Option<String>,
}

struct ResolvedTts {
    api_key: String,
    voice_id: String,
    model_id: String,
    output_format: String,
    base_url: String,
    timeout_secs: u64,
}

/// Speak a short command summary if TTS is enabled for this command. Failures
/// are warnings, not fatal errors: cleanup should never fail because audio did.
pub fn announce(cfg: &Config, overrides: &TtsOverrides, command: &str, message: &str) {
    if let Err(e) = try_announce(cfg, overrides, command, message) {
        eprintln!("warning: TTS skipped: {}", e);
    }
}

fn try_announce(
    cfg: &Config,
    overrides: &TtsOverrides,
    command: &str,
    message: &str,
) -> Result<()> {
    let tts = &cfg.tts;
    let enabled = overrides.enabled.unwrap_or(tts.enabled);
    if !enabled || !command_enabled(&tts.announce, command) {
        return Ok(());
    }
    if !tts.provider.eq_ignore_ascii_case("elevenlabs") {
        return Err(anyhow!("unsupported TTS provider '{}'", tts.provider));
    }

    let Some(api_key) = resolve_api_key(cfg, overrides.api_key.clone())? else {
        eprintln!(
            "warning: TTS enabled but no ElevenLabs API key was found (checked --tts-api-key, \
             deckhand.toml, project .env, ~/.config/deckhand/deckhand.toml, and ELEVENLABS_API_KEY)"
        );
        return Ok(());
    };

    let resolved = ResolvedTts {
        api_key,
        voice_id: overrides
            .voice_id
            .clone()
            .unwrap_or_else(|| tts.voice_id.clone()),
        model_id: overrides
            .model_id
            .clone()
            .unwrap_or_else(|| tts.model_id.clone()),
        output_format: tts.output_format.clone(),
        base_url: tts.base_url.trim_end_matches('/').to_string(),
        timeout_secs: tts.timeout_secs.max(1),
    };

    let audio = synthesize(&resolved, &spoken_text(command, message))?;
    if let Err(e) = play(&audio) {
        eprintln!("warning: TTS playback skipped: {}", e);
    }
    Ok(())
}

fn resolve_api_key(cfg: &Config, cli_key: Option<String>) -> Result<Option<String>> {
    if let Some(key) = clean_key(cli_key) {
        return Ok(Some(key));
    }
    if let Some(key) = clean_key(cfg.tts.api_key.clone()) {
        return Ok(Some(key));
    }
    if let Some(env_name) = cfg.tts.api_key_env.as_deref() {
        if let Some(key) = env_value(env_name) {
            return Ok(Some(key));
        }
    }

    let project_env = cfg.workspace.path.join(".env");
    if let Some(key) = key_from_file(&project_env, "DECKHAND_TTS_API_KEY")? {
        return Ok(Some(key));
    }
    if let Some(key) = key_from_file(&project_env, "ELEVENLABS_API_KEY")? {
        return Ok(Some(key));
    }

    if let Some(path) = user_config_path() {
        if path.exists() {
            let user_cfg = Config::load(&path)?;
            if let Some(key) = clean_key(user_cfg.tts.api_key.clone()) {
                return Ok(Some(key));
            }
            if let Some(env_name) = user_cfg.tts.api_key_env.as_deref() {
                if let Some(key) = env_value(env_name) {
                    return Ok(Some(key));
                }
            }
        }
    }

    if let Some(key) = env_value("DECKHAND_TTS_API_KEY") {
        return Ok(Some(key));
    }
    if let Some(key) = env_value("ELEVENLABS_API_KEY") {
        return Ok(Some(key));
    }
    if let Some(key) = key_from_shell_files("DECKHAND_TTS_API_KEY") {
        return Ok(Some(key));
    }
    if let Some(key) = key_from_shell_files("ELEVENLABS_API_KEY") {
        return Ok(Some(key));
    }

    Ok(None)
}

fn synthesize(tts: &ResolvedTts, text: &str) -> Result<PathBuf> {
    let url = format!(
        "{}/v1/text-to-speech/{}?output_format={}",
        tts.base_url, tts.voice_id, tts.output_format
    );
    let body = json!({
        "text": text,
        "model_id": tts.model_id,
        "voice_settings": {
            "stability": 0.4,
            "similarity_boost": 0.8,
        }
    })
    .to_string();
    let out = temp_audio_path(&tts.output_format);

    let output = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg(&url)
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg("Accept: audio/mpeg")
        .arg("-H")
        .arg(format!("xi-api-key: {}", tts.api_key))
        .arg("--max-time")
        .arg(tts.timeout_secs.to_string())
        .arg("-o")
        .arg(&out)
        .arg("-d")
        .arg(body)
        .output()
        .with_context(|| "failed to run curl; install curl or disable TTS")?;

    if !output.status.success() {
        let _ = fs::remove_file(&out);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!(
            "ElevenLabs request failed ({}): {}",
            output.status,
            if stderr.is_empty() {
                "no stderr".to_string()
            } else {
                stderr
            }
        ));
    }
    if fs::metadata(&out).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = fs::remove_file(&out);
        return Err(anyhow!("ElevenLabs returned an empty audio file"));
    }
    Ok(out)
}

fn play(path: &Path) -> Result<()> {
    let candidates: [(&str, &[&str]); 6] = [
        ("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"]),
        ("mpg123", &["-q"]),
        ("mplayer", &["-really-quiet"]),
        ("play", &["-q"]),
        ("paplay", &[]),
        ("aplay", &[]),
    ];

    for (bin, args) in candidates {
        match Command::new(bin)
            .args(args)
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) if status.success() => {
                let _ = fs::remove_file(path);
                return Ok(());
            }
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        }
    }

    Err(anyhow!(
        "no working audio player found; generated audio saved at {}",
        path.display()
    ))
}

fn temp_audio_path(output_format: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let ext = output_format.split('_').next().unwrap_or("mp3");
    std::env::temp_dir().join(format!(
        "deckhand-tts-{}-{}.{}",
        std::process::id(),
        nanos,
        ext
    ))
}

fn spoken_text(command: &str, message: &str) -> String {
    let command = command.replace('_', " ");
    let message = message.trim().trim_end_matches('.');
    format!("Deckhand {}: {}.", command, message)
}

fn command_enabled(announce: &[String], command: &str) -> bool {
    let wanted = command.replace('-', "_");
    announce.iter().any(|name| name.replace('-', "_") == wanted)
}

fn resolve_api_key_from_file(path: &Path, key: &str) -> Result<Option<String>> {
    key_from_file(path, key)
}

fn key_from_file(path: &Path, key: &str) -> Result<Option<String>> {
    let map = parse_env_file(path)?;
    Ok(map.get(key).cloned().and_then(|v| clean_key(Some(v))))
}

fn parse_env_file(path: &Path) -> Result<HashMap<String, String>> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let mut map = HashMap::new();
            for line in text.lines() {
                if let Some((k, v)) = parse_env_line(line) {
                    map.insert(k, v);
                }
            }
            Ok(map)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
        Err(e) => Err(e).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn parse_env_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let without_export = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
    if let Some((k, v)) = without_export.split_once('=') {
        let key = k.trim();
        if key.is_empty() {
            return None;
        }
        return Some((key.to_string(), clean_value(v)));
    }
    parse_fish_set(without_export)
}

fn parse_fish_set(line: &str) -> Option<(String, String)> {
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("set") {
        return None;
    }
    let rest: Vec<&str> = tokens.collect();
    let mut key_value = rest.iter().copied().filter(|t| !t.starts_with('-'));
    let key = key_value.next()?;
    let value_parts: Vec<&str> = key_value.collect();
    if value_parts.is_empty() {
        return None;
    }
    Some((key.to_string(), clean_value(&value_parts.join(" "))))
}

fn clean_value(raw: &str) -> String {
    let value = raw.trim();
    let bytes = value.as_bytes();
    if value.len() >= 2 && (bytes[0] == b'"' || bytes[0] == b'\'') {
        let quote = bytes[0];
        if let Some(end) = value[1..].find(quote as char) {
            return value[1..1 + end].to_string();
        }
    }

    let mut value = value.to_string();
    if let Some((before, _)) = value.split_once(" #") {
        value = before.trim().to_string();
    }
    value
}

fn clean_key(key: Option<String>) -> Option<String> {
    key.and_then(|k| {
        let trimmed = k.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(|v| clean_key(Some(v)))
}

fn key_from_shell_files(key: &str) -> Option<String> {
    let Some(home) = home_dir() else {
        return None;
    };
    for file in [".bashrc", ".zshrc", ".profile"] {
        if let Ok(Some(value)) = resolve_api_key_from_file(&home.join(file), key) {
            return Some(value);
        }
    }
    None
}

fn user_config_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("deckhand").join("deckhand.toml"));
    }
    home_dir().map(|h| h.join(".config").join("deckhand").join("deckhand.toml"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str, contents: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("deckhand-tts-test-{}-{}", std::process::id(), name));
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        path
    }

    #[test]
    fn parses_env_assignments() {
        let path = temp_file(
            "env",
            r#"
# comment
export DECKHAND_TTS_API_KEY="deck-key"
ELEVENLABS_API_KEY='el-key' # inline comment
EMPTY=
"#,
        );
        let map = parse_env_file(&path).unwrap();
        assert_eq!(map.get("DECKHAND_TTS_API_KEY").unwrap(), "deck-key");
        assert_eq!(map.get("ELEVENLABS_API_KEY").unwrap(), "el-key");
        assert_eq!(map.get("EMPTY").unwrap(), "");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parses_fish_set_lines() {
        let parsed = parse_env_line("set -gx ELEVENLABS_API_KEY 'fish-key'");
        assert_eq!(
            parsed,
            Some(("ELEVENLABS_API_KEY".to_string(), "fish-key".to_string()))
        );
    }

    #[test]
    fn command_matching_normalizes_hyphens() {
        let announce = vec!["auto_clean".to_string(), "clean".to_string()];
        assert!(command_enabled(&announce, "auto-clean"));
        assert!(command_enabled(&announce, "clean"));
        assert!(!command_enabled(&announce, "status"));
    }

    #[test]
    fn spoken_text_adds_period() {
        assert_eq!(
            spoken_text("auto_clean", "finished"),
            "Deckhand auto clean: finished."
        );
        assert_eq!(
            spoken_text("clean", "dry run finished."),
            "Deckhand clean: dry run finished."
        );
    }

    #[test]
    fn cli_key_wins_before_config_and_env() {
        let cfg = Config::default();
        let key = resolve_api_key(&cfg, Some("cli-key".to_string())).unwrap();
        assert_eq!(key.as_deref(), Some("cli-key"));
    }

    #[test]
    fn project_env_key_is_read() {
        let dir = std::env::temp_dir().join(format!("deckhand-tts-project-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".env"), "ELEVENLABS_API_KEY=project-env-key\n").unwrap();
        let mut cfg = Config::default();
        cfg.workspace.path = dir.clone();

        let key = resolve_api_key(&cfg, None).unwrap();
        assert_eq!(key.as_deref(), Some("project-env-key"));

        let _ = fs::remove_dir_all(dir);
    }
}
