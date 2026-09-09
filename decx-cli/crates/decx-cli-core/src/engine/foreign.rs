//! Foreign (externally registered) engines — declarative descriptors stored
//! in `DECX_HOME/engines.json`, no recompilation required.
//!
//! A spec is one launch command template plus optional per-endpoint query
//! templates. Templates are argv arrays spawned directly (no shell, no
//! quoting); exact-match tokens may be the placeholders `{target}` (absolute
//! target path), `{port}` (bound server port, server kind), and `{key}`
//! (query templates only: the function/class key).
//!
//! Registering kuna (<https://github.com/Noelo-Lab/kuna>) as a binary
//! decompiler engine:
//!
//! ```text
//! decx engine register kuna -- kuna decompile-project {target}
//! decx engine query kuna get_method_source -- kuna decompile {target} {key}
//! decx engine query kuna get_class_source -- kuna decompile-project {target}
//! decx project open ./a.out --engine kuna
//! decx code method-source main
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{DecxError, DecxResult};
use crate::fsx;

use super::{Engine, EngineKind, TargetSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSpec {
    pub id: String,
    /// `server` (long-lived DECX-contract HTTP server) or `command`
    /// (one-shot CLI decompiler). Defaults to `command`.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Full argv template for the engine launch: the server spawn line
    /// (server kind) or the analyze job (command kind).
    pub command: Vec<String>,
    /// endpoint name → argv template (`{target}`, `{key}`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub queries: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_kind() -> String {
    "command".to_string()
}

impl EngineSpec {
    pub fn kind(&self) -> EngineKind {
        match self.kind.as_str() {
            "server" => EngineKind::Server,
            _ => EngineKind::Command,
        }
    }

    pub fn to_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "kind": self.kind,
            "command": self.command,
            "queries": self.queries.keys().collect::<Vec<_>>(),
            "description": self.description,
        })
    }
}

/// Substitute `{target}` / `{port}` / `{key}` in a template argv.
pub fn render_template(template: &[String], target: &Path, port: u16, key: Option<&str>) -> Vec<String> {
    template
        .iter()
        .map(|token| match token.as_str() {
            "{target}" => target.display().to_string(),
            "{port}" => port.to_string(),
            "{key}" => key.unwrap_or("").to_string(),
            other => other.to_string(),
        })
        .collect()
}

/// Reject unknown placeholders at registration time (they would silently
/// become literal argv at spawn time). `{key}` is query-template-only.
pub fn validate_template(template: &[String], allow_key: bool) -> DecxResult<()> {
    for token in template {
        if token.starts_with('{') && token.ends_with('}') {
            let allowed = match token.as_str() {
                "{target}" | "{port}" => true,
                "{key}" => allow_key,
                _ => false,
            };
            if !allowed {
                let extra = if allow_key { ", {key}" } else { "" };
                return Err(DecxError::usage(format!(
                    "Invalid placeholder '{token}' in command template (available: {{target}}, {{port}}{extra})"
                )));
            }
        }
    }
    Ok(())
}

/// Resolve a program the way a shell would: paths pass through, bare names
/// are looked up on PATH (current directory included on Windows). std-only.
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

/// An [`Engine`] backed by a registered [`EngineSpec`].
pub struct ForeignEngine {
    pub spec: EngineSpec,
}

impl ForeignEngine {
    pub fn new(spec: EngineSpec) -> Self {
        Self { spec }
    }
}

impl Engine for ForeignEngine {
    fn id(&self) -> &str {
        &self.spec.id
    }

    fn description(&self) -> &str {
        self.spec.description.as_deref().unwrap_or("")
    }

    fn kind(&self) -> EngineKind {
        self.spec.kind()
    }

    fn resolve_binary(&self, _home: &Path) -> DecxResult<PathBuf> {
        let program = self.spec.command.first().map(String::as_str).unwrap_or_default();
        resolve_program(program).ok_or_else(|| {
            DecxError::file(
                format!(
                    "engine '{}': command '{program}' not found on PATH (re-register with a resolvable command)",
                    self.spec.id
                ),
                None,
            )
        })
    }

    fn validate(&self, spec: &TargetSpec) -> DecxResult<()> {
        if !spec.scripts.is_empty() {
            return Err(DecxError::usage(format!(
                "--script is jvm-engine only; '{}' is an externally registered engine",
                self.spec.id
            )));
        }
        Ok(())
    }

    fn build_command(&self, binary: &Path, spec: &TargetSpec) -> DecxResult<Command> {
        let argv = render_template(&self.spec.command, &spec.target, spec.port, None);
        let mut cmd = Command::new(binary);
        cmd.args(argv.iter().skip(1));
        Ok(cmd)
    }

    fn as_foreign(&self) -> Option<&ForeignEngine> {
        Some(self)
    }
}

/// Persistence for foreign engines: `DECX_HOME/engines.json`.
pub struct EngineStore {
    path: PathBuf,
}

impl EngineStore {
    pub fn new(home: &Path) -> Self {
        Self {
            path: home.join("engines.json"),
        }
    }

    pub fn load(&self) -> Vec<EngineSpec> {
        match fsx::read_json(&self.path) {
            Ok(Some(value)) => serde_json::from_value(value).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn save(&self, specs: &[EngineSpec]) -> DecxResult<()> {
        fsx::atomic_write_json(&self.path, &serde_json::to_value(specs).unwrap_or_default())
    }

    pub fn get(&self, id: &str) -> Option<EngineSpec> {
        self.load().into_iter().find(|s| s.id == id)
    }

    /// Register (or replace) an engine, rejecting ids the built-ins own.
    pub fn upsert(&self, spec: EngineSpec, builtin_ids: &[&str]) -> DecxResult<EngineSpec> {
        if builtin_ids.contains(&spec.id.as_str()) {
            return Err(DecxError::usage(format!(
                "Engine id '{}' is reserved by a built-in engine",
                spec.id
            )));
        }
        let mut specs = self.load();
        specs.retain(|s| s.id != spec.id);
        let out = spec.clone();
        specs.push(spec);
        self.save(&specs)?;
        Ok(out)
    }

    /// Set one query template on a registered engine (validates placeholders).
    pub fn set_query(&self, id: &str, endpoint: &str, template: Vec<String>) -> DecxResult<EngineSpec> {
        validate_template(&template, true)?;
        let mut specs = self.load();
        let spec = specs
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| DecxError::not_found("ENGINE_NOT_FOUND", format!("Engine not found: {id}")))?;
        spec.queries.insert(endpoint.to_string(), template);
        let out = spec.clone();
        self.save(&specs)?;
        Ok(out)
    }

    pub fn remove(&self, id: &str) -> DecxResult<EngineSpec> {
        let mut specs = self.load();
        let pos = specs
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| DecxError::not_found("ENGINE_NOT_FOUND", format!("Engine not found: {id}")))?;
        let spec = specs.remove(pos);
        self.save(&specs)?;
        Ok(spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("decx-eng-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn t(strings: &[&str]) -> Vec<String> {
        strings.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn template_rendering_and_validation() {
        let argv = render_template(&t(&["kuna", "decompile", "{target}", "{key}"]), Path::new("/a b/c.out"), 0, Some("main"));
        assert_eq!(argv, t(&["kuna", "decompile", "/a b/c.out", "main"]));

        assert!(validate_template(&t(&["x", "{target}", "{port}"]), false).is_ok());
        assert!(validate_template(&t(&["x", "{bogus}"]), true).is_err());
        // {key} is query-template-only
        assert!(validate_template(&t(&["x", "{key}"]), false).is_err());
        assert!(validate_template(&t(&["x", "{key}"]), true).is_ok());
    }

    #[test]
    fn resolves_platform_shell_from_path() {
        let found = if cfg!(windows) {
            resolve_program("cmd")
        } else {
            resolve_program("sh")
        };
        assert!(found.is_some(), "expected the platform shell on PATH");
    }

    #[test]
    fn resolves_explicit_paths() {
        let found = resolve_program("definitely/not/a/real/binary-name-xyz");
        assert!(found.is_none());
    }

    #[test]
    fn store_roundtrip_upsert_query_remove() {
        let home = tmp_home("store");
        let store = EngineStore::new(&home);
        let spec = store
            .upsert(
                EngineSpec {
                    id: "kuna".into(),
                    kind: "command".into(),
                    command: t(&["kuna", "decompile-project", "{target}"]),
                    queries: BTreeMap::new(),
                    description: Some("kuna decompiler".into()),
                },
                &["jvm", "native"],
            )
            .unwrap();
        assert_eq!(spec.id, "kuna");
        assert!(store.get("kuna").is_some());

        // builtin ids are reserved
        assert!(store
            .upsert(
                EngineSpec {
                    id: "jvm".into(),
                    kind: "server".into(),
                    command: t(&["x"]),
                    queries: BTreeMap::new(),
                    description: None,
                },
                &["jvm", "native"],
            )
            .is_err());

        // unknown engine / bad endpoint template
        assert!(store.set_query("nope", "get_method_source", t(&["x"])).is_err());
        assert!(store.set_query("kuna", "get_method_source", t(&["x", "{bogus}"])).is_err());

        let updated = store
            .set_query("kuna", "get_method_source", t(&["kuna", "decompile", "{target}", "{key}"]))
            .unwrap();
        assert!(updated.queries.contains_key("get_method_source"));

        let removed = store.remove("kuna").unwrap();
        assert_eq!(removed.id, "kuna");
        assert!(store.get("kuna").is_none());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn foreign_engine_build_command_renders_target() {
        let engine = ForeignEngine::new(EngineSpec {
            id: "kuna".into(),
            kind: "command".into(),
            command: t(&["kuna", "decompile-project", "{target}"]),
            queries: BTreeMap::new(),
            description: None,
        });
        assert_eq!(engine.kind(), EngineKind::Command);
        let spec = TargetSpec {
            target: PathBuf::from("/tmp/x.out"),
            port: 0,
            scripts: vec![],
            passthrough: vec![],
        };
        let cmd = engine.build_command(Path::new("/usr/bin/kuna"), &spec).unwrap();
        let rendered: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(rendered, t(&["decompile-project", "/tmp/x.out"]));
    }
}
