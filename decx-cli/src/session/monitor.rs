//! Background monitors: one thread per supervised session, driving the
//! engine-kind-aware probe on a fixed interval. State transitions are
//! persisted and delivered to subscribers by the session manager's single
//! event path (`set_observed` → `record_event`), so monitors never
//! double-report.

use std::sync::{Arc, Weak};
use std::time::Duration;

use super::manager::SessionManager;
use super::model::SessionState;

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

/// Spawn a monitor thread for one session. Runs until stopped, the record
/// disappears, or the session reaches a terminal state.
pub fn spawn_monitor(manager: Weak<SessionManager>, session_name: String, interval: Duration) -> MonitorHandle {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name(format!("decx-monitor-{session_name}"))
        .spawn(move || {
            loop {
                if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                let Some(manager) = manager.upgrade() else {
                    return; // manager dropped — CLI is exiting
                };
                let Some(record) = manager.store().load(&session_name) else {
                    return; // session removed — monitoring ends
                };
                let updated = manager.probe(&record);
                if updated.observed.state == SessionState::Stopped {
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
