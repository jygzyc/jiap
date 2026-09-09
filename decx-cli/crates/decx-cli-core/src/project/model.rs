//! Project data model: records, observed execution state, and the state
//! machine that turns raw probes into monitored states.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::DecxResult;
use crate::fsx;

/// Lifecycle state of a project's background server, as observed by the
/// project manager's monitors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectState {
    /// Process is alive but the health endpoint has never answered yet.
    #[default]
    Starting,
    /// `/health` reports `status: running`.
    Healthy,
    /// Process is alive but `/health` fails (overloaded, still decompiling
    /// after a restart, hung).
    Unreachable,
    /// The process is gone.
    Stopped,
    /// Never probed (fresh CLI install, no monitor attached yet).
    #[serde(rename = "unknown")]
    Unknown,
}

impl ProjectState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectState::Starting => "starting",
            ProjectState::Healthy => "healthy",
            ProjectState::Unreachable => "unreachable",
            ProjectState::Stopped => "stopped",
            ProjectState::Unknown => "unknown",
        }
    }
}

/// Latest probe results persisted with the project so short-lived CLI
/// invocations can surface monitored state without re-probing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ObservedState {
    pub state: ProjectState,
    pub checked_at_ms: u64,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    #[serde(default)]
    pub detail: Option<String>,
    /// Whether this project has ever reported healthy (drives
    /// Starting-vs-Unreachable classification of failures).
    #[serde(default)]
    pub ever_healthy: bool,
}

/// One managed analysis project: a target file loaded by an analysis engine
/// plus everything needed to reach or stop it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    /// sha256 of the target file — the identity used for session reuse.
    pub hash: String,
    pub file: PathBuf,
    /// Engine backend id: `jvm`, `native`, `kuna`, or another adapter id.
    pub engine: String,
    /// `server` (long-lived HTTP engine) or `command` (one-shot decompiler
    /// whose analyze job ran to completion).
    #[serde(default = "default_engine_kind")]
    pub engine_kind: String,
    pub pid: u32,
    pub port: u16,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub log_path: Option<PathBuf>,
    pub created_at_ms: u64,
    #[serde(default)]
    pub observed: ObservedState,
}

fn default_engine_kind() -> String {
    "server".to_string()
}

impl Project {
    /// A command-engine project is usable while its analysis artifacts are
    /// ready, regardless of the (already exited) analyze pid.
    pub fn is_command_kind(&self) -> bool {
        self.engine_kind == "command"
    }

    pub fn to_summary(&self) -> Value {
        let mut obj = serde_json::json!({
            "name": self.name,
            "hash": self.hash,
            "file": self.file.display().to_string(),
            "engine": self.engine,
            "kind": self.engine_kind,
            "pid": self.pid,
            "port": self.port,
            "created_at": self.created_at_ms,
            "state": self.observed.state.as_str(),
            "checked_at": self.observed.checked_at_ms,
        });
        if let Some(latency) = self.observed.latency_ms {
            obj["latency_ms"] = serde_json::json!(latency);
        }
        if let Some(detail) = &self.observed.detail {
            obj["detail"] = serde_json::json!(detail);
        }
        if !self.scripts.is_empty() {
            obj["scripts"] = serde_json::json!(self.scripts);
        }
        obj
    }
}

/// A recorded state transition emitted by monitors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEvent {
    pub project: String,
    pub at_ms: u64,
    pub from: ProjectState,
    pub to: ProjectState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// One probe observation: PID liveness plus the `/health` outcome.
pub struct ProbeOutcome {
    pub pid_alive: bool,
    /// `Ok(json)` when `/health` answered, `Err` otherwise.
    pub health: DecxResult<Value>,
    pub latency_ms: u64,
}

/// Pure state machine: turn a probe outcome into the next project state.
///
/// - process gone → `Stopped`
/// - health says `running` → `Healthy`
/// - process alive, health answered with another status → `Unreachable`
/// - process alive, health unreachable → `Starting` until first success,
///   `Unreachable` afterwards
pub fn evaluate_state(ever_healthy: bool, probe: &ProbeOutcome) -> (ProjectState, Option<String>, Option<u64>) {
    if !probe.pid_alive {
        return (ProjectState::Stopped, Some("process exited".to_string()), None);
    }
    match &probe.health {
        Ok(value) => {
            let latency = Some(probe.latency_ms);
            match value.get("status").and_then(Value::as_str) {
                Some("running") => (ProjectState::Healthy, None, latency),
                other => (
                    ProjectState::Unreachable,
                    Some(format!("health status: {}", other.unwrap_or("<missing>"))),
                    latency,
                ),
            }
        }
        Err(err) => {
            if ever_healthy {
                (
                    ProjectState::Unreachable,
                    Some(format!("health check failed: {}", err.message)),
                    Some(probe.latency_ms),
                )
            } else {
                (
                    ProjectState::Starting,
                    Some(format!("waiting for server ({})", err.message)),
                    Some(probe.latency_ms),
                )
            }
        }
    }
}

/// Convenience: timestamped now.
pub fn now_ms() -> u64 {
    fsx::now_ms()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DecxError;
    use serde_json::json;

    fn probe(pid_alive: bool, health: DecxResult<Value>) -> ProbeOutcome {
        ProbeOutcome {
            pid_alive,
            health,
            latency_ms: 12,
        }
    }

    #[test]
    fn dead_process_is_stopped() {
        let (state, _, _) = evaluate_state(
            true,
            &probe(false, Err(DecxError::connection("refused"))),
        );
        assert_eq!(state, ProjectState::Stopped);
    }

    #[test]
    fn running_health_is_healthy() {
        let (state, detail, latency) = evaluate_state(
            false,
            &probe(true, Ok(json!({ "status": "running" }))),
        );
        assert_eq!(state, ProjectState::Healthy);
        assert!(detail.is_none());
        assert_eq!(latency, Some(12));
    }

    #[test]
    fn failed_health_before_first_success_is_starting() {
        let (state, _, _) = evaluate_state(
            false,
            &probe(true, Err(DecxError::connection("refused"))),
        );
        assert_eq!(state, ProjectState::Starting);
    }

    #[test]
    fn failed_health_after_success_is_unreachable() {
        let (state, detail, _) = evaluate_state(
            true,
            &probe(true, Err(DecxError::timeout("timed out"))),
        );
        assert_eq!(state, ProjectState::Unreachable);
        assert!(detail.unwrap().contains("timed out"));
    }

    #[test]
    fn weird_health_status_is_unreachable() {
        let (state, _, _) = evaluate_state(
            true,
            &probe(true, Ok(json!({ "status": "loading" }))),
        );
        assert_eq!(state, ProjectState::Unreachable);
    }
}
