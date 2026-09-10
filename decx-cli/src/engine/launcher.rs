//! Engine-layer launch/health runtime: primitives for talking to a DECX
//! server once the session layer has spawned it. Session records, reuse
//! decisions, and supervision live in the session layer (`crate::session`).

use std::path::Path;
use std::time::{Duration, Instant};

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

/// Server reachability probe used by `session check`.
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
