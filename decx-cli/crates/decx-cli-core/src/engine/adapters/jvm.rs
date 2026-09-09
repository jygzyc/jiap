//! JVM engine backend: `java -jar decx-server.jar <target> --port N`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use crate::error::{DecxError, DecxResult};
use crate::engine::launcher::{default_java_heap, normalize_jadx_passthrough_args};
use crate::engine::{Engine, EngineKind, TargetSpec};

pub struct JvmEngine;

pub const JAR_NAME: &str = "decx-server.jar";

/// Find decx-server.jar: `DECX_SERVER_HOME` env (file or dir) first, then
/// `<home>/bin/decx-server.jar`.
pub fn find_decx_server_jar(home: &Path) -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("DECX_SERVER_HOME") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.is_file() && candidate.extension().and_then(|e| e.to_str()) == Some("jar") {
                return Some(candidate);
            }
            let from_dir = candidate.join(JAR_NAME);
            if from_dir.exists() {
                return Some(from_dir);
            }
        }
    }
    let installed = home.join("bin").join(JAR_NAME);
    installed.exists().then_some(installed)
}

impl Engine for JvmEngine {
    fn id(&self) -> &'static str {
        "jvm"
    }

    fn description(&self) -> &'static str {
        "decx-server.jar under a JVM (JADX-based, supports --script)"
    }

    fn kind(&self) -> EngineKind {
        EngineKind::Server
    }

    fn resolve_binary(&self, home: &Path) -> DecxResult<PathBuf> {
        find_decx_server_jar(home).ok_or_else(|| {
            DecxError::file(
                "decx-server.jar not found. Run 'decx self install' to install.",
                None,
            )
        })
    }

    fn validate(&self, spec: &TargetSpec) -> DecxResult<()> {
        for script in &spec.scripts {
            let path = PathBuf::from(script);
            if !path.exists() {
                return Err(DecxError::file(
                    format!("Script file not found: {}", path.display()),
                    Some(path.display().to_string()),
                ));
            }
        }
        Ok(())
    }

    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<Command> {
        let mut cmd = Command::new("java");
        cmd.arg(format!("-Xmx{}", default_java_heap()))
            .arg("-jar")
            .arg(binary)
            .arg(&spec.target)
            .arg("--port")
            .arg(spec.port.to_string())
            .args(normalize_jadx_passthrough_args(&spec.passthrough))
            .args(&spec.scripts); // .jadx.kts scripts are positional inputs
        Ok(cmd)
    }

    fn status_info(&self, home: &Path) -> serde_json::Value {
        match self.resolve_binary(home) {
            Ok(path) => {
                let version = crate::installer::read_jar_version_property(&path);
                json!({
                    "ok": true,
                    "info": path.display().to_string(),
                    "version": version,
                })
            }
            Err(err) => json!({ "ok": false, "info": err.message }),
        }
    }
}
