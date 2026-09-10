//! `tools` tool — manage registered external CLI tools (opencli-style
//! `external register`). Registered tools run as `decx <name> [args...]`.

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::iface::{ArgSpec as A, Args, CommandSpec, Interface};
use crate::tools::external::ExternalRegistry;
use crate::tools::ToolContext;

pub fn interface() -> Interface {
    Interface::new(
        "tools",
        "Register and run external CLI tools under the decx command surface",
    )
    .commands(vec![
        CommandSpec::leaf(
            "register",
            "Register an external CLI tool",
            vec![
                A::positional("name", "Tool name (becomes `decx <name> [args...]`)"),
                A::opt("description", "description", "Short description shown by 'tools list'"),
                A::trailing("command", "Command to spawn, after `--` (e.g. `-- gh`)"),
            ],
            run_register,
        ),
        CommandSpec::leaf(
            "remove",
            "Remove a registered tool",
            vec![A::positional("name", "Tool name")],
            run_remove,
        ),
        CommandSpec::leaf("list", "List registered external tools", vec![], run_list),
        CommandSpec::leaf(
            "run",
            "Run a registered tool explicitly (same as `decx <name> [args...]`)",
            vec![
                A::positional("name", "Tool name"),
                A::trailing("args", "Arguments passed to the tool"),
            ],
            run_run,
        ),
    ])
}

fn run_register(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let registry = ExternalRegistry::new(&ctx.home);
    let tool = registry.register(
        a.str("name"),
        a.strs("command").to_vec(),
        a.opt_str("description").map(str::to_string),
    )?;
    Ok(json!({
        "registered": true,
        "usage": format!("decx {} [args...]", tool.name),
        "tool": tool.to_summary(),
    }))
}

fn run_remove(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let tool = ExternalRegistry::new(&ctx.home).remove(a.str("name"))?;
    Ok(json!({ "removed": true, "tool": tool.to_summary() }))
}

fn run_list(ctx: &ToolContext, _a: &Args) -> DecxResult<Value> {
    let tools: Vec<Value> = ExternalRegistry::new(&ctx.home)
        .load()
        .iter()
        .map(|t| t.to_summary())
        .collect();
    Ok(json!({ "total": tools.len(), "tools": tools }))
}

fn run_run(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let name = a.str("name");
    let registry = ExternalRegistry::new(&ctx.home);
    let tool = registry
        .get(name)
        .ok_or_else(|| DecxError::not_found("TOOL_NOT_FOUND", format!("Tool not found: {name}")))?;
    // Child runs with inherited stdio; the entrypoint recognizes the
    // passthrough sentinel and exits with the child's code.
    let code = registry.run_passthrough(&tool, a.strs("args"))?;
    Err(crate::error::DecxError::passthrough_exit(code))
}
