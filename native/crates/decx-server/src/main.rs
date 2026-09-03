//! Native DECX server — headless replacement for the JVM `decx-server`.
//!
//! Contract-compatible with the DECX CLI family:
//! - `GET  /health`                → `{ "status": "running", ... }`
//! - `POST /api/decx/<endpoint>`   → dispatch via decx-core, 200 JSON on success,
//!   `{ "error": "<CODE>", "message": "..." }` with the Kotlin status mapping on failure.
//!
//! Usage: `decx-native-server <target.apk|classes.dex> --port <port> [--warm]`

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use decx_core::{api, DecxError, Project};

#[derive(Clone)]
struct AppState {
    project: Arc<Project>,
    started: Instant,
}

#[tokio::main]
async fn main() {
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
    log(&format!(
        "indexed {} classes from {} dex(es) in {:.2}s",
        project.entries().len(),
        project.dexes.len(),
        load_started.elapsed().as_secs_f64()
    ));

    if warm {
        log("warm decompilation started (parallel)");
        let (ok, failed) = project.warm_all();
        log(&format!("warm decompilation done: {ok} ok, {} failed", failed.len()));
    }

    let port = port;
    let state = AppState {
        project: Arc::new(project),
        started: Instant::now(),
    };
    log(&format!("http server listening on 127.0.0.1:{port}"));

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
    Json(json!({
        "status": "running",
        "server": "decx-native-server",
        "target": state.project.path.display().to_string(),
        "dexCount": state.project.dexes.len(),
        "classCount": state.project.entries().len(),
        "cacheBytes": state.project.cache_len_bytes(),
        "uptimeSecs": state.started.elapsed().as_secs(),
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
                return error_response(&err);
            }
        }
    };
    let started = Instant::now();
    let endpoint_for_log = endpoint.clone();
    let project = state.project.clone();
    // Parsing/decompilation can be CPU-heavy; keep the async runtime free.
    let result = tokio::task::spawn_blocking(move || api::dispatch(&project, &endpoint, &body))
        .await
        .unwrap_or_else(|e| Err(DecxError::internal(format!("task join: {e}"))));
    match result {
        Ok(payload) => {
            log_api(&endpoint_for_log, started, true, None);
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(err) => {
            log_api(&endpoint_for_log, started, false, Some(&err));
            error_response(&err)
        }
    }
}

fn error_response(err: &DecxError) -> Response {
    let status = StatusCode::from_u16(err.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(json!({ "error": err.code, "message": err.message })),
    )
        .into_response()
}

fn log_api(endpoint: &str, started: Instant, ok: bool, err: Option<&DecxError>) {
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
