//! decx-server-sdk — build an engine server that speaks the DECX HTTP
//! contract.
//!
//! Every analysis backend (decx server, kuna, future tools) is served to the
//! CLI through the same wire contract: `GET /health` →
//! `{"status":"running",...}` and `POST /api/decx/<endpoint>` → the
//! `DecxApiResult` envelope. This crate is the runtime half of that scheme:
//! a tool implements [`SdkService`] (one handler per endpoint), and
//! [`serve`] provides the HTTP server — accept loop, request parsing,
//! routing, envelope wrapping, and error/status mapping. Engines built on
//! the SDK compile together with the CLI in one `cargo build`.
//!
//! ```no_run
//! use decx_server_sdk::{serve, SdkService, ServiceArc};
//! use serde_json::{json, Value};
//! struct MyEngine;
//! impl SdkService for MyEngine {
//!     fn handle(&self, endpoint: &str, _body: &Value) -> decx_cli_core::DecxResult<Value> {
//!         Ok(json!({ "endpoint": endpoint }))
//!     }
//! }
//! // decx-server-sdk::serve(25419, ServiceArc::new(MyEngine)).unwrap();
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use decx_cli_core::error::{DecxError, DecxResult};
use serde_json::{json, Value};

/// Readiness marker the CLI greps in server logs before health checking.
pub const READY_MARKER: &str = "DECX Server running at";

/// The analysis endpoints of the DECX contract (mirrors `DecxRoutes`).
pub const KNOWN_ENDPOINTS: &[&str] = &[
    "get_classes",
    "get_class_context",
    "get_class_source",
    "search_global_key",
    "search_class_key",
    "search_method",
    "get_method_source",
    "get_method_context",
    "get_method_cfg",
    "get_method_xref",
    "get_field_xref",
    "get_class_xref",
    "get_implementations",
    "get_subclasses",
    "get_aidl_interfaces",
    "get_app_manifest",
    "get_main_activity",
    "get_application",
    "get_exported_components",
    "get_deep_links",
    "get_dynamic_receivers",
    "get_all_resources",
    "get_resource_file",
    "get_strings",
    "get_system_service_impl",
];

/// The handler surface an engine server implements. Handlers receive the
/// parsed request body and return the endpoint payload — the SDK wraps it in
/// the `{"code":"OK","data":...}` envelope and maps errors.
pub trait SdkService: Send + Sync + 'static {
    /// Human-readable engine name surfaced in `/health`.
    fn server_name(&self) -> &str {
        "decx-engine-server"
    }

    /// Handle one analysis endpoint.
    fn handle(&self, endpoint: &str, body: &Value) -> DecxResult<Value>;

    /// Endpoints this server actually implements (for `engine show`); empty
    /// means "everything the handler accepts".
    fn capabilities(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// Health payload; `status` must be `"running"` when ready.
    fn health(&self) -> Value {
        json!({ "status": "running", "server": self.server_name() })
    }
}

/// Alias so servers can pass `Arc<dyn SdkService>` without naming the trait
/// object type.
pub type ServiceArc = Arc<dyn SdkService>;

/// Bind the server port and print the readiness marker (which the CLI's
/// launch flow greps in the session log).
pub fn bind(port: u16) -> DecxResult<TcpListener> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| DecxError::process(format!("cannot bind port {port}: {e}")))?;
    let addr = listener
        .local_addr()
        .map(|a| a.port().to_string())
        .unwrap_or_else(|_| port.to_string());
    println!("{READY_MARKER} http://127.0.0.1:{addr}");
    Ok(listener)
}

/// Serve forever: one thread per connection, `Connection: close` semantics.
pub fn serve(listener: TcpListener, service: ServiceArc) -> DecxResult<()> {
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        let service = Arc::clone(&service);
        std::thread::spawn(move || {
            let _ = handle_connection(&mut stream, service);
        });
    }
    Ok(())
}

fn handle_connection(stream: &mut TcpStream, service: ServiceArc) -> DecxResult<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(300)))
        .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(300))))
        .map_err(|e| DecxError::internal(e.to_string()))?;

    let Some((method, path, body)) = read_request(stream) else {
        return write_response(stream, 400, &json!({ "error": { "code": "BAD_REQUEST", "message": "malformed request" } }));
    };

    let (status, payload) = match (method.as_str(), path.as_str()) {
        ("GET", "/health") => (200u16, service.health()),
        ("POST", p) if p.starts_with("/api/decx/") => {
            let endpoint = &p["/api/decx/".len()..];
            if !KNOWN_ENDPOINTS.contains(&endpoint) {
                (
                    404,
                    error_body(&DecxError::not_found(
                        "UNKNOWN_ENDPOINT",
                        format!("Unknown endpoint: {endpoint}"),
                    )),
                )
            } else {
                match service.handle(endpoint, &body) {
                    Ok(data) => (
                        200,
                        json!({ "code": "OK", "data": data, "meta": { "server": service.server_name() } }),
                    ),
                    Err(err) => (http_status_for(&err), error_body(&err)),
                }
            }
        }
        _ => (
            404,
            error_body(&DecxError::not_found("UNKNOWN_ENDPOINT", format!("Unknown route: {path}"))),
        ),
    };
    write_response(stream, status, &payload)
}

fn error_body(err: &DecxError) -> Value {
    json!({ "error": { "code": err.code, "message": err.message } })
}

/// Map an error to an HTTP status by its sysexits class: not-found (66) →
/// 404, usage (64) → 400, timeout (75) → 504, internal (70) → 500, else 503.
fn http_status_for(err: &DecxError) -> u16 {
    match err.exit_code {
        code if code == decx_cli_core::error::EX_NOINPUT => 404,
        code if code == decx_cli_core::error::EX_USAGE => 400,
        code if code == decx_cli_core::error::EX_TEMPFAIL => 504,
        code if code == decx_cli_core::error::EX_SOFTWARE && err.code == "INTERNAL_ERROR" => 500,
        _ => 503,
    }
}

/// Read one request (headers + Content-Length body). `None` on malformed
/// input. The connection is closed after one response, so read-to-end of the
/// body length is all we need.
fn read_request(stream: &mut TcpStream) -> Option<(String, String, Value)> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 16 * 1024 * 1024 {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = head.lines();
    let status_line = lines.next()?;
    let mut parts = status_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();

    let mut content_length = 0usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else { continue };
        if name.trim().eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body_bytes = buf[header_end + 4..].to_vec();
    while body_bytes.len() < content_length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        body_bytes.extend_from_slice(&chunk[..n]);
    }
    body_bytes.truncate(content_length);
    let body: Value = if body_bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or(json!({}))
    };
    Some((method, path, body))
}

fn write_response(stream: &mut TcpStream, status: u16, payload: &Value) -> DecxResult<()> {
    let body = serde_json::to_string(payload).unwrap_or_else(|_| "{}".into());
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|e| DecxError::internal(format!("write failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    struct Echo;
    impl SdkService for Echo {
        fn server_name(&self) -> &str {
            "echo-test"
        }
        fn handle(&self, endpoint: &str, body: &Value) -> DecxResult<Value> {
            match endpoint {
                "get_classes" => Ok(json!({ "echo": body["q"], "at": endpoint })),
                "get_strings" => Err(DecxError::not_found("NO_STRINGS_FOUND", "none")),
                _ => Err(DecxError::usage("unsupported")),
            }
        }
    }

    fn spawn_server() -> (u16, std::thread::JoinHandle<()>) {
        let listener = bind(0).unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || serve(listener, Arc::new(Echo)).unwrap());
        (port, handle)
    }

    fn post(port: u16, path: &str, body: &str) -> (u16, Value) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        let head_end = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let status: u16 = String::from_utf8_lossy(&raw[..head_end])
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let body: Value = serde_json::from_slice(&raw[head_end + 4..]).unwrap();
        (status, body)
    }

    #[test]
    fn serves_health_envelope_and_error_mapping() {
        let (port, handle) = spawn_server();

        // health
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(b"GET /health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        assert!(raw.starts_with("HTTP/1.1 200 OK"));
        assert!(raw.contains("\"status\":\"running\""));
        assert!(raw.contains("echo-test"));

        // success envelope
        let (status, body) = post(port, "/api/decx/get_classes", r#"{"q":"serde"}"#);
        assert_eq!(status, 200);
        assert_eq!(body["code"], "OK");
        assert_eq!(body["data"]["echo"], "serde");
        assert_eq!(body["meta"]["server"], "echo-test");

        // handler error -> *_NOT_FOUND maps to 404 with error body
        let (status, body) = post(port, "/api/decx/get_strings", "{}");
        assert_eq!(status, 404);
        assert_eq!(body["error"]["code"], "NO_STRINGS_FOUND");

        // usage error -> 400
        let (status, body) = post(port, "/api/decx/get_classesx", "{}");
        // unknown endpoint itself is 404 UNKNOWN_ENDPOINT
        assert_eq!(status, 404);
        assert_eq!(body["error"]["code"], "UNKNOWN_ENDPOINT");
        drop(handle);
    }

    #[test]
    fn status_mapping_rules() {
        assert_eq!(http_status_for(&DecxError::not_found("CLASS_NOT_FOUND", "x")), 404);
        assert_eq!(http_status_for(&DecxError::usage("x")), 400);
        assert_eq!(http_status_for(&DecxError::timeout("x")), 504);
        assert_eq!(http_status_for(&DecxError::internal("x")), 500);
        assert_eq!(http_status_for(&DecxError::process("x")), 503);
    }

    #[test]
    fn contract_endpoint_list_is_complete() {
        for name in KNOWN_ENDPOINTS {
            assert!(name.starts_with("get_") || name.starts_with("search_"), "{name}");
        }
        assert!(KNOWN_ENDPOINTS.contains(&"get_system_service_impl"));
        assert_eq!(KNOWN_ENDPOINTS.len(), 25);
    }
}
