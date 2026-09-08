//! DECX HTTP client — direct client for the DECX server REST API.
//!
//! Port of the TypeScript `DecxClient`: all methods return the raw
//! `DecxApiResult` JSON envelope — no unwrapping — so callers and the output
//! formatter see exactly what the server sent. Transport is the std-only
//! HTTP/1.1 client in [`crate::net`] (the server is always on 127.0.0.1).

use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};
use crate::net;
use crate::params::{ClassFilter, ClassGrep, ComponentFilter, GlobalSearch, SourceFilter};

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

    pub fn is_healthy(&self) -> bool {
        self.health_check()
            .ok()
            .and_then(|v| v.get("status").and_then(Value::as_str).map(str::to_string))
            .as_deref()
            == Some("running")
    }

    // ── Common code analysis ────────────────────────────────────────────────

    pub fn get_classes(&self, filter: &ClassFilter, page: u64) -> DecxResult<Value> {
        let mut body = filter.to_value();
        body["page"] = json!(page);
        self.request("POST", "/api/decx/get_classes", Some(&body))
    }

    pub fn search_global_key(&self, key: &str, search: &GlobalSearch, page: u64) -> DecxResult<Value> {
        let mut body = search.to_value();
        body["key"] = json!(key);
        body["page"] = json!(page);
        self.request("POST", "/api/decx/search_global_key", Some(&body))
    }

    pub fn get_class_context(&self, cls: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_class_context", Some(&json!({ "cls": cls, "page": page })))
    }

    pub fn get_class_source(
        &self,
        cls: &str,
        smali: bool,
        filter: &SourceFilter,
        page: u64,
    ) -> DecxResult<Value> {
        let mut body = filter.to_value();
        body["cls"] = json!(cls);
        body["smali"] = json!(smali);
        body["page"] = json!(page);
        self.request("POST", "/api/decx/get_class_source", Some(&body))
    }

    pub fn search_class_key(&self, cls: &str, key: &str, grep: &ClassGrep, page: u64) -> DecxResult<Value> {
        let mut body = grep.to_value();
        body["cls"] = json!(cls);
        body["key"] = json!(key);
        body["page"] = json!(page);
        self.request("POST", "/api/decx/search_class_key", Some(&body))
    }

    pub fn search_method(&self, mth: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/search_method", Some(&json!({ "mth": mth, "page": page })))
    }

    // ── Method context ──────────────────────────────────────────────────────

    pub fn get_method_source(&self, mth: &str, smali: bool, page: u64) -> DecxResult<Value> {
        self.request(
            "POST",
            "/api/decx/get_method_source",
            Some(&json!({ "mth": mth, "smali": smali, "page": page })),
        )
    }

    pub fn get_method_source_full(&self, mth: &str, smali: bool, filter: &SourceFilter, page: u64) -> DecxResult<Value> {
        let mut body = filter.to_value();
        body["mth"] = json!(mth);
        body["smali"] = json!(smali);
        body["page"] = json!(page);
        self.request("POST", "/api/decx/get_method_source", Some(&body))
    }

    pub fn get_method_context(&self, mth: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_method_context", Some(&json!({ "mth": mth, "page": page })))
    }

    pub fn get_method_cfg(&self, mth: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_method_cfg", Some(&json!({ "mth": mth, "page": page })))
    }

    pub fn get_method_xref(&self, mth: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_method_xref", Some(&json!({ "mth": mth, "page": page })))
    }

    pub fn get_field_xref(&self, fld: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_field_xref", Some(&json!({ "fld": fld, "page": page })))
    }

    pub fn get_class_xref(&self, cls: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_class_xref", Some(&json!({ "cls": cls, "page": page })))
    }

    pub fn get_implementations(&self, iface: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_implementations", Some(&json!({ "iface": iface, "page": page })))
    }

    pub fn get_subclasses(&self, cls: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_subclasses", Some(&json!({ "cls": cls, "page": page })))
    }

    // ── Android app analysis ────────────────────────────────────────────────

    pub fn get_aidl_interfaces(&self, filter: &ClassFilter, page: u64) -> DecxResult<Value> {
        let mut body = filter.to_value();
        body["page"] = json!(page);
        self.request("POST", "/api/decx/get_aidl_interfaces", Some(&body))
    }

    pub fn get_app_manifest(&self, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_app_manifest", Some(&json!({ "page": page })))
    }

    pub fn get_main_activity(&self, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_main_activity", Some(&json!({ "page": page })))
    }

    pub fn get_application(&self, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_application", Some(&json!({ "page": page })))
    }

    pub fn get_exported_components(&self, filter: &ComponentFilter, page: u64) -> DecxResult<Value> {
        let mut body = filter.to_value();
        body["page"] = json!(page);
        self.request("POST", "/api/decx/get_exported_components", Some(&body))
    }

    pub fn get_deep_links(&self, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_deep_links", Some(&json!({ "page": page })))
    }

    pub fn get_dynamic_receivers(&self, filter: &ClassFilter, page: u64) -> DecxResult<Value> {
        let mut body = filter.to_value();
        body["page"] = json!(page);
        self.request("POST", "/api/decx/get_dynamic_receivers", Some(&body))
    }

    pub fn get_all_resources(&self, includes: &[String], regex: Option<bool>, page: u64) -> DecxResult<Value> {
        let mut filter = json!({ "includes": includes });
        if let Some(r) = regex {
            filter["regex"] = json!(r);
        }
        self.request(
            "POST",
            "/api/decx/get_all_resources",
            Some(&json!({ "filter": filter, "page": page })),
        )
    }

    pub fn get_resource_file(&self, res: &str, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_resource_file", Some(&json!({ "res": res, "page": page })))
    }

    pub fn get_strings(&self, page: u64) -> DecxResult<Value> {
        self.request("POST", "/api/decx/get_strings", Some(&json!({ "page": page })))
    }

    // ── Android framework analysis ──────────────────────────────────────────

    pub fn get_system_service_impl(&self, iface: &str, page: u64) -> DecxResult<Value> {
        self.request(
            "POST",
            "/api/decx/get_system_service_impl",
            Some(&json!({ "iface": iface, "page": page })),
        )
    }
}

fn log_api_call(session: &str, entry: &Value) {
    let dir = crate::config::decx_path(&["logs"]);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("api-{session}.jsonl"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = writeln!(f, "{entry}");
    }
}
