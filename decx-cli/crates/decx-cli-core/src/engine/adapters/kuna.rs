//! Kuna adapter — the reference template for engine adapters.
//!
//! Kuna is imported at the code level: the analyzer lives in the
//! `decx-kuna` crate and `decx-kuna-server` (built by the same workspace
//! `cargo build`) serves the DECX HTTP contract through `decx-server-sdk`.
//! The adapter therefore only has to locate that server binary and launch it;
//! queries flow over HTTP like every other server engine.
//!
//! Adding a new engine means copying this file and adjusting:
//! 1. identity — `id` / `description` / `capabilities`
//! 2. binary discovery — `find_kuna_server`
//! 3. the launch command — `build_command` (`<server> <target> --port N`)
//! plus one line in `adapters/mod.rs::builtin`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::{Engine, EngineKind, TargetSpec};
use crate::error::{DecxError, DecxResult};

pub struct Kuna;

pub const SERVER_BASENAME: &str = "decx-kuna-server";

fn server_binary_name() -> String {
    if cfg!(windows) {
        format!("{SERVER_BASENAME}.exe")
    } else {
        SERVER_BASENAME.to_string()
    }
}

/// Locate `decx-kuna-server`: `DECX_KUNA_SERVER` env (file or dir) -> next to
/// the running `decx` executable (same `cargo build` output directory) ->
/// `<DECX_HOME>/bin` -> PATH.
pub fn find_kuna_server(home: &Path) -> Option<PathBuf> {
    let name = server_binary_name();
    if let Ok(env_path) = std::env::var("DECX_KUNA_SERVER") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.is_file() {
                return Some(candidate);
            }
            let from_dir = candidate.join(&name);
            if from_dir.exists() {
                return Some(from_dir);
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(sibling) = exe.parent().map(|dir| dir.join(&name)) {
            if sibling.exists() {
                return Some(sibling);
            }
        }
    }
    let installed = home.join("bin").join(&name);
    if installed.exists() {
        return Some(installed);
    }
    crate::engine::resolve_program(SERVER_BASENAME)
}

impl Engine for Kuna {
    fn id(&self) -> &'static str {
        "kuna"
    }

    fn description(&self) -> &'static str {
        "Kuna binary decompiler, code-level adapter served by decx-kuna-server (structural mode)"
    }

    fn kind(&self) -> EngineKind {
        EngineKind::Server
    }

    fn capabilities(&self) -> &'static [&'static str] {
        &[
            "get_classes",
            "get_class_source",
            "get_method_source",
            "search_method",
            "search_global_key",
            "get_strings",
        ]
    }

    fn resolve_binary(&self, home: &Path) -> DecxResult<PathBuf> {
        find_kuna_server(home).ok_or_else(|| {
            DecxError::file(
                "decx-kuna-server not found: build the workspace (cargo build --release), \
                 set DECX_KUNA_SERVER, or place it under <DECX_HOME>/bin",
                None,
            )
        })
    }

    /// `session open` launches the code-level kuna server for the target.
    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<Command> {
        let mut cmd = Command::new(binary);
        cmd.arg(&spec.target).arg("--port").arg(spec.port.to_string());
        Ok(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> TargetSpec {
        TargetSpec {
            target: PathBuf::from("/tmp/a.out"),
            port: 25419,
            scripts: vec![],
            passthrough: vec![],
        }
    }

    #[test]
    fn identity_and_capabilities() {
        let kuna = Kuna;
        assert_eq!(kuna.id(), "kuna");
        assert_eq!(kuna.kind(), EngineKind::Server);
        assert!(kuna.capabilities().contains(&"get_method_source"));
    }

    #[test]
    fn launch_command_renders_target_and_port() {
        let cmd = Kuna.build_command(Path::new("/opt/decx-kuna-server"), &spec()).unwrap();
        let argv: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            argv,
            vec!["/tmp/a.out".to_string(), "--port".to_string(), "25419".to_string()]
        );
    }

    #[test]
    fn sibling_of_current_exe_is_found_first() {
        // The test binary lives in target/debug (or similar); drop a fake
        // decx-kuna-server next to it and expect discovery to find it.
        let exe = std::env::current_exe().unwrap();
        let fake = exe.parent().unwrap().join(server_binary_name());
        std::fs::write(&fake, b"stub").unwrap();
        let home = std::env::temp_dir();
        let found = find_kuna_server(&home);
        let _ = std::fs::remove_file(&fake);
        assert_eq!(found.as_deref(), Some(fake.as_path()));
    }
}
