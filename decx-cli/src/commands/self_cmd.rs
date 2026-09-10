//! `self` command group — CLI self-management: engine server installation,
//! settings summary, and DECX_HOME paths.

use serde_json::{json, Value};

use crate::commands::{Args, ToolContext};
use crate::error::{DecxError, DecxResult};
use crate::iface::{ArgSpec as A, CommandSpec, Interface};
use crate::installer;

pub fn interface() -> Interface {
    Interface::new(
        "self",
        "Manage the decx CLI itself: engine servers, settings, paths",
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
            "One snapshot: CLI/server versions, settings, engine discovery",
            vec![],
            run_status,
        ),
        CommandSpec::leaf("path", "Print the DECX_HOME layout locations", vec![], run_path),
    ])
}

fn run_install(ctx: &ToolContext, prerelease: bool) -> DecxResult<Value> {
    let mut settings = crate::settings::Settings::load(&ctx.home);
    let result = installer::install_decx_server(&ctx.home, prerelease, Some(&settings.server_jar.version))?;
    if result["ok"] == json!(true) {
        if let Some(version) = result["version"].as_str() {
            settings.update_server_version(&ctx.home, version)?;
        }
    } else {
        return Err(DecxError::process(
            result["message"].as_str().unwrap_or("installation failed").to_string(),
        ));
    }
    Ok(result)
}

fn settings_summary(settings: &crate::settings::Settings) -> Value {
    json!({
        "settings_version": settings.settings_version,
        "default_engine": settings.effective_engine(None),
        "default_format": if settings.default_format.is_empty() { json!("json") } else { json!(settings.default_format) },
        "server": { "default_port": settings.server.default_port },
        "session": {
            "monitor_interval_secs": settings.session.monitor_interval_secs,
            "open_timeout_secs": settings.session.open_timeout_secs,
        },
        "registered_tools": settings.tools.len(),
    })
}

fn run_status(ctx: &ToolContext, _a: &Args) -> DecxResult<Value> {
    let settings = crate::settings::Settings::load(&ctx.home);
    // The jvm engine's binary is decx-server.jar: report its version.
    let jar = ctx
        .catalog
        .get("jvm")
        .and_then(|engine| crate::engine::resolve_binary(engine, &ctx.home).ok());
    Ok(json!({
        "cli_version": env!("CARGO_PKG_VERSION"),
        "decx_home": ctx.home.display().to_string(),
        "settings": settings_summary(&settings),
        "server_jar": {
            "recorded_version": settings.server_jar.version,
            "installed_version": jar.as_deref().and_then(installer::read_jar_version_property),
            "path": jar.map(|p| p.display().to_string()),
        },
        "engines": ctx.catalog.status(&ctx.home),
        "sessions_dir": ctx.home.join("sessions").display().to_string(),
        "logs_dir": ctx.home.join("logs").display().to_string(),
    }))
}

fn run_path(ctx: &ToolContext, _a: &Args) -> DecxResult<Value> {
    Ok(json!({
        "home": ctx.home.display().to_string(),
        "settings": ctx.home.join("settings.json").display().to_string(),
        "engine_registry": "config.json (compiled into the binary at build time)",
        "sessions": ctx.home.join("sessions").display().to_string(),
        "logs": ctx.home.join("logs").display().to_string(),
        "tmp": ctx.home.join("tmp").display().to_string(),
        "bin": ctx.home.join("bin").display().to_string(),
    }))
}
