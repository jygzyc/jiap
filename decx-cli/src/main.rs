//! decx — DECX CLI entrypoint.
//!
//! The command line is generated from two registration sources:
//! - INTERNAL (Rust): session/engine/settings/tools/self/android management
//!   interfaces registered in `commands::*`.
//! - TOOL DOMAINS (`config.json` → `build.rs` → `engines_gen::TOOLS`):
//!   `java`, `binary`, ... — endpoint-backed leaves served by the session's
//!   engine server (`decx java classes`, `decx binary strings`).
//!
//! Both compile into one clap tree; tool leaves dispatch generically over
//! HTTP, internal leaves into their Rust handlers. Registered external CLI
//! tools run as top-level passthrough (`decx <name> [args...]`).

pub mod android_sdk;
pub mod client;
pub mod commands;
pub mod engine;
pub mod engines_gen;
pub mod error;
pub mod fsx;
pub mod hash;
pub mod iface;
pub mod installer;
pub mod net;
pub mod output;
pub mod ports;
pub mod schema;
pub mod session;
pub mod settings;
pub mod spawn;
pub mod spec;

use std::sync::Arc;

use clap::error::ErrorKind as ClapErrorKind;
use clap::{Arg, ArgAction, Command};

use crate::commands::{ToolContext, ToolRegistry};
use crate::error::{DecxError, EX_OK, EX_USAGE};
use crate::iface::{run_command, ArgKind, ArgSpec, CommandSpec};
use crate::output::Formatter;
use crate::session::SessionManager;

const ROOT_ABOUT: &str =
    "DECX - Decompiler + X: unified CLI for java/binary analysis engines (see: decx engine list)";

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    let home = settings::decx_home();

    // Registered external tools join the top-level surface: `decx <name> ...`.
    if let Some(code) = try_external_passthrough(&home, &argv) {
        return code;
    }

    // Internal management groups + tool domains (`java`, `binary`, ...):
    // distinct top-level names, so a flat concatenation is the whole tree.
    let registry = ToolRegistry::builtins();
    let mut all: Vec<CommandSpec> = Vec::new();
    for iface in &registry.interfaces {
        all.extend(iface.materialize().commands);
    }

    let root = build_root(&all)
        .version(env!("CARGO_PKG_VERSION"))
        .about(ROOT_ABOUT)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .disable_help_subcommand(true);

    let matches = match root.try_get_matches_from(&argv) {
        Ok(matches) => matches,
        Err(err) => return handle_clap_error(err),
    };

    let user_settings = settings::Settings::load(&home);
    let format = match user_settings.effective_format(matches.get_one::<String>("format").map(String::as_str)) {
        Ok(format) => format,
        Err(err) => return report_error(&err),
    };
    let ctx = ToolContext {
        home: home.clone(),
        format,
        manager: SessionManager::open(&home),
        catalog: Arc::new(crate::engine::EngineCatalog::new()),
    };
    let fmt = Formatter::new(format);

    let Some((name, tool_matches)) = matches.subcommand() else {
        return report_error(&DecxError::usage(
            "No command given. Run 'decx --help' to list commands, or 'decx tools list' for registered external tools.",
        ));
    };
    let Some(spec) = all.iter().find(|c| c.name == name) else {
        return report_error(&DecxError::usage(format!("Unknown command '{name}'")));
    };

    match run_command(spec, &[], tool_matches, &ctx) {
        Ok(value) => {
            if value.is_null() {
                fmt.output(&serde_json::json!({ "ok": true }));
            } else {
                fmt.output(&value);
            }
            EX_OK
        }
        Err(err) if err.is_passthrough_exit() => err.exit_code,
        Err(err) => report_error(&err),
    }
}

/// Compile one declared command (recursively) into a clap subcommand.
fn build_command(spec: &CommandSpec) -> Command {
    let mut cmd = Command::new(spec.name).about(spec.about);
    for arg in &spec.args {
        cmd = cmd.arg(build_arg(arg));
    }
    if !spec.subcommands.is_empty() {
        cmd = cmd.subcommand_required(false).arg_required_else_help(false);
        for sub in &spec.subcommands {
            cmd = cmd.subcommand(build_command(sub));
        }
    }
    cmd
}

fn build_arg(arg: &ArgSpec) -> Arg {
    let base = Arg::new(arg.id).help(arg.help);
    match arg.kind {
        ArgKind::Flag => base.long(arg.long).action(ArgAction::SetTrue),
        ArgKind::Value => {
            let a = base.long(arg.long).num_args(1);
            if arg.values.is_empty() {
                a
            } else {
                a.value_parser(clap::builder::PossibleValuesParser::new(arg.values.clone()))
            }
        }
        ArgKind::Multi => base.long(arg.long).action(ArgAction::Append).num_args(1),
        ArgKind::Positional => {
            let a = if arg.required {
                base.value_name(arg.id).num_args(1)
            } else {
                base.value_name(arg.id).num_args(0..=1)
            };
            if arg.values.is_empty() {
                a
            } else {
                a.value_parser(clap::builder::PossibleValuesParser::new(arg.values.clone()))
            }
        }
        ArgKind::Trailing => base
            .value_name(arg.id)
            .num_args(0..)
            .trailing_var_arg(true)
            .allow_hyphen_values(true),
    }
}

/// The root command: global `--format` plus every top-level command group.
fn build_root(all: &[CommandSpec]) -> Command {
    let mut root = Command::new("decx").arg(
        Arg::new("format")
            .long("format")
            .global(true)
            .default_value("json")
            .value_parser(["json", "table"])
            .help("Output format (json | table)"),
    );
    for cmd in all {
        root = root.subcommand(build_command(cmd));
    }
    root
}

/// Print a structured error and return its sysexits exit code.
fn report_error(err: &DecxError) -> i32 {
    Formatter::default().error(err);
    err.exit_code
}

fn handle_clap_error(err: clap::Error) -> i32 {
    match err.kind() {
        ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion => {
            err.print().expect("failed to print clap output");
            EX_OK
        }
        _ => {
            let _ = err.print();
            EX_USAGE
        }
    }
}

/// If argv names a registered external tool, spawn it as passthrough.
fn try_external_passthrough(home: &std::path::Path, argv: &[String]) -> Option<i32> {
    let registry = crate::commands::external::ExternalRegistry::new(home);
    let idx = first_command_index(argv)?;
    let tool = registry.get(&argv[idx])?;
    let code = registry.run_passthrough(&tool, &argv[idx + 1..]).ok()?;
    Some(code)
}

/// Index of the first argv token that could be a command name (skipping the
/// global `--format` flag and its value, and other flags).
fn first_command_index(argv: &[String]) -> Option<usize> {
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == "--" {
            return None;
        }
        if arg == "--format" {
            i += 2; // flag and value
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        return Some(i);
    }
    None
}
