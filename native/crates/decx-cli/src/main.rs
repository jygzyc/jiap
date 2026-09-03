//! decx-native CLI — mirrors decx-cli's command tree:
//! - `process open|list|check|close` — spawn/query/stop decx-native-server
//! - `code ...`                      — analysis queries against a running server
//!
//! stdout stays JSON-only; progress/errors go to stderr.

mod client;
mod sessions;

use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use sessions::{Session, SessionStore};

#[derive(Parser)]
#[command(
    name = "decx-native",
    about = "Native (Rust) DECX CLI — sessions + analysis against decx-native-server",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command_,
}

#[derive(Subcommand)]
enum Command_ {
    /// Server session lifecycle
    Process {
        #[command(subcommand)]
        command: ProcessCommand,
    },
    /// Code-analysis queries against a running session
    Code {
        #[command(subcommand)]
        command: CodeCommand,
    },
}

#[derive(Subcommand)]
enum ProcessCommand {
    /// Open a target (apk/dex) as a new server session
    Open {
        /// Target file: .apk / .dex
        file: PathBuf,
        /// Session name (defaults to the file stem)
        #[arg(long)]
        name: Option<String>,
        /// Server port (auto-assigns a free port when omitted)
        #[arg(long)]
        port: Option<u16>,
        /// Health-wait timeout in seconds
        #[arg(long, default_value_t = 300)]
        timeout: u64,
        /// Replace alive sessions matching the same name or same file
        #[arg(long)]
        force: bool,
        /// Decompile every class right after startup (slower open, instant queries)
        #[arg(long)]
        warm: bool,
    },
    /// List tracked sessions
    List,
    /// Check server health by port or session name
    Check {
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Close one session (name / --name / --port) or all sessions
    Close {
        name: Option<String>,
        #[arg(long)]
        name_flag: Option<String>,
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
enum CodeCommand {
    /// GET /health
    Health,
    /// List / filter classes
    GetClasses {
        #[arg(long)]
        includes: Vec<String>,
        #[arg(long)]
        excludes: Vec<String>,
        #[arg(long, default_value_t = 200)]
        limit: usize,
        /// Treat includes/excludes as literal globs instead of regex
        #[arg(long, default_value_t = true)]
        regex: bool,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Decompile one class to Java (or smali)
    GetClassSource {
        cls: String,
        #[arg(long)]
        smali: bool,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Class summary + source + members
    GetClassContext {
        cls: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Grep decompiled sources globally
    SearchGlobalKey {
        key: String,
        #[arg(long)]
        includes: Vec<String>,
        #[arg(long)]
        excludes: Vec<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        case_sensitive: bool,
        #[arg(long)]
        regex: bool,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Grep inside one class
    SearchClassKey {
        cls: String,
        key: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long)]
        case_sensitive: bool,
        #[arg(long)]
        regex: bool,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Find methods by name (optionally "Class.method")
    SearchMethod {
        mth: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Decompile one method
    GetMethodSource {
        mth: String,
        #[arg(long)]
        smali: bool,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Method source + CFG summary
    GetMethodContext {
        mth: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Method control-flow graph + bytecode listing
    GetMethodCfg {
        mth: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Who calls this method (all dexes)
    GetMethodXref {
        mth: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Who reads/writes this field
    GetFieldXref {
        fld: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Structural references to a class
    GetClassXref {
        cls: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Implementers of an interface
    GetImplementations {
        iface: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
    /// Direct subclasses of a class
    GetSubclasses {
        cls: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = run(cli);
    std::process::exit(code);
}

fn run(cli: Cli) -> i32 {
    match cli.command {
        Command_::Process { command } => run_process(command),
        Command_::Code { command } => run_code(command),
    }
}

// ── process ─────────────────────────────────────────────────────────────────

fn run_process(cmd: ProcessCommand) -> i32 {
    match cmd {
        ProcessCommand::Open { file, name, port, timeout, force, warm } => process_open(file, name, port, timeout, force, warm),
        ProcessCommand::List => process_list(),
        ProcessCommand::Check { port, name } => process_check(port, name),
        ProcessCommand::Close { name, name_flag, port, all } => {
            process_close(name.or(name_flag), port, all)
        }
    }
}

fn process_open(file: PathBuf, name: Option<String>, port: Option<u16>, timeout: u64, force: bool, warm: bool) -> i32 {
    let file = match file.canonicalize() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot resolve {}: {e}", file.display());
            return 1;
        }
    };
    if !file.is_file() {
        eprintln!("error: not a file: {}", file.display());
        return 1;
    }
    let hash = match sessions::file_hash(&file) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("error: cannot hash file: {e}");
            return 1;
        }
    };
    let name = name.unwrap_or_else(|| file.file_stem().unwrap_or_default().to_string_lossy().to_string());

    let mut store = SessionStore::load();
    if force {
        // kill alive sessions with same name or same file hash
        let victims: Vec<Session> = store
            .sessions
            .iter()
            .filter(|s| s.name == name || s.file_hash == hash)
            .cloned()
            .collect();
        for v in victims {
            eprintln!("[force] killing session {} (port {})", v.name, v.port);
            sessions::kill_pid(v.pid);
            store.sessions.retain(|s| s.name != v.name);
        }
    } else if let Some(existing) = store.sessions.iter().find(|s| s.name == name || s.file_hash == hash) {
        if client::health_ok(existing.port) {
            eprintln!(
                "error: session {} already serves this target on port {} — reuse it or pass --force",
                existing.name, existing.port
            );
            return 1;
        }
    }

    let server = sessions::server_exe();
    if !server.exists() {
        eprintln!("error: server binary not found at {} — build it first (cargo build -p decx-server)", server.display());
        return 1;
    }
    let port = port.unwrap_or_else(sessions::pick_free_port);
    let logs_dir = sessions::home().join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);
    let log_path = logs_dir.join(format!("{name}.log"));
    let log_stdout = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot create log file {}: {e}", log_path.display());
            return 1;
        }
    };
    let log_stderr = log_stdout.try_clone().expect("clone log file");

    eprintln!("[open] spawning {} on port {} (log: {})", server.display(), port, log_path.display());
    let mut child = match Command::new(&server)
        .arg(&file)
        .arg("--port")
        .arg(port.to_string())
        .args(warm.then(|| "--warm".to_string()))
        .stdout(std::process::Stdio::from(log_stdout))
        .stderr(std::process::Stdio::from(log_stderr))
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: spawn failed: {e}");
            return 1;
        }
    };
    let pid = child.id();

    // health wait with heartbeat
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let mut last_beat = Instant::now();
    let mut healthy = false;
    while Instant::now() < deadline {
        if client::health_ok(port) {
            healthy = true;
            break;
        }
        if let Some(status) = child.try_wait().ok().flatten() {
            eprintln!("error: server exited during startup (code {status:?}) — see {}", log_path.display());
            return 1;
        }
        if last_beat.elapsed() >= Duration::from_secs(15) {
            last_beat = Instant::now();
            eprintln!("[open] still waiting for health... ({}s elapsed)", timeout.saturating_sub(deadline.elapsed().as_secs().max(0) as u64).min(timeout));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    if !healthy {
        eprintln!(
            "error: timeout after {timeout}s — session record kept; check `decx-native process check --port {port}` or `decx-native process close {name}`"
        );
        let session = Session {
            name: name.clone(),
            port,
            pid,
            target: file.display().to_string(),
            file_hash: hash,
            created_at: sessions::now_string(),
        };
        store.sessions.retain(|s| s.name != name);
        store.sessions.push(session);
        let _ = store.save();
        return 1;
    }
    let _ = child.try_wait(); // non-blocking; server keeps running detached

    let health = client::get(port, "/health").ok().and_then(|r| serde_json::from_str::<Value>(&r.body).ok());
    let session = Session {
        name: name.clone(),
        port,
        pid,
        target: file.display().to_string(),
        file_hash: hash,
        created_at: sessions::now_string(),
    };
    store.sessions.retain(|s| s.name != name);
    store.sessions.push(session);
    let _ = store.save();

    let mut out = json!({ "session": name, "port": port, "pid": pid, "target": file.display().to_string() });
    if let Some(h) = health {
        out["health"] = h;
    }
    println!("{out}");
    0
}

fn process_list() -> i32 {
    let store = SessionStore::load();
    let sessions: Vec<Value> = store
        .sessions
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "port": s.port,
                "pid": s.pid,
                "target": s.target,
                "healthy": client::health_ok(s.port),
            })
        })
        .collect();
    println!("{}", json!({ "sessions": sessions }));
    0
}

fn process_check(port: Option<u16>, name: Option<String>) -> i32 {
    let store = SessionStore::load();
    let resolved_port = match (port, name) {
        (Some(p), _) => Some(p),
        (None, Some(n)) => store.by_name(&n).map(|s| s.port),
        (None, None) => None,
    };
    let Some(port) = resolved_port else {
        eprintln!("error: pass --port <port> or --name <session>");
        return 1;
    };
    match client::get(port, "/health") {
        Ok(resp) if resp.status == 200 => {
            print!("{}", resp.body);
            0
        }
        Ok(resp) => {
            eprintln!("error: unexpected status {}", resp.status);
            1
        }
        Err(e) => {
            eprintln!("error: health check failed on port {port}: {e}");
            1
        }
    }
}

fn process_close(name: Option<String>, port: Option<u16>, all: bool) -> i32 {
    let mut store = SessionStore::load();
    let victims: Vec<Session> = if all {
        store.sessions.clone()
    } else if let Some(n) = &name {
        store.by_name(n).cloned().into_iter().collect()
    } else if let Some(p) = port {
        store.by_port(p).cloned().into_iter().collect()
    } else {
        Vec::new()
    };
    if victims.is_empty() {
        eprintln!("error: no matching session (pass a name, --port, or --all)");
        return 1;
    }
    let mut failed = 0;
    for v in victims {
        eprintln!("[close] stopping {} (pid {}, port {})", v.name, v.pid, v.port);
        if !sessions::kill_pid(v.pid) {
            eprintln!("[close] warning: pid {} still alive", v.pid);
            failed += 1;
            continue; // keep the record on failed kills, like decx-cli
        }
        store.sessions.retain(|s| s.name != v.name);
    }
    let _ = store.save();
    if failed > 0 {
        1
    } else {
        println!("{}", json!({ "closed": true }));
        0
    }
}

// ── code ────────────────────────────────────────────────────────────────────

struct CodeTarget {
    port: u16,
}

fn resolve_target(session: Option<String>, port: Option<u16>) -> Result<CodeTarget, i32> {
    if let Some(p) = port {
        return Ok(CodeTarget { port: p });
    }
    let store = SessionStore::load();
    let port = match session {
        Some(name) => store.by_name(&name).map(|s| s.port),
        None => store.sessions.first().map(|s| s.port),
    };
    match port {
        Some(p) => Ok(CodeTarget { port: p }),
        None => {
            eprintln!("error: no session — run `decx-native process open <file>` first or pass --port");
            Err(1)
        }
    }
}

fn call(port: u16, endpoint: &str, body: Value) -> i32 {
    let resp = match client::post_json(port, &format!("/api/decx/{endpoint}"), &body) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: connection failed on port {port}: {e} — is the server running? see `decx-native process list`");
            return 1;
        }
    };
    match serde_json::from_str::<Value>(&resp.body) {
        Ok(v) => {
            if resp.status == 200 {
                println!("{v}");
                0
            } else {
                let code = v.get("error").and_then(Value::as_str).unwrap_or("UNKNOWN");
                let msg = v.get("message").and_then(Value::as_str).unwrap_or("");
                eprintln!("error: [{code}] {msg}");
                1
            }
        }
        Err(e) => {
            eprintln!("error: bad response (HTTP {}): {e}", resp.status);
            1
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run_code(cmd: CodeCommand) -> i32 {
    use CodeCommand as C;
    let (session, port) = match &cmd {
        C::Health => (None, None),
        C::GetClasses { session, port, .. }
        | C::GetClassSource { session, port, .. }
        | C::GetClassContext { session, port, .. }
        | C::SearchGlobalKey { session, port, .. }
        | C::SearchClassKey { session, port, .. }
        | C::SearchMethod { session, port, .. }
        | C::GetMethodSource { session, port, .. }
        | C::GetMethodContext { session, port, .. }
        | C::GetMethodCfg { session, port, .. }
        | C::GetMethodXref { session, port, .. }
        | C::GetFieldXref { session, port, .. }
        | C::GetClassXref { session, port, .. }
        | C::GetImplementations { session, port, .. }
        | C::GetSubclasses { session, port, .. } => (session.clone(), *port),
    };
    let target = match resolve_target(session, port) {
        Ok(t) => t,
        Err(code) => return code,
    };
    match cmd {
        C::Health => match client::get(target.port, "/health") {
            Ok(r) if r.status == 200 => {
                println!("{}", r.body);
                0
            }
            Ok(r) => {
                eprintln!("error: HTTP {}", r.status);
                1
            }
            Err(e) => {
                eprintln!("error: connection failed: {e}");
                1
            }
        },
        C::GetClasses { includes, excludes, limit, regex, .. } => call(
            target.port,
            "get_classes",
            json!({ "filter": { "limit": limit, "includes": includes, "excludes": excludes, "regex": regex } }),
        ),
        C::GetClassSource { cls, smali, limit, .. } => call(
            target.port,
            "get_class_source",
            json!({ "cls": cls, "smali": smali, "filter": { "limit": limit } }),
        ),
        C::GetClassContext { cls, .. } => call(target.port, "get_class_context", json!({ "cls": cls })),
        C::SearchGlobalKey { key, includes, excludes, limit, case_sensitive, regex, .. } => call(
            target.port,
            "search_global_key",
            json!({ "key": key, "search": { "limit": limit, "caseSensitive": case_sensitive, "regex": regex }, "filter": { "includes": includes, "excludes": excludes } }),
        ),
        C::SearchClassKey { cls, key, limit, case_sensitive, regex, .. } => call(
            target.port,
            "search_class_key",
            json!({ "cls": cls, "key": key, "grep": { "limit": limit, "caseSensitive": case_sensitive, "regex": regex } }),
        ),
        C::SearchMethod { mth, .. } => call(target.port, "search_method", json!({ "mth": mth })),
        C::GetMethodSource { mth, smali, .. } => {
            call(target.port, "get_method_source", json!({ "mth": mth, "smali": smali }))
        }
        C::GetMethodContext { mth, .. } => call(target.port, "get_method_context", json!({ "mth": mth })),
        C::GetMethodCfg { mth, .. } => call(target.port, "get_method_cfg", json!({ "mth": mth })),
        C::GetMethodXref { mth, .. } => call(target.port, "get_method_xref", json!({ "mth": mth })),
        C::GetFieldXref { fld, .. } => call(target.port, "get_field_xref", json!({ "fld": fld })),
        C::GetClassXref { cls, .. } => call(target.port, "get_class_xref", json!({ "cls": cls })),
        C::GetImplementations { iface, .. } => call(target.port, "get_implementations", json!({ "iface": iface })),
        C::GetSubclasses { cls, .. } => call(target.port, "get_subclasses", json!({ "cls": cls })),
    }
}

// keep stdout flushed promptly for piped JSON consumers
fn _flush() {
    let _ = std::io::stdout().flush();
}
