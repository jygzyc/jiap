//! Native engine backend: `decx-native-server <target> --port N` (the
//! zero-dependency Rust engine workspace in `decx-native/`).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{DecxError, DecxResult};

use crate::engine::{Engine, EngineKind, TargetSpec};

pub struct NativeEngine;

pub const BINARY_BASENAME: &str = "decx-native-server";

fn binary_name() -> String {
    if cfg!(windows) {
        format!("{BINARY_BASENAME}.exe")
    } else {
        BINARY_BASENAME.to_string()
    }
}

/// Find decx-native-server: `DECX_NATIVE_SERVER` env (file or dir) >
/// `<home>/bin/decx-native-server[.exe]` > a dev checkout
/// `decx-native/target/release/` near the current directory.
pub fn find_decx_native_server(home: &Path) -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("DECX_NATIVE_SERVER") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.is_file() {
                return Some(candidate);
            }
            let from_dir = candidate.join(binary_name());
            if from_dir.exists() {
                return Some(from_dir);
            }
        }
    }
    let installed = home.join("bin").join(binary_name());
    if installed.exists() {
        return Some(installed);
    }
    // Dev checkout: walk up from the working directory looking for the
    // decx-native workspace build output.
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..4 {
        let candidate = dir.join("decx-native").join("target").join("release").join(binary_name());
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

impl Engine for NativeEngine {
    fn id(&self) -> &'static str {
        "native"
    }

    fn description(&self) -> &'static str {
        "decx-native-server (pure-Rust engine, no JVM; --script unsupported)"
    }

    fn kind(&self) -> EngineKind {
        EngineKind::Server
    }

    fn resolve_binary(&self, home: &Path) -> DecxResult<PathBuf> {
        find_decx_native_server(home).ok_or_else(|| {
            DecxError::file(
                "decx-native-server not found. Set DECX_NATIVE_SERVER, place it under \
                 <DECX_HOME>/bin, or build the decx-native workspace (cargo build --release).",
                None,
            )
        })
    }

    fn validate(&self, spec: &TargetSpec) -> DecxResult<()> {
        if !spec.scripts.is_empty() {
            return Err(DecxError::usage(
                "--script is only supported by the jvm engine; the native engine has no Jadx script runtime",
            ));
        }
        Ok(())
    }

    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<Command> {
        // jadx passthrough flags are JVM-specific and intentionally ignored.
        let mut cmd = Command::new(binary);
        cmd.arg(&spec.target).arg("--port").arg(spec.port.to_string());
        Ok(cmd)
    }
}
