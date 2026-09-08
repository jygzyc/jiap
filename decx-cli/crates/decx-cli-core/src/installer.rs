//! decx-server.jar discovery, version probing, and installation from GitHub
//! releases.
//!
//! Release discovery avoids the rate-limited GitHub REST API: the latest
//! stable version comes from the npm registry (the npm package is published
//! from the same tag as the server jar) and prereleases from the GitHub
//! releases atom feed. Jar assets use deterministic `/releases/download` URLs.

use std::io::Read;
use std::path::Path;

use crate::error::{DecxError, DecxResult};

const NPM_LATEST_URL: &str = "https://registry.npmjs.org/@jygzyc/decx-cli/latest";
const GITHUB_REPO: &str = "jygzyc/decx";
const RELEASES_ATOM_URL: &str = "https://github.com/jygzyc/decx/releases.atom";

/// Compare two semver strings ("2.2.1" vs "2.3.0"). Returns Ordering-like i32.
pub fn compare_semver(a: &str, b: &str) -> i32 {
    let parse = |s: &str| -> Vec<u64> {
        s.split('-')
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (pa, pb) = (parse(a), parse(b));
    for i in 0..3 {
        let va = pa.get(i).copied().unwrap_or(0);
        let vb = pb.get(i).copied().unwrap_or(0);
        if va != vb {
            return if va > vb { 1 } else { -1 };
        }
    }
    0
}

pub fn normalize_version(tag: &str) -> String {
    tag.strip_prefix('v').unwrap_or(tag).to_string()
}

fn asset_url(version: &str) -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/download/v{version}/decx-server-{version}.jar")
}

/// Fetch the latest version string: npm registry for stable releases, the
/// GitHub atom feed for prereleases.
pub fn fetch_latest_version(prerelease: bool) -> DecxResult<String> {
    if prerelease {
        let text = crate::net::http_get_text(RELEASES_ATOM_URL, "application/atom+xml")?;
        let is_prerelease_tag = |t: &str| -> bool {
            let v = t.strip_prefix('v').unwrap_or(t);
            v.split('-').count() > 1 && v.split('.').count() >= 3 && v.chars().next().is_some_and(|c| c.is_ascii_digit())
        };
        let mut latest: Option<String> = None;
        for title in extract_tag_titles(&text) {
            if is_prerelease_tag(&title) {
                let version = normalize_version(&title);
                if latest.as_ref().is_none_or(|cur| compare_semver(&version, cur) > 0) {
                    latest = Some(version);
                }
            }
        }
        return latest.ok_or_else(|| DecxError::connection("No prerelease found".to_string()));
    }

    let body = crate::net::http_get_json(NPM_LATEST_URL)?;
    body.get("version")
        .and_then(serde_json::Value::as_str)
        .map(|v| v.to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| DecxError::connection("npm registry returned no version".to_string()))
}

fn extract_tag_titles(atom: &str) -> Vec<String> {
    let mut titles = Vec::new();
    let mut rest = atom;
    while let Some(start) = rest.find("<title>") {
        let after = &rest[start + 7..];
        if let Some(end) = after.find("</title>") {
            titles.push(after[..end].trim().to_string());
            rest = &after[end + 8..];
        } else {
            break;
        }
    }
    titles
}

/// Read `version=<x.y.z>` from the `version.properties` entry inside a jar by
/// parsing the zip central directory directly (stored or deflated entries).
pub fn read_jar_version_property(jar: &Path) -> Option<String> {
    let buf = std::fs::read(jar).ok()?;
    let eocd = find_eocd(&buf)?;
    let entry_count = u16::from_le_bytes([buf[eocd + 10], buf[eocd + 11]]) as usize;
    let mut p = u32::from_le_bytes([buf[eocd + 16], buf[eocd + 17], buf[eocd + 18], buf[eocd + 19]]) as usize;
    for _ in 0..entry_count {
        if buf.get(p..p + 4)? != [0x50, 0x4b, 0x01, 0x02] {
            return None; // central dir signature
        }
        let method = u16::from_le_bytes([buf[p + 10], buf[p + 11]]);
        let compressed_size = u32::from_le_bytes([buf[p + 20], buf[p + 21], buf[p + 22], buf[p + 23]]) as usize;
        let name_len = u16::from_le_bytes([buf[p + 28], buf[p + 29]]) as usize;
        let extra_len = u16::from_le_bytes([buf[p + 30], buf[p + 31]]) as usize;
        let comment_len = u16::from_le_bytes([buf[p + 32], buf[p + 33]]) as usize;
        let local_offset = u32::from_le_bytes([buf[p + 42], buf[p + 43], buf[p + 44], buf[p + 45]]) as usize;
        let name = String::from_utf8_lossy(&buf[p + 46..p + 46 + name_len]).to_string();
        if name == "version.properties" {
            let l_name_len = u16::from_le_bytes([buf[local_offset + 26], buf[local_offset + 27]]) as usize;
            let l_extra_len = u16::from_le_bytes([buf[local_offset + 28], buf[local_offset + 29]]) as usize;
            let data_start = local_offset + 30 + l_name_len + l_extra_len;
            let data = buf.get(data_start..data_start + compressed_size)?;
            let content = match method {
                8 => inflate_raw(data)?,
                _ => String::from_utf8_lossy(data).to_string(),
            };
            for line in content.lines() {
                if let Some(version) = line.strip_prefix("version=") {
                    let version = version.trim();
                    if !version.is_empty() {
                        return Some(version.to_string());
                    }
                }
            }
            return None;
        }
        p += 46 + name_len + extra_len + comment_len;
    }
    None
}

fn find_eocd(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .rposition(|w| w == [0x50, 0x4b, 0x05, 0x06])
}

fn inflate_raw(data: &[u8]) -> Option<String> {
    let mut decoder = flate2::read::DeflateDecoder::new(data);
    let mut out = String::new();
    decoder.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Install (or skip-if-current) decx-server.jar. `home` is DECX_HOME.
pub fn install_decx_server(home: &Path, prerelease: bool, current_version: Option<&str>) -> DecxResult<serde_json::Value> {
    eprintln!("  Fetching latest {} info...", if prerelease { "prerelease" } else { "release" });
    let version = fetch_latest_version(prerelease)?;
    let install_dir = home.join("bin");
    let install_path = install_dir.join("decx-server.jar");

    // Prefer the version embedded in the installed jar (ground truth) over the
    // config record, which may be stale after a manual jar replacement.
    let jar_version = install_path.exists().then(|| read_jar_version_property(&install_path)).flatten();
    let effective = jar_version.as_deref().or(current_version);
    if effective == Some(version.as_str()) && install_path.exists() {
        return Ok(serde_json::json!({
            "ok": true,
            "message": format!("decx-server is already up to date (v{version})"),
            "version": version,
            "path": install_path.display().to_string(),
        }));
    }

    std::fs::create_dir_all(&install_dir)
        .map_err(|e| DecxError::internal(format!("cannot create {}: {e}", install_dir.display())))?;
    let url = asset_url(&version);
    eprintln!("  Downloading {url} ...");
    let tmp_path = install_path.with_extension("jar.tmp");
    let downloaded = crate::net::download_to_file(&url, &tmp_path)?;
    if downloaded == 0 {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(DecxError::connection(
            "Download failed: empty response body (the release may still be publishing; retry shortly)".to_string(),
        ));
    }
    replace_installed_jar(&tmp_path, &install_path)?;
    Ok(serde_json::json!({
        "ok": true,
        "message": format!("Installed decx-server v{version} to {}", install_path.display()),
        "version": version,
        "path": install_path.display().to_string(),
    }))
}

fn replace_installed_jar(tmp: &Path, install: &Path) -> DecxResult<()> {
    let backup = install.with_extension("jar.bak");
    let had_existing = install.exists();
    if had_existing {
        let _ = std::fs::rename(install, &backup);
    }
    if let Err(e) = std::fs::rename(tmp, install) {
        // Best-effort rollback before surfacing the failure.
        if had_existing && !install.exists() {
            let _ = std::fs::rename(&backup, install);
        }
        let busy = e.kind() == std::io::ErrorKind::PermissionDenied;
        return Err(DecxError::process(if busy {
            format!(
                "Cannot replace {}: the file may be in use by a running session. \
                 Close sessions with 'decx project close --all' and retry.",
                install.display()
            )
        } else {
            format!("Failed to save downloaded file: {e}")
        }));
    }
    if had_existing {
        let _ = std::fs::remove_file(&backup);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compare() {
        assert_eq!(compare_semver("2.2.1", "2.3.0"), -1);
        assert_eq!(compare_semver("2.3.0", "2.3.0"), 0);
        assert_eq!(compare_semver("3.0.0", "2.9.9"), 1);
        assert_eq!(compare_semver("2.2", "2.2.1"), -1);
    }

    #[test]
    fn version_normalization() {
        assert_eq!(normalize_version("v4.2.0"), "4.2.0");
        assert_eq!(normalize_version("4.2.0"), "4.2.0");
    }

    #[test]
    fn atom_titles_extraction() {
        let atom = r#"<feed><entry><title>v4.2.0</title></entry><entry><title>v4.3.0-rc.1</title></entry></feed>"#;
        assert_eq!(extract_tag_titles(atom), vec!["v4.2.0".to_string(), "v4.3.0-rc.1".to_string()]);
    }

    #[test]
    fn jar_version_reader_on_real_zip() {
        // Build a minimal stored-entry zip containing version.properties.
        let content = b"version=4.2.1\n";
        let mut zip: Vec<u8> = Vec::new();
        let name = b"version.properties";
        let crc: u32 = 0; // readers here don't verify crc
        let local_header_offset: u32 = 0;
        // local file header
        zip.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        zip.extend_from_slice(&[10, 0, 0, 0]); // version, flags
        zip.extend_from_slice(&[0, 0]); // method: stored
        zip.extend_from_slice(&[0, 0, 0, 0]); // time/date
        zip.extend_from_slice(&crc.to_le_bytes());
        zip.extend_from_slice(&(content.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(content.len() as u32).to_le_bytes());
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&[0, 0]); // extra len
        zip.extend_from_slice(name);
        zip.extend_from_slice(content);
        let central_offset = zip.len() as u32;
        // central directory header (fixed layout, method=0 stored)
        zip.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]); // sig @0
        zip.extend_from_slice(&[10, 0]); // version made by @4
        zip.extend_from_slice(&[10, 0]); // version needed @6
        zip.extend_from_slice(&[0, 0]); // flags @8
        zip.extend_from_slice(&[0, 0]); // method @10 (stored)
        zip.extend_from_slice(&[0, 0]); // mod time @12
        zip.extend_from_slice(&[0, 0]); // mod date @14
        zip.extend_from_slice(&crc.to_le_bytes()); // crc @16
        zip.extend_from_slice(&(content.len() as u32).to_le_bytes()); // csize @20
        zip.extend_from_slice(&(content.len() as u32).to_le_bytes()); // usize @24
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes()); // name len @28
        zip.extend_from_slice(&[0, 0]); // extra len @30
        zip.extend_from_slice(&[0, 0]); // comment len @32
        zip.extend_from_slice(&[0, 0]); // disk @34
        zip.extend_from_slice(&[0, 0]); // internal attrs @36
        zip.extend_from_slice(&[0, 0, 0, 0]); // external attrs @38
        zip.extend_from_slice(&local_header_offset.to_le_bytes()); // offset @42
        zip.extend_from_slice(name);
        let central_size = (zip.len() - central_offset as usize) as u32;
        // EOCD
        zip.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        zip.extend_from_slice(&[0, 0, 0, 0]);
        zip.extend_from_slice(&1u16.to_le_bytes());
        zip.extend_from_slice(&1u16.to_le_bytes());
        zip.extend_from_slice(&central_size.to_le_bytes());
        zip.extend_from_slice(&central_offset.to_le_bytes());
        zip.extend_from_slice(&[0, 0]);

        let tmp = std::env::temp_dir().join(format!("decx-jar-{}.jar", std::process::id()));
        std::fs::write(&tmp, &zip).unwrap();
        assert_eq!(read_jar_version_property(&tmp).as_deref(), Some("4.2.1"));
        let _ = std::fs::remove_file(&tmp);
    }
}
