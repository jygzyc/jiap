//! `self` tool — install/update the decx-server.jar and report versions.

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::installer;

use super::{matches_flag, Tool, ToolContext};

pub struct SelfTool;

fn command() -> Command {
    Command::new("self")
        .about("Install and update the decx-server.jar backing the jvm engine")
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
                .about("Update the server jar (the Rust CLI itself updates via its package channel)")
                .arg(
                    Arg::new("prerelease")
                        .long("prerelease")
                        .action(ArgAction::SetTrue)
                        .help("Update to the newest prerelease"),
                ),
            Command::new("status")
                .about("Show CLI version, installed server version, and engine binary discovery"),
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

impl Tool for SelfTool {
    fn id(&self) -> &'static str {
        "self"
    }

    fn commands(&self) -> Vec<Command> {
        vec![command()]
    }

    fn run(&self, ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = matches.subcommand() else {
            return Err(DecxError::usage("No self subcommand given (install | update | status)"));
        };
        match name {
            "install" | "update" => {
                let result = run_install(ctx, matches_flag(m, "prerelease"))?;
                if name == "update" {
                    ctx.notice("Note: the Rust decx CLI updates through its own distribution channel (cargo build / release binaries), not npm.");
                }
                Ok(result)
            }
            "status" => {
                let config = crate::config::Config::load(&ctx.home);
                Ok(json!({
                    "cli_version": env!("CARGO_PKG_VERSION"),
                    "server_jar": {
                        "recorded_version": config.server_jar.version,
                        "installed_version": crate::engine::adapters::jvm::find_decx_server_jar(&ctx.home)
                            .as_deref()
                            .and_then(installer::read_jar_version_property),
                        "path": crate::engine::adapters::jvm::find_decx_server_jar(&ctx.home)
                            .map(|p| p.display().to_string()),
                    },
                    "engines": crate::engine::launcher::engine_status(&ctx.home, &ctx.engines),
                    "decx_home": ctx.home.display().to_string(),
                }))
            }
            other => Err(DecxError::usage(format!("Unknown self subcommand '{other}'"))),
        }
    }
}
