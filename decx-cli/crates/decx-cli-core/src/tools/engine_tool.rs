//! `engine` tool — introspect the engine adapters compiled into the CLI
//! (identity, kind, capabilities, binary discovery).

use clap::{Arg, ArgMatches, Command};
use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};

use super::{Tool, ToolContext};

pub struct EngineTool;

fn command() -> Command {
    Command::new("engine")
        .about("Inspect the analysis engine adapters (identity, kind, capabilities, discovery)")
        .long_about(
            "Engines are code-level adapters: one file under \
             decx-cli-core/src/engine/adapters/ plus one line in its builtin() manifest. \
             `decx engine list` shows what this build ships and whether each binary is \
             discoverable; `show` details one adapter, including which analysis endpoints \
             command engines implement.",
        )
        .subcommands([
            Command::new("list").about("List engine adapters and their discovery status"),
            Command::new("show")
                .about("Show one engine adapter in detail")
                .arg(Arg::new("id").required(true).value_name("ID")),
        ])
}

fn summary(engine: &dyn crate::engine::Engine, home: &std::path::Path) -> Value {
    let mut info = json!({
        "id": engine.id(),
        "kind": engine.kind().as_str(),
        "description": engine.description(),
    });
    if engine.kind() == crate::engine::EngineKind::Command {
        info["capabilities"] = json!(engine.capabilities());
    }
    let _ = home;
    info
}

impl Tool for EngineTool {
    fn id(&self) -> &'static str {
        "engine"
    }

    fn commands(&self) -> Vec<Command> {
        vec![command()]
    }

    fn run(&self, ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = matches.subcommand() else {
            return Err(DecxError::usage("No engine subcommand given (list | show)"));
        };
        match name {
            "list" => {
                let items: Vec<Value> = ctx
                    .engines
                    .ids()
                    .iter()
                    .filter_map(|id| ctx.engines.get(id))
                    .map(|engine| {
                        let mut info = summary(engine.as_ref(), &ctx.home);
                        let status = engine.status_info(&ctx.home);
                        info["binary_ok"] = status["ok"].clone();
                        info["binary"] = status["info"].clone();
                        info
                    })
                    .collect();
                Ok(json!({ "total": items.len(), "engines": items }))
            }
            "show" => {
                let id = m.get_one::<String>("id").map(String::as_str).unwrap_or_default();
                let engine = ctx
                    .engines
                    .get(id)
                    .ok_or_else(|| DecxError::not_found("ENGINE_NOT_FOUND", format!("Engine not found: {id}")))?;
                let mut info = summary(engine.as_ref(), &ctx.home);
                let status = engine.status_info(&ctx.home);
                info["binary"] = status;
                Ok(info)
            }
            other => Err(DecxError::usage(format!("Unknown engine subcommand '{other}'"))),
        }
    }
}
