//! Engine layer — governs the plugged-in tool server backends.
//!
//! Every analysis backend (decx server, decx-native, kuna, future tools) is
//! served to the CLI as a long-lived HTTP server speaking the DECX contract.
//! One self-contained adapter file per engine under [`adapters`] plus one
//! line in the [`adapters::builtin`] manifest; the runtime (`decx-server-sdk`
//! on the engine side, the session layer on the CLI side) owns everything
//! else. To add an engine: copy `adapters/kuna.rs`, adjust identity /
//! discovery / launch command, add one manifest line.

pub mod adapters;
pub mod launcher;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};

/// Everything an adapter needs to build one launch.
pub struct TargetSpec {
    pub target: PathBuf,
    pub port: u16,
    pub scripts: Vec<String>,
    /// jadx passthrough args (jvm only; ignored by other engines).
    pub passthrough: Vec<String>,
}

/// The unified adapter protocol. Adapters are pure: they locate their
/// binary and assemble the server launch command; the session layer
/// supervises what they spawn.
pub trait Engine: Send + Sync {
    /// Engine id used on the wire and in session records (`jvm`, `native`,
    /// `kuna`, ...).
    fn id(&self) -> &'static str;

    fn description(&self) -> &'static str {
        ""
    }

    /// Locate the engine binary; fail with an actionable message when absent.
    fn resolve_binary(&self, home: &Path) -> DecxResult<PathBuf>;

    /// Validate launch options the adapter does not support (e.g. `--script`
    /// is JVM-only).
    fn validate(&self, spec: &TargetSpec) -> DecxResult<()> {
        let _ = spec;
        Ok(())
    }

    /// Assemble the server spawn command.
    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<Command>;

    /// Discovery status for `session check` / `self status`.
    fn status_info(&self, home: &Path) -> Value {
        match self.resolve_binary(home) {
            Ok(path) => json!({ "ok": true, "info": path.display().to_string() }),
            Err(err) => json!({ "ok": false, "info": err.message }),
        }
    }
}

/// Resolve a program the way a shell would: paths pass through, bare names
/// are looked up on PATH (Windows: current directory + `.exe` fallback).
/// std-only.
pub fn resolve_program(program: &str) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }
    if program.contains('/') || program.contains('\\') || Path::new(program).is_absolute() {
        return is_executable_file(Path::new(program)).then(|| PathBuf::from(program));
    }
    let path_var = std::env::var("PATH").ok()?;
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
    if cfg!(windows) {
        // Windows CreateProcess falls back to the current directory.
        dirs.extend(std::env::current_dir().ok());
    }
    dirs.into_iter().find_map(|dir| {
        let direct = dir.join(program);
        is_executable_file(&direct).then_some(direct).or_else(|| {
            // Windows PATH entries may omit the .exe suffix.
            if cfg!(windows) {
                let with_exe = dir.join(format!("{program}.exe"));
                is_executable_file(&with_exe).then_some(with_exe)
            } else {
                None
            }
        })
    })
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if cfg!(windows) {
        path.extension().is_none_or(|e| e == "exe" || e == "cmd" || e == "bat")
    } else {
        true
    }
}

/// Registry of engine adapters, assembled from the [`adapters::builtin`]
/// manifest.
pub struct EngineRegistry {
    engines: Vec<Arc<dyn Engine>>,
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self {
            engines: adapters::builtin(),
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Engine>> {
        self.engines.iter().find(|e| e.id() == id).cloned()
    }

    /// Resolve the default engine id: `DECX_ENGINE` env (fallback `jvm`).
    pub fn default_engine_id() -> String {
        std::env::var("DECX_ENGINE")
            .ok()
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "jvm".to_string())
    }

    pub fn resolve(&self, id: Option<&str>) -> DecxResult<Arc<dyn Engine>> {
        let id = id.map(str::to_string).unwrap_or_else(Self::default_engine_id);
        self.get(&id).ok_or_else(|| {
            DecxError::usage(format!(
                "Unknown engine '{id}' (available: {})",
                self.ids().join(", ")
            ))
        })
    }

    pub fn ids(&self) -> Vec<&str> {
        self.engines.iter().map(|e| e.id()).collect()
    }

    /// Discovery status for every adapter (for `session check` /
    /// `self status`).
    pub fn status(&self, home: &Path) -> Value {
        let mut map = serde_json::Map::new();
        for id in self.ids() {
            let Some(engine) = self.get(id) else { continue };
            let mut info = engine.status_info(home);
            if !engine.description().is_empty() {
                info["description"] = json!(engine.description());
            }
            map.insert(id.to_string(), info);
        }
        Value::Object(map)
    }
}
