//! `engine` tool — the unified decompiler entry point's plugin surface.
//!
//! Register external analysis engines declaratively (no recompilation):
//! one-shot command decompilers such as kuna by default, or `--server`
//! engines that start a long-lived DECX-contract HTTP server.

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{json, Value};

use crate::engine::foreign::{validate_template, EngineSpec, EngineStore};
use crate::error::{DecxError, DecxResult};

use super::{matches_flag, matches_many, Tool, ToolContext};

pub struct EngineTool;

const REGISTER_EXAMPLES: &str = "\
Examples:
  decx engine register kuna -- kuna decompile-project {target}
  decx engine query kuna get_method_source -- kuna decompile {target} {key}
  decx project open ./a.out --engine kuna
  decx code method-source main";

fn command() -> Command {
    Command::new("engine")
        .about("Register and inspect analysis engines (built-in + external decompilers)")
        .long_about(format!(
            "Register external decompiler engines without recompiling decx. Templates are argv \
             arrays (spawned directly, no shell); tokens may be the exact placeholders {{target}} \
             (absolute target path), {{port}} (server engines), and {{key}} (query templates).\n\n{REGISTER_EXAMPLES}"
        ))
        .subcommands([
            Command::new("register")
                .about("Register (or replace) an external engine")
                .arg(Arg::new("id").required(true).value_name("ID"))
                .arg(
                    Arg::new("server")
                        .long("server")
                        .action(ArgAction::SetTrue)
                        .help("The command starts a long-lived DECX-contract HTTP server (default: one-shot command decompiler)"),
                )
                .arg(Arg::new("description").long("description").num_args(1).help("Short description shown by 'decx engine list'"))
                .arg(
                    Arg::new("command")
                        .value_name("COMMAND")
                        .num_args(1..)
                        .last(true)
                        .required(true)
                        .help("Engine launch template, after `--` (e.g. `-- kuna decompile-project {target}`)"),
                ),
            Command::new("query")
                .about("Map one analysis endpoint to a command template")
                .long_about(
                    "Register how one DECX endpoint is answered by a command engine. ENDPOINT is a \
                     DECX endpoint name (get_method_source, get_class_source, get_strings, ...); \
                     the template may use {target} and {key} (the queried function/class name).",
                )
                .arg(Arg::new("id").required(true).value_name("ID"))
                .arg(
                    Arg::new("endpoint")
                        .required(true)
                        .value_name("ENDPOINT")
                        .help("DECX endpoint name, e.g. get_method_source"),
                )
                .arg(
                    Arg::new("command")
                        .value_name("COMMAND")
                        .num_args(1..)
                        .last(true)
                        .required(true)
                        .help("Query template, after `--` (e.g. `-- kuna decompile {target} {key}`)"),
                ),
            Command::new("list").about("List built-in and registered engines"),
            Command::new("show")
                .about("Show one engine's registration")
                .arg(Arg::new("id").required(true).value_name("ID")),
            Command::new("remove")
                .about("Remove a registered engine")
                .arg(Arg::new("id").required(true).value_name("ID")),
        ])
}

fn require(m: &ArgMatches, id: &str) -> String {
    m.get_one::<String>(id).cloned().unwrap_or_default()
}

fn validate_id(id: &str) -> DecxResult<()> {
    let valid = !id.is_empty()
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !id.starts_with('-');
    if valid {
        Ok(())
    } else {
        Err(DecxError::usage(format!(
            "Invalid engine id '{id}': use letters, digits, '-' or '_' (no leading '-')"
        )))
    }
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
            return Err(DecxError::usage("No engine subcommand given (register | query | list | show | remove)"));
        };
        let store = EngineStore::new(&ctx.home);
        match name {
            "register" => {
                let id = require(m, "id");
                validate_id(&id)?;
                let template = matches_many(m, "command");
                if template.is_empty() {
                    return Err(DecxError::usage("A command template is required (use `-- <command...>`)"));
                }
                validate_template(&template, false)?;
                let spec = EngineSpec {
                    id: id.clone(),
                    kind: if matches_flag(m, "server") { "server".into() } else { "command".into() },
                    command: template,
                    queries: Default::default(),
                    description: m.get_one::<String>("description").cloned().filter(|s| !s.is_empty()),
                };
                store.upsert(spec, &ctx.engines.ids())?;
                Ok(json!({
                    "registered": true,
                    "engine": id,
                    "next": [
                        format!("decx engine query {id} get_method_source -- <command {{target}} {{key}}>"),
                        format!("decx project open <file> --engine {id}"),
                    ],
                }))
            }
            "query" => {
                let id = require(m, "id");
                let endpoint = require(m, "endpoint");
                if endpoint.is_empty()
                    || !endpoint.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                {
                    return Err(DecxError::usage(format!(
                        "Invalid endpoint name '{endpoint}' (lowercase letters, digits, underscore)"
                    )));
                }
                let template = matches_many(m, "command");
                validate_template(&template, true)?;
                let spec = store.set_query(&id, &endpoint, template)?;
                Ok(json!({
                    "engine": id,
                    "endpoint": endpoint,
                    "command": spec.queries.get(&endpoint),
                    "configured": spec.queries.keys().collect::<Vec<_>>(),
                }))
            }
            "list" => {
                // Built-ins come from the registry; foreign engines from the
                // store (source of truth — includes just-registered ids the
                // in-process registry has not merged).
                let mut items: Vec<Value> = Vec::new();
                for id in ctx.engines.ids() {
                    let Some(engine) = ctx.engines.get(id) else { continue };
                    if engine.as_foreign().is_some() {
                        continue; // listed from the store below
                    }
                    items.push(json!({
                        "id": engine.id(),
                        "kind": engine.kind().as_str(),
                        "description": engine.description(),
                        "builtin": true,
                    }));
                }
                for spec in store.load() {
                    items.push(spec.to_summary());
                }
                Ok(json!({ "total": items.len(), "engines": items }))
            }
            "show" => {
                let id = require(m, "id");
                if let Some(spec) = store.get(&id) {
                    return Ok(spec.to_summary());
                }
                let engine = ctx.engines.get(&id).ok_or_else(|| {
                    DecxError::not_found("ENGINE_NOT_FOUND", format!("Engine not found: {id}"))
                })?;
                Ok(json!({
                    "id": engine.id(),
                    "kind": engine.kind().as_str(),
                    "description": engine.description(),
                    "builtin": true,
                }))
            }
            "remove" => {
                let spec = store.remove(&require(m, "id"))?;
                Ok(json!({ "removed": true, "engine": spec.to_summary() }))
            }
            other => Err(DecxError::usage(format!("Unknown engine subcommand '{other}'"))),
        }
    }
}
