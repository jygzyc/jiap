//! Unified tool integration surface (the opencli-inspired extension point).
//!
//! Every first-party command group is a [`Tool`]: it declares its clap
//! subcommand tree and executes against a shared [`ToolContext`]. The
//! [`ToolRegistry`] assembles the full CLI tree from all registered tools, so
//! new integrations plug in by implementing one trait.
//!
//! External CLI tools join the same surface through `decx tools register`
//! (mirroring opencli's `external register`): a registered binary becomes
//! reachable as `decx <name> [args...]` and is listed by `decx tools list`.

pub mod adb;
pub mod android_tool;
pub mod code_tool;
pub mod engine_tool;
pub mod external;
pub mod project_tool;
pub mod self_tool;
pub mod tools_tool;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::Value;

use crate::engine::EngineRegistry;
use crate::error::{DecxError, DecxResult};
use crate::output::OutputFormat;
use crate::project::ProjectManager;

/// Shared execution context handed to every tool.
pub struct ToolContext {
    pub home: PathBuf,
    pub format: OutputFormat,
    pub manager: Arc<ProjectManager>,
    pub engines: Arc<EngineRegistry>,
}

impl ToolContext {
    pub fn notice(&self, msg: &str) {
        eprintln!("  {msg}");
    }
}

/// A pluggable CLI tool. `commands()` may return more than one command to
/// install aliases (e.g. `project` and the legacy `process`).
pub trait Tool: Send + Sync {
    /// Tool id for diagnostics and registry bookkeeping.
    fn id(&self) -> &'static str;

    /// Top-level subcommand(s) this tool owns.
    fn commands(&self) -> Vec<Command>;

    /// Execute the matched subcommand. `matches` is the match for the
    /// top-level command returned by [`Tool::commands`].
    fn run(&self, ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<Value>;
}

/// Global options available on every subcommand.
pub fn global_args() -> Vec<Arg> {
    vec![Arg::new("format")
        .long("format")
        .global(true)
        .default_value("json")
        .value_parser(["json", "table"])
        .help("Output format (json | table)")]
}

/// Per-subcommand target selection args (`-s/--session`, `--port`). Defined
/// on the leaves that need them rather than globally, because `project open`
/// reuses `--port` with server-bind semantics.
pub fn target_args() -> Vec<Arg> {
    vec![
        Arg::new("session")
            .long("session")
            .short('s')
            .num_args(1)
            .help("Select a named DECX project; required when multiple are running"),
        Arg::new("port")
            .long("port")
            .num_args(1)
            .help("Connect to a DECX HTTP server on this port"),
    ]
}

/// Registry of first-party tools plus the external-tool passthrough table.
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
    /// subcommand name -> index into `tools`
    index: Vec<(String, usize)>,
    pub external: external::ExternalRegistry,
}

impl ToolRegistry {
    /// Registry with all built-in tools.
    pub fn with_builtins(home: &std::path::Path) -> Self {
        let mut reg = Self {
            tools: Vec::new(),
            index: Vec::new(),
            external: external::ExternalRegistry::new(home),
        };
        reg.register(Arc::new(project_tool::ProjectTool));
        reg.register(Arc::new(code_tool::CodeTool));
        reg.register(Arc::new(android_tool::AndroidTool));
        reg.register(Arc::new(engine_tool::EngineTool));
        reg.register(Arc::new(tools_tool::ToolsTool));
        reg.register(Arc::new(self_tool::SelfTool));
        reg
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let idx = self.tools.len();
        for cmd in tool.commands() {
            let name = cmd.get_name().to_string();
            self.index.push((name, idx));
        }
        self.tools.push(tool);
    }

    pub fn builtin_names(&self) -> Vec<String> {
        self.index.iter().map(|(n, _)| n.clone()).collect()
    }

    /// Assemble the root clap command.
    pub fn build_root(&self, version: &'static str, about: &'static str) -> Command {
        let mut root = Command::new("decx")
            .version(version)
            .about(about)
            .subcommand_required(false)
            .arg_required_else_help(false)
            .disable_help_subcommand(true);
        for arg in global_args() {
            root = root.arg(arg);
        }
        for tool in &self.tools {
            for cmd in tool.commands() {
                root = root.subcommand(cmd);
            }
        }
        root
    }

    /// Route a parsed root to the owning tool.
    pub fn dispatch(&self, ctx: &ToolContext, root: &ArgMatches) -> DecxResult<Value> {
        let Some((name, matches)) = root.subcommand() else {
            return Err(DecxError::usage(
                "No command given. Run 'decx --help' to list commands, \
                 or 'decx tools list' for registered external tools.",
            ));
        };
        let idx = self
            .index
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, i)| *i)
            .ok_or_else(|| DecxError::usage(format!("Unknown command '{name}'")))?;
        self.tools[idx].run(ctx, matches)
    }
}

/// Shared `--session` / `--port` / auto-select resolution (mirrors
/// client-helper.ts). Returns `(port, selected_project_name)`.
pub fn resolve_target(ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<(u16, Option<String>)> {
    let port_opt = matches.get_one::<String>("port").filter(|s| !s.is_empty());
    let session_opt = matches.get_one::<String>("session").filter(|s| !s.is_empty());
    match (port_opt, session_opt) {
        (Some(_), Some(_)) => Err(DecxError::usage("Cannot specify both --session and --port")),
        (Some(port), None) => Ok((crate::ports::parse_server_port(port)?, None)),
        (None, Some(name)) => {
            let project = ctx
                .manager
                .get(name)
                .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Session not found: {name}")))?;
            Ok((project.port, Some(project.name)))
        }
        (None, None) => {
            if let Some(project) = ctx.manager.auto_select() {
                return Ok((project.port, Some(project.name)));
            }
            let config = crate::config::Config::load(&ctx.home);
            Ok((config.server.default_port, None))
        }
    }
}

pub fn client_for(ctx: &ToolContext, matches: &ArgMatches) -> DecxResult<(crate::client::DecxClient, Option<String>)> {
    let (port, session) = resolve_target(ctx, matches)?;
    Ok((
        crate::client::DecxClient::with_options(port, crate::client::DEFAULT_TIMEOUT_SECS, session.clone()),
        session,
    ))
}

/// Read a repeatable string option as Vec<String>.
pub fn matches_many(matches: &ArgMatches, id: &str) -> Vec<String> {
    matches
        .get_many::<String>(id)
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default()
}

/// Read an optional u64 option.
pub fn matches_u64(matches: &ArgMatches, id: &str) -> Option<u64> {
    matches.get_one::<String>(id).and_then(|v| v.parse().ok())
}

/// Read an optional flag-with-default-false.
pub fn matches_flag(matches: &ArgMatches, id: &str) -> bool {
    matches.get_flag(id)
}

/// Helper for building repeatable `--include-package` style args.
pub fn repeatable_arg(id: &'static str, long: &'static str, help: &'static str) -> Arg {
    Arg::new(id)
        .long(long)
        .help(help)
        .action(ArgAction::Append)
        .num_args(1)
}

// ── Unified analysis query surface ──────────────────────────────────────────

/// The query surface analysis commands talk to: the DECX HTTP server of a
/// server-engine project, or the command templates of a command-engine
/// project (kuna-style one-shot decompilers). Both return the same JSON
/// envelope shape, so callers stay engine-agnostic.
pub enum AnalysisClient {
    Http(crate::client::DecxClient),
    Command(CommandQuery),
}

/// Endpoint execution for command engines: render the registered template,
/// spawn it with a hard timeout, and wrap stdout in the DECX envelope.
pub struct CommandQuery {
    pub engine: String,
    pub queries: std::collections::BTreeMap<String, Vec<String>>,
    pub target: PathBuf,
}

impl CommandQuery {
    pub fn endpoints(&self) -> Vec<&str> {
        self.queries.keys().map(String::as_str).collect()
    }

    pub fn query(&self, endpoint: &str, key: Option<&str>) -> DecxResult<Value> {
        let template = self.queries.get(endpoint).ok_or_else(|| {
            let configured = self.endpoints();
            let configured = if configured.is_empty() {
                "<none>".to_string()
            } else {
                configured.join(", ")
            };
            DecxError::server(
                "UNSUPPORTED_BY_ENGINE",
                format!(
                    "engine '{}' (command) has no '{endpoint}' template; configured: {configured}",
                    self.engine
                ),
            )
        })?;
        let argv = crate::engine::foreign::render_template(template, &self.target, 0, key);
        let program = argv.first().cloned().unwrap_or_default();
        let resolved = crate::engine::foreign::resolve_program(&program).ok_or_else(|| {
            DecxError::file(
                format!("engine '{}': command '{program}' not found on PATH", self.engine),
                None,
            )
        })?;
        let mut cmd = std::process::Command::new(resolved);
        cmd.args(argv.iter().skip(1));
        let output = crate::spawn::run_with_timeout(&mut cmd, std::time::Duration::from_secs(300))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(DecxError::server(
                "ENGINE_QUERY_FAILED",
                format!(
                    "engine '{}' query failed (exit {}): {}",
                    self.engine,
                    output.status.code().unwrap_or(-1),
                    if stderr.is_empty() { "<no stderr>" } else { &stderr }
                ),
            ));
        }
        Ok(serde_json::json!({
            "code": "OK",
            "data": { "source": String::from_utf8_lossy(&output.stdout) },
            "meta": { "engine": self.engine, "mode": "command" },
        }))
    }
}

/// Route one endpoint call: HTTP for server projects, command template for
/// command projects. Every analysis endpoint goes through here, so command
/// engines can override any endpoint by registering a template for it.
pub fn call(
    client: &AnalysisClient,
    endpoint: &str,
    key: Option<&str>,
    http: impl FnOnce(&crate::client::DecxClient) -> DecxResult<Value>,
) -> DecxResult<Value> {
    match client {
        AnalysisClient::Http(client) => http(client),
        AnalysisClient::Command(query) => query.query(endpoint, key),
    }
}

/// Resolve the analysis client for the selected target: `--port` forces
/// HTTP; `-s/--session` or auto-select may land on a command-engine project.
pub fn analysis_client(ctx: &ToolContext, m: &ArgMatches) -> DecxResult<AnalysisClient> {
    let port_opt = m.get_one::<String>("port").map(String::as_str).filter(|s| !s.is_empty());
    let session_opt = m.get_one::<String>("session").map(String::as_str).filter(|s| !s.is_empty());
    let project = match (port_opt, session_opt) {
        (Some(_), Some(_)) => return Err(DecxError::usage("Cannot specify both --session and --port")),
        (Some(port), None) => {
            return Ok(AnalysisClient::Http(crate::client::DecxClient::new(
                crate::ports::parse_server_port(port)?,
            )))
        }
        (None, Some(name)) => ctx.manager.get(name),
        (None, None) => ctx.manager.auto_select(),
    };
    let Some(project) = project else {
        let config = crate::config::Config::load(&ctx.home);
        return Ok(AnalysisClient::Http(crate::client::DecxClient::new(
            config.server.default_port,
        )));
    };
    if project.is_command_kind() {
        let spec = ctx.engines.foreign_spec(&project.engine).ok_or_else(|| {
            DecxError::not_found(
                "ENGINE_NOT_FOUND",
                format!(
                    "project '{}' uses command engine '{}' which is no longer registered; \
                     re-register it with 'decx engine register {} -- <command...>'",
                    project.name, project.engine, project.engine
                ),
            )
        })?;
        return Ok(AnalysisClient::Command(CommandQuery {
            engine: spec.id,
            queries: spec.queries,
            target: project.file,
        }));
    }
    Ok(AnalysisClient::Http(crate::client::DecxClient::new(project.port)))
}
