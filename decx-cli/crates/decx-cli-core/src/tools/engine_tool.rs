//! `engine` tool — introspect the engine adapters compiled into the CLI.

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::iface::{Args, CommandSpec, Interface};
use crate::tools::ToolContext;

pub fn interface() -> Interface {
    Interface::new(
        "engine",
        "Inspect the analysis engine adapters (identity, description, discovery)",
    )
    .commands(vec![
        CommandSpec::leaf(
            "list",
            "List engine adapters and their discovery status",
            vec![],
            run_list,
        ),
        CommandSpec::leaf(
            "show",
            "Show one engine adapter in detail",
            vec![crate::iface::ArgSpec::positional("id", "Engine id (see engine list)")],
            run_show,
        ),
    ])
}

fn summary(engine: &dyn crate::engine::Engine) -> Value {
    json!({
        "id": engine.id(),
        "description": engine.description(),
    })
}

fn run_list(ctx: &ToolContext, _a: &Args) -> DecxResult<Value> {
    let items: Vec<Value> = ctx
        .engines
        .ids()
        .iter()
        .filter_map(|id| ctx.engines.get(id))
        .map(|engine| {
            json!({
                "engine": summary(engine.as_ref()),
                "binary": engine.status_info(&ctx.home),
            })
        })
        .collect();
    Ok(json!({ "total": items.len(), "engines": items }))
}

fn run_show(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let id = a.str("id");
    let engine = ctx
        .engines
        .get(id)
        .ok_or_else(|| DecxError::not_found("ENGINE_NOT_FOUND", format!("Engine not found: {id}")))?;
    Ok(json!({
        "engine": summary(engine.as_ref()),
        "binary": engine.status_info(&ctx.home),
    }))
}
