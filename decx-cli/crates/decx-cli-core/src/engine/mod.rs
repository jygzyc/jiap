//! Pluggable analysis engine adapters — the unified decompiler entry point.
//!
//! The opencli adapter model applied to decompiler backends: every engine is
//! one self-contained adapter file under [`adapters`] (identity, binary
//! discovery, launch command, and — for command engines — endpoint handlers),
//! registered with a single line in [`adapters::builtin`]. The runtime owns
//! everything else: project supervision, background monitoring, argument
//! parsing, the DECX result envelope, and exit codes.
//!
//! Two adapter kinds:
//!
//! - [`EngineKind::Server`] — a long-lived HTTP server speaking the DECX
//!   contract (built-ins: `jvm` decx-server.jar, `native`
//!   decx-native-server). Queries flow through [`crate::client::DecxClient`].
//! - [`EngineKind::Command`] — a one-shot CLI decompiler (built-in: `kuna`,
//!   the documented template). `project open` runs the adapter's analyze
//!   command as a monitored background job whose exit code drives the
//!   project state machine; each analysis endpoint maps to a [`Engine::query`]
//!   handler whose stdout is wrapped in the DECX envelope.
//!
//! To add an engine: copy `adapters/kuna.rs`, adjust identity / discovery /
//! analyze command / handlers, and add one line to `adapters::builtin()`.

pub mod adapters;
pub mod launcher;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    /// Long-lived HTTP server speaking the DECX contract.
    Server,
    /// One-shot CLI decompiler; endpoints map to [`Engine::query`] handlers.
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

/// Everything an adapter needs to build one launch.
pub struct TargetSpec {
    pub target: PathBuf,
    pub port: u16,
    pub scripts: Vec<String>,
    /// jadx passthrough args (jvm only; ignored by other engines).
    pub passthrough: Vec<String>,
}

/// The unified engine protocol — the `cli({ ... func })` declaration of
/// engines. Adapters are pure: they locate their binary, build commands, and
/// answer queries; the runtime supervises what it spawned.
pub trait Engine: Send + Sync {
    /// Engine id used on the wire and in project records (`jvm`, `native`,
    /// `kuna`, ...).
    fn id(&self) -> &'static str;

    fn description(&self) -> &'static str {
        ""
    }

    fn kind(&self) -> EngineKind;

    /// Analysis endpoints this adapter answers (command engines); used by
    /// `decx engine show` and unsupported-endpoint errors. Server engines
    /// answer the full DECX endpoint set over HTTP.
    fn capabilities(&self) -> &'static [&'static str] {
        &[]
    }

    /// Locate the engine binary; fail with an actionable message when absent.
    fn resolve_binary(&self, home: &Path) -> DecxResult<PathBuf>;

    /// Validate launch options the adapter does not support (e.g. `--script`
    /// is JVM-only).
    fn validate(&self, spec: &TargetSpec) -> DecxResult<()> {
        let _ = spec;
        Ok(())
    }

    /// The launch command: the server spawn line for Server kind, the
    /// analyze job for Command kind.
    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<Command>;

    /// Answer one analysis endpoint for a command-engine project (stdout is
    /// expected to be the decompiled artifact). Server engines serve over
    /// HTTP and keep this default.
    fn query(&self, _project: &Session, endpoint: &str, _key: Option<&str>) -> DecxResult<Value> {
        Err(unsupported_endpoint(self.id(), self.capabilities(), endpoint))
    }

    /// Discovery status for `project check` / `self status`.
    fn status_info(&self, home: &Path) -> Value {
        match self.resolve_binary(home) {
            Ok(path) => json!({ "ok": true, "info": path.display().to_string() }),
            Err(err) => json!({ "ok": false, "info": err.message }),
        }
    }
}

/// The standard error for an endpoint an adapter does not implement.
pub fn unsupported_endpoint(engine_id: &str, capabilities: &[&str], endpoint: &str) -> DecxError {
    DecxError::server(
        "UNSUPPORTED_BY_ENGINE",
        format!(
            "engine '{engine_id}' does not implement '{endpoint}'{}",
            if capabilities.is_empty() {
                String::new()
            } else {
                format!("; supported: {}", capabilities.join(", "))
            }
        ),
    )
}

/// Run one command-engine query handler and wrap stdout in the DECX envelope
/// — the opencli `func(args) → rows` contract: handlers only produce output.
pub fn execute_query(engine_id: &str, cmd: &mut Command) -> DecxResult<Value> {
    let output = crate::spawn::run_with_timeout(cmd, Duration::from_secs(300))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(DecxError::server(
            "ENGINE_QUERY_FAILED",
            format!(
                "engine '{engine_id}' query failed (exit {}): {}",
                output.status.code().unwrap_or(-1),
                if stderr.is_empty() { "<no stderr>" } else { &stderr }
            ),
        ));
    }
    Ok(json!({
        "code": "OK",
        "data": { "source": String::from_utf8_lossy(&output.stdout) },
        "meta": { "engine": engine_id, "mode": "command" },
    }))
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
        // Accept both `kuna` and `kuna.exe` spellings.
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

    /// Discovery status for every adapter (for `session check` /
    /// `self status`): binary path / kind / capabilities per engine id.
    pub fn status(&self, home: &Path) -> Value {
        let mut map = serde_json::Map::new();
        for id in self.ids() {
            let Some(engine) = self.get(id) else { continue };
            let mut info = engine.status_info(home);
            info["kind"] = json!(engine.kind().as_str());
            if engine.kind() == EngineKind::Command {
                info["capabilities"] = json!(engine.capabilities());
            }
            if !engine.description().is_empty() {
                info["description"] = json!(engine.description());
            }
            map.insert(id.to_string(), info);
        }
        Value::Object(map)
    }
}
