//! `DECX_HOME` layout and runtime user settings (`settings.json`).
//!
//! All CLI data (settings, sessions, logs, downloads, installed binaries)
//! lives under one root: `DECX_HOME` when set, otherwise `~/.decx`.
//!
//! This is the *runtime user settings* file — not to be confused with the
//! compile-time engine registry `config.json` at the workspace root, which
//! is parsed by `build.rs` into the command surface.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{DecxError, DecxResult};
use crate::fsx;

/// Resolve the DECX home directory: `DECX_HOME` env override, else `~/.decx`.
pub fn decx_home() -> PathBuf {
    if let Ok(home) = std::env::var("DECX_HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    user_home().join(".decx")
}

fn user_home() -> PathBuf {
    for key in ["DECX_USER_HOME", "USERPROFILE", "HOME"] {
        if let Ok(home) = std::env::var(key) {
            if !home.trim().is_empty() {
                return PathBuf::from(home);
            }
        }
    }
    PathBuf::from(".")
}

/// Join segments under the DECX home directory.
pub fn decx_path(parts: &[&str]) -> PathBuf {
    let mut path = decx_home();
    for part in parts {
        path.push(part);
    }
    path
}

pub const DEFAULT_SERVER_PORT: u16 = 25419;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerJarConfig {
    #[serde(default = "default_version")]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub default_port: u16,
}

fn default_version() -> String {
    "0.0.0".to_string()
}

fn default_port() -> u16 {
    DEFAULT_SERVER_PORT
}

/// External-CLI-tool registration (`decx tools register`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    /// Full argv to spawn, e.g. `["gh", "pr"]`.
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub registered_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDefaults {
    #[serde(default = "default_monitor_interval")]
    pub monitor_interval_secs: u64,
    #[serde(default = "default_open_timeout")]
    pub open_timeout_secs: u64,
}

fn default_monitor_interval() -> u64 {
    5
}

fn default_open_timeout() -> u64 {
    300
}

impl Default for SessionDefaults {
    fn default() -> Self {
        Self {
            monitor_interval_secs: default_monitor_interval(),
            open_timeout_secs: default_open_timeout(),
        }
    }
}

/// `settings.json` — the runtime user settings: CLI defaults, the installed
/// server version, session defaults, and the external-tool registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_settings_version")]
    pub settings_version: u8,
    /// Engine used when `--engine` is absent (`""` = DECX_ENGINE env / jvm).
    #[serde(default)]
    pub default_engine: String,
    /// Output format used when `--format` is absent (`""` = json).
    #[serde(default)]
    pub default_format: String,
    #[serde(default = "default_server_jar")]
    pub server_jar: ServerJarConfig,
    #[serde(default = "default_server")]
    pub server: ServerConfig,
    #[serde(default)]
    pub session: SessionDefaults,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
}

fn default_settings_version() -> u8 {
    1
}

fn default_server_jar() -> ServerJarConfig {
    ServerJarConfig {
        version: default_version(),
    }
}

fn default_server() -> ServerConfig {
    ServerConfig {
        default_port: default_port(),
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_version: default_settings_version(),
            default_engine: String::new(),
            default_format: String::new(),
            server_jar: default_server_jar(),
            server: default_server(),
            session: SessionDefaults::default(),
            tools: Vec::new(),
        }
    }
}

impl Settings {
    /// Load `settings.json` from a home directory, falling back to the
    /// legacy `config.json` name, then to defaults for missing/malformed
    /// files.
    pub fn load(home: &Path) -> Self {
        let path = home.join("settings.json");
        match fsx::read_json(&path) {
            Ok(Some(value)) => serde_json::from_value(value).unwrap_or_default(),
            _ => match fsx::read_json(&home.join("config.json")) {
                Ok(Some(value)) => serde_json::from_value(value).unwrap_or_default(),
                _ => Self::default(),
            },
        }
    }

    /// The effective output format: explicit value > settings > json.
    pub fn effective_format(&self, explicit: Option<&str>) -> Result<crate::output::OutputFormat, DecxError> {
        match explicit.filter(|s| !s.is_empty()) {
            Some(raw) => crate::output::OutputFormat::parse(raw),
            None if !self.default_format.is_empty() => {
                crate::output::OutputFormat::parse(&self.default_format)
            }
            None => Ok(crate::output::OutputFormat::default()),
        }
    }

    /// The effective default engine: explicit value > DECX_ENGINE > settings > jvm.
    pub fn effective_engine(&self, explicit: Option<&str>) -> String {
        let from_flag = explicit.map(str::to_string).filter(|s| !s.is_empty());
        let from_env = std::env::var("DECX_ENGINE")
            .ok()
            .map(|v| v.trim().to_lowercase())
            .filter(|s| !s.is_empty());
        let from_config = Some(self.default_engine.clone()).filter(|s| !s.is_empty());
        from_flag
            .or(from_env)
            .or(from_config)
            .unwrap_or_else(|| "jvm".to_string())
    }

    /// Persist to `<home>/settings.json` atomically.
    pub fn save(&self, home: &Path) -> DecxResult<()> {
        fsx::atomic_write_json(
            &home.join("settings.json"),
            &serde_json::to_value(self).unwrap_or(json!({})),
        )
    }

    pub fn update_server_version(&mut self, home: &Path, version: &str) -> DecxResult<()> {
        self.server_jar.version = version.to_string();
        self.save(home)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_file_returns_defaults() {
        let tmp = std::env::temp_dir().join(format!("decx-cfg-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let cfg = Settings::load(&tmp);
        assert_eq!(cfg.server.default_port, DEFAULT_SERVER_PORT);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("decx-cfg-test2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let mut cfg = Settings::default();
        cfg.update_server_version(&tmp, "9.9.9").unwrap();
        let loaded = Settings::load(&tmp);
        assert_eq!(loaded.server_jar.version, "9.9.9");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn legacy_config_json_still_loads() {
        let tmp = std::env::temp_dir().join(format!("decx-cfg-test3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("config.json"),
            r#"{"server": {"default_port": 31000}}"#,
        )
        .unwrap();
        let cfg = Settings::load(&tmp);
        assert_eq!(cfg.server.default_port, 31000);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn decx_home_env_override() {
        // SAFETY: tests run single-threaded per process for env mutation here;
        // cargo test runs this in one thread but env is global. Guard with a
        // save/restore to avoid polluting other tests.
        let saved = std::env::var("DECX_HOME").ok();
        std::env::set_var("DECX_HOME", "/tmp/decx-override");
        assert_eq!(decx_home(), PathBuf::from("/tmp/decx-override"));
        match saved {
            Some(v) => std::env::set_var("DECX_HOME", v),
            None => std::env::remove_var("DECX_HOME"),
        }
    }
}
