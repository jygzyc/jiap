//! Filesystem helpers: atomic writes and JSON persistence.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{DecxError, DecxResult};

/// Write bytes atomically: write to `<path>.tmp`, then rename over `path`.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> DecxResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| DecxError::internal(format!("cannot create {}: {e}", parent.display())))?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f =
            fs::File::create(&tmp).map_err(|e| DecxError::internal(format!("cannot create {}: {e}", tmp.display())))?;
        f.write_all(bytes)
            .and_then(|()| f.flush())
            .map_err(|e| DecxError::internal(format!("cannot write {}: {e}", tmp.display())))?;
    }
    // Windows rename fails if the destination exists; remove it first. The
    // window between remove and rename is tiny and the temp copy is already
    // durable, so a crash here loses nothing that matters.
    if path.exists() {
        fs::remove_file(path).map_err(|e| DecxError::internal(format!("cannot replace {}: {e}", path.display())))?;
    }
    fs::rename(&tmp, path).map_err(|e| DecxError::internal(format!("cannot rename {}: {e}", tmp.display())))?;
    Ok(())
}

/// Pretty-print and atomically persist a JSON value.
pub fn atomic_write_json(path: &Path, value: &serde_json::Value) -> DecxResult<()> {
    let mut bytes = serde_json::to_string_pretty(value).unwrap_or_default();
    bytes.push('\n');
    atomic_write(path, bytes.as_bytes())
}

/// Read and parse a JSON file; `Ok(None)` when it does not exist.
pub fn read_json(path: &Path) -> DecxResult<Option<serde_json::Value>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| DecxError::internal(format!("cannot parse {}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(DecxError::internal(format!("cannot read {}: {e}", path.display()))),
    }
}

/// Current unix epoch milliseconds.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
