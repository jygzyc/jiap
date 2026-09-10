//! Tool layer — capabilities and interfaces.
//!
//! Each tool declares an [`Interface`](crate::iface::Interface) (pure data:
//! commands, arguments, help) plus handler functions; decx-cli's generic
//! engine turns those declarations into the command line. This module hosts
//! the shared handler context and the analysis-query surface
//! ([`AnalysisClient`]) that routes an endpoint to the right engine.

pub mod adb;
pub mod android_tool;
pub mod code_tool;
pub mod config_tool;
pub mod engine_tool;
pub mod external;
pub mod self_tool;
pub mod session_tool;
pub mod tools_tool;

use std::path::PathBuf;
use std::sync::Arc;


use crate::engine::EngineRegistry;
use crate::error::{DecxError, DecxResult};
use crate::iface::{Args, Interface};
use crate::output::OutputFormat;
use crate::session::SessionManager;

/// Shared execution context handed to every handler.
pub struct ToolContext {
    pub home: PathBuf,
    pub format: OutputFormat,
    pub manager: Arc<SessionManager>,
    pub engines: Arc<EngineRegistry>,
}

impl ToolContext {
    pub fn notice(&self, msg: &str) {
        eprintln!("  {msg}");
    }
}

/// The analysis-target selector shared by every query command
/// (`--session <name>` / `--port <port>`; absent = auto-select).
pub fn target_args() -> Vec<crate::iface::ArgSpec> {
    vec![
        crate::iface::ArgSpec::opt(
            "session",
            "session",
            "Select a named session; required when multiple are running",
        ),
        crate::iface::ArgSpec::opt("port", "port", "Connect to a DECX HTTP server on this port"),
    ]
}

/// Shared `--session` / `--port` / auto-select resolution.
/// Returns `(port, selected_session_name)`.
pub fn resolve_target(ctx: &ToolContext, args: &Args) -> DecxResult<(u16, Option<String>)> {
    let port = args.opt_str("port");
    let session = args.opt_str("session");
    match (port, session) {
        (Some(_), Some(_)) => Err(DecxError::usage("Cannot specify both --session and --port")),
        (Some(port), None) => Ok((crate::ports::parse_server_port(port)?, None)),
        (None, Some(name)) => {
            let session = ctx
                .manager
                .get(name)
                .ok_or_else(|| DecxError::not_found("SESSION_NOT_FOUND", format!("Session not found: {name}")))?;
            Ok((session.port, Some(session.name)))
        }
        (None, None) => {
            if let Some(session) = ctx.manager.auto_select() {
                return Ok((session.port, Some(session.name)));
            }
            let config = crate::config::Config::load(&ctx.home);
            Ok((config.server.default_port, None))
        }
    }
}

/// Resolve the HTTP client for the selected target: `--port` forces the
/// port; `--session`/auto-select resolve through the session manager.
pub fn analysis_client(ctx: &ToolContext, args: &Args) -> DecxResult<crate::client::DecxClient> {
    let port = args.opt_str("port");
    let session = args.opt_str("session");
    let record = match (port, session) {
        (Some(_), Some(_)) => return Err(DecxError::usage("Cannot specify both --session and --port")),
        (Some(port), None) => {
            return Ok(crate::client::DecxClient::new(crate::ports::parse_server_port(port)?))
        }
        (None, Some(name)) => ctx.manager.get(name),
        (None, None) => ctx.manager.auto_select(),
    };
    match record {
        Some(record) => Ok(crate::client::DecxClient::new(record.port)),
        None => {
            let config = crate::config::Config::load(&ctx.home);
            Ok(crate::client::DecxClient::new(config.server.default_port))
        }
    }
}

/// Registry of tool interfaces (the tool layer's manifest).
pub struct ToolRegistry {
    pub interfaces: Vec<Interface>,
}

impl ToolRegistry {
    pub fn builtins() -> Self {
        Self {
            interfaces: vec![
                session_tool::interface(),
                code_tool::interface(),
                android_tool::interface(),
                engine_tool::interface(),
                config_tool::interface(),
                tools_tool::interface(),
                self_tool::interface(),
            ],
        }
    }
}
