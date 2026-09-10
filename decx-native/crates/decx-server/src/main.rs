//! decx-native-server — standalone DECX analysis server.
//! usage: decx-native-server <target.apk|classes.dex|xx.jar> [--port N] [--warm]

mod envelope;
mod http;
mod routes;

use decx_core::Project;
use decx_json::Json;
use routes::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut target: Option<String> = None;
    let mut port: u16 = 25419;
    let mut warm = false;
    let mut taint_rules_path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" if i + 1 < args.len() => {
                port = args[i + 1].parse().unwrap_or(25419);
                i += 2;
            }
            "--warm" => {
                warm = true;
                i += 1;
            }
            "--taint-rules" if i + 1 < args.len() => {
                taint_rules_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--help" | "-h" => {
                println!("decx-native-server <target.apk|classes.dex|xx.jar> [--port N] [--warm] [--taint-rules <rules.json>]");
                return;
            }
            a if !a.starts_with('-') => {
                target = Some(a.to_string());
                i += 1;
            }
            _ => {
                eprintln!("unknown flag {}", args[i]);
                std::process::exit(2);
            }
        }
    }
    let Some(target) = target else {
        eprintln!("usage: decx-native-server <target.apk|classes.dex|xx.jar> [--port N] [--warm]");
        std::process::exit(2);
    };
    let data = match std::fs::read(&target) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("cannot read '{target}': {e}");
            std::process::exit(1);
        }
    };
    eprintln!("decx-native: loading {target}");
    let t0 = std::time::Instant::now();
    let project = match Project::load(&target, data) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("decx-native: load failed: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "decx-native: {} dex(es), {} classes, {} methods, index built in {:.2}s",
        project.dexes.len(),
        project.total_classes(),
        project.total_methods(),
        t0.elapsed().as_secs_f64()
    );
    if warm {
        // force-emit every class once to warm caches / smoke the emitter
        let t1 = std::time::Instant::now();
        let mut n = 0usize;
        for (di, d) in project.dexes.iter().enumerate() {
            for def in &d.class_defs {
                let _ = decx_core::java::emit_class(d, def, &project.dex_names[di]);
                n += 1;
            }
        }
        eprintln!("decx-native: warm: emitted {n} classes in {:.2}s", t1.elapsed().as_secs_f64());
    }
    // taint rule set: --taint-rules <path> > DECX_TAINT_RULES > embedded defaults
    let rules_path = taint_rules_path
        .or_else(|| std::env::var("DECX_TAINT_RULES").ok().filter(|s| !s.is_empty()));
    let taint_rules = match &rules_path {
        Some(path) => match decx_taint::Rules::load_file(path) {
            Ok(r) => {
                eprintln!(
                    "decx-native: taint rules loaded from '{path}' ({} sources, {} sinks)",
                    r.sources.len(),
                    r.sinks.len()
                );
                r
            }
            Err(e) => {
                eprintln!("decx-native: {e}");
                std::process::exit(1);
            }
        },
        None => decx_taint::Rules::default_rules(),
    };

    let state = Arc::new(AppState {
        project: Arc::new(project),
        started: std::time::Instant::now(),
        calls: std::sync::atomic::AtomicU64::new(0),
        taint_rules,
    });

    let addr = format!("127.0.0.1:{port}");
    println!("http server listening on {addr}");
    println!("DECX Server running at http://127.0.0.1:{port}");

    let st = state.clone();
    let handler = move |req: http::Request| -> (u16, String) {
        let st = st.clone();
        let path = req.path.trim_end_matches('/').to_string();
        if req.method == "GET" && (path == "/health" || path.is_empty()) {
            return (200, routes::health(&st, port).to_string());
        }
        if req.method == "POST" {
            if let Some(ep) = path.strip_prefix("/api/decx/") {
                let body = if req.body.is_empty() {
                    Json::obj0()
                } else {
                    decx_json::parse(&String::from_utf8_lossy(&req.body)).unwrap_or_else(|e| {
                        eprintln!("bad json from client: {e}");
                        Json::obj0()
                    })
                };
                return (200, routes::dispatch(&st, ep, &body).to_string());
            }
        }
        let err = envelope::error_envelope("unknown", &Json::obj0(), "UNKNOWN_ENDPOINT", "no such route");
        (404, err.to_string())
    };
    // keep the Arc usage honest even if no atomic read happens here
    let _ = state.calls.load(Ordering::Relaxed);
    if let Err(e) = http::serve(&addr, handler) {
        eprintln!("decx-native: server error: {e}");
        std::process::exit(1);
    }
}
