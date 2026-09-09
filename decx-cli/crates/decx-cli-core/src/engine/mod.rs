//! Pluggable analysis engine backends — the unified decompiler entry point.
//!
//! An [`Engine`] knows how to launch one analysis backend for a target.
//! Two kinds exist:
//!
//! - [`EngineKind::Server`] — a long-lived HTTP server speaking the DECX
//!   contract (`/health`, `/api/decx/*`). Built-ins: `jvm` (decx-server.jar)
//!   and `native` (decx-native-server).
//! - [`EngineKind::Command`] — a one-shot CLI decompiler such as kuna
//!   (<https://github.com/Noelo-Lab/kuna>). `project open` runs the analyze
//!   command as a monitored background job; `code` queries map to
//!   per-endpoint command templates and return the same JSON envelope.
//!
//! Engines plug in without recompiling: `decx engine register` stores a
//! declarative [`EngineSpec`](foreign::EngineSpec) under
//! `DECX_HOME/engines.json` and [`EngineRegistry`] merges it with the
//! built-ins (the opencli `external register` idea, applied to engines).

pub mod foreign;
pub mod jvm;
pub mod launcher;
pub mod native;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};

pub use foreign::{EngineSpec, EngineStore, ForeignEngine};
pub use jvm::JvmEngine;
pub use native::NativeEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    /// Long-lived HTTP server speaking the DECX contract.
    Server,
    /// One-shot CLI decompiler; queries map to command templates.
    Command,
}

impl EngineKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EngineKind::Server => "server",
            EngineKind::Command => "command",
        }
    }
}

/// Everything an engine needs to build one launch.
pub struct TargetSpec {
    pub target: PathBuf,
    pub port: u16,
    pub scripts: Vec<String>,
    /// jadx passthrough args (jvm only; ignored by other engines).
    pub passthrough: Vec<String>,
}

pub trait Engine: Send + Sync {
    /// Engine id used on the wire and in project records (`jvm`, `native`,
    /// or a registered foreign id like `kuna`).
    fn id(&self) -> &str;

    fn description(&self) -> &str {
        ""
    }

    fn kind(&self) -> EngineKind;

    /// Locate the engine binary; fail with an actionable message when absent.
    fn resolve_binary(&self, home: &Path) -> DecxResult<PathBuf>;

    /// Validate options that not every engine supports (e.g. `--script` is
    /// JVM-only).
    fn validate(&self, spec: &TargetSpec) -> DecxResult<()> {
        let _ = spec;
        Ok(())
    }

    /// Assemble the launch command: the server spawn for Server kind, the
    /// analyze job for Command kind.
    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<std::process::Command>;

    /// Downcast hook for foreign-engine capabilities (query templates).
    fn as_foreign(&self) -> Option<&ForeignEngine> {
        None
    }

    /// Discovery status for `project check` / `self status`.
    fn status_info(&self, home: &Path) -> Value {
        match self.resolve_binary(home) {
            Ok(path) => json!({ "ok": true, "info": path.display().to_string() }),
            Err(err) => json!({ "ok": false, "info": err.message }),
        }
    }
}

/// Registry of engine backends: built-ins plus every engine registered
/// through `decx engine register` (persisted in `engines.json`).
pub struct EngineRegistry {
    engines: Vec<Arc<dyn Engine>>,
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineRegistry {
    /// Built-in engines only (`jvm`, `native`).
    pub fn new() -> Self {
        Self {
            engines: vec![Arc::new(JvmEngine), Arc::new(NativeEngine)],
        }
    }

    /// Built-ins merged with the foreign engines registered under `home`.
    pub fn load(home: &Path) -> Self {
        let mut reg = Self::new();
        for spec in EngineStore::new(home).load() {
            reg.engines.push(Arc::new(ForeignEngine::new(spec)));
        }
        reg
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Engine>> {
        self.engines.iter().find(|e| e.id() == id).cloned()
    }

    /// The registered spec of a foreign engine, if present.
    pub fn foreign_spec(&self, id: &str) -> Option<EngineSpec> {
        self.engines
            .iter()
            .find(|e| e.id() == id)
            .and_then(|e| e.as_foreign())
            .map(|f| f.spec.clone())
    }

    /// Resolve the default engine id from `DECX_ENGINE` (fallback `jvm`).
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
}
