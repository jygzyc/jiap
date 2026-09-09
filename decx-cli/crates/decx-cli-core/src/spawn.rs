//! Background process primitives: detached spawning, PID liveness, and
//! verified process-tree kills.
//!
//! Spawned DECX servers must outlive the CLI process: on Unix the child gets
//! its own process group (`process_group(0)`), on Windows it is created with
//! `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`. Kills always verify death by
//! polling, never by trusting the killer's exit code.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::error::{DecxError, DecxResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillResult {
    Killed,
    AlreadyDead,
    Failed,
}

/// Spawn `cmd` detached with stdout/stderr appended to `log_path`.
/// Returns the child PID; the child is intentionally not waited on.
pub fn spawn_detached(cmd: &mut Command, log_path: &Path) -> DecxResult<u32> {
    use std::process::Stdio;

    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DecxError::process(format!("cannot create log dir {}: {e}", parent.display())))?;
    }
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| DecxError::process(format!("cannot open log {}: {e}", log_path.display())))?;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(log_file.try_clone().map_err(|e| DecxError::process(e.to_string()))?))
        .stderr(Stdio::from(log_file));
    detach(cmd);

    let child = cmd
        .spawn()
        .map_err(|e| DecxError::process(format!("failed to spawn {}: {e}", cmd.get_program().to_string_lossy())))?;
    Ok(child.id())
}

#[cfg(unix)]
fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    // New process group + session: the server survives CLI exit and can be
    // killed as a whole group via kill(-pid).
    cmd.process_group(0);
}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

/// Whether `pid` refers to a live process. Guards against PID reuse only
/// weakly (the health endpoint is the authoritative liveness signal); this
/// mirrors the TypeScript check.
pub fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // signal 0 = existence probe; EPERM means it exists but is not ours.
        let r = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if r == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(windows)]
    {
        unsafe {
            let handle = win32::OpenProcess(win32::PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            win32::CloseHandle(handle);
            true
        }
    }
}

/// Minimal kernel32 bindings. Declared inline (rather than pulling in
/// `windows-sys`) because GNU-toolchain environments need dlltool to build
/// import libraries, which is not always installed.
#[cfg(windows)]
mod win32 {
    pub type HANDLE = *mut core::ffi::c_void;
    pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    #[repr(C)]
    pub struct MemoryStatusEx {
        pub dw_length: u32,
        pub dw_memory_load: u32,
        pub ull_total_phys: u64,
        pub ull_avail_phys: u64,
        pub ull_total_page_file: u64,
        pub ull_avail_page_file: u64,
        pub ull_total_virtual: u64,
        pub ull_avail_virtual: u64,
        pub ull_avail_ext_virtual: u64,
    }

    extern "system" {
        pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> HANDLE;
        pub fn CloseHandle(handle: HANDLE) -> i32;
        pub fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }
}

fn wait_for_death(pid: u32, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !pid_alive(pid)
}

/// Kill an entire process tree (server + its JVM children), verifying death at
/// every step. Only `Killed`/`AlreadyDead` may lead to dropping a project
/// record — `Failed` means the tree is still running.
pub fn kill_tree(pid: u32) -> KillResult {
    if !pid_alive(pid) {
        return KillResult::AlreadyDead;
    }
    #[cfg(unix)]
    {
        let neg = -(pid as libc::pid_t);
        unsafe {
            libc::kill(neg, libc::SIGTERM);
        }
        if wait_for_death(pid, Duration::from_secs(2)) {
            return KillResult::Killed;
        }
        unsafe {
            libc::kill(neg, libc::SIGKILL);
        }
        if wait_for_death(pid, Duration::from_secs(2)) {
            return KillResult::Killed;
        }
        // last resort: the process may not be a group leader
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
        if wait_for_death(pid, Duration::from_secs(1)) {
            return KillResult::Killed;
        }
        KillResult::Failed
    }
    #[cfg(windows)]
    {
        let try_taskkill = |force: bool| {
            let mut cmd = Command::new("taskkill");
            cmd.arg("/T");
            if force {
                cmd.arg("/F");
            }
            cmd.arg("/PID").arg(pid.to_string());
            cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok()
        };
        // Graceful first (often a no-op for windowless JVMs), then verified
        // force kill — taskkill's exit code alone is not proof the tree died.
        try_taskkill(false);
        if wait_for_death(pid, Duration::from_secs(2)) {
            return KillResult::Killed;
        }
        try_taskkill(true);
        if wait_for_death(pid, Duration::from_secs(2)) {
            return KillResult::Killed;
        }
        KillResult::Failed
    }
}

/// Spawn a child (not detached) whose stdout/stderr append to `log_path`.
/// For one-shot analyze jobs whose exit code decides the project state; the
/// caller keeps the `Child` to reap the exit code.
pub fn spawn_logged_child(cmd: &mut Command, log_path: &Path) -> DecxResult<std::process::Child> {
    use std::process::Stdio;

    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DecxError::process(format!("cannot create log dir {}: {e}", parent.display())))?;
    }
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| DecxError::process(format!("cannot open log {}: {e}", log_path.display())))?;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(log_file.try_clone().map_err(|e| DecxError::process(e.to_string()))?))
        .stderr(Stdio::from(log_file));
    cmd.spawn()
        .map_err(|e| DecxError::process(format!("failed to spawn {}: {e}", cmd.get_program().to_string_lossy())))
}

/// Run a command to completion under a hard timeout. stdout/stderr are
/// drained concurrently so a full pipe cannot deadlock the child (the
/// wait-then-read ordering deadlocks on outputs larger than the OS pipe
/// buffer).
pub fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> DecxResult<std::process::Output> {
    use std::io::Read;
    use std::process::Stdio;

    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| DecxError::process(format!("failed to spawn {}: {e}", cmd.get_program().to_string_lossy())))?;
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = out_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = err_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DecxError::timeout(format!(
                    "command '{}' timed out after {}ms",
                    cmd.get_program().to_string_lossy(),
                    timeout.as_millis()
                )));
            }
            Err(e) => return Err(DecxError::process(format!("failed to wait for child: {e}"))),
        }
    };
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    Ok(std::process::Output { status, stdout, stderr })
}

/// Total physical memory in bytes (used to size the JVM heap).
pub fn total_memory_bytes() -> Option<u64> {
    #[cfg(windows)]
    {
        unsafe {
            let mut status: win32::MemoryStatusEx = std::mem::zeroed();
            status.dw_length = std::mem::size_of::<win32::MemoryStatusEx>() as u32;
            if win32::GlobalMemoryStatusEx(&mut status) != 0 {
                return Some(status.ull_total_phys);
            }
            None
        }
    }
    #[cfg(all(unix, target_os = "linux"))]
    {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kb: u64 = rest.trim().trim_end_matches(" kB").trim().parse().ok()?;
                return Some(kb * 1024);
            }
        }
        None
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let out = Command::new("sysctl").arg("-n").arg("hw.memsize").output().ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        s.trim().parse().ok()
    }
}

/// Default JVM heap: 2/3 of machine memory in GiB, rounded down (mirrors the
/// TypeScript CLI's `-Xmx` sizing).
pub fn default_java_heap() -> String {
    let gib = total_memory_bytes()
        .map(|bytes| bytes / (1024 * 1024 * 1024) * 2 / 3)
        .unwrap_or(4);
    format!("{gib}g")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_liveness_self_and_bogus() {
        assert!(pid_alive(std::process::id()));
        // PID 4_000_000 is above typical pid_max limits on both platforms
        assert!(!pid_alive(4_000_000));
    }

    #[test]
    fn heap_sizing_is_sane() {
        let heap = default_java_heap();
        assert!(heap.ends_with('g'));
        assert!(heap.trim_end_matches('g').parse::<u64>().is_ok());
    }

    #[test]
    fn kill_reports_already_dead() {
        assert_eq!(kill_tree(4_000_000), KillResult::AlreadyDead);
    }
}
