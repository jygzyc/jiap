//! `self` tool — CLI self-management: engine server installation, unified
//! configuration summary, and DECX_HOME paths.
//!
//! Commands:
//! - `self install [--prerelease]` — install/update decx-server.jar
//! - `self update`                 — same as install + a channel reminder
//! - `self status`                 — one JSON snapshot of everything managed
//! - `self path`                   — DECX_HOME layout locations

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::installer;

use super::{matches_flag, Tool, ToolContext};

pub struct SelfTool;

fn command() -> Command {
    Command::new("self")
        .about("Manage the decx CLI itself: engine servers, unified configuration, paths")
        .subcommands([
            Command::new("install")
                .about("Install or update decx-server.jar from GitHub releases")
                .arg(
                    Arg::new("prerelease")
                        .long("prerelease")
                        .action(ArgAction::SetTrue)
                        .help("Install the newest prerelease instead of the latest stable"),
                ),
            Command::new("update")
                .about("Update the server jar (the Rust CLI updates via its own distribution channel)")
                .arg(
                    Arg::new("prerelease")
                        .long("prerelease")
                        .action(ArgAction::SetTrue)
                        .help("Update to the newest prerelease"),
                ),
            Command::new("status")
                .about("One snapshot: CLI/server versions, unified configuration, engine discovery"),
            Command::new("path")
                .about("Print the DECX_HOME layout locations"),
        ])
}

fn run_install(ctx: &ToolContext, prerelease: bool) -> DecxResult<Value> {
    let mut config = crate::config::Config::load(&ctx.home);
    let result = installer::install_decx_server(&ctx.home, prerelease, Some(&config.server_jar.version))?;
    if result["ok"] == json!(true) {
        if let Some(version) = result["version"].as_str() {
            config.update_server_version(&ctx.home, version)?;
        }
    } else {
        return Err(DecxError::process(
            result["message"].as_str().unwrap_or("installation failed").to_string(),
        ));
    }
    Ok(result)
}

/// The unified-configuration summary embedded in `self status`.
fn config_summary(config: &crate::config::Config) -> Value {
    json!({
        "config_version": config.config_version,
        "default_engine": config.effective_engine(None),
        "default_engine_configured": config.default_engine,
        "default_format": if config.default_format.is_empty() { json!("json") } else { json!(config.default_format) },
        "server": { "default_port": config.server.default_port },
        "session": {
            "monitor_interval_secs": config.session.monitor_interval_secs,
            "open_timeout_secs": config.session.open_timeout_secs,
        },
        "registered_tools": config.tools.len(),
    })
}

impl Tool for SelfTool {
    fn id(&self) -> &'static str {
        "self"
    }

    fn commands(&self) -> Vec<Command> {
        vec![command()]
    }

    fn run(&self, ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = matches.subcommand() else {
            return Err(DecxError::usage("No self subcommand given (install | update | status | path)"));
        };
        match name {
            "install" | "update" => {
                let result = run_install(ctx, matches_flag(m, "prerelease"))?;
                if name == "update" {
                    ctx.notice(
                        "Note: the Rust decx CLI updates through its own distribution channel \
                         (cargo build / release binaries), not npm.",
                    );
                }
                Ok(result)
            }
            "status" => {
                let config = crate::config::Config::load(&ctx.home);
                let jar = crate::engine::adapters::jvm::find_decx_server_jar(&ctx.home);
                Ok(json!({
                    "cli_version": env!("CARGO_PKG_VERSION"),
                    "decx_home": ctx.home.display().to_string(),
                    "config": config_summary(&config),
                    "server_jar": {
                        "recorded_version": config.server_jar.version,
                        "installed_version": jar.as_deref().and_then(installer::read_jar_version_property),
                        "path": jar.map(|p| p.display().to_string()),
                    },
                    "engines": ctx.engines.status(&ctx.home),
                    "sessions_dir": ctx.home.join("sessions").display().to_string(),
                    "logs_dir": ctx.home.join("logs").display().to_string(),
                }))
            }
            "path" => Ok(json!({
                "home": ctx.home.display().to_string(),
                "config": ctx.home.join("config.json").display().to_string(),
                "sessions": ctx.home.join("sessions").display().to_string(),
                "logs": ctx.home.join("logs").display().to_string(),
                "tmp": ctx.home.join("tmp").display().to_string(),
                "bin": ctx.home.join("bin").display().to_string(),
            })),
            other => Err(DecxError::usage(format!("Unknown self subcommand '{other}'"))),
        }
    }
}
