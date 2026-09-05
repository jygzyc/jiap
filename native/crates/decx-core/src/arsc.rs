//! resources.arsc parser — the subset DECX needs:
//! - global string pool
//! - packages with their type/key string pools
//! - `<string>` resources (default config preferred) for `get_strings`
//!
//! Mirrors what jadx's `ResourcesService`/`IResTable` expose to the Kotlin layer.
//! Type/key string pools are discovered positionally (first two string-pool
//! chunks inside a package) because some builders — notably aapt2 — write
//! non-standard `typeStrings`/`keyStrings` header fields.

use serde_json::Value;

use crate::axml::StringPool;

const RES_TABLE_TYPE: u16 = 0x0002;
const RES_STRING_POOL_TYPE: u16 = 0x0001;
const RES_TABLE_PACKAGE_TYPE: u16 = 0x0200;
const RES_TABLE_TYPE_TYPE: u16 = 0x0201;
const RES_TABLE_TYPE_SPEC_TYPE: u16 = 0x0202;

#[derive(Debug)]
pub struct ArscError(pub String);

impl std::fmt::Display for ArscError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "arsc: {}", self.0)
    }
}

type Result<T> = std::result::Result<T, ArscError>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(ArscError(msg.into()))
}

fn u16_at(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn u32_at(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

#[derive(Debug, Clone)]
pub struct StringResource {
    /// Resource key name (e.g. `app_name`).
    pub name: String,
    /// Resolved value.
    pub value: String,
}

#[derive(Debug, Default)]
pub struct ArscTable {
    pub global_pool: StringPool,
    pub packages: Vec<ArscPackage>,
}

#[derive(Debug, Default)]
pub struct ArscPackage {
    pub name: String,
    pub strings: Vec<StringResource>,
}

struct RawTypeEntry {
    key_idx: u32,
    data_type: u8,
    data: u32,
    config_is_default: bool,
    type_name: String,
}

/// Parse a resources.arsc buffer.
pub fn parse(buf: &[u8]) -> Result<ArscTable> {
    if buf.len() < 12 {
        return err("arsc too small");
    }
    let table_type = u16_at(buf, 0);
    if table_type != RES_TABLE_TYPE {
        return err(format!("unexpected header type 0x{table_type:04x}"));
    }
    // RES_TABLE_HEADER: u16 type, u16 headerSize, u32 size, u32 packageCount.
    let header_size = u16_at(buf, 2) as usize;
    let package_count = u32_at(buf, 8) as usize;

    let mut table = ArscTable::default();
    let mut off = header_size;
    let mut seen_packages = 0usize;

    while off + 8 <= buf.len() && seen_packages < package_count {
        let ctype = u16_at(buf, off);
        let csize = u32_at(buf, off + 4) as usize;
        #[cfg(test)]
        eprintln!("arsc walk: off={off} type=0x{ctype:04x} size={csize}");
        if ctype == RES_STRING_POOL_TYPE && table.global_pool.strings.is_empty() {
            table.global_pool = StringPool::parse(buf, off).map_err(|e| ArscError(e.to_string()))?;
        } else if ctype == RES_TABLE_PACKAGE_TYPE {
            if let Some(pkg) = parse_package(buf, off, &table.global_pool) {
                table.packages.push(pkg);
            }
            seen_packages += 1;
        }
        if csize == 0 {
            break;
        }
        off += csize;
    }
    Ok(table)
}

fn pool_get(pool: &StringPool, idx: u32) -> String {
    pool.get(idx as usize).unwrap_or("").to_string()
}

fn parse_package(buf: &[u8], off: usize, global: &StringPool) -> Option<ArscPackage> {
    if off + 288 > buf.len() {
        return None;
    }
    let pkg_header_size = u16_at(buf, off + 2) as usize;
    let pkg_size = u32_at(buf, off + 4) as usize; // chunk header: type@0, headerSize@2, size@4
    let name: String = {
        let mut units = Vec::with_capacity(128);
        for i in 0..128 {
            let u = u16_at(buf, off + 12 + i * 2);
            if u == 0 {
                break;
            }
            units.push(u);
        }
        String::from_utf16_lossy(&units)
    };

    let mut pkg = ArscPackage { name, strings: Vec::new() };

    // Walk package sub-chunks starting right after the package header.
    // String pools are discovered positionally: #0 = type names, #1 = key names.
    let mut entries: Vec<RawTypeEntry> = Vec::new();
    let mut type_pool = StringPool::default();
    let mut key_pool = StringPool::default();
    let mut pools_seen = 0usize;
    let mut cur = off + pkg_header_size.max(288);
    let end = off + pkg_size;
    while cur + 8 <= end.min(buf.len()) {
        let ctype = u16_at(buf, cur);
        let csize = u32_at(buf, cur + 4) as usize;
        match ctype {
            RES_STRING_POOL_TYPE => {
                match pools_seen {
                    0 => {
                        type_pool = StringPool::parse(buf, cur).unwrap_or_default();
                        pools_seen = 1;
                    }
                    1 => {
                        key_pool = StringPool::parse(buf, cur).unwrap_or_default();
                        pools_seen = 2;
                    }
                    _ => {}
                }
            }
            RES_TABLE_TYPE_TYPE => {
                if cur + 24 > buf.len() {
                    break;
                }
                let type_id = buf[cur + 8];
                let entry_count = u32_at(buf, cur + 12) as usize;
                let entries_start = u32_at(buf, cur + 16) as usize;
                let config_size = u32_at(buf, cur + 20) as usize;
                let type_name = pool_get(&type_pool, (type_id as u32).saturating_sub(1));
                // Default config: all-zero bytes (only the mandatory size field set).
                let config_off = cur + 20;
                let config_is_default = {
                    let cend = (config_off + config_size as usize).min(buf.len());
                    let blob = &buf[(config_off + 4).min(cend)..cend];
                    blob.iter().all(|&b| b == 0)
                };
                let offsets_base = cur + 20 + config_size as usize;
                for i in 0..entry_count {
                    let opos = offsets_base + i * 4;
                    if opos + 4 > buf.len() {
                        break;
                    }
                    let eoff = u32_at(buf, opos);
                    if eoff == 0xFFFF_FFFF {
                        continue;
                    }
                    let ebase = cur + entries_start + eoff as usize;
                    if ebase + 16 > buf.len() {
                        continue;
                    }
                    let _entry_size = u16_at(buf, ebase);
                    let flags = u16_at(buf, ebase + 2);
                    let key_idx = u32_at(buf, ebase + 4);
                    if flags & 0x0001 != 0 {
                        continue; // complex (array/map) entry — not a plain string
                    }
                    // Res_value starts at ebase+8: u16 size, u8 res0, u8 dataType, u32 data
                    let data_type = buf[ebase + 11];
                    let data = u32_at(buf, ebase + 12);
                    entries.push(RawTypeEntry {
                        key_idx,
                        data_type,
                        data,
                        config_is_default,
                        type_name: type_name.clone(),
                    });
                }
            }
            RES_TABLE_TYPE_SPEC_TYPE => {}
            _ => {}
        }
        if csize == 0 {
            break;
        }
        cur += csize;
    }

    // Collect `<string>` resources, preferring default configs.
    #[cfg(test)]
    eprintln!(
        "pkg '{}': pools_seen={} type_names={:?} entries={} string_entries={}",
        pkg.name,
        pools_seen,
        type_pool.strings.iter().take(25).cloned().collect::<Vec<_>>(),
        entries.len(),
        entries.iter().filter(|e| e.type_name == "string").count()
    );
    for e in &entries {
        if e.type_name != "string" || e.data_type != 0x03 {
            continue;
        }
        let value = pool_get(global, e.data);
        if value.is_empty() {
            continue;
        }
        let name = pool_get(&key_pool, e.key_idx);
        if name.is_empty() {
            continue;
        }
        if let Some(existing) = pkg.strings.iter_mut().find(|s| s.name == name) {
            if e.config_is_default {
                existing.value = value;
            }
            continue;
        }
        pkg.strings.push(StringResource { name, value });
    }
    Some(pkg)
}

/// Render package strings as a `res/values/strings.xml` document (the form
/// jadx's resource bridge exposes).
pub fn strings_to_xml(pkg: &ArscPackage) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n");
    let mut sorted = pkg.strings.clone();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for s in &sorted {
        let esc = s
            .value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        out.push_str(&format!("    <string name=\"{}\">{}</string>\n", s.name, esc));
    }
    out.push_str("</resources>\n");
    out
}

/// Marker keeping serde_json referenced (packages may expose JSON meta later).
#[allow(dead_code)]
fn _json_marker(_v: &Value) {}
