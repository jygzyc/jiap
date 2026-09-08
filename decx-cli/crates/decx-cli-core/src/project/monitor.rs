//! Background monitors: one thread per supervised project, polling PID
//! liveness and `/health`, driving the state machine, persisting observed
//! state, and broadcasting transition events.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Weak};
use std::time::Duration;

use super::manager::ProjectManager;
use super::model::{evaluate_state, ProjectEvent, ProjectState};

/// Default monitor polling interval.
pub const DEFAULT_MONITOR_INTERVAL: Duration = Duration::from_secs(5);

/// Handle controlling one monitor thread.
pub struct MonitorHandle {
    pub stop: Arc<std::sync::atomic::AtomicBool>,
    pub thread: Option<std::thread::JoinHandle<()>>,
}

impl MonitorHandle {
    pub fn stop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Spawn a monitor thread for one project.
///
/// The closure runs until stopped or the project record disappears. Every
/// probe produces a persisted `ObservedState`; state transitions additionally
/// append an event to the store and broadcast to subscribers.
pub fn spawn_monitor(
    manager: Weak<ProjectManager>,
    project: String,
    interval: Duration,
    subscriber: Option<Sender<ProjectEvent>>,
) -> MonitorHandle {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name(format!("decx-monitor-{project}"))
        .spawn(move || {
            let mut ever_healthy = false;
            let mut current = ProjectState::Unknown;
            loop {
                if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let Some(manager) = manager.upgrade() else {
                    return; // manager dropped — CLI is exiting
                };
                let Some(project_record) = manager.store().load(&project) else {
                    return; // project removed — monitoring ends
                };

                let probe_start = std::time::Instant::now();
                let client = crate::client::DecxClient::with_options(project_record.port, 2, None);
                let health = client.health_check();
                let probe = super::model::ProbeOutcome {
                    pid_alive: crate::spawn::pid_alive(project_record.pid),
                    health,
                    latency_ms: probe_start.elapsed().as_millis() as u64,
                };
                let (state, detail, latency) =
                    evaluate_state(ever_healthy || project_record.observed.ever_healthy, &probe);
                if state == ProjectState::Healthy {
                    ever_healthy = true;
                }

                manager.store().update_observed(
                    &project,
                    &super::model::ObservedState {
                        state,
                        checked_at_ms: super::model::now_ms(),
                        latency_ms: latency,
                        detail: detail.clone(),
                        ever_healthy: ever_healthy || project_record.observed.ever_healthy,
                    },
                );

                if state != current {
                    let event = ProjectEvent {
                        project: project.clone(),
                        at_ms: super::model::now_ms(),
                        from: current,
                        to: state,
                        detail,
                    };
                    manager.store().append_event(&event);
                    manager.broadcast(&event);
                    if let Some(tx) = &subscriber {
                        let _ = tx.send(event);
                    }
                    current = state;
                }

                // A stopped project has nothing left to watch.
                if state == ProjectState::Stopped {
                    return;
                }

                // Sleep in small steps so `stop` is honored promptly.
                let mut remaining = interval;
                while remaining > Duration::ZERO {
                    if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    let step = remaining.min(Duration::from_millis(200));
                    std::thread::sleep(step);
                    remaining = remaining.saturating_sub(step);
                }
            }
        })
        .expect("failed to spawn monitor thread");
    MonitorHandle {
        stop,
        thread: Some(thread),
    }
}
