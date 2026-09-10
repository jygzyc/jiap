//! decx — DECX CLI entrypoint.
//!
//! The command line is *generated* from the tool layer's declared
//! interfaces (the standard protocol in `decx-cli-core::iface`): this
//! binary contains no tool-specific code — one generic compiler turns the
//! declarations into the clap tree, and dispatch routes parsed arguments
//! back to the leaf handlers. Registered external CLI tools run as
//! top-level passthrough (`decx <name> [args...]`).

use std::sync::Arc;

use clap::error::ErrorKind as ClapErrorKind;
use clap::{Arg, ArgAction, Command};
use decx_cli_core::error::{DecxError, EX_OK, EX_USAGE};
use decx_cli_core::iface::{run_command, ArgKind, ArgSpec, CommandSpec, Interface};
use decx_cli_core::output::Formatter;
use decx_cli_core::tools::{ToolContext, ToolRegistry};
use decx_cli_core::{config, engine, session};

const ROOT_ABOUT: &str =
    "DECX - Decompiler + X, CLI for deeper analysis of decompiled Java code, powered by JADX and custom extensions";

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    let home = config::decx_home();

    // Registered external tools join the top-level surface: `decx <name> ...`.
    if let Some(code) = try_external_passthrough(&home, &argv) {
        return code;
    }

    let registry = ToolRegistry::builtins();
    let interfaces: Vec<Interface> = registry.interfaces.iter().map(|i| i.materialize()).collect();
    let mut root = build_root(&interfaces);
    root = root
        .version(env!("CARGO_PKG_VERSION"))
        .about(ROOT_ABOUT)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .disable_help_subcommand(true);

    let matches = match root.try_get_matches_from(&argv) {
        Ok(matches) => matches,
        Err(err) => return handle_clap_error(err),
    };

    let unified = config::Config::load(&home);
    let format = match unified.effective_format(matches.get_one::<String>("format").map(String::as_str)) {
        Ok(format) => format,
        Err(err) => return report_error(&err),
    };
    let ctx = ToolContext {
        home: home.clone(),
        format,
        manager: session::SessionManager::open(&home),
        engines: Arc::new(engine::EngineRegistry::new()),
    };
    let fmt = Formatter::new(format);

    let Some((name, tool_matches)) = matches.subcommand() else {
        return report_error(&DecxError::usage(
            "No command given. Run 'decx --help' to list commands, or 'decx tools list' for registered external tools.",
        ));
    };
    let Some(iface) = interfaces.iter().find(|i| i.tool == name) else {
        return report_error(&DecxError::usage(format!("Unknown command '{name}'")));
    };

    match run_command(iface.commands.first().expect("interface root"), &[], tool_matches, &ctx) {
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
                a.value_parser(clap::builder::PossibleValuesParser::new(arg.values))
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
                a.value_parser(clap::builder::PossibleValuesParser::new(arg.values))
            }
        }
        ArgKind::Trailing => base.value_name(arg.id)
            .num_args(0..)
            .trailing_var_arg(true)
            .allow_hyphen_values(true),
    }
}

/// The root command: global `--format` plus every tool interface.
fn build_root(interfaces: &[Interface]) -> Command {
    let mut root = Command::new("decx").arg(
        Arg::new("format")
            .long("format")
            .global(true)
            .default_value("json")
            .value_parser(["json", "table"])
            .help("Output format (json | table)"),
    );
    for iface in interfaces {
        // The materialized interface carries one root spec named after the
        // tool — build it directly (no extra nesting level).
        for cmd in &iface.commands {
            root = root.subcommand(build_command(cmd));
        }
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
    let registry = decx_cli_core::tools::external::ExternalRegistry::new(home);
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
