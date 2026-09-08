//! [`ProjectManager`] — the independent project manager.
//!
//! Owns every analysis project record, supervises background server processes,
//! and exposes monitoring: deep health refreshes, per-project monitor threads,
//! a bounded in-memory event ring (mirrored to disk), and event subscriptions
//! for `project watch`.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use crate::client::DecxClient;
use crate::error::DecxResult;
use crate::spawn;

use super::model::{evaluate_state, ObservedState, ProbeOutcome, Project, ProjectEvent, ProjectState};
use super::monitor::{spawn_monitor, MonitorHandle};
use super::store::ProjectStore;

const EVENT_RING_CAPACITY: usize = 200;
/// Sessions older than this are pruned even if their PID looks alive.
const SESSION_MAX_AGE_MS: u64 = 30 * 24 * 60 * 60 * 1000;

struct Inner {
    monitors: HashMap<String, MonitorHandle>,
    events: HashMap<String, VecDeque<ProjectEvent>>,
    subscribers: Vec<(String, Sender<ProjectEvent>)>,
}

pub struct ProjectManager {
    home: PathBuf,
    store: ProjectStore,
    inner: Mutex<Inner>,
}

impl ProjectManager {
    /// Open (or lazily create) the project store under `home`.
    pub fn open(home: &Path) -> Arc<ProjectManager> {
        Arc::new(Self {
            home: home.to_path_buf(),
            store: ProjectStore::new(home),
            inner: Mutex::new(Inner {
                monitors: HashMap::new(),
                events: HashMap::new(),
                subscribers: Vec::new(),
            }),
        })
    }

    pub fn store(&self) -> &ProjectStore {
        &self.store
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    // ── Record management ───────────────────────────────────────────────────

    pub fn create(&self, project: Project) -> DecxResult<()> {
        self.store.save(&project)
    }

    pub fn get(&self, name: &str) -> Option<Project> {
        self.store.load(name)
    }

    /// Remove a record and stop any in-process monitor watching it.
    pub fn remove(&self, name: &str) {
        self.stop_monitor(name);
        self.store.remove(name);
        self.inner.lock().unwrap().events.remove(name);
    }

    pub fn list(&self) -> Vec<Project> {
        self.store.list()
    }

    /// Projects whose PID is still live (fast check, no HTTP probing).
    pub fn list_alive(&self) -> Vec<Project> {
        self.list()
            .into_iter()
            .filter(|p| spawn::pid_alive(p.pid))
            .collect()
    }

    /// Auto-select the single alive project, if exactly one exists.
    pub fn auto_select(&self) -> Option<Project> {
        let alive = self.list_alive();
        if alive.len() == 1 {
            Some(alive.into_iter().next().unwrap())
        } else {
            None
        }
    }

    /// Drop records whose process is gone (or that expired). Returns how many
    /// were removed.
    pub fn cleanup_dead(&self) -> usize {
        let now = crate::fsx::now_ms();
        let mut removed = 0;
        for project in self.list() {
            let expired = now.saturating_sub(project.created_at_ms) > SESSION_MAX_AGE_MS;
            if expired || !spawn::pid_alive(project.pid) {
                self.remove(&project.name);
                removed += 1;
            }
        }
        removed
    }

    // ── Probing / monitoring ────────────────────────────────────────────────

    /// One live probe of a project: PID liveness + `/health`, state machine,
    /// persist observed state, record transitions.
    pub fn probe(&self, project: &Project) -> Project {
        let started = std::time::Instant::now();
        let client = DecxClient::with_options(project.port, 2, None);
        let health = client.health_check();
        let probe = ProbeOutcome {
            pid_alive: spawn::pid_alive(project.pid),
            health,
            latency_ms: started.elapsed().as_millis() as u64,
        };
        let (state, detail, latency) =
            evaluate_state(project.observed.ever_healthy, &probe);
        let ever_healthy = project.observed.ever_healthy || state == ProjectState::Healthy;
        let observed = ObservedState {
            state,
            checked_at_ms: crate::fsx::now_ms(),
            latency_ms: latency,
            detail: detail.clone(),
            ever_healthy,
        };
        self.store.update_observed(&project.name, &observed);
        if let Some(mut updated) = self.store.load(&project.name) {
            if updated.observed.state != project.observed.state {
                self.record_event(
                    &project.name,
                    project.observed.state,
                    updated.observed.state,
                    updated.observed.detail.clone(),
                );
            }
            updated.observed = observed;
            updated
        } else {
            let mut clone = project.clone();
            clone.observed = observed;
            clone
        }
    }

    /// Deep-refresh a named project (returns None when unknown).
    pub fn refresh(&self, name: &str) -> Option<Project> {
        self.get(name).map(|p| self.probe(&p))
    }

    /// Deep-refresh every project (used by `list --probe` / `status`).
    pub fn refresh_all(&self) {
        let projects = self.list();
        for p in projects {
            self.probe(&p);
        }
    }

    fn record_event(&self, project: &str, from: ProjectState, to: ProjectState, detail: Option<String>) {
        let event = ProjectEvent {
            project: project.to_string(),
            at_ms: crate::fsx::now_ms(),
            from,
            to,
            detail,
        };
        self.store.append_event(&event);
        let mut inner = self.inner.lock().unwrap();
        let ring = inner.events.entry(project.to_string()).or_default();
        if ring.len() == EVENT_RING_CAPACITY {
            ring.pop_front();
        }
        ring.push_back(event.clone());
        inner.subscribers.retain(|(name, tx)| {
            (name.as_str() == project || name.as_str() == "*") && tx.send(event.clone()).is_ok()
        });
    }

    /// Broadcast an externally produced event (used by monitors).
    pub(crate) fn broadcast(&self, event: &ProjectEvent) {
        let mut inner = self.inner.lock().unwrap();
        let ring = inner
            .events
            .entry(event.project.clone())
            .or_default();
        if ring.len() == EVENT_RING_CAPACITY {
            ring.pop_front();
        }
        ring.push_back(event.clone());
        inner.subscribers.retain(|(name, tx)| {
            (name.as_str() == event.project.as_str() || name.as_str() == "*") && tx.send(event.clone()).is_ok()
        });
    }

    /// Recent events for a project: in-memory ring first, then the on-disk log.
    pub fn recent_events(&self, project: &str, limit: usize) -> Vec<ProjectEvent> {
        let mut events = self.store.read_events(project, EVENT_RING_CAPACITY);
        if events.is_empty() {
            return events;
        }
        if events.len() > limit {
            events.drain(..events.len() - limit);
        }
        events
    }

    /// Start a background monitor thread for one project (idempotent: an
    /// existing monitor for the same project is stopped first).
    pub fn start_monitor(self: &Arc<Self>, project: &str, interval: Duration) {
        self.stop_monitor(project);
        let weak: Weak<ProjectManager> = Arc::downgrade(self);
        let handle = spawn_monitor(weak, project.to_string(), interval, None);
        self.inner.lock().unwrap().monitors.insert(project.to_string(), handle);
    }

    pub fn stop_monitor(&self, project: &str) {
        if let Some(mut handle) = self.inner.lock().unwrap().monitors.remove(project) {
            handle.stop();
        }
    }

    /// Subscribe to state-transition events for one project (or `"*) for all.
    pub fn subscribe(self: &Arc<Self>, project: &str) -> Receiver<ProjectEvent> {
        let (tx, rx) = channel();
        self.inner
            .lock()
            .unwrap()
            .subscribers
            .push((project.to_string(), tx));
        rx
    }

    /// Live monitor state for diagnostics (`project list` footer).
    pub fn monitored_projects(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        let mut names: Vec<String> = inner.monitors.keys().cloned().collect();
        names.sort();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::model::ObservedState;

    fn temp_home() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("decx-mgr-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample(name: &str, port: u16, pid: u32) -> Project {
        Project {
            name: name.to_string(),
            hash: format!("hash-{name}"),
            file: PathBuf::from("/tmp/demo.apk"),
            engine: "jvm".into(),
            pid,
            port,
            scripts: vec![],
            log_path: None,
            created_at_ms: crate::fsx::now_ms(),
            observed: ObservedState::default(),
        }
    }

    #[test]
    fn create_list_remove() {
        let home = temp_home();
        let mgr = ProjectManager::open(&home);
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
    fn probe_transitions_and_persists() {
        let home = temp_home();
        let mgr = ProjectManager::open(&home);
        // dead pid + no server on port -> Stopped
        mgr.create(sample("dead", 30011, 4_000_000)).unwrap();
        let after = mgr.probe(&mgr.get("dead").unwrap());
        assert_eq!(after.observed.state, ProjectState::Stopped);
        assert_eq!(mgr.get("dead").unwrap().observed.state, ProjectState::Stopped);
        // event was recorded
        let events = mgr.recent_events("dead", 10);
        assert!(events.iter().any(|e| e.to == ProjectState::Stopped));
        mgr.remove("dead");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn monitor_thread_runs_and_stops_on_removal() {
        let home = temp_home();
        let mgr = ProjectManager::open(&home);
        mgr.create(sample("watched", 30012, 4_000_000)).unwrap();
        let rx = Arc::clone(&mgr).subscribe("watched");
        Arc::clone(&mgr).start_monitor("watched", Duration::from_millis(100));
        // dead pid: monitor should quickly record Stopped and exit
        let event = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("monitor must emit a Stopped event for a dead pid");
        assert_eq!(event.to, ProjectState::Stopped);
        // monitor stops itself after Stopped
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&home);
    }
}
