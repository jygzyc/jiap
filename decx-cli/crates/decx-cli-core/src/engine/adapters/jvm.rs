//! JVM engine adapter: `java -jar decx-server.jar <target> --port N` — the
//! JADX-based server backend, including the jadx passthrough normalization
//! (a direct port of the TypeScript launcher, with its unit tests).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use crate::error::{DecxError, DecxResult};
use crate::engine::{Engine, EngineKind, TargetSpec};
use crate::spawn::default_java_heap;

pub struct JvmEngine;

pub const JAR_NAME: &str = "decx-server.jar";

/// Drop the `printable` token from a `--rename-flags` value so obfuscated
/// Unicode identifiers survive decompilation. `None` when the value cannot be
/// parsed safely — the caller then leaves the user's spelling untouched.
pub fn sanitize_rename_flags_value(value: &str) -> Option<String> {
    let raw = value.trim();
    if raw.is_empty() {
        return None;
    }
    let upper = raw.to_uppercase();
    if upper == "NONE" {
        return Some("NONE".to_string());
    }
    if upper == "ALL" {
        return Some("CASE,VALID".to_string());
    }
    let tokens: Vec<&str> = raw.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
    if tokens.is_empty()
        || !tokens
            .iter()
            .all(|t| matches!(t.to_uppercase().as_str(), "CASE" | "VALID" | "PRINTABLE" | "ALL"))
    {
        return None;
    }
    let kept: Vec<&str> = tokens
        .into_iter()
        .filter(|t| !t.eq_ignore_ascii_case("printable"))
        .collect();
    Some(if kept.is_empty() { "NONE".to_string() } else { kept.join(",").to_uppercase() })
}

const RENAME_FLAGS_ARGS: [&str; 2] = ["--rename-flags", "-rf"];

fn has_rename_flags_arg(args: &[String]) -> bool {
    args.iter().any(|arg| {
        RENAME_FLAGS_ARGS
            .iter()
            .any(|name| arg == name || arg.strip_prefix(&format!("{name}=")).is_some())
    })
}

fn strip_printable_rename_flag(args: &[String]) -> Vec<String> {
    let mut result = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let eq_form = RENAME_FLAGS_ARGS
            .iter()
            .find_map(|name| arg.strip_prefix(&format!("{name}=")).map(|v| (name.to_string(), v.to_string())));
        if RENAME_FLAGS_ARGS.contains(&arg.as_str()) {
            let value = args.get(i + 1).map(String::as_str).unwrap_or("");
            result.push(arg.clone());
            result.push(sanitize_rename_flags_value(value).unwrap_or_else(|| value.to_string()));
            i += 2;
        } else if let Some((name, value)) = eq_form {
            let sanitized = sanitize_rename_flags_value(&value).unwrap_or(value);
            result.push(format!("{name}={sanitized}"));
            i += 1;
        } else {
            result.push(arg.clone());
            i += 1;
        }
    }
    result
}

/// Normalize jadx passthrough args exactly like the TypeScript CLI:
/// strip `--deobf`, guarantee `--show-bad-code`, `--no-imports`,
/// `-Pdex-input.verify-checksum=no`, and default `--rename-flags case,valid`
/// (DECX queries original symbol names; `printable` would hide obfuscated
/// Unicode identifiers behind `m0`-style aliases).
pub fn normalize_jadx_passthrough_args(args: &[String]) -> Vec<String> {
    let filtered: Vec<String> = args.iter().filter(|a| a.as_str() != "--deobf").cloned().collect();
    let mut result = strip_printable_rename_flag(&filtered);
    for required in ["--show-bad-code", "--no-imports", "-Pdex-input.verify-checksum=no"] {
        if !result.iter().any(|a| a == required) {
            result.push(required.to_string());
        }
    }
    if !has_rename_flags_arg(&result) {
        result.push("--rename-flags".to_string());
        result.push("case,valid".to_string());
    }
    result
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rename_flags() {
        assert_eq!(sanitize_rename_flags_value("case,valid").as_deref(), Some("CASE,VALID"));
        assert_eq!(sanitize_rename_flags_value("CASE, PRINTABLE").as_deref(), Some("CASE"));
        assert_eq!(sanitize_rename_flags_value("all").as_deref(), Some("CASE,VALID"));
        assert_eq!(sanitize_rename_flags_value("printable").as_deref(), Some("NONE"));
        assert_eq!(sanitize_rename_flags_value("none").as_deref(), Some("NONE"));
        assert_eq!(sanitize_rename_flags_value("bogus"), None);
        assert_eq!(sanitize_rename_flags_value(""), None);
    }

    #[test]
    fn normalize_passthrough_args() {
        let args: Vec<String> = vec!["--deobf".into(), "--py-flag".into()];
        let out = normalize_jadx_passthrough_args(&args);
        assert!(!out.contains(&"--deobf".to_string()));
        assert!(out.contains(&"--show-bad-code".to_string()));
        assert!(out.contains(&"--no-imports".to_string()));
        assert!(out.contains(&"-Pdex-input.verify-checksum=no".to_string()));
        assert!(out.windows(2).any(|w| w[0] == "--rename-flags" && w[1] == "case,valid"));
    }

    #[test]
    fn normalize_keeps_user_rename_flags_but_strips_printable() {
        let args: Vec<String> = vec!["--rename-flags".into(), "printable,case".into()];
        let out = normalize_jadx_passthrough_args(&args);
        assert!(out.windows(2).any(|w| w[0] == "--rename-flags" && w[1] == "CASE"));
        // no default injection when user supplied flags
        assert!(!out.windows(2).any(|w| w[0] == "--rename-flags" && w[1] == "case,valid"));

        let eq: Vec<String> = vec!["-rf=printable".into()];
        let out = normalize_jadx_passthrough_args(&eq);
        assert!(out.contains(&"-rf=NONE".to_string()));
    }
}
