//! Engine layer --?runtime for the engines registered at compile time.
//!
//! There are no engine adapters in this workspace: every engine (jvm
//! decx-server.jar, native decx-native-server, kuna decx-kuna-server, ...)
//! is declared in the compile-time `config.json`, compiled into
//! [`crate::engines_gen::ENGINES`], and served remotely by its own server
//! binary (built in the decx repository). This module owns the generic
//! runtime around those statics: binary discovery, launch-command assembly
//! (templates + script/passthrough support + `--engine-arg` extras), and
//! discovery status. Process supervision and health waiting live in
//! [`launcher`] and the session layer.

pub mod launcher;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::spec::{BinaryKind, EngineSpec, MapDefault, ParamKind};

/// Everything needed to build one launch.
pub struct TargetSpec {
    pub target: PathBuf,
    pub port: u16,
    pub scripts: Vec<String>,
    /// jadx passthrough args (jvm only; ignored by other engines).
    pub passthrough: Vec<String>,
    /// Validated `--engine-arg` extras rendered as tokens (`--warm`,
    /// `--taint-rules <file>`).
    pub engine_args: Vec<String>,
}

/// The compile-time engine catalog: statics from `engines_gen` (built by
/// `build.rs` from `config.json`). Cheap to construct --?a slice reference.
#[derive(Debug, Clone, Copy)]
pub struct EngineCatalog {
    pub engines: &'static [EngineSpec],
}

impl Default for EngineCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineCatalog {
    pub fn new() -> Self {
        Self {
            engines: crate::engines_gen::ENGINES,
        }
    }

    /// The process-wide catalog (statics --?no state to build).
    pub fn global() -> Self {
        Self::new()
    }

    pub fn get(&self, id: &str) -> Option<&'static EngineSpec> {
        self.engines.iter().find(|e| e.id == id)
    }

    pub fn ids(&self) -> Vec<&'static str> {
        self.engines.iter().map(|e| e.id).collect()
    }

    /// Resolve the default engine id: `DECX_ENGINE` env (fallback `jvm`).
    pub fn default_engine_id() -> String {
        std::env::var("DECX_ENGINE")
            .ok()
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "jvm".to_string())
    }

    pub fn resolve(&self, id: Option<&str>) -> DecxResult<&'static EngineSpec> {
        let id = id
            .map(str::to_string)
            .unwrap_or_else(Self::default_engine_id);
        self.get(&id).ok_or_else(|| {
            DecxError::usage(format!(
                "Unknown engine '{id}' (available: {})",
                self.ids().join(", ")
            ))
        })
    }

    /// Discovery status for every engine (for `session check` /
    /// `self status`).
    pub fn status(&self, home: &Path) -> Value {
        let mut map = serde_json::Map::new();
        for engine in self.engines {
            map.insert(engine.id.to_string(), self.status_info(engine, home));
        }
        Value::Object(map)
    }

    pub fn status_info(&self, engine: &'static EngineSpec, home: &Path) -> Value {
        status_info(engine, home)
    }
}

/// Discovery status for one engine (for `session check` / `engine list` /
/// `self status`), including the jar version for `java-jar` engines.
pub fn status_info(engine: &'static EngineSpec, home: &Path) -> Value {
    let mut info = match resolve_binary(engine, home) {
        Ok(path) => {
            let mut value = json!({ "ok": true, "info": path.display().to_string() });
            if engine.binary.kind == BinaryKind::JavaJar {
                value["version"] =
                    json!(crate::installer::read_jar_version_property(&path));
            }
            value
        }
        Err(err) => json!({ "ok": false, "info": err.message }),
    };
    if !engine.description.is_empty() {
        info["description"] = json!(engine.description);
    }
    info
}

// ------ binary discovery ------------------------------------------------------------------------------------------------------------------------------------------------------------------------

fn platform_name(spec: &crate::spec::BinarySpec) -> String {
    if cfg!(windows) && spec.exe_suffix {
        format!("{}.exe", spec.path)
    } else {
        spec.path.to_string()
    }
}

/// Resolve an engine binary per its compile-time spec: env override (file or
/// containing dir) --?sibling of the running `decx` executable --?/// `<DECX_HOME>/bin` --?dev-checkout `search_dirs` (walking up) --?PATH.
pub fn resolve_binary(spec: &'static EngineSpec, home: &Path) -> DecxResult<PathBuf> {
    let name = platform_name(&spec.binary);

    // 1. env override (file or dir)
    if !spec.binary.env.is_empty() {
        if let Ok(env_path) = std::env::var(spec.binary.env) {
            let trimmed = env_path.trim();
            if !trimmed.is_empty() {
                let candidate = PathBuf::from(trimmed);
                if candidate.is_file() {
                    return Ok(candidate);
                }
                let from_dir = candidate.join(&name);
                if from_dir.exists() {
                    return Ok(from_dir);
                }
            }
        }
    }

    // 2. next to the running decx executable (same cargo build output dir)
    if spec.binary.search_sibling {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(sibling) = exe.parent().map(|dir| dir.join(&name)) {
                if sibling.exists() {
                    return Ok(sibling);
                }
            }
        }
    }

    // 3. installed under <DECX_HOME>/bin
    let installed = home.join("bin").join(&name);
    if installed.exists() {
        return Ok(installed);
    }

    // 4. dev checkout: walk up from the working directory
    if !spec.binary.search_dirs.is_empty() {
        if let Ok(mut dir) = std::env::current_dir() {
            for _ in 0..4 {
                for rel in spec.binary.search_dirs {
                    let candidate = dir.join(rel).join(&name);
                    if candidate.exists() {
                        return Ok(candidate);
                    }
                }
                if !dir.pop() {
                    break;
                }
            }
        }
    }

    // 5. PATH (native programs only)
    if spec.binary.kind == BinaryKind::Program {
        if let Some(found) = resolve_program(&spec.binary.path) {
            return Ok(found);
        }
    }

    Err(DecxError::file(
        format!(
            "{} not found: set {}, place it under <DECX_HOME>/bin, or install/build the engine{}",
            name,
            if spec.binary.env.is_empty() { "its env override".to_string() } else { spec.binary.env.to_string() },
            if spec.binary.kind == BinaryKind::JavaJar {
                " (decx self install for the jvm engine)"
            } else {
                ""
            },
        ),
        None,
    ))
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
        path.extension().is_none_or(|e| e == "exe" || e == "cmd" || e == "bat")
    } else {
        true
    }
}

// ------ launch-command assembly ---------------------------------------------------------------------------------------------------------------------------------------------------

/// Validate launch options against the engine's compile-time capabilities:
/// `--script` and jadx passthrough need declared support; `--engine-arg`
/// extras must reference declared launch params.
pub fn validate_launch(
    spec: &'static EngineSpec,
    target: &TargetSpec,
    engine_args: &[(String, Option<String>)],
) -> DecxResult<()> {
    if !target.scripts.is_empty() {
        if spec.launch.scripts {
            for script in &target.scripts {
                let path = PathBuf::from(script);
                if !path.exists() {
                    return Err(DecxError::file(
                        format!("Script file not found: {}", path.display()),
                        Some(path.display().to_string()),
                    ));
                }
            }
        } else {
            return Err(DecxError::usage(format!(
                "--script is only supported by engines that declare launch scripts ({} does not have a Jadx script runtime)",
                spec.id
            )));
        }
    }
    for (id, value) in engine_args {
        let Some(param) = spec.find_param(id) else {
            return Err(DecxError::usage(format!(
                "engine '{}' declares no launch param '{id}' (see: decx engine show {})",
                spec.id, spec.id
            )));
        };
        if param.kind == ParamKind::Value && value.is_none() {
            return Err(DecxError::usage(format!(
                "--engine-arg {id}=<value> requires a value (see: decx engine show {})",
                spec.id
            )));
        }
    }
    Ok(())
}

/// Render validated `--engine-arg` extras into launch tokens.
pub fn render_engine_args(
    spec: &'static EngineSpec,
    engine_args: &[(String, Option<String>)],
) -> Vec<String> {
    let mut tokens = Vec::new();
    for (id, value) in engine_args {
        let Some(param) = spec.find_param(id) else { continue };
        match param.kind {
            ParamKind::Flag => tokens.push(param.long.to_string()),
            ParamKind::Value => {
                tokens.push(param.long.to_string());
                tokens.push(value.clone().unwrap_or_default());
            }
        }
    }
    tokens
}

/// Assemble the server spawn command from the engine's compile-time launch
/// template: placeholders are expanded, jadx passthrough is normalized
/// (`passthrough: "jadx"`), scripts appended (`scripts: "positional"`), and
/// validated engine args appended.
pub fn build_launch_command(
    spec: &'static EngineSpec,
    binary: &Path,
    target: &TargetSpec,
    engine_args: &[(String, Option<String>)],
) -> DecxResult<Command> {
    let mut argv: Vec<String> = Vec::with_capacity(spec.launch.command.len() + 8);
    for token in spec.launch.command {
        argv.push(match *token {
            "{binary}" => binary.display().to_string(),
            "{target}" => target.target.display().to_string(),
            "{port}" => target.port.to_string(),
            "{java_heap}" => crate::spawn::default_java_heap(),
            other => other.to_string(),
        });
    }
    argv.extend(render_engine_args(spec, engine_args));
    if spec.launch.trailing_args {
        // `session open -- <args>` passthrough: forwarded verbatim; the
        // engine server owns normalization (e.g. jadx flags for jvm).
        argv.extend(target.passthrough.iter().cloned());
    }
    if spec.launch.scripts {
        // Script files (e.g. .jadx.kts) are positional inputs after the target.
        argv.extend(target.scripts.iter().cloned());
    }

    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    Ok(cmd)
}

/// The default value of a launch param, if statically declared.
pub fn param_default(param: &crate::spec::ParamSpec) -> Option<Value> {
    match param.default {
        MapDefault::U64(n) => Some(json!(n)),
        MapDefault::Str(s) => Some(json!(s)),
        MapDefault::Bool(b) => Some(json!(b)),
        MapDefault::None | MapDefault::EmptyList => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines_gen::{ENGINES, TOOLS};

    fn catalog() -> EngineCatalog {
        EngineCatalog::new()
    }

    #[test]
    fn compiled_registry_has_the_documented_engines() {
        let ids = catalog().ids();
        assert!(ids.contains(&"jvm"));
        assert!(ids.contains(&"native"));
        assert!(ids.contains(&"kuna"));
    }

    #[test]
    fn resolve_unknown_engine_lists_alternatives() {
        let err = catalog().resolve(Some("bogus")).expect_err("must fail");
        assert!(err.message.contains("jvm"));
    }

    #[test]
    fn jvm_launch_command_uses_java_and_template() {
        let spec = catalog().get("jvm").unwrap();
        let script_path = std::env::temp_dir().join("decx_test_launch.jadx.kts");
        std::fs::write(&script_path, "// launch test").unwrap();
        let script = script_path.to_string_lossy().to_string();
        let target = TargetSpec {
            target: PathBuf::from("/tmp/a.apk"),
            port: 25419,
            scripts: vec![script.clone()],
            passthrough: vec!["--deobf".to_string()],
            engine_args: vec![],
        };
        validate_launch(spec, &target, &[]).unwrap();
        let cmd = build_launch_command(spec, Path::new("/opt/decx-server.jar"), &target, &[]).unwrap();
        let argv: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(argv.contains(&"/tmp/a.apk".to_string()));
        assert!(argv.windows(2).any(|w| w[0] == "--port" && w[1] == "25419"));
        // trailing_args passthrough is forwarded VERBATIM (jvm's server owns
        // jadx normalization; the CLI does not rewrite jadx flags).
        assert!(argv.contains(&"--deobf".to_string()));
        // scripts appended positionally at the end
        assert_eq!(argv.last().unwrap(), &script);
    }

    #[test]
    fn native_launch_command_is_direct() {
        let spec = catalog().get("native").unwrap();
        let target = TargetSpec {
            target: PathBuf::from("/tmp/a.apk"),
            port: 30001,
            scripts: vec![],
            passthrough: vec![],
            engine_args: vec![],
        };
        let extras = vec![("warm".to_string(), None)];
        validate_launch(spec, &target, &extras).unwrap();
        let cmd = build_launch_command(spec, Path::new("/opt/decx-native-server"), &target, &extras).unwrap();
        let argv: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(argv[0], "/tmp/a.apk".to_string());
        assert!(argv.windows(2).any(|w| w[0] == "--port" && w[1] == "30001"));
        assert!(argv.contains(&"--warm".to_string()));
    }

    #[test]
    fn scripts_rejected_for_engines_without_support() {
        let spec = catalog().get("native").unwrap();
        let target = TargetSpec {
            target: PathBuf::from("/tmp/a.apk"),
            port: 1,
            scripts: vec!["/tmp/s.jadx.kts".to_string()],
            passthrough: vec![],
            engine_args: vec![],
        };
        let err = validate_launch(spec, &target, &[]).expect_err("must fail");
        assert!(err.message.contains("--script"));
    }

    #[test]
    fn engine_arg_validated_against_declared_params() {
        let spec = catalog().get("kuna").unwrap();
        let err = validate_launch(spec, &dummy_target(), &[("warm".to_string(), None)])
            .expect_err("kuna declares no --warm");
        assert!(err.message.contains("declares no launch param"));
    }

    fn dummy_target() -> TargetSpec {
        TargetSpec {
            target: PathBuf::from("/tmp/a.out"),
            port: 25419,
            scripts: vec![],
            passthrough: vec![],
            engine_args: vec![],
        }
    }

    #[test]
    fn every_tool_leaf_is_served_by_at_least_one_engine() {
        // Tool engines must exist, and a leaf's narrowing list must keep at
        // least one of the tool's engines able to serve it.
        for tool in TOOLS {
            assert!(!tool.engines.is_empty(), "tool {} serves no engine", tool.name);
            for engine_id in tool.engines {
                assert!(
                    ENGINES.iter().any(|e| e.id == *engine_id),
                    "tool {} references unknown engine {engine_id}",
                    tool.name
                );
            }
            fn walk(cmd: &'static crate::spec::CmdSpec, tool: &'static crate::spec::ToolSpec) {
                if cmd.subs.is_empty() {
                    let servable = cmd.engines.is_empty()
                        || cmd.engines.iter().any(|e| tool.engines.contains(e));
                    assert!(servable, "leaf {}.{} excludes every tool engine", tool.name, cmd.name);
                } else {
                    for sub in cmd.subs {
                        walk(sub, tool);
                    }
                }
            }
            for cmd in tool.commands {
                walk(cmd, tool);
            }
        }
    }

    #[test]
    fn tool_leaf_paths_cover_expected_commands() {
        let mut paths = Vec::new();
        for tool in TOOLS {
            for path in tool.command_paths() {
                paths.push(path);
            }
        }
        assert!(paths.contains(&"java.classes".to_string()));
        assert!(paths.contains(&"java.taint-scan".to_string()));
        assert!(paths.contains(&"java.manifest".to_string()));
        assert!(paths.contains(&"binary.strings".to_string()));
    }
}