//! Session lifecycle orchestration — the session layer's core flow.
//!
//! `open_session` is what every entry into a session funnels through
//! (`decx session open`, `decx android framework open`, ...): it resolves the
//! target (URL inputs download), applies the reuse policy, delegates the
//! launch to the engine layer (server spawn or one-shot analyze job), then
//! records and supervises the resulting session. Engine adapters stay pure —
//! the session layer owns records, decisions, and monitoring.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::engine::launcher::{wait_for_server, HEARTBEAT_INTERVAL};
use crate::engine::{Engine, EngineKind, EngineRegistry, TargetSpec};
use crate::error::{DecxError, DecxResult};
use crate::fsx;
use crate::hash::hash_file;
use crate::ports::{parse_server_port, select_available_server_port};
use crate::session::{ObservedState, Session, SessionManager, SessionState};

/// One request to open a session, produced by whichever tool needs an engine
/// run. `origin` records the invoking tool (`decx session open`,
/// `decx android framework open`, ...) for session bookkeeping.
pub struct OpenRequest {
    pub file: String,
    pub engine_id: Option<String>,
    pub port: Option<String>,
    pub name: Option<String>,
    pub force: bool,
    pub scripts: Vec<String>,
    pub passthrough: Vec<String>,
    pub timeout_secs: u64,
    pub origin: String,
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
            origin: "decx session open".to_string(),
        }
    }
}

// ── Reuse policy (pure) ─────────────────────────────────────────────────────

fn same_scripts(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y)
}

/// Sessions a `--force` spawn must replace: any session under the target
/// name, plus any session holding the same file hash (same file = same
/// contract).
pub fn pick_force_replace_sessions(alive: &[Session], name: &str, hash: &str) -> Vec<Session> {
    alive
        .iter()
        .filter(|s| s.name == name || s.hash == hash)
        .cloned()
        .collect()
}

/// What opening a target should do about an already-loaded session.
pub enum OpenReuseDecision {
    Reuse(Session),
    Error(String),
    Spawn { remove_stale_name: Option<String> },
}

/// Decide reuse/replace/spawn for a target with sha256 `hash` (pure, no I/O).
pub fn decide_open_reuse(
    hash: &str,
    _name: &str,
    force: bool,
    alive: &[Session],
    existing_by_name: Option<&Session>,
    scripts: &[String],
) -> OpenReuseDecision {
    if !force {
        if let Some(reuse) = alive
            .iter()
            .find(|s| s.hash == hash && same_scripts(&s.scripts, scripts))
        {
            return OpenReuseDecision::Reuse(reuse.clone());
        }
        if let Some(live) = alive
            .iter()
            .find(|s| s.hash == hash && !same_scripts(&s.scripts, scripts))
        {
            return OpenReuseDecision::Error(format!(
                "Session '{}' is already running for this target with a different script set. \
                 Use --force to restart with the new scripts.",
                live.name
            ));
        }
        if let Some(existing) = existing_by_name {
            if existing.hash != hash {
                return OpenReuseDecision::Error(format!(
                    "Session '{}' already exists for a different target (hash: {}). \
                     Use --force to overwrite, or --name to choose a different session name.",
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

// ── Target resolution ───────────────────────────────────────────────────────

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

/// The last non-empty line of a log file, if any (heartbeat tails).
pub fn last_log_line(log_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(log_path).ok()?;
    content.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_string)
}

fn default_session_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "target".to_string())
}

// ── Open flow ───────────────────────────────────────────────────────────────

/// Full session-open flow. Returns the JSON summary printed to stdout.
pub fn open_session(
    mgr: &Arc<SessionManager>,
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

    let name = req.name.clone().unwrap_or_else(|| default_session_name(&resolved_file));
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

    // `--force` means restart: stop alive sessions this spawn replaces (same
    // name or same file hash). A failed kill aborts the spawn so the old
    // engine process cannot be orphaned while its record is overwritten.
    for stale in pick_force_replace_sessions(&mgr.list_alive(), &name, &file_hash) {
        let result = crate::spawn::kill_tree(stale.pid);
        if result == crate::spawn::KillResult::Failed {
            return Err(DecxError::process(format!(
                "--force could not stop previous session '{}' (pid {}, port {}); refusing to spawn a \
                 duplicate. Kill pid {} manually, then retry.",
                stale.name, stale.pid, stale.port, stale.pid
            )));
        }
        mgr.remove(&stale.name);
    }

    // Command engines (one-shot decompilers like kuna): run the analyze job
    // as a background child and record its exit — no HTTP server to wait for.
    if engine.kind() == EngineKind::Command {
        return open_command_session(mgr, &engine, &binary, name, resolved_file, file_hash, req, notice);
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

    mgr.create(Session {
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
        origin: Some(req.origin.clone()),
        observed: ObservedState {
            state: SessionState::Starting,
            ..Default::default()
        },
    })?;

    let timeout = Duration::from_secs(req.timeout_secs.max(1));
    notice(&format!("Waiting for engine '{name}' (pid {pid}) on port {port}..."));
    let mut heartbeat = |elapsed: u64, last_line: Option<&str>| {
        let tail = last_line
            .map(|l| format!(" | {}", l.chars().take(120).collect::<String>()))
            .unwrap_or_default();
        notice(&format!(
            "Waiting for engine '{name}' (pid {pid})... {elapsed}s elapsed{tail}"
        ));
    };
    let ready = wait_for_server(port, timeout, Some(&log_path), || !crate::spawn::pid_alive(pid), &mut heartbeat);

    if ready {
        mgr.refresh(&name);
        return Ok(json!({
            "name": name,
            "hash": hash_file(&resolved_file)?,
            "pid": pid,
            "port": port,
            "engine": engine.id(),
            "kind": "server",
            "file": resolved_file.display().to_string(),
            "log": log_path.display().to_string(),
            "scripts": req.scripts,
            "reused": false,
        }));
    }

    if !crate::spawn::pid_alive(pid) {
        mgr.remove(&name);
        let last = last_log_line(&log_path).unwrap_or_else(|| "<no log output>".to_string());
        return Err(DecxError::process(format!(
            "engine exited unexpectedly. Last log line: {last}. Log: {}",
            log_path.display()
        )));
    }

    // Timed out but the process is alive: keep the record so the engine stays
    // reachable instead of becoming an untracked orphan.
    Err(DecxError::process(format!(
        "Server did not become healthy within {}s on port {port}, but the process (pid {pid}) is still \
         running — it is likely still analyzing a large target. The session '{name}' was kept; poll it \
         with 'decx session status {name}', or stop it with 'decx session close {name}'. Log: {}",
        req.timeout_secs,
        log_path.display()
    )))
}

/// Session open for command engines: spawn the analyze job (stdout/stderr
/// appended to the session log), wait bounded for its exit code, and record
/// the outcome through the session manager. On timeout the record is kept
/// and the job keeps running.
fn open_command_session(
    mgr: &Arc<SessionManager>,
    engine: &Arc<dyn Engine>,
    binary: &Path,
    name: String,
    resolved_file: PathBuf,
    file_hash: String,
    req: &OpenRequest,
    mut notice: impl FnMut(&str),
) -> DecxResult<Value> {
    let home = mgr.home();
    let engine_id = engine.id().to_string();
    let spec = TargetSpec {
        target: resolved_file.clone(),
        port: 0,
        scripts: req.scripts.clone(),
        passthrough: vec![],
    };
    engine.validate(&spec)?;
    let mut command = engine.build_command(binary, &spec)?;

    let log_path = home.join("logs").join(format!("{name}.log"));
    let mut child = crate::spawn::spawn_logged_child(&mut command, &log_path)?;
    let pid = child.id();

    mgr.create(Session {
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
        origin: Some(req.origin.clone()),
        observed: ObservedState {
            state: SessionState::Starting,
            ..Default::default()
        },
    })?;

    notice(&format!("Running {engine_id} analyze '{name}' (pid {pid})..."));
    let timeout_secs = req.timeout_secs.max(1);
    let started = Instant::now();
    let deadline = started + Duration::from_secs(timeout_secs);
    let mut last_heartbeat: Option<Instant> = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                mgr.set_observed(
                    &name,
                    ObservedState {
                        state: SessionState::Healthy,
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
                    "analyze still running after {timeout_secs}s (pid {pid}); session '{name}' was kept — \
                     poll with 'decx session status {name}' or stop with 'decx session close {name}'. Log: {}",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn session(name: &str, hash: &str, pid: u32, scripts: &[&str]) -> Session {
        Session {
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
            origin: None,
            observed: Default::default(),
        }
    }

    #[test]
    fn reuse_decision_reuses_same_hash_and_scripts() {
        let alive = vec![session("demo", "h1", 111, &[])];
        match decide_open_reuse("h1", "demo", false, &alive, None, &[]) {
            OpenReuseDecision::Reuse(s) => assert_eq!(s.name, "demo"),
            _ => panic!("expected reuse"),
        }
    }

    #[test]
    fn reuse_decision_errors_on_script_mismatch() {
        let alive = vec![session("demo", "h1", 111, &["s.jadx.kts"])];
        match decide_open_reuse("h1", "demo", false, &alive, None, &[]) {
            OpenReuseDecision::Error(msg) => assert!(msg.contains("different script set")),
            _ => panic!("expected error"),
        }
    }

    #[test]
    fn reuse_decision_refuses_name_collision() {
        let existing = session("demo", "other", 111, &[]);
        match decide_open_reuse("h1", "demo", false, &[], Some(&existing), &[]) {
            OpenReuseDecision::Error(msg) => assert!(msg.contains("different target")),
            _ => panic!("expected error"),
        }
    }

    #[test]
    fn reuse_decision_clears_stale_name() {
        let existing = session("demo", "h1", 111, &[]);
        match decide_open_reuse("h1", "demo", false, &[], Some(&existing), &[]) {
            OpenReuseDecision::Spawn { remove_stale_name } => {
                assert_eq!(remove_stale_name.as_deref(), Some("demo"))
            }
            _ => panic!("expected spawn"),
        }
    }

    #[test]
    fn force_bypasses_everything() {
        let alive = vec![session("demo", "h1", 111, &[])];
        match decide_open_reuse("h1", "demo", true, &alive, Some(&alive[0]), &[]) {
            OpenReuseDecision::Spawn { remove_stale_name } => assert!(remove_stale_name.is_none()),
            _ => panic!("expected spawn"),
        }
    }

    #[test]
    fn force_replace_picks_same_name_or_hash() {
        let alive = vec![
            session("demo", "h1", 111, &[]),
            session("other", "h1", 222, &[]),
            session("unrelated", "h2", 333, &[]),
        ];
        let picked = pick_force_replace_sessions(&alive, "demo", "h1");
        let names: Vec<&str> = picked.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["demo", "other"]);
    }

    #[test]
    fn default_session_name_strips_extension() {
        assert_eq!(default_session_name(Path::new("/a/b/demo.apk")), "demo");
        assert_eq!(default_session_name(Path::new("x.jar")), "x");
    }
}
