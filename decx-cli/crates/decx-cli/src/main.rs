//! decx — DECX CLI entrypoint (Rust rewrite).
//!
//! Assembles the command tree from the core tool registry, runs the matched
//! tool against a shared [`ToolContext`], prints JSON/table output on stdout,
//! structured errors on stderr, and exits with sysexits-style codes.
//!
//! External tools registered through `decx tools register <name>` run as
//! top-level `decx <name> [args...]` passthrough (opencli-style).

use std::sync::Arc;

use clap::error::ErrorKind as ClapErrorKind;
use decx_cli_core::error::{DecxError, EX_OK, EX_USAGE};
use decx_cli_core::output::Formatter;
use decx_cli_core::tools::{ToolContext, ToolRegistry};
use decx_cli_core::{config, engine, session};

const ROOT_ABOUT: &str =
    "DECX - Decompiler + X, CLI for deeper analysis of decompiled Java code, powered by JADX and custom extensions";

fn main() {
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    let home = config::decx_home();

    // Registered external tools join the top-level surface: `decx <name> ...`.
    if let Some(code) = try_external_passthrough(&home, &argv) {
        return code;
    }

    let registry = ToolRegistry::with_builtins(&home);
    let mut root = registry.build_root(env!("CARGO_PKG_VERSION"), ROOT_ABOUT);
    root = root.after_help(after_help_text());

    let matches = match root.try_get_matches_from(&argv) {
        Ok(matches) => matches,
        Err(err) => return handle_clap_error(err, &argv),
    };

    let unified = config::Config::load(&home);
    let format = match unified.effective_format(matches.get_one::<String>("format").map(String::as_str)) {
        Ok(format) => format,
        Err(err) => return report_error(&err),
    };
    let manager = session::SessionManager::open(&home);
    let ctx = ToolContext {
        home: home.clone(),
        format,
        manager,
        engines: Arc::new(engine::EngineRegistry::new()),
    };
    let fmt = Formatter::new(format);

    match registry.dispatch(&ctx, &matches) {
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

/// Print a structured error and return its sysexits exit code.
fn report_error(err: &DecxError) -> i32 {
    let fmt = Formatter::default();
    fmt.error(err);
    err.exit_code
}

fn handle_clap_error(err: clap::Error, argv: &[String]) -> i32 {
    let code = match err.kind() {
        ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion => {
            err.print().expect("failed to print clap output");
            return EX_OK;
        }
        ClapErrorKind::InvalidValue
        | ClapErrorKind::UnknownArgument
        | ClapErrorKind::MissingRequiredArgument
        | ClapErrorKind::WrongNumberOfValues
        | ClapErrorKind::InvalidSubcommand
        | ClapErrorKind::ArgumentConflict
        | ClapErrorKind::MissingSubcommand => EX_USAGE,
        _ => EX_USAGE,
    };
    // clap's rendering is already helpful; keep it on stderr.
    let _ = err.print();
    let _ = argv;
    code
}

/// If argv names a registered external tool, spawn it as passthrough.
/// Returns `Some(exit_code)` when handled, `None` when argv does not target an
/// external tool (including when the first token is a flag or a builtin).
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

fn after_help_text() -> &'static str {
    "Session manager:\n  decx session open <file> [--engine ...]   start a session, supervise the engine run\n  \
     decx session watch [name]                   stream background state transitions\n  \
     decx session events [name]                  replay recorded transitions\n  \
     decx session list --probe                   deep health-check every session\n\n\
     Extension:\n  decx tools register <name> -- <command...>  plug any CLI into the decx surface\n  \
     engine adapters:                            one file + one line in engine/adapters\n"
}
