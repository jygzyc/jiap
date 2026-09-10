//! `session` tool — the session layer's CLI surface (open/close/list/status/
//! check/watch/events), declared as an [`Interface`].

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::iface::{ArgSpec as A, Args, CommandSpec, Interface};
use crate::session::lifecycle::{open_session, OpenRequest};
use crate::session::SessionState;
use crate::tools::{target_args, ToolContext};

pub fn interface() -> Interface {
    Interface::new(
        "session",
        "Manage analysis sessions: open, inspect, monitor, and stop engine runs",
    )
    .commands(vec![
        CommandSpec::leaf(
            "open",
            "Start an engine session for an APK, DEX, JAR, AAR, or framework jar",
            vec![
                A::positional("file", "Target file (local path or http(s) URL)"),
                A::one_of(
                    "engine",
                    "engine",
                    &["jvm", "native", "kuna"],
                    "Engine backend (default: config / DECX_ENGINE / jvm)",
                ),
                A::opt("port", "port", "DECX HTTP server port to bind"),
                A::opt("name", "name", "Session name (default: input filename without extension)"),
                A::multi(
                    "script",
                    "script",
                    "Jadx Kotlin script (.jadx.kts) run during decompilation; repeatable (jvm only)",
                ),
                A::flag("force", "force", "Restart matching sessions first (same name or file)"),
                A::opt("timeout", "timeout", "Seconds to wait for the server to become healthy"),
                A::trailing("jadx", "Everything after the file is forwarded to jadx-cli (jvm engine)"),
            ],
            run_open,
        ),
        CommandSpec::leaf(
            "close",
            "Stop one or more recorded sessions",
            {
                let mut args = target_args();
                args.push(A::pos_opt("name", "Session name"));
                args.push(A::flag("all", "all", "Stop all running sessions"));
                args
            },
            run_close,
        ),
        CommandSpec::leaf(
            "list",
            "List recorded sessions and their monitored state",
            vec![A::flag(
                "probe",
                "probe",
                "Deep health-check every session (default: fast liveness + last observed state)",
            )],
            run_list,
        ),
        CommandSpec::leaf(
            "status",
            "Check health for one session or server port",
            {
                let mut args = target_args();
                args.push(A::pos_opt("name", "Session name"));
                args
            },
            run_status,
        ),
        CommandSpec::leaf(
            "check",
            "Check engine binaries, port availability, and server health",
            target_args(),
            run_check,
        ),
        CommandSpec::leaf(
            "watch",
            "Stream session state transitions until interrupted",
            vec![
                A::pos_opt("name", "Session to watch (default: all alive sessions)"),
                A::opt("interval", "interval", "Monitor poll interval in seconds"),
            ],
            run_watch,
        ),
        CommandSpec::leaf(
            "events",
            "Show recent recorded state transitions for a session",
            vec![
                A::pos_opt("name", "Session name"),
                A::opt("limit", "limit", "Maximum number of events"),
            ],
            run_events,
        ),
    ])
}

fn run_open(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let config = crate::config::Config::load(&ctx.home);
    let req = OpenRequest {
        file: a.str("file").to_string(),
        engine_id: Some(config.effective_engine(a.opt_str("engine"))),
        port: a.opt_str("port").map(str::to_string),
        name: a.opt_str("name").map(str::to_string),
        force: a.flag("force"),
        scripts: a.strs("script").to_vec(),
        passthrough: a.strs("jadx").to_vec(),
        timeout_secs: a.u64("timeout").unwrap_or(config.session.open_timeout_secs),
        origin: "decx session open".into(),
    };
    open_session(&ctx.manager, &ctx.engines, &req, |msg| ctx.notice(msg))
}

fn resolve_close_target(ctx: &ToolContext, a: &Args) -> DecxResult<Vec<crate::session::Session>> {
    ctx.manager.cleanup_dead();
    let all = a.flag("all");
    let name = a.opt_str("name");
    let port = a.opt_str("port");

    if all {
        if name.is_some() || port.is_some() {
            return Err(DecxError::usage("Cannot combine --all with a session name or --port"));
        }
        return Ok(ctx.manager.list_alive());
    }
    if name.is_some() && port.is_some() {
        return Err(DecxError::usage("Cannot specify both a session name and --port"));
    }
    if let Some(port) = port {
        let port = crate::ports::parse_server_port(port)?;
        return ctx
            .manager
            .list_alive()
            .into_iter()
            .find(|s| s.port == port)
            .map(|s| vec![s])
            .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Session not found on port: {port}")));
    }
    if let Some(name) = name {
        return ctx
            .manager
            .get(name)
            .map(|s| vec![s])
            .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Session not found: {name}")));
    }
    let alive = ctx.manager.list_alive();
    match alive.len() {
        1 => Ok(alive),
        0 => Err(DecxError::not_found("SESSION_NOT_FOUND", "No running sessions")),
        _ => Err(DecxError::usage("Multiple running sessions: specify a name, --port, or use --all")),
    }
}

fn run_close(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let cleaned = ctx.manager.cleanup_dead();
    let targets = resolve_close_target(ctx, a)?;
    let mut killed = Vec::new();
    let mut dead = Vec::new();
    let mut failed = Vec::new();
    for session in targets {
        let result = crate::spawn::kill_tree(session.pid);
        if result == crate::spawn::KillResult::Failed {
            // Keep the record: removing it would orphan the running engine.
            failed.push(session.name.clone());
            continue;
        }
        ctx.manager.remove(&session.name);
        if result == crate::spawn::KillResult::Killed {
            killed.push(session.name);
        } else {
            dead.push(session.name);
        }
    }
    if failed.is_empty() {
        Ok(json!({ "cleaned": cleaned, "killed": killed, "dead": dead, "failed": failed }))
    } else {
        let names = failed.join(", ");
        Err(DecxError::process(format!(
            "Failed to stop session(s): {names}; the processes are still running. Kill them manually, then retry."
        ))
        .with_details(json!({ "cleaned": cleaned, "killed": killed, "dead": dead, "failed": failed })))
    }
}

fn run_list(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let cleaned = ctx.manager.cleanup_dead();
    if a.flag("probe") {
        ctx.manager.refresh_all();
    }
    let sessions: Vec<Value> = ctx.manager.list().iter().map(crate::session::Session::to_summary).collect();
    Ok(json!({
        "cleaned": cleaned,
        "sessions": sessions,
        "monitored": ctx.manager.monitored_sessions(),
    }))
}

fn run_status(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let (port, session) = super::resolve_target(ctx, a)?;
    let client = crate::client::DecxClient::with_options(port, 10, None);
    match client.health_check() {
        Ok(health) => Ok(json!({ "ok": true, "port": port, "session": session, "health": health })),
        Err(err) => Err(err.with_details(json!({ "port": port, "session": session }))),
    }
}

fn run_check(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    ctx.manager.cleanup_dead();
    let (port, session) = super::resolve_target(ctx, a)?;
    let (server_ok, server_info) = crate::engine::launcher::check_server(port, 3);
    let port_available = crate::ports::is_port_available(port);
    let port_info = if port_available {
        format!("Port {port} is available")
    } else if let Some(name) = &session {
        format!("Port {port} is in use by session '{name}'")
    } else {
        format!("Port {port} is already in use")
    };
    let config = crate::config::Config::load(&ctx.home);
    Ok(json!({
        "session": session.clone().map(|n| json!({ "name": n, "port": port })),
        "server": { "ok": server_ok, "info": server_info },
        "engines": ctx.engines.status(&ctx.home),
        "default_port": config.server.default_port,
        "port": { "ok": port_available, "info": port_info },
    }))
}

fn run_watch(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let config = crate::config::Config::load(&ctx.home);
    let interval = Duration::from_secs(a.u64("interval").unwrap_or(config.session.monitor_interval_secs).max(1));
    let name = a.opt_str("name");
    ctx.manager.cleanup_dead();

    let targets: Vec<String> = match name {
        Some(name) => vec![ctx
            .manager
            .get(name)
            .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Session not found: {name}")))?
            .name],
        None => ctx.manager.list_alive().into_iter().map(|s| s.name).collect(),
    };
    if targets.is_empty() {
        return Err(DecxError::not_found("SESSION_NOT_FOUND", "No running sessions to watch"));
    }

    let mgr = Arc::clone(&ctx.manager);
    let rx = Arc::clone(&mgr).subscribe("*");
    for target in &targets {
        Arc::clone(&mgr).start_monitor(target, interval);
    }

    // Initial snapshot so watchers see the baseline, then stream events.
    let mut snapshot = Value::Array(vec![]);
    for target in &targets {
        if let Some(session) = mgr.get(target) {
            snapshot.as_array_mut().unwrap().push(session.to_summary());
        }
    }
    let fmt = crate::output::Formatter::new(ctx.format);
    fmt.output(&json!({ "watching": targets, "snapshot": snapshot }));

    loop {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(event) => match ctx.format {
                crate::output::OutputFormat::Json => {
                    let mut body = serde_json::to_value(&event).unwrap_or(json!({}));
                    body["kind"] = json!("state-change");
                    println!("{body}");
                }
                crate::output::OutputFormat::Table => {
                    println!(
                        "[{}] {} {} -> {}{}",
                        event.at_ms,
                        event.session,
                        event.from.as_str(),
                        event.to.as_str(),
                        event.detail.as_deref().map(|d| format!(" ({d})")).unwrap_or_default()
                    );
                }
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Quiet keepalive: end once every watched session stopped.
                for target in &targets {
                    if let Some(session) = mgr.get(target) {
                        if session.observed.state == SessionState::Stopped {
                            return Ok(json!({ "watching": targets, "ended": target }));
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Ok(json!({ "watching": targets, "ended": true }));
            }
        }
    }
}

fn run_events(ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    let name = a.opt_str("name");
    let limit = a.u64("limit").unwrap_or(20) as usize;
    let name = match name {
        Some(name) => name.to_string(),
        None => ctx
            .manager
            .auto_select()
            .map(|s| s.name)
            .ok_or_else(|| DecxError::usage("Specify a session name (no single running session to default to)"))?,
    };
    let events = ctx.manager.recent_events(&name, limit);
    Ok(json!({ "session": name, "events": events }))
}
