//! Load DEX bytes from `.dex` files or APK/ZIP archives.

use std::io::Cursor;
use std::path::Path;

use zip::ZipArchive;

use crate::error::{DexDecompilerError, Result};
use crate::{parse_dex, DexFile};

/// True if `name` looks like a Dalvik `classes*.dex` entry.
pub fn is_classes_dex_name(name: &str) -> bool {
    let name = name.trim_start_matches('/');
    name == "classes.dex" || (name.starts_with("classes") && name.ends_with(".dex"))
}

fn dex_sort_key(name: &str) -> (u32, String) {
    let base = name.rsplit('/').next().unwrap_or(name);
    if base == "classes.dex" {
        return (1, base.to_string());
    }
    if let Some(rest) = base
        .strip_prefix("classes")
        .and_then(|s| s.strip_suffix(".dex"))
    {
        if let Ok(n) = rest.parse::<u32>() {
            return (n, base.to_string());
        }
    }
    (u32::MAX, base.to_string())
}

/// Extract `(entry_name, bytes)` for every `classes*.dex` inside an APK/ZIP.
pub fn extract_dex_entries_from_apk(apk_bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let cursor = Cursor::new(apk_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| DexDecompilerError::Parse(format!("open APK/ZIP: {e}")))?;
    let mut names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| DexDecompilerError::Parse(format!("zip entry: {e}")))?;
        let name = file.name().to_string();
        if is_classes_dex_name(&name) {
            names.push(name);
        }
    }
    names.sort_by_key(|n| dex_sort_key(n));

    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let mut file = archive
            .by_name(&name)
            .map_err(|e| DexDecompilerError::Parse(format!("zip get {name}: {e}")))?;
        let mut buf = Vec::new();
        std::io::copy(&mut file, &mut buf)
            .map_err(|e| DexDecompilerError::Parse(format!("zip read {name}: {e}")))?;
        out.push((name, buf));
    }
    if out.is_empty() {
        return Err(DexDecompilerError::Parse(
            "APK/ZIP contains no classes*.dex".into(),
        ));
    }
    Ok(out)
}

/// Extract `AndroidManifest.xml` bytes from an APK/ZIP (binary AXML or text).
pub fn extract_android_manifest_from_apk(apk_bytes: &[u8]) -> Result<Vec<u8>> {
    let cursor = Cursor::new(apk_bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| DexDecompilerError::Parse(format!("open APK/ZIP: {e}")))?;
    let mut file = archive.by_name("AndroidManifest.xml").map_err(|e| {
        DexDecompilerError::Parse(format!("AndroidManifest.xml not found in APK: {e}"))
    })?;
    let mut buf = Vec::new();
    std::io::copy(&mut file, &mut buf)
        .map_err(|e| DexDecompilerError::Parse(format!("read AndroidManifest.xml: {e}")))?;
    Ok(buf)
}

/// True if bytes look like UTF-8/ASCII text XML (decoded manifest), not binary AXML.
pub fn looks_like_text_xml(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(256)];
    if sample.starts_with(&[0x03, 0x00, 0x08, 0x00]) {
        return false;
    }
    let Ok(s) = std::str::from_utf8(sample) else {
        return false;
    };
    let t = s.trim_start();
    t.starts_with("<?xml") || t.starts_with("<manifest") || t.starts_with("<")
}

/// Load one path as DEX or APK (auto-detect by magic / extension).
pub fn load_dexes_from_path(path: &Path) -> Result<Vec<DexFile>> {
    let bytes = std::fs::read(path)
        .map_err(|e| DexDecompilerError::Parse(format!("read {}: {e}", path.display())))?;
    load_dexes_from_bytes(&bytes, path)
}

/// Parse raw bytes as a single DEX, or as an APK/ZIP containing DEX files.
pub fn load_dexes_from_bytes(bytes: &[u8], hint_path: &Path) -> Result<Vec<DexFile>> {
    if looks_like_dex(bytes) {
        return Ok(vec![parse_dex(bytes)?]);
    }
    if looks_like_zip(bytes)
        || hint_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                matches!(
                    e.to_ascii_lowercase().as_str(),
                    "apk" | "zip" | "jar" | "aar"
                )
            })
    {
        let entries = extract_dex_entries_from_apk(bytes)?;
        let mut dexes = Vec::with_capacity(entries.len());
        for (name, data) in entries {
            let dex = parse_dex(&data)
                .map_err(|e| DexDecompilerError::Parse(format!("parse DEX entry {name}: {e}")))?;
            dexes.push(dex);
        }
        return Ok(dexes);
    }
    // Last resort: try DEX parse for better error message.
    Ok(vec![parse_dex(bytes)?])
}

/// Load and concatenate DEX files from one or more paths (DEX and/or APK).
pub fn load_dexes_from_paths(paths: &[impl AsRef<Path>]) -> Result<Vec<DexFile>> {
    let mut all = Vec::new();
    for p in paths {
        all.extend(load_dexes_from_path(p.as_ref())?);
    }
    if all.is_empty() {
        return Err(DexDecompilerError::Parse("no DEX inputs".into()));
    }
    Ok(all)
}

fn looks_like_dex(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[0..4] == b"dex\n"
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && (bytes[0..2] == [0x50, 0x4b]) // PK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_dex_name_filter() {
        assert!(is_classes_dex_name("classes.dex"));
        assert!(is_classes_dex_name("classes2.dex"));
        assert!(is_classes_dex_name("classes15.dex"));
        assert!(!is_classes_dex_name("AndroidManifest.xml"));
        assert!(!is_classes_dex_name("lib/arm64/libfoo.so"));
    }

    #[test]
    fn text_xml_detection() {
        assert!(looks_like_text_xml(b"<?xml version=\"1.0\"?><manifest/>"));
        assert!(looks_like_text_xml(
            b"<manifest package=\"a.b\"></manifest>"
        ));
        assert!(!looks_like_text_xml(&[0x03, 0x00, 0x08, 0x00, 0x00, 0x00]));
    }

    #[test]
    fn sort_key_orders_multidex() {
        let mut names: Vec<String> = vec![
            "classes3.dex".into(),
            "classes.dex".into(),
            "classes2.dex".into(),
        ];
        names.sort_by_key(|n| dex_sort_key(n));
        assert_eq!(
            names,
            vec![
                "classes.dex".to_string(),
                "classes2.dex".to_string(),
                "classes3.dex".to_string()
            ]
        );
    }
}
