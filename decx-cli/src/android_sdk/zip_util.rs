//! Cross-platform zip/jar helpers via external tools.
//!
//! Port of `decx-cli/src/android/zip-utils.ts`: on Windows there is no
//! `zip`/`unzip`, but Windows 10+ ships bsdtar (libarchive) at
//! `C:\Windows\System32\tar.exe` which reads and writes zip archives; other
//! platforms use the Info-ZIP binaries.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{DecxError, DecxResult};

const WINDOWS_BSD_TAR: &str = r"C:\Windows\System32\tar.exe";

enum ZipTool {
    Bsdtar(String),
    InfoZip,
}

fn is_windows() -> bool {
    cfg!(windows)
}

fn resolve_zip_tool() -> ZipTool {
    if is_windows() && Path::new(WINDOWS_BSD_TAR).exists() {
        ZipTool::Bsdtar(WINDOWS_BSD_TAR.to_string())
    } else {
        ZipTool::InfoZip
    }
}

fn run(tool: &mut Command, label: &str) -> DecxResult<std::process::Output> {
    tool.output()
        .map_err(|e| DecxError::file(format!("{label}: {e}"), None))
        .and_then(|out| {
            if out.status.success() {
                Ok(out)
            } else {
                let msg = String::from_utf8_lossy(&out.stderr);
                let msg = if msg.trim().is_empty() {
                    String::from_utf8_lossy(&out.stdout).trim().to_string()
                } else {
                    msg.trim().to_string()
                };
                Err(DecxError::file(
                    if msg.is_empty() { format!("{label} failed") } else { msg },
                    None,
                ))
            }
        })
}

/// List entry names in a zip/jar archive.
pub fn list_zip_entries(archive: &Path) -> DecxResult<Vec<String>> {
    let out = match resolve_zip_tool() {
        ZipTool::Bsdtar(bin) => run(Command::new(bin).arg("-tf").arg(archive), &format!("failed to list '{}'", archive.display()))?,
        ZipTool::InfoZip => run(Command::new("unzip").arg("-Z1").arg(archive), &format!("failed to list '{}'", archive.display()))?,
    };
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// Extract a single zip/jar entry to a file. Streams to a file descriptor so
/// large entries never pass through memory.
pub fn extract_zip_entry(archive: &Path, entry: &str, target: &Path) -> DecxResult<()> {
    let mut file = std::fs::File::create(target)
        .map_err(|e| DecxError::file(format!("failed to create '{}': {e}", target.display()), Some(target.display().to_string())))?;
    let mut cmd = match resolve_zip_tool() {
        ZipTool::Bsdtar(bin) => {
            let mut c = Command::new(bin);
            c.arg("-xOf").arg(archive).arg(entry);
            c
        }
        ZipTool::InfoZip => {
            let mut c = Command::new("unzip");
            c.arg("-p").arg(archive).arg(entry);
            c
        }
    };
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd
        .spawn()
        .map_err(|e| DecxError::file(format!("failed to read '{entry}': {e}"), None))?;
    let output = child
        .wait_with_output()
        .map_err(|e| DecxError::file(format!("failed to read '{entry}': {e}"), None))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(target);
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(DecxError::file(
            if msg.is_empty() { format!("failed to extract '{entry}' from {}", archive.display()) } else { msg },
            None,
        ));
    }
    file.write_all(&output.stdout)
        .map_err(|e| DecxError::file(format!("failed to write '{}': {e}", target.display()), None))?;
    Ok(())
}

/// Create a zip/jar archive from `entries` resolved relative to `cwd`.
pub fn create_zip_archive(archive: &Path, entries: &[String], cwd: &Path) -> DecxResult<()> {
    let label = format!("failed to create '{}'", archive.display());
    match resolve_zip_tool() {
        ZipTool::Bsdtar(bin) => {
            let mut c = Command::new(&bin);
            c.arg("--format=zip").arg("-cf").arg(archive).args(entries).current_dir(cwd);
            run(&mut c, &label)?;
        }
        ZipTool::InfoZip => {
            let mut c = Command::new("zip");
            c.arg("-q").arg("-r").arg(archive).args(entries).current_dir(cwd);
            run(&mut c, &label)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn zip_roundtrip_via_external_tool() {
        let dir = std::env::temp_dir().join(format!("decx-zip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/classes.dex"), b"DEX-CONTENT").unwrap();

        let archive = dir.join("out.jar");
        create_zip_archive(&archive, &["sub".to_string()], &dir).unwrap();
        assert!(archive.exists());

        let entries = list_zip_entries(&archive).unwrap();
        assert!(entries.iter().any(|e| e.replace('\\', "/").ends_with("classes.dex")));

        let target = dir.join("extracted.dex");
        let entry = entries
            .iter()
            .find(|e| e.replace('\\', "/").ends_with("classes.dex"))
            .unwrap()
            .clone();
        extract_zip_entry(&archive, &entry, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"DEX-CONTENT");
        let _ = fs::remove_dir_all(&dir);
    }
}
