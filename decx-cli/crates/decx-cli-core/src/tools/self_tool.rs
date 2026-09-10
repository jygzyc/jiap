//! `self` tool — CLI self-management: engine server installation, unified
//! configuration summary, and DECX_HOME paths.

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::iface::{ArgSpec as A, Args, CommandSpec, Interface};
use crate::tools::ToolContext;
use crate::installer;

pub fn interface() -> Interface {
    Interface::new(
        "self",
        "Manage the decx CLI itself: engine servers, unified configuration, paths",
    )
    .commands(vec![
        CommandSpec::leaf(
            "install",
            "Install or update decx-server.jar from GitHub releases",
            vec![A::flag("prerelease", "prerelease", "Install the newest prerelease instead of the latest stable")],
            |ctx, a| run_install(ctx, a.flag("prerelease")),
        ),
        CommandSpec::leaf(
            "update",
            "Update the server jar (the Rust CLI updates via its own distribution channel)",
            vec![A::flag("prerelease", "prerelease", "Update to the newest prerelease")],
            |ctx, a| {
                let result = run_install(ctx, a.flag("prerelease"))?;
                ctx.notice(
                    "Note: the Rust decx CLI updates through its own distribution channel \
                     (cargo build / release binaries), not npm.",
                );
                Ok(result)
            },
        ),
        CommandSpec::leaf(
            "status",
            "One snapshot: CLI/server versions, unified configuration, engine discovery",
            vec![],
            run_status,
        ),
        CommandSpec::leaf("path", "Print the DECX_HOME layout locations", vec![], run_path),
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

fn run_status(ctx: &ToolContext, _a: &Args) -> DecxResult<Value> {
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

fn run_path(ctx: &ToolContext, _a: &Args) -> DecxResult<Value> {
    Ok(json!({
        "home": ctx.home.display().to_string(),
        "config": ctx.home.join("config.json").display().to_string(),
        "sessions": ctx.home.join("sessions").display().to_string(),
        "logs": ctx.home.join("logs").display().to_string(),
        "tmp": ctx.home.join("tmp").display().to_string(),
        "bin": ctx.home.join("bin").display().to_string(),
    }))
}
