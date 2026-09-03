//! Local session store: `$DECX_NATIVE_HOME/sessions.json` (default `~/.decx-native`).
//! Mirrors decx-cli's session records: name, port, pid, target path, file hash.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Session {
    pub name: String,
    pub port: u16,
    pub pid: u32,
    pub target: String,
    pub file_hash: u64,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct SessionStore {
    #[serde(rename = "sessions", default)]
    pub sessions: Vec<Session>,
}

pub fn home() -> PathBuf {
    if let Ok(h) = std::env::var("DECX_NATIVE_HOME") {
        if !h.is_empty() {
            return PathBuf::from(h);
        }
    }
    let user_home = std::env::var("USER_HOME")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&user_home).join(".decx-native")
}

impl SessionStore {
    pub fn path() -> PathBuf {
        home().join("sessions.json")
    }

    pub fn load() -> Self {
        match std::fs::read_to_string(Self::path()) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => SessionStore::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).unwrap_or_default())
    }

    pub fn by_name(&self, name: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.name == name)
    }

    pub fn by_port(&self, port: u16) -> Option<&Session> {
        self.sessions.iter().find(|s| s.port == port)
    }
}

pub fn file_hash(path: &Path) -> std::io::Result<u64> {
    let bytes = std::fs::read(path)?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(hasher.finish())
}

pub fn now_string() -> String {
    // std-only timestamp (secs since epoch); avoids a chrono dependency
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

/// Find the sibling server binary (same directory as the CLI executable).
pub fn server_exe() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let dir = exe.parent().unwrap_or(Path::new("."));
    let name = if cfg!(windows) { "decx-native-server.exe" } else { "decx-native-server" };
    dir.join(name)
}

/// Kill a process tree, platform-appropriate. Returns true when the pid is gone.
pub fn kill_pid(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    // verify death by probing a couple of times
    for _ in 0..20 {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    !pid_alive(pid)
}

pub fn pid_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}")])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
}

/// Pick a free TCP port in 30000..40000.
pub fn pick_free_port() -> u16 {
    for _ in 0..200 {
        let port = 30000 + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u16)
            .unwrap_or(0) % 10000);
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    30000
}
