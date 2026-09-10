//! `engine` command group — query the engines compiled into the CLI.
//!
//! This is the documentation query over the compile-time registry: `engine
//! show <id>` renders everything `config.json` declares for that engine's
//! server — which tool domains it serves (and which of their leaves are
//! engine-specific), how its binary is discovered, and exactly how it is
//! launched (command + parseable parameters).

use serde_json::{json, Value};

use crate::commands::{Args, ToolContext};
use crate::error::{DecxError, DecxResult};
use crate::iface::{ArgSpec as A, CommandSpec, Interface};
use crate::spec::EngineSpec;

pub fn interface() -> Interface {
    Interface::new(
        "engine",
        "Query the compiled-in engine registry: tools served, launch configuration",
    )
    .commands(vec![
        CommandSpec::leaf(
            "list",
            "List engines and their binary discovery status",
            vec![],
            run_list,
        ),
        CommandSpec::leaf(
            "show",
            "Show one engine in detail: docs, commands, launch command and parameters",
            vec![A::positional("id", "Engine id (see engine list)")],
            run_show,
        ),
    ])
}

/// Tool domains (`java`, `binary`, ...) this engine serves.
fn tools_of(engine_id: &str) -> Vec<&'static crate::spec::ToolSpec> {
    crate::engines_gen::TOOLS
        .iter()
        .filter(|tool| tool.engines.contains(&engine_id))
        .collect()
}

fn summary(engine: &'static EngineSpec, home: &std::path::Path) -> Value {
    let tools = tools_of(engine.id);
    let leaf_count: usize = tools.iter().map(|t| t.command_paths_for(engine.id).len()).sum();
    json!({
        "id": engine.id,
        "description": engine.description,
        "tools": tools.iter().map(|t| t.name).collect::<Vec<_>>(),
        "command_count": leaf_count,
        "binary": crate::engine::status_info(engine, home),
    })
}

fn run_list(ctx: &ToolContext, _a: &Args) -> DecxResult<Value> {
    let items: Vec<Value> = ctx
        .catalog
        .engines
        .iter()
        .map(|engine| summary(engine, &ctx.home))
        .collect();
    Ok(json!({ "total": items.len(), "engines": items }))
}

fn run_show(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let id = a.str("id")?;
    let engine = ctx
        .catalog
        .get(id)
        .ok_or_else(|| DecxError::not_found("ENGINE_NOT_FOUND", format!("Engine not found: {id}")))?;

    // Commands this engine can serve, grouped by tool domain. Command-level
    // narrowing (e.g. taint-scan is native-only) is resolved per leaf.
    let mut commands: Vec<Value> = Vec::new();
    for tool in tools_of(engine.id) {
        for path in tool.command_paths_for(engine.id) {
            // Full paths here (`java.classes`); leaf lookup is tool-relative.
            let rel = path
                .strip_prefix(tool.name)
                .and_then(|p| p.strip_prefix('.'))
                .unwrap_or(&path);
            let leaf = tool.find_leaf(rel);
            commands.push(json!({
                "path": path,
                "endpoint": leaf.and_then(|l| l.route).map(|r| r.endpoint),
            }));
        }
    }
    commands.sort_by(|x, y| x["path"].as_str().cmp(&y["path"].as_str()));

    // Launch configuration: the parseable command + parameters.
    let params: Vec<Value> = engine
        .launch
        .params
        .iter()
        .map(|p| {
            let mut item = json!({
                "id": p.id,
                "long": p.long,
                "kind": match p.kind {
                    crate::spec::ParamKind::Flag => "flag",
                    crate::spec::ParamKind::Value => "value",
                },
                "help": p.help,
            });
            if let Some(default) = crate::engine::param_default(p) {
                item["default"] = default;
            }
            item
        })
        .collect();

    Ok(json!({
        "engine": {
            "id": engine.id,
            "description": engine.description,
            "docs": engine.docs,
            "tools": tools_of(engine.id).iter().map(|t| t.name).collect::<Vec<_>>(),
            "commands": commands,
            "binary": crate::engine::status_info(engine, &ctx.home),
            "launch": {
                "command": render_launch_command(engine),
                "scripts": engine.launch.scripts,
                "trailing_args": engine.launch.trailing_args,
                "params": params,
            },
        },
    }))
}

/// Human-readable launch command with placeholders resolved where possible
/// (the target path is always a placeholder at doc time).
fn render_launch_command(engine: &'static EngineSpec) -> String {
    engine
        .launch
        .command
        .iter()
        .map(|token| match *token {
            "{binary}" => engine.binary.path,
            "{java_heap}" => "<2/3 of RAM>",
            "{target}" => "<target>",
            "{port}" => "<port>",
            other => other,
        })
        .collect::<Vec<_>>()
        .join(" ")
}
