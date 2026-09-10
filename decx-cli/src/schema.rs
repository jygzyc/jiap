//! Serde model + validation for the unified compile-time engine config
//! (`config.json` at the crate root).
//!
//! Architecture split:
//! - **CLI-internal management commands** (session, engine, settings, tools,
//!   self, android device/framework) are registered in Rust code, marked
//!   `internal` — they never appear in config.json.
//! - **config.json registers the TOOL domains and their engines**: a tool
//!   (`java`, `binary`, ...) is a top-level command namespace whose leaves
//!   are endpoint-backed commands served by the tool's engines
//!   (`java` → jvm + native, `binary` → kuna, later ida, ...). Engines are
//!   pure: binary discovery + launch template + launch params + docs.
//!
//! This module is compiled twice: into the CLI itself (for unit tests that
//! keep the checked-in `config.json` honest) and into `build.rs` (via
//! `#[path]`), which validates the config at COMPILE TIME and generates
//! `engines_gen.rs`. It must therefore only depend on `serde` / `serde_json`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── config.json model ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UnifiedConfig {
    pub version: u32,
    /// CLI-side query conveniences (`--session`, `--port`, `--page`) pushed
    /// into every tool-declared leaf at compile time. NOT functionality —
    /// transport and pagination concerns owned by the CLI.
    #[serde(default)]
    pub common_args: Vec<ArgDef>,
    /// Tool domains: top-level command namespaces backed by engine servers.
    #[serde(default)]
    pub tools: Vec<ToolDef>,
    /// Pure engines: identity, docs, binary discovery, launch knowledge.
    pub engines: Vec<EngineDef>,
}

/// A tool domain (`decx java ...`, `decx binary ...`): a command namespace
/// whose leaves POST endpoints on the session's engine server. `engines`
/// lists which engines can serve this tool; a command may narrow it further
/// (`engines` on the command, e.g. taint-scan is native-only).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub about: String,
    pub engines: Vec<String>,
    #[serde(default)]
    pub commands: Vec<CommandDef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArgKindDef {
    Flag,
    Value,
    Multi,
    Positional,
    Trailing,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ArgDef {
    pub id: String,
    pub kind: ArgKindDef,
    #[serde(default)]
    pub long: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub values: Vec<String>,
    /// Documentation-only type hint for value args (`string` | `u64`).
    #[serde(default, rename = "type")]
    pub arg_type: Option<String>,
    #[serde(default)]
    pub help: String,
}

/// One command a tool exposes. Three leaf flavors:
/// - endpoint leaf (`endpoint` + `request`): POSTs to the session's engine
///   server — the generic dispatcher handles it.
/// - local leaf (`local`: handler id): implemented in-process by a Rust
///   handler registered in `commands::local` (e.g. adb device inspection,
///   which never touches an engine server). Declared in config.json so the
///   tool-domain surface stays the single command registry.
/// A group (`subcommands` non-empty) nests.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandDef {
    pub name: String,
    pub about: String,
    #[serde(default)]
    pub args: Vec<ArgDef>,
    #[serde(default)]
    pub subcommands: Vec<CommandDef>,
    #[serde(default)]
    pub engines: Vec<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub request: Vec<FieldMapDef>,
    /// Handler id of a LOCAL command (`java.android.device.system-services`);
    /// looked up in `commands::local` at startup. Mutually exclusive with
    /// `endpoint`.
    #[serde(default)]
    pub local: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldTypeDef {
    #[serde(rename = "string")]
    Str,
    U64,
    Bool,
    #[serde(rename = "string[]")]
    StrList,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldMapDef {
    /// Source argument id (must be declared on the leaf or in common args).
    pub arg: String,
    /// Dotted destination path in the request body (`filter.includes`).
    pub field: String,
    #[serde(rename = "type")]
    pub ty: FieldTypeDef,
    /// `set` = include only when the arg was provided; `always` = always
    /// include (falling back to `default` when absent).
    #[serde(default = "default_when")]
    pub when: String,
    /// Constant used when `always` and the arg is absent.
    #[serde(default)]
    pub default: Option<Value>,
    /// bool: emitted value is `!flag` instead of `flag`.
    #[serde(default)]
    pub invert: bool,
}

fn default_when() -> String {
    "set".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngineDef {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub docs: String,
    pub binary: BinaryDef,
    pub launch: LaunchDef,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BinaryDef {
    /// `program` (native binary) | `java-jar` (run under `java -jar`).
    pub kind: String,
    /// Binary basename (`decx-native-server`) or jar name (`decx-server.jar`).
    pub path: String,
    /// Env var that overrides discovery (file or containing dir).
    #[serde(default)]
    pub env: String,
    /// Append `.exe` on Windows when resolving.
    #[serde(default)]
    pub exe_suffix: bool,
    /// Look next to the running `decx` executable (same build output dir).
    #[serde(default)]
    pub search_sibling: bool,
    /// Extra relative directories to walk up from the working directory
    /// (dev checkouts), e.g. `decx-native/target/release`.
    #[serde(default)]
    pub search_dirs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LaunchDef {
    /// Launch command template with `{binary}` / `{target}` / `{port}` /
    /// `{java_heap}` placeholders.
    pub command: Vec<String>,
    /// `positional` = script files appended as positional inputs.
    #[serde(default)]
    pub scripts: String,
    /// The server accepts extra trailing arguments after the target
    /// (`session open -- --flags ...`); passed through verbatim.
    #[serde(default)]
    pub trailing_args: bool,
    /// Parameters the launched server binary can parse (documentation +
    /// `session open --engine-arg` validation).
    #[serde(default)]
    pub params: Vec<ParamDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParamDef {
    pub id: String,
    /// Long form including dashes (`--port`).
    pub long: String,
    /// `flag` | `value`.
    pub kind: String,
    #[serde(default)]
    pub help: String,
    #[serde(default, rename = "type")]
    pub param_type: Option<String>,
    #[serde(default)]
    pub default: Option<Value>,
}

// ── validation ──────────────────────────────────────────────────────────────

/// Validate a parsed config. Returns every problem found; empty = valid.
pub fn validate(cfg: &UnifiedConfig) -> Vec<String> {
    let mut errs = Vec::new();

    if cfg.version != 1 {
        errs.push(format!("config.version must be 1, got {}", cfg.version));
    }
    validate_arg_list(&cfg.common_args, "common_args", &mut errs);

    // ── engines: identity, binary, launch ──
    let mut engine_ids: Vec<&str> = Vec::new();
    for engine in &cfg.engines {
        let tag = format!("engine '{}'", engine.id);
        if engine.id.is_empty()
            || !engine
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            errs.push(format!("{tag}: id must be lowercase [a-z0-9-]"));
        }
        if engine_ids.contains(&engine.id.as_str()) {
            errs.push(format!("{tag}: duplicate engine id"));
        }
        engine_ids.push(&engine.id);

        match engine.binary.kind.as_str() {
            "program" | "java-jar" => {}
            other => errs.push(format!(
                "{tag}: binary.kind must be 'program' or 'java-jar', got '{other}'"
            )),
        }
        if engine.binary.path.is_empty() {
            errs.push(format!("{tag}: binary.path must not be empty"));
        }
        if engine.launch.command.is_empty() {
            errs.push(format!("{tag}: launch.command must not be empty"));
        } else {
            for token in &engine.launch.command {
                if token.starts_with('{') && token.ends_with('}') {
                    match token.as_str() {
                        "{binary}" | "{target}" | "{port}" | "{java_heap}" => {}
                        other => errs.push(format!(
                            "{tag}: unknown launch placeholder '{other}' (allowed: {{binary}}, {{target}}, {{port}}, {{java_heap}})"
                        )),
                    }
                }
            }
            let flat = engine.launch.command.join(" ");
            if !flat.contains("{target}") {
                errs.push(format!("{tag}: launch.command must contain the {{target}} placeholder"));
            }
            if !flat.contains("{port}") {
                errs.push(format!("{tag}: launch.command must contain the {{port}} placeholder"));
            }
        }
        if !engine.launch.scripts.is_empty() && engine.launch.scripts != "positional" {
            errs.push(format!("{tag}: launch.scripts must be empty or 'positional'"));
        }
        let mut param_ids: Vec<&str> = Vec::new();
        for param in &engine.launch.params {
            if param.id.is_empty() {
                errs.push(format!("{tag}: launch param with empty id"));
            }
            if !param.long.starts_with("--") {
                errs.push(format!(
                    "{tag}: launch param '{}' long must start with '--'",
                    param.id
                ));
            }
            match param.kind.as_str() {
                "flag" | "value" => {}
                other => errs.push(format!(
                    "{tag}: launch param '{}' kind must be 'flag' or 'value', got '{other}'",
                    param.id
                )),
            }
            if let Some(t) = &param.param_type {
                if t != "string" && t != "u16" && t != "u64" {
                    errs.push(format!(
                        "{tag}: launch param '{}' type must be 'string' | 'u16' | 'u64', got '{t}'",
                        param.id
                    ));
                }
            }
            if param_ids.contains(&param.id.as_str()) {
                errs.push(format!("{tag}: duplicate launch param '{}'", param.id));
            }
            param_ids.push(&param.id);
        }
    }

    // ── tools: namespaces over engines ──
    let mut tool_names: Vec<&str> = Vec::new();
    for tool in &cfg.tools {
        let tag = format!("tool '{}'", tool.name);
        if tool.name.is_empty()
            || !tool
                .name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            errs.push(format!("{tag}: name must be lowercase [a-z0-9-]"));
        }
        if tool_names.contains(&tool.name.as_str()) {
            errs.push(format!("{tag}: duplicate tool name"));
        }
        tool_names.push(&tool.name);

        if tool.engines.is_empty() {
            errs.push(format!("{tag}: must list at least one engine"));
        }
        for id in &tool.engines {
            if !engine_ids.contains(&id.as_str()) {
                errs.push(format!("{tag}: references unknown engine '{id}'"));
            }
        }
        if tool.commands.is_empty() {
            errs.push(format!("{tag}: declares no commands"));
        }
        let mut names: Vec<&str> = Vec::new();
        for cmd in &tool.commands {
            if names.contains(&cmd.name.as_str()) {
                errs.push(format!("{tag}: duplicate command '{}'", cmd.name));
            }
            names.push(&cmd.name);
            validate_command(cmd, &tool.name, &tool.engines, &cfg.common_args, &mut errs);
        }
    }

    // Every engine should serve at least one tool (or it is unreachable
    // from the command surface entirely).
    for id in &engine_ids {
        let used = cfg.tools.iter().any(|t| t.engines.iter().any(|e| e == id));
        if !used {
            errs.push(format!("engine '{id}': serves no tool (list it in a tool's engines)"));
        }
    }

    errs
}

fn validate_arg_list(args: &[ArgDef], where_: &str, errs: &mut Vec<String>) {
    let mut ids: Vec<&str> = Vec::new();
    for arg in args {
        if arg.id.is_empty() {
            errs.push(format!("{where_}: arg with empty id"));
        }
        if ids.contains(&arg.id.as_str()) {
            errs.push(format!("{where_}: duplicate arg id '{}'", arg.id));
        }
        ids.push(&arg.id);
        let needs_long = matches!(
            arg.kind,
            ArgKindDef::Value | ArgKindDef::Multi | ArgKindDef::Flag
        );
        if needs_long && arg.long.is_empty() {
            errs.push(format!(
                "{where_}: arg '{}' of kind {:?} needs a --long name",
                arg.id, arg.kind
            ));
        }
        if !arg.values.is_empty()
            && !matches!(arg.kind, ArgKindDef::Value | ArgKindDef::Positional)
        {
            errs.push(format!(
                "{where_}: arg '{}' restricts values but is not a value/positional arg",
                arg.id
            ));
        }
        if let Some(t) = &arg.arg_type {
            if t != "string" && t != "u64" {
                errs.push(format!(
                    "{where_}: arg '{}' type must be 'string' or 'u64', got '{t}'",
                    arg.id
                ));
            }
        }
    }
}

fn validate_command(
    cmd: &CommandDef,
    tool_name: &str,
    tool_engines: &[String],
    common: &[ArgDef],
    errs: &mut Vec<String>,
) {
    let where_ = format!("tool '{tool_name}' command '{}'", cmd.name);
    if cmd.name.is_empty() {
        errs.push(format!("{where_}: empty name"));
    }
    if !cmd.subcommands.is_empty() {
        if cmd.endpoint.is_some() {
            errs.push(format!("{where_}: group cannot declare an endpoint"));
        }
        let mut names: Vec<&str> = Vec::new();
        for sub in &cmd.subcommands {
            if names.contains(&sub.name.as_str()) {
                errs.push(format!("{where_}: duplicate subcommand '{}'", sub.name));
            }
            names.push(&sub.name);
            validate_command(sub, tool_name, tool_engines, common, errs);
        }
        return;
    }

    // Leaf: either a server-implemented endpoint command or a local
    // handler command — exactly one of the two.
    if let Some(local) = &cmd.local {
        let where_l = format!("{where_} (local)");
        if local.is_empty() {
            errs.push(format!("{where_}: empty local handler id"));
        }
        if cmd.endpoint.is_some() {
            errs.push(format!("{where_}: cannot declare both endpoint and local"));
        }
        if !cmd.request.is_empty() {
            errs.push(format!("{where_l}: cannot declare request mappings"));
        }
        if !cmd.engines.is_empty() {
            errs.push(format!("{where_l}: cannot narrow engines (never hits a server)"));
        }
        validate_arg_list(&cmd.args, &where_, errs);
        check_common_collisions(&cmd.args, common, &where_, errs);
        return;
    }
    let Some(endpoint) = &cmd.endpoint else {
        errs.push(format!("{where_}: leaf command must declare an endpoint or a local handler"));
        return;
    };
    if endpoint.is_empty() {
        errs.push(format!("{where_}: empty endpoint"));
    }

    for id in &cmd.engines {
        if !tool_engines.contains(id) {
            errs.push(format!(
                "{where_}: narrows to engine '{id}' which the tool does not list"
            ));
        }
    }

    validate_arg_list(&cmd.args, &where_, errs);
    check_common_collisions(&cmd.args, common, &where_, errs);
    let arg_kind = |id: &str| -> Option<ArgKindDef> {
        cmd.args
            .iter()
            .chain(common.iter())
            .find(|a| a.id == id)
            .map(|a| a.kind)
    };

    for map in &cmd.request {
        let mtag = format!("{where_}: request mapping '{}'", map.field);
        let Some(kind) = arg_kind(&map.arg) else {
            errs.push(format!("{mtag}: references undeclared arg '{}'", map.arg));
            continue;
        };
        match map.ty {
            FieldTypeDef::Bool => {
                if kind != ArgKindDef::Flag {
                    errs.push(format!("{mtag}: bool mapping must reference a flag arg"));
                }
            }
            FieldTypeDef::StrList => {
                if kind != ArgKindDef::Multi {
                    errs.push(format!("{mtag}: string[] mapping must reference a multi arg"));
                }
            }
            FieldTypeDef::Str | FieldTypeDef::U64 => {
                if !matches!(kind, ArgKindDef::Value | ArgKindDef::Positional) {
                    errs.push(format!(
                        "{mtag}: {:?} mapping must reference a value/positional arg",
                        map.ty
                    ));
                }
            }
        }
        if map.field.is_empty() || map.field.split('.').any(str::is_empty) {
            errs.push(format!("{mtag}: invalid field path"));
        }
        match map.when.as_str() {
            "set" | "always" => {}
            other => errs.push(format!(
                "{mtag}: when must be 'set' or 'always', got '{other}'"
            )),
        }
        if map.invert && map.ty != FieldTypeDef::Bool {
            errs.push(format!("{mtag}: invert only applies to bool mappings"));
        }
        if let Some(default) = &map.default {
            let ok = match map.ty {
                FieldTypeDef::Str => default.is_string(),
                FieldTypeDef::U64 => default.is_u64(),
                FieldTypeDef::Bool => default.is_boolean(),
                FieldTypeDef::StrList => default
                    .as_array()
                    .map(|a| a.iter().all(Value::is_string))
                    .unwrap_or(false),
            };
            if !ok {
                errs.push(format!("{mtag}: default value type does not match mapping type"));
            }
            if map.when != "always" {
                errs.push(format!("{mtag}: default only applies when 'always'"));
            }
        }
    }
}

fn check_common_collisions(args: &[ArgDef], common: &[ArgDef], where_: &str, errs: &mut Vec<String>) {
    for arg in args {
        if common.iter().any(|c| c.id == arg.id) {
            errs.push(format!(
                "{where_}: arg '{}' collides with a common arg id",
                arg.id
            ));
        }
    }
}

// -- tests: keep the checked-in config.json honest --------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn load_repo_config() -> UnifiedConfig {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/config.json");
        let raw = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("config.json is not valid JSON: {e}"))
    }

    fn leaf<'a>(tree: &'a [CommandDef], name: &str) -> &'a CommandDef {
        tree.iter().find(|c| c.name == name).expect("command")
    }

    fn leaf_mut<'a>(tree: &'a mut [CommandDef], name: &str) -> &'a mut CommandDef {
        tree.iter_mut().find(|c| c.name == name).expect("command")
    }

    #[test]
    fn repo_config_is_valid() {
        let cfg = load_repo_config();
        let errs = validate(&cfg);
        assert!(
            errs.is_empty(),
            "config.json validation errors:\n{}",
            errs.join("\n")
        );
    }

    #[test]
    fn repo_config_covers_the_documented_tools_and_engines() {
        let cfg = load_repo_config();
        let tools: Vec<&str> = cfg.tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tools.contains(&"java"));
        assert!(tools.contains(&"binary"));
        let ids: Vec<&str> = cfg.engines.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"jvm"));
        assert!(ids.contains(&"native"));
        assert!(ids.contains(&"kuna"));
        assert!(cfg.engines.len() >= 3);
    }

    #[test]
    fn java_tool_narrows_taint_scan_to_native() {
        let cfg = load_repo_config();
        let java = cfg.tools.iter().find(|t| t.name == "java").unwrap();
        assert_eq!(java.engines, ["jvm", "native"]);
        let taint = leaf(&java.commands, "taint-scan");
        assert_eq!(taint.endpoint.as_deref(), Some("taint_scan"));
        assert_eq!(taint.engines, ["native"]);
        // generic commands carry no narrowing (serve every tool engine)
        assert!(leaf(&java.commands, "classes").engines.is_empty());
        assert_eq!(
            leaf(&java.commands, "manifest").endpoint.as_deref(),
            Some("get_app_manifest")
        );
    }

    #[test]
    fn binary_tool_serves_kuna_commands() {
        let cfg = load_repo_config();
        let binary = cfg.tools.iter().find(|t| t.name == "binary").unwrap();
        assert_eq!(binary.engines, ["kuna"]);
        assert_eq!(
            leaf(&binary.commands, "classes").endpoint.as_deref(),
            Some("get_classes")
        );
        // binary method-source takes a function name, not a DEX signature
        assert_eq!(leaf(&binary.commands, "method-source").args.len(), 1);
    }

    #[test]
    fn engines_carry_no_command_trees() {
        let cfg = load_repo_config();
        // command declarations live on tools, never on engines
        assert!(cfg.engines.iter().all(|e| e.docs.len() > 20));
    }

    #[test]
    fn invalid_placeholder_is_rejected() {
        let mut cfg = load_repo_config();
        cfg.engines[0].launch.command = vec!["{bogus}".to_string()];
        let errs = validate(&cfg);
        assert!(errs.iter().any(|e| e.contains("unknown launch placeholder")));
    }

    #[test]
    fn undeclared_mapping_arg_is_rejected() {
        let mut cfg = load_repo_config();
        let java = cfg.tools.iter_mut().find(|t| t.name == "java").unwrap();
        leaf_mut(&mut java.commands, "classes").request[0].arg = "no-such-arg".into();
        let errs = validate(&cfg);
        assert!(errs.iter().any(|e| e.contains("references undeclared arg")));
    }

    #[test]
    fn unknown_tool_engine_is_rejected() {
        let mut cfg = load_repo_config();
        cfg.tools[0].engines.push("no-such-engine".into());
        let errs = validate(&cfg);
        assert!(errs.iter().any(|e| e.contains("references unknown engine")));
    }

    #[test]
    fn command_engine_narrowing_outside_tool_is_rejected() {
        let mut cfg = load_repo_config();
        let java = cfg.tools.iter_mut().find(|t| t.name == "java").unwrap();
        leaf_mut(&mut java.commands, "classes").engines = vec!["kuna".into()];
        let errs = validate(&cfg);
        assert!(errs
            .iter()
            .any(|e| e.contains("which the tool does not list")));
    }

    #[test]
    fn engine_serving_no_tool_is_rejected() {
        let mut cfg = load_repo_config();
        let orphan = serde_json::from_value::<EngineDef>(serde_json::json!({
            "id": "orphan",
            "description": "d",
            "docs": "docs",
            "binary": { "kind": "program", "path": "x" },
            "launch": { "command": ["{binary}", "{target}", "--port", "{port}"] }
        }))
        .unwrap();
        cfg.engines.push(orphan);
        let errs = validate(&cfg);
        assert!(errs.iter().any(|e| e.contains("serves no tool")));
    }

    #[test]
    fn common_arg_collision_is_rejected() {
        let mut cfg = load_repo_config();
        let java = cfg.tools.iter_mut().find(|t| t.name == "java").unwrap();
        leaf_mut(&mut java.commands, "classes").args.push(ArgDef {
            id: "page".into(),
            kind: ArgKindDef::Value,
            long: "page".into(),
            required: false,
            values: vec![],
            arg_type: None,
            help: "collision".into(),
        });
        let errs = validate(&cfg);
        assert!(errs.iter().any(|e| e.contains("collides with a common arg")));
    }

    #[test]
    fn java_android_device_commands_are_local() {
        let cfg = load_repo_config();
        let java = cfg.tools.iter().find(|t| t.name == "java").unwrap();
        let android = leaf(&java.commands, "android");
        let device = leaf(&android.subcommands, "device");
        let ss = leaf(&device.subcommands, "system-services");
        assert_eq!(
            ss.local.as_deref(),
            Some("java.android.device.system-services")
        );
        assert!(ss.endpoint.is_none() && ss.request.is_empty());
        let pi = leaf(&device.subcommands, "permission-info");
        assert!(pi.local.is_some());
        // positional permission arg is declared
        assert!(pi.args.iter().any(|a| a.id == "permission" && a.required));
    }

    #[test]
    fn endpoint_and_local_are_mutually_exclusive() {
        let mut cfg = load_repo_config();
        let java = cfg.tools.iter_mut().find(|t| t.name == "java").unwrap();
        leaf_mut(&mut java.commands, "classes").local = Some("x.y".into());
        let errs = validate(&cfg);
        assert!(errs.iter().any(|e| e.contains("cannot declare both endpoint and local")));
    }

    #[test]
    fn local_command_with_request_mappings_is_rejected() {
        let mut cfg = load_repo_config();
        let java = cfg.tools.iter_mut().find(|t| t.name == "java").unwrap();
        let leaf = leaf_mut(&mut java.commands, "classes");
        leaf.endpoint = None;
        leaf.local = Some("some.handler".into());
        // leave the request mappings in place → must be rejected
        let errs = validate(&cfg);
        assert!(errs
            .iter()
            .any(|e| e.contains("cannot declare request mappings")));
    }
}
