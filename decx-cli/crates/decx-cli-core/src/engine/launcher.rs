//! `project open` flow: target resolution, reuse decisions, detached spawn,
//! verified health wait, and force-replace semantics.
//!
//! The pure decision helpers (`decide_open_reuse`,
//! `normalize_jadx_passthrough_args`, `sanitize_rename_flags_value`) are
//! direct ports of the TypeScript launcher with their unit tests.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::engine::{Engine, EngineKind, EngineRegistry, TargetSpec};
use crate::error::{DecxError, DecxResult};
use crate::fsx;
use crate::hash::hash_file;
use crate::ports::{parse_server_port, select_available_server_port};
use crate::project::{Project, ProjectManager, ProjectState};

pub use crate::spawn::default_java_heap;

// ── Pure decision helpers ───────────────────────────────────────────────────

/// Drop the `printable` token from a `--rename-flags` value so obfuscated
/// Unicode identifiers survive decompilation. `None` when the value cannot be
/// parsed safely — the caller then leaves the user's spelling untouched.
pub fn sanitize_rename_flags_value(value: &str) -> Option<String> {
    let raw = value.trim();
    if raw.is_empty() {
        return None;
    }
    let upper = raw.to_uppercase();
    if upper == "NONE" {
        return Some("NONE".to_string());
    }
    if upper == "ALL" {
        return Some("CASE,VALID".to_string());
    }
    let tokens: Vec<&str> = raw.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
    if tokens.is_empty()
        || !tokens
            .iter()
            .all(|t| matches!(t.to_uppercase().as_str(), "CASE" | "VALID" | "PRINTABLE" | "ALL"))
    {
        return None;
    }
    let kept: Vec<&str> = tokens
        .into_iter()
        .filter(|t| !t.eq_ignore_ascii_case("printable"))
        .collect();
    Some(if kept.is_empty() { "NONE".to_string() } else { kept.join(",").to_uppercase() })
}

const RENAME_FLAGS_ARGS: [&str; 2] = ["--rename-flags", "-rf"];

fn has_rename_flags_arg(args: &[String]) -> bool {
    args.iter().any(|arg| {
        RENAME_FLAGS_ARGS.iter().any(|name| {
            arg == name || arg.strip_prefix(&format!("{name}=")).is_some()
        })
    })
}

fn strip_printable_rename_flag(args: &[String]) -> Vec<String> {
    let mut result = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let eq_form = RENAME_FLAGS_ARGS
            .iter()
            .find_map(|name| arg.strip_prefix(&format!("{name}=")).map(|v| (name.to_string(), v.to_string())));
        if RENAME_FLAGS_ARGS.contains(&arg.as_str()) {
            let value = args.get(i + 1).map(String::as_str).unwrap_or("");
            result.push(arg.clone());
            result.push(sanitize_rename_flags_value(value).unwrap_or_else(|| value.to_string()));
            i += 2;
        } else if let Some((name, value)) = eq_form {
            let sanitized = sanitize_rename_flags_value(&value).unwrap_or(value);
            result.push(format!("{name}={sanitized}"));
            i += 1;
        } else {
            result.push(arg.clone());
            i += 1;
        }
    }
    result
}

/// Normalize jadx passthrough args exactly like the TypeScript CLI:
/// strip `--deobf`, guarantee `--show-bad-code`, `--no-imports`,
/// `-Pdex-input.verify-checksum=no`, and default `--rename-flags case,valid`
/// (DECX queries original symbol names; `printable` would hide obfuscated
/// Unicode identifiers behind `m0`-style aliases).
pub fn normalize_jadx_passthrough_args(args: &[String]) -> Vec<String> {
    let filtered: Vec<String> = args.iter().filter(|a| a.as_str() != "--deobf").cloned().collect();
    let mut result = strip_printable_rename_flag(&filtered);
    for required in ["--show-bad-code", "--no-imports", "-Pdex-input.verify-checksum=no"] {
        if !result.iter().any(|a| a == required) {
            result.push(required.to_string());
        }
    }
    if !has_rename_flags_arg(&result) {
        result.push("--rename-flags".to_string());
        result.push("case,valid".to_string());
    }
    result
}

fn same_scripts(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y)
}

/// Sessions a `--force` spawn must replace: any project under the target name,
/// plus any project holding the same file hash (same file = same contract).
pub fn pick_force_replace_projects(alive: &[Project], name: &str, hash: &str) -> Vec<Project> {
    alive
        .iter()
        .filter(|p| p.name == name || p.hash == hash)
        .cloned()
        .collect()
}

/// What `project open` should do about an already-loaded target.
pub enum OpenReuseDecision {
    Reuse(Project),
    Error(String),
    Spawn { remove_stale_name: Option<String> },
}

/// Decide reuse/replace/spawn for a target with sha256 `hash` (pure, no I/O).
#[allow(clippy::too_many_arguments)]
pub fn decide_open_reuse(
    hash: &str,
    _name: &str,
    force: bool,
    alive: &[Project],
    existing_by_name: Option<&Project>,
    scripts: &[String],
) -> OpenReuseDecision {
    if !force {
        if let Some(reuse) = alive
            .iter()
            .find(|p| p.hash == hash && same_scripts(&p.scripts, scripts))
        {
            return OpenReuseDecision::Reuse(reuse.clone());
        }
        if let Some(live) = alive
            .iter()
            .find(|p| p.hash == hash && !same_scripts(&p.scripts, scripts))
        {
            return OpenReuseDecision::Error(format!(
                "Project '{}' is already running for this target with a different script set. \
                 Use --force to restart with the new scripts.",
                live.name
            ));
        }
        if let Some(existing) = existing_by_name {
            if existing.hash != hash {
                return OpenReuseDecision::Error(format!(
                    "Project '{}' already exists for a different target (hash: {}). \
                     Use --force to overwrite, or --name to choose a different project name.",
                    existing.name, existing.hash
                ));
            }
            return OpenReuseDecision::Spawn {
                remove_stale_name: Some(existing.name.clone()),
            };
        }
    }
    OpenReuseDecision::Spawn { remove_stale_name: None }
}

// ── File input resolution ───────────────────────────────────────────────────

/// Resolve a target path; `http(s)://` inputs download into `<home>/tmp`.
pub fn resolve_file_input(home: &Path, input: &str) -> DecxResult<PathBuf> {
    if !input.starts_with("http://") && !input.starts_with("https://") {
        return Ok(PathBuf::from(input));
    }
    let tmp_dir = home.join("tmp");
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| DecxError::internal(format!("cannot create {}: {e}", tmp_dir.display())))?;

    let url_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())[..8].to_string()
    };
    let raw_name = input.rsplit('/').next().unwrap_or("target.bin");
    let filename = if has_known_ext(raw_name) {
        format!("{url_hash}_{raw_name}")
    } else {
        format!("{raw_name}_{url_hash}.bin")
    };
    let local_path = tmp_dir.join(filename);
    if local_path.exists() {
        return Ok(local_path);
    }

    eprintln!("  Downloading {input} ...");
    crate::net::download_to_file(input, &local_path)?;
    eprintln!("  Saved to {}", local_path.display());
    Ok(local_path)
}

fn has_known_ext(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".apk", ".dex", ".jar", ".class", ".aar"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

// ── Health wait ─────────────────────────────────────────────────────────────

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

/// Poll for server readiness: log-file ready marker first, then the health
/// endpoint. `heartbeat` fires roughly every 15s while waiting.
pub fn wait_for_server(
    port: u16,
    timeout: Duration,
    log_path: Option<&Path>,
    process_exited: impl Fn() -> bool,
    mut heartbeat: impl FnMut(u64, Option<&str>),
) -> bool {
    let start = Instant::now();
    let mut last_log_line: Option<String> = None;
    let mut last_heartbeat = Instant::now();
    while start.elapsed() < timeout {
        if process_exited() {
            return false;
        }
        if let Some(log) = log_path {
            if let Ok(content) = std::fs::read_to_string(log) {
                last_log_line = content
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map(str::to_string)
                    .or(last_log_line);
                if content.contains("DECX Server running at") {
                    let client = crate::client::DecxClient::new(port);
                    if client.is_healthy() {
                        return true;
                    }
                }
            }
        }
        // Fallback: plain health probe (every other second)
        if start.elapsed().as_secs() % 2 == 0 {
            let client = crate::client::DecxClient::new(port);
            if client.is_healthy() {
                return true;
            }
        }
        if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            last_heartbeat = Instant::now();
            heartbeat(start.elapsed().as_secs(), last_log_line.as_deref());
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    false
}

/// The last line of a server log file, if any.
pub fn last_log_line(log_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(log_path).ok()?;
    content.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_string)
}

// ── Open flow ───────────────────────────────────────────────────────────────

pub struct OpenRequest {
    pub file: String,
    pub engine_id: Option<String>,
    pub port: Option<String>,
    pub name: Option<String>,
    pub force: bool,
    pub scripts: Vec<String>,
    pub passthrough: Vec<String>,
    pub timeout_secs: u64,
}

impl Default for OpenRequest {
    fn default() -> Self {
        Self {
            file: String::new(),
            engine_id: None,
            port: None,
            name: None,
            force: false,
            scripts: vec![],
            passthrough: vec![],
            timeout_secs: 300,
        }
    }
}

fn is_pid_alive_portable(pid: u32) -> bool {
    crate::spawn::pid_alive(pid)
}

/// Spawn an `exit` probe helper shared with the wait loop.
struct ExitProbe {
    pid: u32,
}

impl ExitProbe {
    fn exited(&self) -> bool {
        !is_pid_alive_portable(self.pid)
    }
}

/// Full `project open` flow. Returns the JSON summary printed to stdout.
pub fn open_analysis_target(
    mgr: &Arc<ProjectManager>,
    engines: &EngineRegistry,
    req: &OpenRequest,
    mut notice: impl FnMut(&str),
) -> DecxResult<Value> {
    let home = mgr.home();
    let engine = engines.resolve(req.engine_id.as_deref())?;
    let requested_port = match &req.port {
        Some(p) => Some(parse_server_port(p)?),
        None => None,
    };

    let binary = engine.resolve_binary(home)?;
    let resolved_file = resolve_file_input(home, &req.file)?;
    if !resolved_file.exists() {
        return Err(DecxError::file(
            format!("File not found: {}", resolved_file.display()),
            Some(resolved_file.display().to_string()),
        ));
    }

    let mut scripts = Vec::new();
    for script in &req.scripts {
        let resolved = resolve_file_input(home, script)?;
        if !resolved.exists() {
            return Err(DecxError::file(
                format!("Script file not found: {}", resolved.display()),
                Some(resolved.display().to_string()),
            ));
        }
        scripts.push(resolved.display().to_string());
    }

    let name = req.name.clone().unwrap_or_else(|| {
        default_project_name(&resolved_file)
    });
    let file_hash = hash_file(&resolved_file)?;

    let decision = decide_open_reuse(
        &file_hash,
        &name,
        req.force,
        &mgr.list_alive(),
        mgr.get(&name).as_ref(),
        &scripts,
    );
    match decision {
        OpenReuseDecision::Reuse(reuse) => {
            return Ok(json!({
                "name": reuse.name,
                "hash": reuse.hash,
                "pid": reuse.pid,
                "port": reuse.port,
                "engine": reuse.engine,
                "kind": reuse.engine_kind,
                "file": resolved_file.display().to_string(),
                "reused": true,
            }));
        }
        OpenReuseDecision::Error(message) => return Err(DecxError::process(message)),
        OpenReuseDecision::Spawn { remove_stale_name } => {
            if let Some(stale) = remove_stale_name {
                mgr.remove(&stale);
            }
        }
    }

    // `--force` means restart: stop alive servers this spawn replaces (same
    // name or same file hash). A failed kill aborts the spawn so the old
    // server cannot be orphaned while its record is overwritten.
    for stale in pick_force_replace_projects(&mgr.list_alive(), &name, &file_hash) {
        let result = crate::spawn::kill_tree(stale.pid);
        if result == crate::spawn::KillResult::Failed {
            return Err(DecxError::process(format!(
                "--force could not stop previous project '{}' (pid {}, port {}); refusing to spawn a \
                 duplicate server. Kill pid {} manually, then retry.",
                stale.name, stale.pid, stale.port, stale.pid
            )));
        }
        mgr.remove(&stale.name);
    }

    // Command engines (one-shot decompilers like kuna): run the analyze job
    // as a background child and record its exit — no HTTP server to wait for.
    if engine.kind() == EngineKind::Command {
        return open_command_engine(mgr, &engine, &binary, name, resolved_file, file_hash, scripts, req.timeout_secs, notice);
    }

    let port = select_available_server_port(requested_port)?;
    let spec = TargetSpec {
        target: resolved_file.clone(),
        port,
        scripts: scripts.clone(),
        passthrough: req.passthrough.clone(),
    };
    engine.validate(&spec)?;
    let mut command = engine.build_command(&binary, &spec)?;

    let log_path = home.join("logs").join(format!("{name}.log"));
    let pid = crate::spawn::spawn_detached(&mut command, &log_path)?;

    let project = Project {
        name: name.clone(),
        hash: file_hash,
        file: resolved_file.clone(),
        engine: engine.id().to_string(),
        engine_kind: EngineKind::Server.as_str().to_string(),
        pid,
        port,
        scripts,
        log_path: Some(log_path.clone()),
        created_at_ms: fsx::now_ms(),
        observed: crate::project::ObservedState {
            state: ProjectState::Starting,
            ..Default::default()
        },
    };
    mgr.create(project)?;

    let timeout = Duration::from_secs(req.timeout_secs.max(1));
    let probe = ExitProbe { pid };
    notice(&format!("Waiting for decx-server '{name}' (pid {pid}) on port {port}..."));
    let mut heartbeat = |elapsed: u64, last_line: Option<&str>| {
        let tail = last_line
            .map(|l| format!(" | {}", l.chars().take(120).collect::<String>()))
            .unwrap_or_default();
        notice(&format!(
            "Waiting for decx-server '{name}' (pid {pid})... {elapsed}s elapsed{tail}"
        ));
    };
    let ready = wait_for_server(port, timeout, Some(&log_path), || probe.exited(), &mut heartbeat);

    if ready {
        mgr.refresh(&name);
        return Ok(json!({
            "name": name,
            "hash": hash_file(&resolved_file)?,
            "pid": pid,
            "port": port,
            "engine": engine.id(),
            "file": resolved_file.display().to_string(),
            "log": log_path.display().to_string(),
            "scripts": req.scripts,
            "reused": false,
        }));
    }

    if probe.exited() {
        mgr.remove(&name);
        let last = last_log_line(&log_path).unwrap_or_else(|| "<no log output>".to_string());
        return Err(DecxError::process(format!(
            "decx-server exited unexpectedly. Last log line: {last}. Log: {}",
            log_path.display()
        )));
    }

    // Timed out but the process is alive: keep the record so the server stays
    // reachable instead of becoming an untracked orphan.
    Err(DecxError::process(format!(
        "Server did not become healthy within {}s on port {port}, but the process (pid {pid}) is still \
         running — it is likely still decompiling a large target. The project '{name}' was kept; poll it \
         with 'decx project status {name}', or stop it with 'decx project close {name}'. Log: {}",
        req.timeout_secs,
        log_path.display()
    )))
}

/// `project open` for command engines: spawn the analyze job (stdout/stderr
/// appended to the project log), wait bounded for its exit code, and record
/// the outcome through the project manager. On timeout the record is kept
/// and the job keeps running.
fn open_command_engine(
    mgr: &Arc<ProjectManager>,
    engine: &Arc<dyn Engine>,
    binary: &Path,
    name: String,
    resolved_file: PathBuf,
    file_hash: String,
    scripts: Vec<String>,
    timeout_secs: u64,
    mut notice: impl FnMut(&str),
) -> DecxResult<Value> {
    let home = mgr.home();
    let engine_id = engine.id().to_string();
    let spec = TargetSpec {
        target: resolved_file.clone(),
        port: 0,
        scripts,
        passthrough: vec![],
    };
    engine.validate(&spec)?;
    let mut command = engine.build_command(binary, &spec)?;

    let log_path = home.join("logs").join(format!("{name}.log"));
    let mut child = crate::spawn::spawn_logged_child(&mut command, &log_path)?;
    let pid = child.id();

    mgr.create(Project {
        name: name.clone(),
        hash: file_hash.clone(),
        file: resolved_file.clone(),
        engine: engine_id.clone(),
        engine_kind: "command".into(),
        pid,
        port: 0,
        scripts: vec![],
        log_path: Some(log_path.clone()),
        created_at_ms: fsx::now_ms(),
        observed: crate::project::ObservedState {
            state: ProjectState::Starting,
            ..Default::default()
        },
    })?;

    notice(&format!("Running {engine_id} analyze '{name}' (pid {pid})..."));
    let timeout_secs = timeout_secs.max(1);
    let started = Instant::now();
    let deadline = started + Duration::from_secs(timeout_secs);
    let mut last_heartbeat: Option<Instant> = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                mgr.set_observed(
                    &name,
                    crate::project::ObservedState {
                        state: ProjectState::Healthy,
                        checked_at_ms: fsx::now_ms(),
                        latency_ms: None,
                        detail: Some("analyze finished (exit 0)".into()),
                        ever_healthy: true,
                    },
                );
                return Ok(json!({
                    "name": name,
                    "hash": file_hash,
                    "pid": pid,
                    "engine": engine_id,
                    "kind": "command",
                    "file": resolved_file.display().to_string(),
                    "log": log_path.display().to_string(),
                    "reused": false,
                }));
            }
            Ok(Some(status)) => {
                mgr.remove(&name);
                let last = last_log_line(&log_path).unwrap_or_else(|| "<no log output>".to_string());
                return Err(DecxError::process(format!(
                    "engine '{engine_id}' analyze failed (exit {}). Last log line: {last}. Log: {}",
                    status.code().map(|c| c.to_string()).unwrap_or_else(|| "<signal>".into()),
                    log_path.display()
                )));
            }
            Ok(None) if Instant::now() >= deadline => {
                // Timed out but the job is alive: keep the record (starting,
                // pid tracked) so it stays reachable and killable.
                return Err(DecxError::process(format!(
                    "analyze still running after {timeout_secs}s (pid {pid}); project '{name}' was kept — \
                     poll with 'decx project status {name}' or stop with 'decx project close {name}'. Log: {}",
                    log_path.display()
                )));
            }
            Ok(None) => {
                if last_heartbeat.is_none_or(|at| at.elapsed() >= HEARTBEAT_INTERVAL) {
                    last_heartbeat = Some(Instant::now());
                    let tail = last_log_line(&log_path)
                        .map(|l| format!(" | {}", l.chars().take(120).collect::<String>()))
                        .unwrap_or_default();
                    notice(&format!(
                        "Running {engine_id} analyze '{name}' (pid {pid})... {}s elapsed{tail}",
                        started.elapsed().as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(e) => return Err(DecxError::process(format!("failed to wait for analyze job: {e}"))),
        }
    }
}

fn default_project_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "target".to_string())
}

/// Server reachability probe used by `project check`.
pub fn check_server(port: u16, retries: u32) -> (bool, String) {
    for i in 0..retries {
        let client = crate::client::DecxClient::with_options(port, 2, None);
        if client.is_healthy() {
            return (true, format!("Server running on port {port}"));
        }
        if i + 1 < retries {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    (false, format!("No server on port {port}"))
}

/// Discovery status for every engine adapter (for `project check` /
/// `self status`).
pub fn engine_status(home: &Path, engines: &EngineRegistry) -> Value {
    let mut map = serde_json::Map::new();
    for id in engines.ids() {
        let Some(engine) = engines.get(id) else { continue };
        let mut info = engine.status_info(home);
        info["kind"] = json!(engine.kind().as_str());
        if engine.kind() == EngineKind::Command {
            info["capabilities"] = json!(engine.capabilities());
        }
        if !engine.description().is_empty() {
            info["description"] = json!(engine.description());
        }
        map.insert(id.to_string(), info);
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ObservedState;

    #[test]
    fn sanitize_rename_flags() {
        assert_eq!(sanitize_rename_flags_value("case,valid").as_deref(), Some("CASE,VALID"));
        assert_eq!(sanitize_rename_flags_value("CASE, PRINTABLE").as_deref(), Some("CASE"));
        assert_eq!(sanitize_rename_flags_value("all").as_deref(), Some("CASE,VALID"));
        assert_eq!(sanitize_rename_flags_value("printable").as_deref(), Some("NONE"));
        assert_eq!(sanitize_rename_flags_value("none").as_deref(), Some("NONE"));
        assert_eq!(sanitize_rename_flags_value("bogus"), None);
        assert_eq!(sanitize_rename_flags_value(""), None);
    }

    #[test]
    fn normalize_passthrough_args() {
        let args: Vec<String> = vec!["--deobf".into(), "--py-flag".into()];
        let out = normalize_jadx_passthrough_args(&args);
        assert!(!out.contains(&"--deobf".to_string()));
        assert!(out.contains(&"--show-bad-code".to_string()));
        assert!(out.contains(&"--no-imports".to_string()));
        assert!(out.contains(&"-Pdex-input.verify-checksum=no".to_string()));
        assert!(out.windows(2).any(|w| w[0] == "--rename-flags" && w[1] == "case,valid"));
    }

    #[test]
    fn normalize_keeps_user_rename_flags_but_strips_printable() {
        let args: Vec<String> = vec!["--rename-flags".into(), "printable,case".into()];
        let out = normalize_jadx_passthrough_args(&args);
        assert!(out.windows(2).any(|w| w[0] == "--rename-flags" && w[1] == "CASE"));
        // no default injection when user supplied flags
        assert!(!out.windows(2).any(|w| w[0] == "--rename-flags" && w[1] == "case,valid"));

        let eq: Vec<String> = vec!["-rf=printable".into()];
        let out = normalize_jadx_passthrough_args(&eq);
        assert!(out.contains(&"-rf=NONE".to_string()));
    }

    fn project(name: &str, hash: &str, pid: u32, scripts: &[&str]) -> Project {
        Project {
            name: name.to_string(),
            hash: hash.to_string(),
            file: PathBuf::from("/tmp/x.apk"),
            engine: "jvm".into(),
            engine_kind: "server".into(),
            pid,
            port: 30000,
            scripts: scripts.iter().map(|s| s.to_string()).collect(),
            log_path: None,
            created_at_ms: 0,
            observed: ObservedState::default(),
        }
    }

    #[test]
    fn reuse_decision_reuses_same_hash_and_scripts() {
        let alive = vec![project("demo", "h1", 111, &[])];
        match decide_open_reuse("h1", "demo", false, &alive, None, &[]) {
            OpenReuseDecision::Reuse(p) => assert_eq!(p.name, "demo"),
            _ => panic!("expected reuse"),
        }
    }

    #[test]
    fn reuse_decision_errors_on_script_mismatch() {
        let alive = vec![project("demo", "h1", 111, &["s.jadx.kts"])];
        match decide_open_reuse("h1", "demo", false, &alive, None, &[]) {
            OpenReuseDecision::Error(msg) => assert!(msg.contains("different script set")),
            _ => panic!("expected error"),
        }
    }

    #[test]
    fn reuse_decision_refuses_name_collision() {
        let existing = project("demo", "other", 111, &[]);
        match decide_open_reuse("h1", "demo", false, &[], Some(&existing), &[]) {
            OpenReuseDecision::Error(msg) => assert!(msg.contains("different target")),
            _ => panic!("expected error"),
        }
    }

    #[test]
    fn reuse_decision_clears_stale_name() {
        let existing = project("demo", "h1", 111, &[]);
        match decide_open_reuse("h1", "demo", false, &[], Some(&existing), &[]) {
            OpenReuseDecision::Spawn { remove_stale_name } => {
                assert_eq!(remove_stale_name.as_deref(), Some("demo"))
            }
            _ => panic!("expected spawn"),
        }
    }

    #[test]
    fn force_bypasses_everything() {
        let alive = vec![project("demo", "h1", 111, &[])];
        match decide_open_reuse("h1", "demo", true, &alive, Some(&alive[0]), &[]) {
            OpenReuseDecision::Spawn { remove_stale_name } => assert!(remove_stale_name.is_none()),
            _ => panic!("expected spawn"),
        }
    }

    #[test]
    fn force_replace_picks_same_name_or_hash() {
        let alive = vec![
            project("demo", "h1", 111, &[]),
            project("other", "h1", 222, &[]),
            project("unrelated", "h2", 333, &[]),
        ];
        let picked = pick_force_replace_projects(&alive, "demo", "h1");
        let names: Vec<&str> = picked.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["demo", "other"]);
    }

    #[test]
    fn default_project_name_strips_extension() {
        assert_eq!(default_project_name(Path::new("/a/b/demo.apk")), "demo");
        assert_eq!(default_project_name(Path::new("x.jar")), "x");
    }
}
