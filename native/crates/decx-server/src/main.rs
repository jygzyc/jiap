//! Native DECX server — headless replacement for the JVM `decx-server`.
//!
//! Contract-compatible with the DECX CLI family:
//! - `GET  /health`                → Kotlin-compatible health document
//!   (`status/version/url/port/timestamp/active_operations/endpoint_stats/cache`
//!   plus a `native` sub-object with engine details)
//! - `POST /api/decx/<endpoint>`   → dispatch via decx-core; success returns the
//!   full `DecxApiResult` envelope (200), errors return the error envelope with
//!   the Kotlin status mapping (`{ok:false, kind, query, error:{code,message}}`)
//!
//! Usage: `decx-native-server <target.apk|classes.dex> --port <port> [--warm]`

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use decx_core::{api, DecxError, Project};

/// Per-request timeout before a 504 REQUEST_TIMEOUT (matches Kotlin DecxServer).
/// Override with `DECX_NATIVE_REQUEST_TIMEOUT_SECS` — cold full-archive sweeps
/// (search/warm on huge apps) legitimately exceed the 120s default.
fn request_timeout() -> Duration {
    Duration::from_secs(
        std::env::var("DECX_NATIVE_REQUEST_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120),
    )
}

#[derive(Clone)]
struct AppState {
    project: Arc<Project>,
    started: Instant,
    port: u16,
    in_flight: Arc<AtomicU64>,
    endpoint_stats: Arc<Mutex<HashMap<String, (u64, u128)>>>,
}

fn main() {
    // dexdec's recursive DEX/IR processing overflows the 1 MiB Windows default
    // main-thread stack at opt-level >= 2 (huge inlined frames). Run everything
    // on explicitly big-stack threads instead; tokio's worker/blocking threads
    // get the same treatment for request-time decompilation.
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(run)
        .expect("spawn main thread");
    handle.join().expect("main thread panicked");
}

fn run() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(64 * 1024 * 1024)
        .build()
        .expect("tokio runtime");
    runtime.block_on(async_main());
}

async fn async_main() {
    log("process start");
    let mut target: Option<PathBuf> = None;
    let mut port: u16 = 25419;
    let mut warm = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                port = args.next().and_then(|v| v.parse().ok()).unwrap_or(port);
            }
            "--warm" => warm = true,
            "--help" | "-h" => {
                eprintln!("usage: decx-native-server <target.apk|classes.dex> [--port <port>] [--warm]");
                return;
            }
            other => target = Some(PathBuf::from(other)),
        }
    }
    log("args parsed");
    let Some(target) = target else {
        eprintln!("error: missing target file (apk/dex)");
        std::process::exit(2);
    };
    if !target.exists() {
        eprintln!("error: target not found: {}", target.display());
        std::process::exit(2);
    }

    log(&format!("loading target {}", target.display()));
    let load_started = Instant::now();
    let project = match Project::open(&target) {
        Ok(p) => p,
        Err(e) => {
            log(&format!("load failed: {e}"));
            std::process::exit(1);
        }
    };
    log("project open done");
    log(&format!(
        "indexed {} classes in {:.2}s",
        project.entries().len(),
        load_started.elapsed().as_secs_f64()
    ));

    if warm {
        log("warm decompilation started (parallel)");
        let (ok, failed) = project.warm_all();
        log(&format!("warm decompilation done: {ok} ok, {} failed", failed.len()));
    }

    let state = AppState {
        project: Arc::new(project),
        started: Instant::now(),
        port,
        in_flight: Arc::new(AtomicU64::new(0)),
        endpoint_stats: Arc::new(Mutex::new(HashMap::new())),
    };
    log(&format!("http server listening on 127.0.0.1:{port}"));
    // Ready marker consumed by decx-cli's waitForServer (launcher.ts).
    log(&format!("DECX Server running at http://127.0.0.1:{port}"));

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/decx/{endpoint}", post(api_call))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("bind 127.0.0.1");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("server");
}

fn log(msg: &str) {
    // stderr only — stdout stays clean for process supervisors
    eprintln!("[decx-native-server] {msg}");
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let stats: Value = {
        let guard = state.endpoint_stats.lock().map(|s| {
            let mut keys: Vec<&String> = s.keys().collect();
            keys.sort();
            let mut m = serde_json::Map::new();
            for k in keys {
                let (count, total_ms) = s[k];
                m.insert(
                    k.clone(),
                    json!({
                        "count": count,
                        "total_ms": total_ms as u64,
                        "avg_ms": if count > 0 { (total_ms / count as u128) as u64 } else { 0 },
                    }),
                );
            }
            Value::Object(m)
        });
        guard.unwrap_or(Value::Object(serde_json::Map::new()))
    };
    Json(json!({
        // Kotlin-compatible fields (decx-cli `process check` reads `status`).
        "status": "running",
        "version": env!("CARGO_PKG_VERSION"),
        "url": format!("http://127.0.0.1:{}", state.port),
        "port": state.port,
        "timestamp": timestamp,
        "active_operations": state.in_flight.load(Ordering::Relaxed),
        "endpoint_stats": stats,
        "cache": {
            "cacheBytes": state.project.cache_len_bytes(),
            "classCount": state.project.entries().len(),
        },
        // Native-engine extras.
        "native": {
            "server": "decx-native-server",
            "target": state.project.path.display().to_string(),
            "classCount": state.project.entries().len(),
            "cacheBytes": state.project.cache_len_bytes(),
            "uptimeSecs": state.started.elapsed().as_secs(),
        },
    }))
}

async fn api_call(
    State(state): State<AppState>,
    Path(endpoint): Path<String>,
    body: String,
) -> Response {
    let body: Value = if body.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                let err = DecxError::invalid_parameter(format!("bad JSON body: {e}"));
                log_api(&endpoint, Instant::now(), false, Some(&err));
                return error_response(&endpoint, &json!({}), &err);
            }
        }
    };
    let started = Instant::now();
    state.in_flight.fetch_add(1, Ordering::Relaxed);
    let endpoint_for_log = endpoint.clone();
    let project = state.project.clone();
    // Parsing/decompilation can be CPU-heavy; keep the async runtime free.
    // A blocking task cannot be cancelled, but the response times out exactly
    // like the Kotlin server (504 REQUEST_TIMEOUT after 120s).
    let dispatch_body = body.clone();
    let work =
        tokio::task::spawn_blocking(move || api::dispatch(&project, &endpoint, &dispatch_body));
    let result = match tokio::time::timeout(request_timeout(), work).await {
        Ok(joined) => joined.unwrap_or_else(|e| Err(DecxError::internal(format!("task join: {e}")))),
        Err(_) => {
            let path = format!("/api/decx/{endpoint_for_log}");
            let elapsed_ms = request_timeout().as_millis();
            Err(DecxError::new(
                "REQUEST_TIMEOUT",
                format!("Request timeout after {elapsed_ms} ms: {path}"),
            ))
        }
    };
    state.in_flight.fetch_sub(1, Ordering::Relaxed);
    match result {
        Ok(payload) => {
            record_stats(&state, &endpoint_for_log, started);
            log_api(&endpoint_for_log, started, true, None);
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(err) => {
            record_stats(&state, &endpoint_for_log, started);
            log_api(&endpoint_for_log, started, false, Some(&err));
            error_response(&endpoint_for_log, &body, &err)
        }
    }
}

fn record_stats(state: &AppState, endpoint: &str, started: Instant) {
    let elapsed = started.elapsed().as_millis();
    if let Ok(mut stats) = state.endpoint_stats.lock() {
        let entry = stats.entry(endpoint.to_string()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += elapsed;
    }
}

fn error_response(endpoint: &str, body: &Value, err: &DecxError) -> Response {
    let status = StatusCode::from_u16(err.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let envelope = api::error_envelope(endpoint, body, err);
    (status, Json(envelope)).into_response()
}

fn log_api(endpoint: &str, started: Instant, _ok: bool, err: Option<&DecxError>) {
    match err {
        None => log(&format!(
            "POST /api/decx/{endpoint} -> 200 ({} ms)",
            started.elapsed().as_millis()
        )),
        Some(e) => log(&format!(
            "POST /api/decx/{endpoint} -> {} {} ({} ms)",
            e.http_status(),
            e.code,
            started.elapsed().as_millis()
        )),
    }
}
