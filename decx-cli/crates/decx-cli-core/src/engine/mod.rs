//! Pluggable analysis engine backends.
//!
//! An [`Engine`] knows how to locate its server binary and assemble the
//! detached spawn command for one analysis target. Built-in backends:
//! `jvm` (decx-server.jar under a JVM) and `native` (decx-native-server).
//! New backends implement the trait and register in the [`EngineRegistry`].

pub mod jvm;
pub mod launcher;
pub mod native;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};

pub use jvm::JvmEngine;
pub use native::NativeEngine;

/// Everything an engine needs to build one server spawn.
pub struct TargetSpec {
    pub target: PathBuf,
    pub port: u16,
    pub scripts: Vec<String>,
    /// jadx passthrough args (jvm only; ignored by other engines).
    pub passthrough: Vec<String>,
}

pub trait Engine: Send + Sync {
    /// Engine id used on the wire and in project records (`jvm`, `native`).
    fn id(&self) -> &'static str;

    fn description(&self) -> &'static str;

    /// Locate the server binary; fail with an actionable message when absent.
    fn resolve_binary(&self, home: &Path) -> DecxResult<PathBuf>;

    /// Validate options that not every engine supports (e.g. `--script` is
    /// JVM-only).
    fn validate(&self, spec: &TargetSpec) -> DecxResult<()> {
        let _ = spec;
        Ok(())
    }

    /// Assemble the spawn command for one target.
    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<std::process::Command>;

    /// Discovery status for `process check`.
    fn status_info(&self, home: &Path) -> Value {
        match self.resolve_binary(home) {
            Ok(path) => json!({ "ok": true, "info": path.display().to_string() }),
            Err(err) => json!({ "ok": false, "info": err.message }),
        }
    }
}

/// Registry of engine backends with default resolution (`DECX_ENGINE` env,
/// falling back to `jvm`).
pub struct EngineRegistry {
    engines: Vec<std::sync::Arc<dyn Engine>>,
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self {
            engines: vec![std::sync::Arc::new(JvmEngine), std::sync::Arc::new(NativeEngine)],
        }
    }

    pub fn register(&mut self, engine: std::sync::Arc<dyn Engine>) {
        self.engines.push(engine);
    }

    pub fn get(&self, id: &str) -> Option<std::sync::Arc<dyn Engine>> {
        self.engines.iter().find(|e| e.id() == id).cloned()
    }

    /// Resolve the default engine id from `DECX_ENGINE` (fallback `jvm`).
    pub fn default_engine_id() -> String {
        std::env::var("DECX_ENGINE")
            .ok()
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "jvm".to_string())
    }

    pub fn resolve(&self, id: Option<&str>) -> DecxResult<std::sync::Arc<dyn Engine>> {
        let id = id.map(str::to_string).unwrap_or_else(Self::default_engine_id);
        self.get(&id).ok_or_else(|| {
            DecxError::usage(format!(
                "Unknown engine '{id}' (available: {})",
                self.engines.iter().map(|e| e.id()).collect::<Vec<_>>().join(", ")
            ))
        })
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.engines.iter().map(|e| e.id()).collect()
    }
}
