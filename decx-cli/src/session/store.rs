//! On-disk session persistence: one JSON file per session under
//! `<home>/projects/`, plus an append-only `events.jsonl` per session.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{DecxError, DecxResult};
use crate::fsx;

use super::model::{Session, SessionEvent};

pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    pub fn new(home: &Path) -> Self {
        Self {
            dir: home.join("sessions"),
        }
    }

    pub fn project_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    pub fn events_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.events.jsonl"))
    }

    pub fn save(&self, session: &Session) -> DecxResult<()> {
        fsx::atomic_write_json(
            &self.project_path(&session.name),
            &serde_json::to_value(session)
                .map_err(|e| DecxError::internal(format!("cannot serialize session: {e}")))?,
        )
    }

    pub fn load(&self, name: &str) -> Option<Session> {
        let bytes = fs::read(self.project_path(name)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn remove(&self, name: &str) {
        let _ = fs::remove_file(self.project_path(name));
        let _ = fs::remove_file(self.events_path(name));
    }

    pub fn list(&self) -> Vec<Session> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if path.to_string_lossy().ends_with(".events.jsonl") {
                continue;
            }
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(session) = serde_json::from_slice::<Session>(&bytes) {
                    out.push(session);
                }
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Persist observed state after a probe; loads fresh, mutates, saves to
    /// avoid clobbering concurrent edits.
    pub fn update_observed(&self, name: &str, observed: &super::model::ObservedState) {
        let Some(mut session) = self.load(name) else {
            return;
        };
        session.observed = observed.clone();
        if let Err(e) = self.save(&session) {
            if std::env::var("DECX_DEBUG").ok().as_deref() == Some("1") {
                eprintln!("[DEBUG] failed to persist observed state for {name}: {e}");
            }
        }
    }

    pub fn append_event(&self, event: &SessionEvent) {
        let _ = fs::create_dir_all(&self.dir);
        let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.events_path(&event.session))
        else {
            return;
        };
        use std::io::Write;
        if let Ok(line) = serde_json::to_string(event) {
            let _ = writeln!(f, "{line}");
        }
    }

    pub fn read_events(&self, name: &str, limit: usize) -> Vec<SessionEvent> {
        let Ok(content) = fs::read_to_string(self.events_path(name)) else {
            return Vec::new();
        };
        let mut events: Vec<SessionEvent> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        if events.len() > limit {
            events.drain(..events.len() - limit);
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::model::{ObservedState, SessionState};

    fn sample(name: &str) -> Session {
        Session {
            name: name.to_string(),
            hash: "deadbeef".into(),
            file: PathBuf::from("/tmp/demo.apk"),
            engine: "jvm".into(),
            pid: 4242,
            port: 30001,
            scripts: vec![],
            log_path: None,
            origin: None,
            created_at_ms: 1_700_000_000_000,
            observed: ObservedState::default(),
        }
    }

    fn temp_home() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("decx-store-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn save_load_remove_roundtrip() {
        let home = temp_home();
        let store = SessionStore::new(&home);
        store.save(&sample("demo")).unwrap();
        let loaded = store.load("demo").unwrap();
        assert_eq!(loaded.hash, "deadbeef");
        assert_eq!(loaded.port, 30001);
        store.remove("demo");
        assert!(store.load("demo").is_none());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn list_skips_corrupt_and_event_files() {
        let home = temp_home();
        let store = SessionStore::new(&home);
        store.save(&sample("a")).unwrap();
        store.save(&sample("b")).unwrap();
        fs::write(store.project_path("corrupt"), "{not json").unwrap();
        fs::write(store.events_path("c"), "{}\n").unwrap();
        let names: Vec<String> = store.list().into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn events_append_and_bounded_read() {
        let home = temp_home();
        let store = SessionStore::new(&home);
        for i in 0..5 {
            store.append_event(&SessionEvent {
                session: "demo".into(),
                at_ms: i,
                from: SessionState::Starting,
                to: SessionState::Healthy,
                detail: None,
            });
        }
        let events = store.read_events("demo", 3);
        assert_eq!(events.len(), 3);
        assert_eq!(events.last().unwrap().at_ms, 4);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn update_observed_persists() {
        let home = temp_home();
        let store = SessionStore::new(&home);
        store.save(&sample("demo")).unwrap();
        store.update_observed(
            "demo",
            &ObservedState {
                state: SessionState::Healthy,
                checked_at_ms: 42,
                latency_ms: Some(7),
                detail: None,
                ever_healthy: true,
            },
        );
        let loaded = store.load("demo").unwrap();
        assert_eq!(loaded.observed.state, SessionState::Healthy);
        assert!(loaded.observed.ever_healthy);
        assert_eq!(loaded.pid, 4242, "other fields preserved");
        let _ = fs::remove_dir_all(&home);
    }
}
