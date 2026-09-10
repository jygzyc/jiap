//! Compile-time registration: parse + validate the unified `config.json`
//! and generate the static tool/engine registry (`engines_gen.rs`).
//!
//! What the CLI knows about the world lives in `config.json`:
//! - `tools[]` — tool domains (`java`, `binary`, ...): top-level command
//!   namespaces whose leaves POST endpoints on engine servers. Command-level
//!   `engines` lists narrow a leaf to specific engines (e.g. taint-scan is
//!   native-only).
//! - `engines[]` — pure engines: binary discovery + launch template +
//!   launch params + docs. Engines carry NO command trees.
//!
//! CLI-internal management commands (session/engine/settings/tools/self/
//! android device+framework) are registered in Rust (`src/commands/`), never
//! here. This build script is the compiler: an invalid config fails the
//! build; a valid one becomes statics. Nothing is parsed from disk at
//! runtime. The engine servers themselves (decx-server.jar,
//! decx-native-server, decx-kuna-server, ...) are NOT part of this workspace.

#[path = "src/schema.rs"]
mod schema;

use schema::*;
use std::env;
use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let config_path = Path::new(&manifest_dir).join("config.json");
    println!("cargo:rerun-if-changed={}", config_path.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/schema.rs");

    let raw = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", config_path.display()));
    let cfg: UnifiedConfig = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("config.json is not valid JSON: {e}"));

    let errs = validate(&cfg);
    if !errs.is_empty() {
        let bullets: Vec<String> = errs.iter().map(|e| format!("  - {e}")).collect();
        panic!(
            "config.json failed validation ({} error{}):\n{}",
            errs.len(),
            if errs.len() == 1 { "" } else { "s" },
            bullets.join("\n")
        );
    }

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let generated = render(&cfg);
    std::fs::write(Path::new(&out_dir).join("engines_gen.rs"), generated)
        .expect("cannot write engines_gen.rs");
}

// ── code generation ─────────────────────────────────────────────────────────

// NOTE: the generated file is pulled in via include!(), so it may only ever
// use `//` line comments — never `//!` docs or `#![...]` inner attributes.

/// Escape a string as a Rust string literal.
fn lit(s: &str) -> String {
    format!("{:?}", s)
}

fn lit_list(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| lit(s)).collect();
    format!("&[{}]", inner.join(", "))
}

fn arg_kind(kind: ArgKindDef) -> &'static str {
    match kind {
        ArgKindDef::Flag => "ArgKind::Flag",
        ArgKindDef::Value => "ArgKind::Value",
        ArgKindDef::Multi => "ArgKind::Multi",
        ArgKindDef::Positional => "ArgKind::Positional",
        ArgKindDef::Trailing => "ArgKind::Trailing",
    }
}

fn field_ty(ty: FieldTypeDef) -> &'static str {
    match ty {
        FieldTypeDef::Str => "MapType::Str",
        FieldTypeDef::U64 => "MapType::U64",
        FieldTypeDef::Bool => "MapType::Bool",
        FieldTypeDef::StrList => "MapType::StrList",
    }
}

fn gen_arg(arg: &ArgDef) -> String {
    format!(
        "            ArgSpecS {{ id: {}, long: {}, kind: {}, required: {}, values: {}, help: {} }},\n",
        lit(&arg.id),
        lit(&arg.long),
        arg_kind(arg.kind),
        arg.required,
        lit_list(&arg.values),
        lit(&arg.help),
    )
}

fn gen_default(default: &Option<serde_json::Value>, ty: FieldTypeDef) -> String {
    let Some(value) = default else {
        return "MapDefault::None".to_string();
    };
    match ty {
        FieldTypeDef::Str => format!(
            "MapDefault::Str({})",
            lit(value.as_str().unwrap_or_default())
        ),
        FieldTypeDef::U64 => format!("MapDefault::U64({})", value.as_u64().unwrap_or(0)),
        FieldTypeDef::Bool => format!("MapDefault::Bool({})", value.as_bool().unwrap_or(false)),
        FieldTypeDef::StrList => "MapDefault::EmptyList".to_string(),
    }
}

/// Generate one command spec. `common` args are baked into every ENDPOINT
/// leaf at compile time (`--session`, `--port`, `--page`); local leaves
/// (in-process handlers) get only their own declared args.
fn gen_command(cmd: &CommandDef, common: &[ArgDef], out: &mut String) {
    let _ = writeln!(out, "        CmdSpec {{");
    let _ = writeln!(out, "            name: {},", lit(&cmd.name));
    let _ = writeln!(out, "            about: {},", lit(&cmd.about));
    let _ = writeln!(out, "            engines: {},", lit_list(&cmd.engines));
    if cmd.subcommands.is_empty() {
        let _ = writeln!(out, "            args: &[");
        if cmd.local.is_none() {
            for arg in common {
                out.push_str(&gen_arg(arg));
            }
        }
        for arg in &cmd.args {
            out.push_str(&gen_arg(arg));
        }
        let _ = writeln!(out, "            ],");
        let _ = writeln!(out, "            subs: &[],");
        if let Some(endpoint) = &cmd.endpoint {
            let _ = writeln!(out, "            route: Some(&Route {{");
            let _ = writeln!(out, "                endpoint: {},", lit(endpoint));
            let _ = writeln!(out, "                request: &[");
            for map in &cmd.request {
                let _ = writeln!(
                    out,
                    "                    FieldMap {{ arg: {}, field: {}, ty: {}, when: {}, default: {}, invert: {} }},",
                    lit(&map.arg),
                    lit(&map.field),
                    field_ty(map.ty),
                    if map.when == "always" { "MapWhen::Always" } else { "MapWhen::Set" },
                    gen_default(&map.default, map.ty),
                    map.invert,
                );
            }
            let _ = writeln!(out, "                ],");
            let _ = writeln!(out, "            }}),");
        } else {
            let _ = writeln!(out, "            route: None,");
        }
        let _ = writeln!(
            out,
            "            local: {},",
            cmd.local
                .as_ref()
                .map(|id| format!("Some({})", lit(id)))
                .unwrap_or_else(|| "None".to_string())
        );
    } else {
        let _ = writeln!(out, "            args: &[],");
        let _ = writeln!(out, "            subs: &[");
        for sub in &cmd.subcommands {
            gen_command(sub, common, out);
        }
        let _ = writeln!(out, "            ],");
        let _ = writeln!(out, "            route: None,");
        let _ = writeln!(out, "            local: None,");
    }
    let _ = writeln!(out, "        }},");
}

fn gen_param(param: &ParamDef) -> String {
    let default = match &param.default {
        Some(v) if v.is_u64() => format!("MapDefault::U64({})", v.as_u64().unwrap_or(0)),
        Some(v) if v.is_string() => {
            format!("MapDefault::Str({})", lit(v.as_str().unwrap_or_default()))
        }
        Some(v) if v.is_boolean() => format!("MapDefault::Bool({})", v.as_bool().unwrap_or(false)),
        _ => "MapDefault::None".to_string(),
    };
    format!(
        "            ParamSpec {{ id: {}, long: {}, kind: {}, help: {}, default: {} }},\n",
        lit(&param.id),
        lit(&param.long),
        match param.kind.as_str() {
            "flag" => "ParamKind::Flag",
            _ => "ParamKind::Value",
        },
        lit(&param.help),
        default,
    )
}

fn render(cfg: &UnifiedConfig) -> String {
    let mut out = String::new();
    // Line comments only — this file is include!()-ed (see note above).
    let _ = writeln!(
        out,
        "// @generated by build.rs from config.json — do not edit by hand."
    );
    let _ = writeln!(
        out,
        "// Tool domains (top-level command namespaces) + engines compiled into this CLI."
    );
    let _ = writeln!(out, "// See `spec.rs` for the types.");
    let _ = writeln!(out);
    let _ = writeln!(out, "use crate::spec::*;");
    let _ = writeln!(out);

    // Tool statics: one CmdSpec tree per tool, common args baked into leaves.
    let mut tool_static_names: Vec<String> = Vec::new();
    for tool in &cfg.tools {
        let static_name = format!("TOOL_COMMANDS_{}", tool.name.to_uppercase().replace('-', "_"));
        let _ = writeln!(
            out,
            "// Commands of the '{}' tool domain{}.",
            tool.name,
            format!(" (engines: {})", tool.engines.join(", "))
        );
        let _ = writeln!(out, "static {static_name}: &[CmdSpec] = &[");
        for cmd in &tool.commands {
            gen_command(cmd, &cfg.common_args, &mut out);
        }
        let _ = writeln!(out, "];");
        let _ = writeln!(out);
        tool_static_names.push(static_name);
    }

    // The tool registry.
    let _ = writeln!(
        out,
        "/// Tool domains registered at compile time (command namespaces served by engine servers)."
    );
    let _ = writeln!(out, "pub static TOOLS: &[ToolSpec] = &[");
    for (tool, static_name) in cfg.tools.iter().zip(&tool_static_names) {
        let _ = writeln!(out, "    ToolSpec {{");
        let _ = writeln!(out, "        name: {},", lit(&tool.name));
        let _ = writeln!(out, "        about: {},", lit(&tool.about));
        let _ = writeln!(out, "        engines: {},", lit_list(&tool.engines));
        let _ = writeln!(out, "        commands: {static_name},");
        let _ = writeln!(out, "    }},");
    }
    let _ = writeln!(out, "];");
    let _ = writeln!(out);

    // Engines: identity + binary + launch. The `tools` back-reference is
    // derived from tools[].engines so there is one source of truth.
    let _ = writeln!(
        out,
        "/// Engines compiled into this CLI (binary discovery + launch + docs; commands live on tools)."
    );
    let _ = writeln!(out, "pub static ENGINES: &[EngineSpec] = &[");
    for engine in &cfg.engines {
        let serves: Vec<String> = cfg
            .tools
            .iter()
            .filter(|t| t.engines.iter().any(|e| e == &engine.id))
            .map(|t| t.name.clone())
            .collect();
        let _ = writeln!(out, "    EngineSpec {{");
        let _ = writeln!(out, "        id: {},", lit(&engine.id));
        let _ = writeln!(out, "        description: {},", lit(&engine.description));
        let _ = writeln!(out, "        docs: {},", lit(&engine.docs));
        let _ = writeln!(
            out,
            "        binary: BinarySpec {{ kind: {}, path: {}, env: {}, exe_suffix: {}, search_sibling: {}, search_dirs: {} }},",
            match engine.binary.kind.as_str() {
                "java-jar" => "BinaryKind::JavaJar",
                _ => "BinaryKind::Program",
            },
            lit(&engine.binary.path),
            lit(&engine.binary.env),
            engine.binary.exe_suffix,
            engine.binary.search_sibling,
            lit_list(&engine.binary.search_dirs),
        );
        let _ = writeln!(out, "        launch: LaunchSpec {{");
        let _ = writeln!(
            out,
            "            command: {},",
            lit_list(&engine.launch.command)
        );
        let _ = writeln!(
            out,
            "            scripts: {},",
            engine.launch.scripts == "positional"
        );
        let _ = writeln!(out, "            trailing_args: {},", engine.launch.trailing_args);
        let _ = writeln!(out, "            params: &[");
        for param in &engine.launch.params {
            out.push_str(&gen_param(param));
        }
        let _ = writeln!(out, "            ],");
        let _ = writeln!(out, "        }},");
        let _ = writeln!(out, "        tools: {},", lit_list(&serves));
        let _ = writeln!(out, "    }},");
    }
    let _ = writeln!(out, "];");
    out
}
