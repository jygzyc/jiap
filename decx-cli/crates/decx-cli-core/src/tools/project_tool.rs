//! `project` tool — the project manager's CLI surface (legacy alias: `process`).
//!
//! Commands: open / close / list / status / check / watch / events. `watch`
//! and `events` expose the background monitoring: `watch` attaches to the
//! live event stream of one or all projects, `events` replays recorded state
//! transitions from the project manager's event log.

use std::sync::Arc;
use std::time::Duration;

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{json, Value};

use crate::engine::launcher::{check_server, engine_status, open_analysis_target, OpenRequest};
use crate::error::{DecxError, DecxResult};
use crate::project::ProjectState;

use super::{matches_flag, matches_many, matches_u64, target_args, Tool, ToolContext};

pub struct ProjectTool;

fn open_command() -> Command {
    Command::new("open")
        .about("Start a DECX server project for an APK, DEX, JAR, AAR, or framework jar")
        .long_about(
            "Start a DECX server for a target file and record a reusable project. \
             Unknown options after <FILE> are forwarded to jadx-cli (jvm engine), \
             including `-P<key>=<value>` project properties. Known decx options must \
             come before <FILE>.",
        )
        .arg(Arg::new("file").required(true).value_name("FILE"))
        .arg(
            Arg::new("engine")
                .long("engine")
                .help("Analysis engine backend (jvm | native; default: DECX_ENGINE or jvm)"),
        )
        .arg(Arg::new("port").long("port").help("DECX HTTP server port to bind"))
        .arg(
            Arg::new("name")
                .long("name")
                .short('n')
                .help("Project name (default: input filename without extension)"),
        )
        .arg(
            Arg::new("script")
                .long("script")
                .action(ArgAction::Append)
                .num_args(1)
                .help("Jadx Kotlin script (.jadx.kts) run during decompilation; repeatable (jvm engine only)"),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .action(ArgAction::SetTrue)
                .help("Restart even when a matching project exists; alive servers for the same name or file are stopped first"),
        )
        .arg(
            Arg::new("timeout")
                .long("timeout")
                .help("Seconds to wait for the server to become healthy (default 300)"),
        )
        .arg(
            Arg::new("passthrough")
                .value_name("JADX_ARGS")
                .num_args(0..)
                .trailing_var_arg(true)
                .allow_hyphen_values(true)
                .help("Everything after FILE is forwarded to jadx-cli"),
        )
}

fn project_command(name: &'static str, hidden: bool) -> Command {
    let cmd = Command::new(name)
        .about("Manage analysis projects: start, inspect, monitor, and stop DECX server sessions")
        .subcommands([
            open_command(),
            Command::new("close")
                .about("Stop one or more recorded DECX server projects")
                .args(target_args())
                .arg(Arg::new("name").num_args(0..=1).value_name("NAME"))
                .arg(
                    Arg::new("all")
                        .long("all")
                        .short('a')
                        .action(ArgAction::SetTrue)
                        .help("Stop all recorded running projects"),
                ),
            Command::new("list")
                .about("List recorded DECX projects and their monitored state")
                .arg(
                    Arg::new("probe")
                        .long("probe")
                        .action(ArgAction::SetTrue)
                        .help("Deep health-check every project (default: fast PID liveness + last observed state)"),
                ),
            Command::new("status")
                .about("Check health for one project or server port")
                .args(target_args())
                .arg(Arg::new("name").num_args(0..=1).value_name("NAME")),
            Command::new("check")
                .about("Check installed server binaries, port availability, and server health")
                .args(target_args()),
            Command::new("watch")
                .about("Stream project state transitions until interrupted")
                .arg(
                    Arg::new("name")
                        .num_args(0..=1)
                        .value_name("NAME")
                        .help("Project to watch (default: all alive projects)"),
                )
                .arg(Arg::new("interval").long("interval").help("Monitor poll interval in seconds (default 5)")),
            Command::new("events")
                .about("Show recent recorded state transitions for a project")
                .arg(Arg::new("name").num_args(0..=1).value_name("NAME"))
                .arg(Arg::new("limit").long("limit").help("Maximum number of events (default 20)")),
        ]);
    if hidden {
        cmd.hide(true)
    } else {
        cmd
    }
}

impl ProjectTool {
    fn run_open(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let req = OpenRequest {
            file: m.get_one::<String>("file").cloned().unwrap_or_default(),
            engine_id: m.get_one::<String>("engine").cloned(),
            port: m.get_one::<String>("port").cloned(),
            name: m.get_one::<String>("name").cloned(),
            force: matches_flag(m, "force"),
            scripts: matches_many(m, "script"),
            passthrough: matches_many(m, "passthrough"),
            timeout_secs: matches_u64(m, "timeout").unwrap_or(300),
        };
        open_analysis_target(&ctx.manager, &ctx.engines, &req, |msg| ctx.notice(msg))
    }

    fn resolve_close_target(ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Vec<crate::project::Project>> {
        ctx.manager.cleanup_dead();
        let all = matches_flag(m, "all");
        let name = m.get_one::<String>("name").map(String::as_str).filter(|s| !s.is_empty());
        let port = m.get_one::<String>("port").map(String::as_str).filter(|s| !s.is_empty());

        if all {
            if name.is_some() || port.is_some() {
                return Err(DecxError::usage("Cannot combine --all with a project name or --port"));
            }
            return Ok(ctx.manager.list_alive());
        }
        if name.is_some() && port.is_some() {
            return Err(DecxError::usage("Cannot specify both a project name and --port"));
        }
        if let Some(port) = port {
            let port = crate::ports::parse_server_port(port)?;
            return ctx
                .manager
                .list_alive()
                .into_iter()
                .find(|p| p.port == port)
                .map(|p| vec![p])
                .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Project not found on port: {port}")));
        }
        if let Some(name) = name {
            return ctx
                .manager
                .get(name)
                .map(|p| vec![p])
                .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Project not found: {name}")));
        }
        let alive = ctx.manager.list_alive();
        match alive.len() {
            1 => Ok(alive),
            0 => Err(DecxError::not_found("SESSION_NOT_FOUND", "No running projects")),
            _ => Err(DecxError::usage("Multiple running projects: specify a name, --port, or use --all")),
        }
    }

    fn run_close(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let cleaned = ctx.manager.cleanup_dead();
        let targets = Self::resolve_close_target(ctx, m)?;
        let mut killed = Vec::new();
        let mut dead = Vec::new();
        let mut failed = Vec::new();
        for project in targets {
            let result = crate::spawn::kill_tree(project.pid);
            if result == crate::spawn::KillResult::Failed {
                // Keep the record: removing it would orphan the running server.
                failed.push(project.name.clone());
                continue;
            }
            ctx.manager.remove(&project.name);
            if result == crate::spawn::KillResult::Killed {
                killed.push(project.name);
            } else {
                dead.push(project.name);
            }
        }
        if failed.is_empty() {
            Ok(json!({ "cleaned": cleaned, "killed": killed, "dead": dead, "failed": failed }))
        } else {
            let name = failed.join(", ");
            Err(DecxError::process(format!(
                "Failed to stop project(s): {name}; the processes are still running. Kill them manually, then retry."
            ))
            .with_details(json!({ "cleaned": cleaned, "killed": killed, "dead": dead, "failed": failed })))
        }
    }

    fn run_list(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let cleaned = ctx.manager.cleanup_dead();
        if matches_flag(m, "probe") {
            ctx.manager.refresh_all();
        }
        let projects: Vec<Value> = ctx.manager.list().iter().map(crate::project::Project::to_summary).collect();
        Ok(json!({
            "cleaned": cleaned,
            "projects": projects,
            "monitored": ctx.manager.monitored_projects(),
        }))
    }

    fn run_status(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let (port, session) = super::resolve_target(ctx, m)?;
        // Command-engine projects have no server to ask: report the recorded
        // analysis outcome (state machine maintained by open/probe).
        if port == 0 {
            let name = session.clone().unwrap_or_default();
            let project = ctx
                .manager
                .get(&name)
                .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Project not found: {name}")))?;
            let observed = &project.observed;
            return Ok(json!({
                "ok": observed.state == ProjectState::Healthy,
                "project": project.name,
                "engine": project.engine,
                "kind": "command",
                "state": observed.state.as_str(),
                "detail": observed.detail,
                "log": project.log_path.as_ref().map(|p| p.display().to_string()),
            }));
        }
        let client = crate::client::DecxClient::with_options(port, 10, None);
        match client.health_check() {
            Ok(health) => Ok(json!({ "ok": true, "port": port, "project": session, "health": health })),
            Err(err) => Err(err.with_details(json!({ "port": port, "project": session }))),
        }
    }

    fn run_check(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        ctx.manager.cleanup_dead();
        let (port, session) = super::resolve_target(ctx, m)?;
        let (server_ok, server_info) = check_server(port, 3);
        let port_available = crate::ports::is_port_available(port);
        let port_info = if port_available {
            format!("Port {port} is available")
        } else if let Some(name) = &session {
            format!("Port {port} is in use by project '{name}'")
        } else {
            format!("Port {port} is already in use")
        };
        let config = crate::config::Config::load(&ctx.home);
        Ok(json!({
            "project": session.clone().map(|n| json!({ "name": n, "port": port })),
            "server": { "ok": server_ok, "info": server_info },
            "binaries": engine_status(&ctx.home, &ctx.engines),
            "default_port": config.server.default_port,
            "port": { "ok": port_available, "info": port_info },
        }))
    }

    fn run_watch(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let interval = Duration::from_secs(matches_u64(m, "interval").unwrap_or(5).max(1));
        let name = m.get_one::<String>("name").map(String::as_str).filter(|s| !s.is_empty());
        ctx.manager.cleanup_dead();

        let targets: Vec<String> = match name {
            Some(name) => vec![ctx
                .manager
                .get(name)
                .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Project not found: {name}")))?
                .name],
            None => ctx.manager.list_alive().into_iter().map(|p| p.name).collect(),
        };
        if targets.is_empty() {
            return Err(DecxError::not_found("SESSION_NOT_FOUND", "No running projects to watch"));
        }

        let mgr = Arc::clone(&ctx.manager);
        let rx = Arc::clone(&mgr).subscribe("*");
        for target in &targets {
            Arc::clone(&mgr).start_monitor(target, interval);
        }

        // Initial snapshot so watchers see the baseline, then stream events.
        let mut snapshot = Value::Array(vec![]);
        for target in &targets {
            if let Some(p) = mgr.get(target) {
                snapshot.as_array_mut().unwrap().push(p.to_summary());
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
                            event.project,
                            event.from.as_str(),
                            event.to.as_str(),
                            event.detail.as_deref().map(|d| format!(" ({d})")).unwrap_or_default()
                        );
                    }
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Quiet keepalive: end the watch once every watched project stopped.
                    for target in &targets {
                        if let Some(project) = mgr.get(target) {
                            if project.observed.state == ProjectState::Stopped {
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

    fn run_events(&self, ctx: &ToolContext, m: &ArgMatches) -> DecxResult<Value> {
        let name = m.get_one::<String>("name").map(String::as_str).filter(|s| !s.is_empty());
        let limit = matches_u64(m, "limit").unwrap_or(20) as usize;
        let name = match name {
            Some(name) => name.to_string(),
            None => ctx
                .manager
                .auto_select()
                .map(|p| p.name)
                .ok_or_else(|| DecxError::usage("Specify a project name (no single running project to default to)"))?,
        };
        let events = ctx.manager.recent_events(&name, limit);
        Ok(json!({ "project": name, "events": events }))
    }
}

impl Tool for ProjectTool {
    fn id(&self) -> &'static str {
        "project"
    }

    fn commands(&self) -> Vec<Command> {
        // `project` is the primary name; `process` stays as a hidden alias so
        // existing scripts keep working.
        vec![project_command("project", false), project_command("process", true)]
    }

    fn run(&self, _ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value> {
        let Some((name, m)) = matches.subcommand() else {
            return Err(DecxError::usage(
                "No project subcommand given (open | close | list | status | check | watch | events)",
            ));
        };
        match name {
            "open" => self.run_open(_ctx, m),
            "close" => self.run_close(_ctx, m),
            "list" => self.run_list(_ctx, m),
            "status" => self.run_status(_ctx, m),
            "check" => self.run_check(_ctx, m),
            "watch" => self.run_watch(_ctx, m),
            "events" => self.run_events(_ctx, m),
            other => Err(DecxError::usage(format!("Unknown project subcommand '{other}'"))),
        }
    }
}
