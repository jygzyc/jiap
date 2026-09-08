//! `DECX_HOME` layout and `config.json` management.
//!
//! All CLI data (config, projects, logs, downloads, installed server) lives
//! under one root: `DECX_HOME` when set, otherwise `~/.decx`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::DecxResult;
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

/// `config.json` — installed server version and the default server port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_server_jar")]
    pub server_jar: ServerJarConfig,
    #[serde(default = "default_server")]
    pub server: ServerConfig,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            server_jar: default_server_jar(),
            server: default_server(),
        }
    }
}

impl Config {
    /// Load `config.json` from a home directory, falling back to defaults for
    /// missing or malformed files.
    pub fn load(home: &Path) -> Self {
        let path = home.join("config.json");
        match fsx::read_json(&path) {
            Ok(Some(value)) => serde_json::from_value(value).unwrap_or_default(),
            _ => Self::default(),
        }
    }

    /// Persist to `<home>/config.json` atomically.
    pub fn save(&self, home: &Path) -> DecxResult<()> {
        fsx::atomic_write_json(&home.join("config.json"), &serde_json::to_value(self).unwrap_or(json!({})))
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
        let cfg = Config::load(&tmp);
        assert_eq!(cfg.server.default_port, DEFAULT_SERVER_PORT);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("decx-cfg-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let mut cfg = Config::default();
        cfg.update_server_version(&tmp, "9.9.9").unwrap();
        let loaded = Config::load(&tmp);
        assert_eq!(loaded.server_jar.version, "9.9.9");
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
