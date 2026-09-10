//! Command layer: CLI-internal management commands + the tool-domain
//! surface compiled in from `config.json` (`engines_gen::TOOLS` /
//! `engines_gen::ENGINES`).
//!
//! Registration split:
//! - INTERNAL (here, in Rust): session management, engine docs/status,
//!   device and framework tooling, settings, external tools,
//!   self-management. Never in config.json.
//! - TOOL DOMAINS (config.json -> build.rs -> `engines_gen::TOOLS`):
//!   `java`, `binary`, ... -- top-level namespaces whose leaves POST
//!     endpoints on the session's engine server.
//!
//! Dispatch for tool leaves is fully generic: resolve the target
//! (`--session` / `--port` / auto-select), check the session's engine
//! serves the tool + leaf (command-level `engines` narrowing), build the
//! request body from the compile-time mappings, POST it, return the
//! envelope.

pub mod engine_cmd;
pub mod external;
pub mod local;
pub mod self_cmd;
pub mod session_cmd;
pub mod settings_cmd;
pub mod tools_cmd;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::error::{DecxError, DecxResult};
use crate::engine::EngineCatalog;
use crate::iface::{ArgSpec, Interface};
use crate::output::OutputFormat;
use crate::session::SessionManager;
use crate::spec::{CmdSpec, FieldMap, MapDefault, MapType, MapWhen, ToolSpec};

/// Shared execution context handed to every handler.
pub struct ToolContext {
    pub home: PathBuf,
    pub format: OutputFormat,
    pub manager: Arc<SessionManager>,
    pub catalog: Arc<EngineCatalog>,
}

impl ToolContext {
    pub fn notice(&self, msg: &str) {
        eprintln!("  {msg}");
    }
}

/// Parsed argument values of one leaf command (filled by the generic
/// dispatcher following the declaration).
#[derive(Default)]
pub struct Args {
    strings: HashMap<String, String>,
    multis: HashMap<String, Vec<String>>,
    flags: HashMap<String, bool>,
}

impl Args {
    pub fn insert_string(&mut self, id: &str, value: String) {
        self.strings.insert(id.to_string(), value);
    }

    pub fn insert_multi(&mut self, id: &str, values: Vec<String>) {
        self.multis.insert(id.to_string(), values);
    }

    pub fn insert_flag(&mut self, id: &str, value: bool) {
        self.flags.insert(id.to_string(), value);
    }

    /// Required string value.
    pub fn str(&self, id: &str) -> DecxResult<&str> {
        self.strings
            .get(id)
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| DecxError::usage(format!("missing required argument '{id}'")))
    }

    /// Optional string value (empty = absent).
    pub fn opt_str(&self, id: &str) -> Option<&str> {
        self.strings.get(id).map(|s| s.as_str()).filter(|s| !s.is_empty())
    }

    /// Optional u64 value.
    pub fn u64(&self, id: &str) -> Option<u64> {
        self.opt_str(id).and_then(|v| v.parse().ok())
    }

    /// Multi/trailing values (possibly empty).
    pub fn strs(&self, id: &str) -> &[String] {
        self.multis.get(id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Whether a flag was given (never required).
    pub fn flag(&self, id: &str) -> bool {
        *self.flags.get(id).unwrap_or(&false)
    }
}

/// The analysis-target selector shared by every engine-backed command
/// (`--session <name>` / `--port <port>`; absent = auto-select). Declared
/// once in `config.json` `common_args` and pushed into every engine leaf at
/// compile time — this mirrors it for native commands that need targets.
pub fn target_args() -> Vec<ArgSpec> {
    vec![
        ArgSpec::opt(
            "session",
            "session",
            "Select a named session; required when multiple are running",
        ),
        ArgSpec::opt("port", "port", "Connect to a DECX HTTP server on this port"),
    ]
}

/// Resolve `(client, engine_id)` for an engine-backed command:
/// `--port` forces direct connection (engine unknown — the server itself
/// validates); `--session`/auto-select resolve through the session manager
/// and carry the session's engine id (used for capability routing).
pub fn analysis_target(
    ctx: &ToolContext,
    args: &Args,
) -> DecxResult<(crate::client::DecxClient, Option<&'static str>)> {
    let port = args.opt_str("port");
    let session = args.opt_str("session");
    let record = match (port, session) {
        (Some(_), Some(_)) => {
            return Err(DecxError::usage("Cannot specify both --session and --port"))
        }
        (Some(port), None) => {
            let client = crate::client::DecxClient::with_options(
                crate::ports::parse_server_port(port)?,
                crate::client::DEFAULT_TIMEOUT_SECS,
                None,
            );
            return Ok((client, None));
        }
        (None, Some(name)) => ctx.manager.get(name),
        (None, None) => ctx.manager.auto_select(),
    };
    match record {
        Some(record) => {
            let engine_id = ctx.catalog.get(&record.engine).map(|e| e.id);
            let client = crate::client::DecxClient::with_options(
                record.port,
                crate::client::DEFAULT_TIMEOUT_SECS,
                Some(record.name.clone()),
            );
            Ok((client, engine_id))
        }
        None => {
            let settings = crate::settings::Settings::load(&ctx.home);
            Ok((
                crate::client::DecxClient::new(settings.server.default_port),
                None,
            ))
        }
    }
}

/// Dispatch a tool-domain leaf (`decx java classes`, `decx binary
/// strings`): resolve the analysis target, verify the session's engine
/// serves this tool (and the leaf's engine narrowing, e.g. taint-scan is
/// native-only), build the request body from the compile-time mappings,
/// POST it, and return the envelope.
pub fn run_route(
    ctx: &ToolContext,
    tool: &'static ToolSpec,
    leaf: &'static CmdSpec,
    args: &Args,
) -> DecxResult<Value> {
    let (client, engine_id) = analysis_target(ctx, args)?;
    let path = format!("{}.{}", tool.name, leaf.name);

    // The session's engine must serve the tool domain; a leaf's narrowing
    // list (empty = every tool engine) further restricts it.
    if let Some(id) = engine_id {
        if !tool.engines.contains(&id) || !leaf.supports_engine(id) {
            return Err(DecxError::not_found(
                "ENGINE_UNSUPPORTED_COMMAND",
                format!(
                    "engine '{id}' does not implement '{path}' \
                     (see: decx engine show {id})"
                ),
            ));
        }
    }
    let route = leaf
        .route
        .ok_or_else(|| DecxError::internal(format!("'{path}' declares no endpoint")))?;

    let body = build_request_body(route.request, args);
    client.post_endpoint(route.endpoint, &body)
}

/// Apply compile-time field mappings: `set` mappings emit only when the arg
/// was provided; `always` mappings emit every time with the declared
/// default when absent; bools may be inverted (`--no-regex` → regex=false);
/// `string[]` always emits an array.
pub fn build_request_body(request: &'static [FieldMap], args: &Args) -> Value {
    let mut body = serde_json::Map::new();
    for map in request {
        if let Some(value) = field_value(map, args) {
            set_dotted(&mut body, map.field, value);
        }
    }
    Value::Object(body)
}

fn field_value(map: &'static FieldMap, args: &Args) -> Option<Value> {
    let provided = match map.ty {
        MapType::Bool => args.flag(map.arg),
        MapType::StrList => !args.strs(map.arg).is_empty(),
        _ => args.opt_str(map.arg).is_some(),
    };
    match map.when {
        MapWhen::Set => {
            if !provided {
                return None;
            }
            literal_value(map, args)
        }
        MapWhen::Always => {
            if provided {
                literal_value(map, args)
            } else {
                Some(match map.default {
                    MapDefault::None => return None,
                    MapDefault::Str(s) => Value::String(s.to_string()),
                    MapDefault::U64(n) => Value::from(n),
                    MapDefault::Bool(b) => Value::from(b),
                    MapDefault::EmptyList => Value::Array(Vec::new()),
                })
            }
        }
    }
}

fn literal_value(map: &'static FieldMap, args: &Args) -> Option<Value> {
    Some(match map.ty {
        MapType::Str => Value::String(args.str(map.arg).ok()?.to_string()),
        MapType::U64 => Value::from(args.u64(map.arg)?),
        MapType::Bool => Value::from(if map.invert {
            !args.flag(map.arg)
        } else {
            args.flag(map.arg)
        }),
        MapType::StrList => Value::Array(
            args.strs(map.arg)
                .iter()
                .map(|s| Value::String(s.clone()))
                .collect(),
        ),
    })
}

/// Set `body["a"]["b"] = value`, creating intermediate objects.
fn set_dotted(body: &mut serde_json::Map<String, Value>, field: &str, value: Value) {
    let mut segments = field.split('.');
    let last = segments.next_back().expect("non-empty field");
    let mut cursor = body;
    for segment in segments {
        let entry = cursor
            .entry(segment.to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(serde_json::Map::new());
        }
        cursor = entry.as_object_mut().expect("object");
    }
    cursor.insert(last.to_string(), value);
}

/// Registry of command interfaces (the CLI's own manifest): internal
/// management groups (Rust-registered) + one interface per tool domain
/// compiled in from `config.json` (`engines_gen::TOOLS`).
pub struct ToolRegistry {
    pub interfaces: Vec<Interface>,
}

impl ToolRegistry {
    pub fn builtins() -> Self {
        let mut interfaces = vec![
            session_cmd::interface(),
            engine_cmd::interface(),
            settings_cmd::interface(),
            tools_cmd::interface(),
            self_cmd::interface(),
        ];
        // Tool domains from config.json (`java`, `binary`, ...): every leaf
        // is endpoint-routed; common args are already baked in by build.rs.
        interfaces.extend(crate::iface::tools_interfaces(crate::engines_gen::TOOLS));
        Self { interfaces }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_leaf(path: &str) -> (&'static ToolSpec, &'static CmdSpec) {
        crate::spec::find_leaf(path).expect("leaf")
    }

    #[test]
    fn set_and_always_mappings_build_the_exact_wire_body() {
        // java.classes from the compiled-in tool registry.
        let (_, classes) = find_leaf("java.classes");
        let route = classes.route.unwrap();
        let mut args = Args::default();
        args.insert_multi("include-package", vec!["com.foo".into(), "com.bar".into()]);
        args.insert_flag("no-regex", true);
        args.insert_string("page", "2".into());
        let body = build_request_body(route.request, &args);
        assert_eq!(body["page"], serde_json::json!(2));
        assert_eq!(body["filter"]["includes"], serde_json::json!(["com.foo", "com.bar"]));
        assert_eq!(body["filter"]["excludes"], serde_json::json!([]));
        assert_eq!(body["filter"]["regex"], serde_json::json!(false)); // --no-regex inverted
        assert!(body.get("filter").unwrap().get("limit").is_none()); // set-mapping absent
    }

    #[test]
    fn always_defaults_emit_when_args_absent() {
        let (_, manifest) = find_leaf("java.manifest");
        let route = manifest.route.unwrap();
        let body = build_request_body(route.request, &Args::default());
        assert_eq!(body, serde_json::json!({ "page": 1 }));
    }

    #[test]
    fn set_mapping_absent_when_flag_missing() {
        let (_, classes) = find_leaf("java.classes");
        let route = classes.route.unwrap();
        let body = build_request_body(route.request, &Args::default());
        assert_eq!(body["page"], serde_json::json!(1));
        assert!(body.get("filter").unwrap().get("regex").is_none());
    }

    #[test]
    fn taint_scan_narrows_to_native() {
        let (java, taint) = find_leaf("java.taint-scan");
        assert_eq!(java.name, "java");
        assert_eq!(taint.engines, ["native"]);
        assert!(!taint.supports_engine("jvm"));
        assert!(taint.supports_engine("native"));
        // generic leaves serve every tool engine
        let (_, classes) = find_leaf("java.classes");
        assert!(classes.engines.is_empty());
        assert!(classes.supports_engine("jvm") && classes.supports_engine("native"));
    }

    #[test]
    fn binary_tool_leaves_resolve() {
        let (binary, ms) = find_leaf("binary.method-source");
        assert_eq!(binary.engines, ["kuna"]);
        let route = ms.route.unwrap();
        assert_eq!(route.endpoint, "get_method_source");
        let arg_ids: Vec<&str> = ms.args.iter().map(|a| a.id).collect();
        assert!(arg_ids.contains(&"function"));
        assert!(!arg_ids.contains(&"signature"));
    }
}
