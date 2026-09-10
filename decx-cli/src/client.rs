//! DECX HTTP client — direct client for the DECX server REST API.
//!
//! One generic method: [`DecxClient::post_endpoint`] posts a request body
//! to `POST /api/decx/<endpoint>` and returns the raw `DecxApiResult` JSON
//! envelope — no unwrapping, no per-endpoint typing. The request bodies are
//! assembled from the compile-time route mappings (see
//! [`crate::commands::run_route`]), so adding an endpoint to an engine's
//! `config.json` declaration is all that is needed to expose it here.
//! Transport is the std-only HTTP/1.1 client in [`crate::net`] (the server
//! is always on 127.0.0.1).

use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::net;

pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

pub struct DecxClient {
    port: u16,
    timeout: Duration,
    session_name: Option<String>,
}

impl DecxClient {
    pub fn new(port: u16) -> Self {
        Self::with_options(port, DEFAULT_TIMEOUT_SECS, None)
    }

    pub fn with_options(port: u16, timeout_secs: u64, session_name: Option<String>) -> Self {
        Self {
            port,
            timeout: Duration::from_secs(timeout_secs.max(1)),
            session_name,
        }
    }

    /// The port this client talks to.
    pub fn port(&self) -> u16 {
        self.port
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> DecxResult<Value> {
        if std::env::var("DECX_DEBUG").ok().as_deref() == Some("1") {
            eprintln!("[DEBUG] {method} http://127.0.0.1:{}{path}", self.port);
            if let Some(b) = body {
                eprintln!("[DEBUG] Body: {b}");
            }
        }
        let started = std::time::Instant::now();
        let result = self.dispatch(method, path, body);
        if let Some(session) = &self.session_name {
            let status = if result.is_ok() { "ok" } else { "error" };
            log_api_call(
                session,
                &json!({
                    "method": method,
                    "path": path,
                    "duration_ms": started.elapsed().as_millis() as u64,
                    "status": status,
                }),
            );
        }
        result
    }

    fn dispatch(&self, method: &str, path: &str, body: Option<&Value>) -> DecxResult<Value> {
        let response = net::request(self.port, method, path, body, self.timeout)?;
        if response.status == 200 {
            return response.body_json();
        }
        // Server error bodies look like { "error": "E0xx", "message": "..." }
        // or { "error": { "code", "message" } }; fall back to the HTTP status.
        let mut code = format!("HTTP_{}", response.status);
        let mut message = format!("HTTP {}", response.status);
        if let Ok(parsed) = response.body_json() {
            match parsed.get("error") {
                Some(Value::Object(obj)) => {
                    if let Some(c) = obj.get("code").and_then(Value::as_str) {
                        code = c.to_string();
                    }
                    if let Some(m) = obj.get("message").and_then(Value::as_str) {
                        message = m.to_string();
                    }
                }
                Some(Value::String(c)) => {
                    code = c.clone();
                    message = c.clone();
                    if let Some(m) = parsed.get("message").and_then(Value::as_str) {
                        message = m.to_string();
                    }
                }
                _ => {}
            }
        }
        Err(DecxError::server(code, message))
    }

    // ── Health ──────────────────────────────────────────────────────────────

    pub fn health_check(&self) -> DecxResult<Value> {
        self.request("GET", "/health", None)
    }

    /// True when `/health` answered HTTP 200 with a running-ish status.
    /// Engines report either `"running"` (jvm/kuna) or `"ok"` (native) —
    /// a 200 from the health endpoint is the authoritative readiness signal
    /// (same semantics as the TypeScript launcher's `response.ok`).
    pub fn is_healthy(&self) -> bool {
        match self.health_check() {
            Ok(value) => match value.get("status").and_then(Value::as_str) {
                None => true,
                Some(status) => status == "running" || status == "ok",
            },
            Err(_) => false,
        }
    }

    // ── The engine API ──────────────────────────────────────────────────────

    /// POST a request body to `POST /api/decx/<endpoint>`. This single
    /// generic method carries every engine-declared command.
    pub fn post_endpoint(&self, endpoint: &str, body: &Value) -> DecxResult<Value> {
        self.request("POST", &format!("/api/decx/{endpoint}"), Some(body))
    }
}

fn log_api_call(session: &str, entry: &Value) {
    let dir = crate::settings::decx_path(&["logs"]);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("api-{session}.jsonl"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = writeln!(f, "{entry}");
    }
}
