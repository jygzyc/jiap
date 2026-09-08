//! `tools` tool — manage registered external CLI tools (opencli-style
//! `external register`). Registered tools also run directly as
//! `decx <name> [args...]` from the entrypoint's passthrough.

use clap::{Arg, ArgMatches, Command};
use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::tools::external::{ExternalRegistry, ExternalTool};

use super::{matches_many, Tool, ToolContext};

pub struct ToolsTool;

fn command() -> Command {
    Command::new("tools")
        .about("Register and run external CLI tools under the decx command surface")
        .long_about(
            "Plug any existing CLI into decx: `decx tools register <name> -- <command...>` makes \
             `<command>` reachable as `decx <name> [args...]` with inherited stdio and exit-code \
             propagation — the same unified-surface idea as opencli's `external register`.",
        )
        .subcommands([
            Command::new("register")
                .about("Register an external CLI tool")
                .arg(Arg::new("name").required(true).value_name("NAME"))
                .arg(
                    Arg::new("description")
                        .long("description")
                        .num_args(1)
                        .help("Short description shown by 'decx tools list'"),
                )
                .arg(
                    Arg::new("command")
                        .value_name("COMMAND")
                        .num_args(1..)
                        .last(true)
                        .required(true)
                        .help("Command to spawn, after `--` (e.g. `decx tools register gh -- gh`)")
                ),
            Command::new("remove")
                .about("Remove a registered tool")
                .arg(Arg::new("name").required(true).value_name("NAME")),
            Command::new("list").about("List registered external tools"),
            Command::new("run")
                .about("Run a registered tool explicitly (same as `decx <name> [args...]`)")
                .arg(Arg::new("name").required(true).value_name("NAME"))
                .arg(
                    Arg::new("args")
                        .value_name("ARGS")
                        .num_args(0..)
                        .trailing_var_arg(true)
                        .allow_hyphen_values(true)
                        .help("Arguments passed to the tool"),
                ),
        ])
}

impl Tool for ToolsTool {
    fn id(&self) -> &'static str {
        "tools"
    }

    fn commands(&self) -> Vec<Command> {
        vec![command()]
    }

    fn run(&self, ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value> {
        let registry = ExternalRegistry::new(&ctx.home);
        let Some((name, m)) = matches.subcommand() else {
            return Err(DecxError::usage("No tools subcommand given (register | remove | list | run)"));
        };
        match name {
            "register" => {
                let tool = registry.register(
                    m.get_one::<String>("name").map(String::as_str).unwrap_or_default(),
                    matches_many(m, "command"),
                    m.get_one::<String>("description").cloned().filter(|s| !s.is_empty()),
                )?;
                Ok(json!({
                    "registered": true,
                    "usage": format!("decx {} [args...]", tool.name),
                    "tool": tool.to_summary(),
                }))
            }
            "remove" => {
                let tool = registry.remove(m.get_one::<String>("name").map(String::as_str).unwrap_or_default())?;
                Ok(json!({ "removed": true, "tool": tool.to_summary() }))
            }
            "list" => {
                let tools: Vec<Value> = registry.load().iter().map(ExternalTool::to_summary).collect();
                Ok(json!({ "total": tools.len(), "tools": tools }))
            }
            "run" => {
                let tool_name = m.get_one::<String>("name").map(String::as_str).unwrap_or_default();
                let tool = registry
                    .get(tool_name)
                    .ok_or_else(|| DecxError::not_found("TOOL_NOT_FOUND", format!("Tool not found: {tool_name}")))?;
                let args = matches_many(m, "args");
                // Runs a child process with inherited stdio; the entrypoint
                // recognizes the passthrough sentinel and exits with the
                // child's code without printing.
                let code = registry.run_passthrough(&tool, &args)?;
                Err(DecxError::passthrough_exit(code))
            }
            other => Err(DecxError::usage(format!("Unknown tools subcommand '{other}'"))),
        }
    }
}
