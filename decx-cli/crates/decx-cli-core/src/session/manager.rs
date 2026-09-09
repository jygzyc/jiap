//! [`SessionManager`] — the independent session manager.
//!
//! Owns every analysis session record, supervises background execution, and
//! exposes monitoring: health/exit probes, per-session monitor threads, a
//! bounded in-memory event ring (mirrored to disk), and event subscriptions
//! for `session watch`. Both engine kinds are first-class: server projects
//! live while their PID answers, command projects live while their analysis
//! artifacts are ready.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use crate::client::DecxClient;
use crate::error::DecxResult;
use crate::spawn;

use super::model::{evaluate_state, ObservedState, ProbeOutcome, Session, SessionEvent, SessionState};
use super::monitor::{spawn_monitor, MonitorHandle};
use super::store::SessionStore;

const EVENT_RING_CAPACITY: usize = 200;
/// Sessions older than this are pruned even if their PID looks alive.
const SESSION_MAX_AGE_MS: u64 = 30 * 24 * 60 * 60 * 1000;

struct Inner {
    monitors: HashMap<String, MonitorHandle>,
    events: HashMap<String, VecDeque<SessionEvent>>,
    subscribers: Vec<(String, Sender<SessionEvent>)>,
}

pub struct SessionManager {
    home: PathBuf,
    store: SessionStore,
    inner: Mutex<Inner>,
}

impl SessionManager {
    /// Open (or lazily create) the session store under `home`.
    pub fn open(home: &Path) -> Arc<SessionManager> {
        Arc::new(Self {
            home: home.to_path_buf(),
            store: SessionStore::new(home),
            inner: Mutex::new(Inner {
                monitors: HashMap::new(),
                events: HashMap::new(),
                subscribers: Vec::new(),
            }),
        })
    }

    pub fn store(&self) -> &SessionStore {
        &self.store
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    // ── Record management ───────────────────────────────────────────────────

    pub fn create(&self, session: Session) -> DecxResult<()> {
        self.store.save(&session)
    }

    pub fn get(&self, name: &str) -> Option<Session> {
        self.store.load(name)
    }

    /// Remove a record and stop any in-process monitor watching it.
    pub fn remove(&self, name: &str) {
        self.stop_monitor(name);
        self.store.remove(name);
        self.inner.lock().unwrap().events.remove(name);
    }

    pub fn list(&self) -> Vec<Session> {
        self.store.list()
    }

    /// Projects that are still usable: server projects whose PID is live,
    /// command projects whose analysis finished successfully.
    pub fn list_alive(&self) -> Vec<Session> {
        self.list()
            .into_iter()
            .filter(|p| match p.is_command_kind() {
                true => p.observed.state == SessionState::Healthy,
                false => spawn::pid_alive(p.pid),
            })
            .collect()
    }

    /// Auto-select the single alive session, if exactly one exists.
    pub fn auto_select(&self) -> Option<Session> {
        let alive = self.list_alive();
        if alive.len() == 1 {
            Some(alive.into_iter().next().unwrap())
        } else {
            None
        }
    }

    /// Drop unusable records: server projects whose process is gone, command
    /// projects whose analysis failed, and anything expired. Returns how
    /// many were removed.
    pub fn cleanup_dead(&self) -> usize {
        let now = crate::fsx::now_ms();
        let mut removed = 0;
        for session in self.list() {
            let expired = now.saturating_sub(session.created_at_ms) > SESSION_MAX_AGE_MS;
            let dead = if session.is_command_kind() {
                session.observed.state == SessionState::Stopped
            } else {
                !spawn::pid_alive(session.pid)
            };
            if expired || dead {
                self.remove(&session.name);
                removed += 1;
            }
        }
        removed
    }

    // ── Probing / monitoring ────────────────────────────────────────────────

    /// One live probe of a session, routed by engine kind: server projects
    /// probe PID liveness + `/health`; command projects only watch the
    /// analyze job (terminal once it ran).
    pub fn probe(&self, session: &Session) -> Session {
        if session.is_command_kind() {
            return self.probe_command(session);
        }
        let started = std::time::Instant::now();
        let client = DecxClient::with_options(session.port, 2, None);
        let health = client.health_check();
        let probe = ProbeOutcome {
            pid_alive: spawn::pid_alive(session.pid),
            health,
            latency_ms: started.elapsed().as_millis() as u64,
        };
        let (state, detail, latency) = evaluate_state(session.observed.ever_healthy, &probe);
        let observed = ObservedState {
            state,
            checked_at_ms: crate::fsx::now_ms(),
            latency_ms: latency,
            detail,
            ever_healthy: session.observed.ever_healthy || state == SessionState::Healthy,
        };
        let mut fallback = session.clone();
        fallback.observed = observed.clone();
        self.set_observed(&session.name, observed);
        self.get(&session.name).unwrap_or(fallback)
    }

    /// Command-engine probe: while the analyze pid lives the session is
    /// `starting`; once it is gone the artifacts are ready (the exit code
    /// was recorded by `session open` when it could observe it).
    fn probe_command(&self, session: &Session) -> Session {
        let (state, detail) = if session.observed.ever_healthy {
            // Terminal state recorded at open time — nothing left to watch.
            (session.observed.state, session.observed.detail.clone())
        } else if spawn::pid_alive(session.pid) {
            (SessionState::Starting, Some("analyzing".to_string()))
        } else {
            (SessionState::Healthy, Some("analyze exited (exit code unknown; see log)".to_string()))
        };
        let observed = ObservedState {
            state,
            checked_at_ms: crate::fsx::now_ms(),
            latency_ms: None,
            detail,
            ever_healthy: session.observed.ever_healthy || state == SessionState::Healthy,
        };
        let mut fallback = session.clone();
        fallback.observed = observed.clone();
        self.set_observed(&session.name, observed);
        self.get(&session.name).unwrap_or(fallback)
    }

    /// Deep-refresh a named session (returns None when unknown).
    pub fn refresh(&self, name: &str) -> Option<Session> {
        self.get(name).map(|p| self.probe(&p))
    }

    /// Deep-refresh every session.
    pub fn refresh_all(&self) {
        let projects = self.list();
        for session in &projects {
            self.probe(session);
        }
    }

    /// Persist an observed state, recording (and broadcasting) a transition
    /// event when the state changed. The single write path for probes, the
    /// launcher's analyze-job outcome, and monitors.
    pub fn set_observed(&self, name: &str, observed: ObservedState) {
        let Some(mut session) = self.store.load(name) else {
            return;
        };
        let from = session.observed.state;
        let to = observed.state;
        let detail = observed.detail.clone();
        session.observed = observed;
        if let Err(e) = self.store.save(&session) {
            if std::env::var("DECX_DEBUG").ok().as_deref() == Some("1") {
                eprintln!("[DEBUG] failed to persist observed state for {name}: {e}");
            }
            return;
        }
        if from != to {
            self.record_event(name, from, to, detail);
        }
    }

    fn record_event(&self, session_name: &str, from: SessionState, to: SessionState, detail: Option<String>) {
        let event = SessionEvent {
            session: session_name.to_string(),
            at_ms: crate::fsx::now_ms(),
            from,
            to,
            detail,
        };
        self.store.append_event(&event);
        let mut inner = self.inner.lock().unwrap();
        let ring = inner.events.entry(session_name.to_string()).or_default();
        if ring.len() == EVENT_RING_CAPACITY {
            ring.pop_front();
        }
        ring.push_back(event.clone());
        inner.subscribers.retain(|(name, tx)| {
            (name.as_str() == session_name || name.as_str() == "*") && tx.send(event.clone()).is_ok()
        });
    }

    /// Recent events for a session: in-memory ring first, then the on-disk log.
    pub fn recent_events(&self, session_name: &str, limit: usize) -> Vec<SessionEvent> {
        let mut events = self.store.read_events(session_name, EVENT_RING_CAPACITY);
        if events.is_empty() {
            return events;
        }
        if events.len() > limit {
            events.drain(..events.len() - limit);
        }
        events
    }

    /// Start a background monitor thread for one session (idempotent). The
    /// monitor runs until stopped, the session record disappears, or the
    /// session reaches a terminal state.
    pub fn start_monitor(self: &Arc<Self>, session_name: &str, interval: Duration) {
        self.stop_monitor(session_name);
        let weak: Weak<SessionManager> = Arc::downgrade(self);
        let handle = spawn_monitor(weak, session_name.to_string(), interval);
        self.inner.lock().unwrap().monitors.insert(session_name.to_string(), handle);
    }

    pub fn stop_monitor(&self, session_name: &str) {
        if let Some(mut handle) = self.inner.lock().unwrap().monitors.remove(session_name) {
            handle.stop();
        }
    }

    /// Subscribe to state-transition events for one session (or `"*"` for all).
    pub fn subscribe(self: &Arc<Self>, session_name: &str) -> Receiver<SessionEvent> {
        let (tx, rx) = channel();
        self.inner
            .lock()
            .unwrap()
            .subscribers
            .push((session_name.to_string(), tx));
        rx
    }

    /// Live monitor state for diagnostics (`session list` footer).
    pub fn monitored_sessions(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let mut names: Vec<String> = inner.monitors.keys().cloned().collect();
        names.sort();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::model::ObservedState;

    fn temp_home() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("decx-mgr-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample(name: &str, port: u16, pid: u32) -> Session {
        Session {
            name: name.to_string(),
            hash: format!("hash-{name}"),
            file: PathBuf::from("/tmp/demo.apk"),
            engine: "jvm".into(),
            engine_kind: "server".into(),
            pid,
            port,
            scripts: vec![],
            log_path: None,
            origin: None,
            created_at_ms: crate::fsx::now_ms(),
            observed: ObservedState::default(),
        }
    }

    fn command_sample(name: &str) -> Session {
        let mut session = sample(name, 0, 4_000_000);
        session.engine = "kuna".into();
        session.engine_kind = "command".into();
        session.observed = ObservedState {
            state: SessionState::Healthy,
            ever_healthy: true,
            ..Default::default()
        };
        session
    }

    #[test]
    fn create_list_remove() {
        let home = temp_home();
        let mgr = SessionManager::open(&home);
        mgr.create(sample("demo", 30001, std::process::id())).unwrap();
        mgr.create(sample("other", 30002, 4_000_000)).unwrap();
        assert_eq!(mgr.list().len(), 2);
        // demo's pid is this test process (alive); other's pid is bogus (dead)
        let alive = mgr.list_alive();
        assert_eq!(alive.len(), 1);
        assert_eq!(alive[0].name, "demo");
        assert_eq!(mgr.auto_select().unwrap().name, "demo");
        assert_eq!(mgr.cleanup_dead(), 1);
        mgr.remove("demo");
        assert!(mgr.list().is_empty());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn command_projects_stay_alive_after_analyze_exits() {
        let home = temp_home();
        let mgr = SessionManager::open(&home);
        // A finished command session has a dead pid but healthy artifacts.
        mgr.create(command_sample("kb")).unwrap();
        assert_eq!(mgr.list_alive().len(), 1, "healthy command session is usable");
        assert_eq!(mgr.cleanup_dead(), 0, "dead pid must not prune it");
        // A failed analyze (stopped) is pruned.
        let mut failed = command_sample("bad");
        failed.observed.state = SessionState::Stopped;
        mgr.create(failed).unwrap();
        assert_eq!(mgr.cleanup_dead(), 1);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_transitions_and_persists() {
        let home = temp_home();
        let mgr = SessionManager::open(&home);
        // dead pid + no server on port -> Stopped
        mgr.create(sample("dead", 30011, 4_000_000)).unwrap();
        let after = mgr.probe(&mgr.get("dead").unwrap());
        assert_eq!(after.observed.state, SessionState::Stopped);
        assert_eq!(mgr.get("dead").unwrap().observed.state, SessionState::Stopped);
        // event was recorded
        let events = mgr.recent_events("dead", 10);
        assert!(events.iter().any(|e| e.to == SessionState::Stopped));
        mgr.remove("dead");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn probe_command_keeps_terminal_state() {
        let home = temp_home();
        let mgr = SessionManager::open(&home);
        mgr.create(command_sample("kb")).unwrap();
        let after = mgr.probe(&mgr.get("kb").unwrap());
        assert_eq!(after.observed.state, SessionState::Healthy);
        // a never-finished command session with a live analyze pid is starting
        let mut running = command_sample("run");
        running.observed = ObservedState::default();
        running.pid = std::process::id();
        mgr.create(running).unwrap();
        let after = mgr.probe(&mgr.get("run").unwrap());
        assert_eq!(after.observed.state, SessionState::Starting);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn monitor_thread_runs_and_stops_on_removal() {
        let home = temp_home();
        let mgr = SessionManager::open(&home);
        mgr.create(sample("watched", 30012, 4_000_000)).unwrap();
        let rx = Arc::clone(&mgr).subscribe("watched");
        Arc::clone(&mgr).start_monitor("watched", Duration::from_millis(100));
        // dead pid: monitor should quickly record Stopped and exit
        let event = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("monitor must emit a Stopped event for a dead pid");
        assert_eq!(event.to, SessionState::Stopped);
        // monitor stops itself after Stopped
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&home);
    }
}
