//! Kuna adapter — the reference template for command-engine adapters.
//!
//! Kuna (<https://github.com/Noelo-Lab/kuna>) is an agent-first binary
//! decompiler with a one-shot CLI:
//!
//! ```text
//! kuna decompile <file> <function>   # decompiled C on stdout
//! kuna decompile-project <file>      # whole-binary analysis
//! ```
//!
//! Adding a new engine means copying this file and adjusting four things:
//!
//! 1. identity — `id` / `description` / `kind` / `capabilities`
//! 2. binary discovery — `find_kuna` (env override + PATH)
//! 3. the analyze command — `build_command` (runs as a monitored background
//!    job during `project open`; its exit code drives the project state)
//! 4. endpoint handlers — `query` (DECX endpoint → engine invocation)
//!
//! plus one line in `adapters/mod.rs::builtin`. Project supervision,
//! monitoring, `project check`, and the `decx code` routing pick it up
//! automatically.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::error::{DecxError, DecxResult};
use crate::project::Project;

use crate::engine::{execute_query, unsupported_endpoint, Engine, EngineKind, TargetSpec};

pub struct Kuna;

/// `DECX_KUNA` (file or its directory) wins; otherwise `kuna` from PATH.
fn find_kuna() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("DECX_KUNA") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.is_file() {
                return Some(candidate);
            }
            let name = if cfg!(windows) { "kuna.exe" } else { "kuna" };
            let from_dir = candidate.join(name);
            if from_dir.exists() {
                return Some(from_dir);
            }
        }
    }
    crate::engine::resolve_program("kuna")
}

impl Engine for Kuna {
    fn id(&self) -> &'static str {
        "kuna"
    }

    fn description(&self) -> &'static str {
        "Kuna binary decompiler (agent-first, Ghidra-derived; DECX_KUNA overrides the binary)"
    }

    fn kind(&self) -> EngineKind {
        EngineKind::Command
    }

    fn capabilities(&self) -> &'static [&'static str] {
        &["get_method_source", "get_class_source"]
    }

    fn resolve_binary(&self, _home: &Path) -> DecxResult<PathBuf> {
        find_kuna().ok_or_else(|| {
            DecxError::file(
                "kuna not found: install it on PATH or set DECX_KUNA (https://github.com/Noelo-Lab/kuna)",
                None,
            )
        })
    }

    /// `project open` runs the whole-binary analysis as a background job.
    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<Command> {
        let mut cmd = Command::new(binary);
        cmd.arg("decompile-project").arg(&spec.target);
        Ok(cmd)
    }

    /// The unified query handlers: DECX endpoint → kuna invocation. stdout is
    /// wrapped in the DECX envelope by [`execute_query`].
    fn query(&self, project: &Project, endpoint: &str, key: Option<&str>) -> DecxResult<Value> {
        let mut cmd = match endpoint {
            "get_method_source" => {
                let mut cmd = Command::new(self.resolve_binary(Path::new(""))?);
                cmd.arg("decompile").arg(&project.file).arg(key.unwrap_or_default());
                cmd
            }
            "get_class_source" => {
                let mut cmd = Command::new(self.resolve_binary(Path::new(""))?);
                cmd.arg("decompile-project").arg(&project.file);
                cmd
            }
            other => return Err(unsupported_endpoint(self.id(), self.capabilities(), other)),
        };
        execute_query(self.id(), &mut cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_spec() -> TargetSpec {
        TargetSpec {
            target: PathBuf::from("/tmp/a.out"),
            port: 0,
            scripts: vec![],
            passthrough: vec![],
        }
    }

    fn project() -> Project {
        Project {
            name: "kb".into(),
            hash: "h".into(),
            file: PathBuf::from("/tmp/a.out"),
            engine: "kuna".into(),
            engine_kind: "command".into(),
            pid: 0,
            port: 0,
            scripts: vec![],
            log_path: None,
            created_at_ms: 0,
            observed: Default::default(),
        }
    }

    #[test]
    fn identity_and_capabilities() {
        let kuna = Kuna;
        assert_eq!(kuna.id(), "kuna");
        assert_eq!(kuna.kind(), EngineKind::Command);
        assert!(kuna.capabilities().contains(&"get_method_source"));
    }

    #[test]
    fn analyze_command_renders_target() {
        let cmd = Kuna.build_command(Path::new("/usr/bin/kuna"), &target_spec()).unwrap();
        let argv: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(argv, vec!["decompile-project".to_string(), "/tmp/a.out".to_string()]);
    }

    #[test]
    fn unsupported_endpoint_lists_capabilities() {
        let err = Kuna
            .query(&project(), "get_classes", None)
            .expect_err("get_classes has no handler");
        assert_eq!(err.code, "UNSUPPORTED_BY_ENGINE");
        assert!(err.message.contains("get_method_source"), "{}", err.message);
    }

    #[test]
    fn env_override_locates_binary() {
        let tmp = std::env::temp_dir().join(format!("decx-kuna-{}.exe", std::process::id()));
        std::fs::write(&tmp, b"#!/bin/sh\n").unwrap();
        // SAFETY: single-threaded test; save/restore to avoid polluting others.
        let saved = std::env::var("DECX_KUNA").ok();
        std::env::set_var("DECX_KUNA", &tmp);
        assert_eq!(find_kuna().as_deref(), Some(tmp.as_path()));
        match saved {
            Some(v) => std::env::set_var("DECX_KUNA", v),
            None => std::env::remove_var("DECX_KUNA"),
        }
        let _ = std::fs::remove_file(&tmp);
    }
}
