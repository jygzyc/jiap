//! Load / save ProGuard / R8, Tiny, and Enigma mapping files into a [`RenameMap`].

use std::fs;
use std::path::Path;

use super::rename::RenameMap;
use crate::error::{DexDecompilerError, Result};

/// Mapping file format for load/save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MappingFormat {
    ProGuard,
    Tiny,
    Enigma,
}

/// Detect mapping format from contents (or path extension) and parse into a [`RenameMap`].
///
/// Supported:
/// - **ProGuard / R8** (`-keep` style maps: `com.Foo -> a.a:`)
/// - **Tiny** (tab-separated `c`/`f`/`m` lines, Tiny v1)
/// - **Enigma** (`CLASS` / `FIELD` / `METHOD` lines)
pub fn load_mapping_file(path: &Path) -> Result<RenameMap> {
    let text = fs::read_to_string(path).map_err(|e| {
        DexDecompilerError::Decompilation(format!("read mapping {}: {e}", path.display()))
    })?;
    load_mapping_str(&text, path.to_string_lossy().as_ref())
}

/// Parse mapping text. `hint` is a path or label used for format sniffing.
pub fn load_mapping_str(text: &str, hint: &str) -> Result<RenameMap> {
    let fmt = detect_format(text, hint);
    match fmt {
        MappingFormat::ProGuard => Ok(parse_proguard(text)),
        MappingFormat::Tiny => Ok(parse_tiny(text)),
        MappingFormat::Enigma => Ok(parse_enigma(text)),
    }
}

/// Serialize a [`RenameMap`] to mapping-file text (name-only; no descriptors / line ranges).
pub fn save_mapping_str(map: &RenameMap, format: MappingFormat) -> String {
    match format {
        MappingFormat::ProGuard => format_proguard(map),
        MappingFormat::Tiny => format_tiny(map),
        MappingFormat::Enigma => format_enigma(map),
    }
}

/// Write a [`RenameMap`] to `path` in the given format.
pub fn save_mapping_file(path: &Path, map: &RenameMap, format: MappingFormat) -> Result<()> {
    let text = save_mapping_str(map, format);
    fs::write(path, text).map_err(|e| {
        DexDecompilerError::Decompilation(format!("write mapping {}: {e}", path.display()))
    })
}

/// Guess format from a path extension / name (`*.tiny`, `*enigma*`, else ProGuard).
pub fn mapping_format_from_path(path: &Path) -> MappingFormat {
    detect_format("", path.to_string_lossy().as_ref())
}

fn detect_format(text: &str, hint: &str) -> MappingFormat {
    let lower = hint.to_ascii_lowercase();
    if lower.ends_with(".tiny") || lower.contains("tiny") {
        return MappingFormat::Tiny;
    }
    if lower.contains("enigma") {
        return MappingFormat::Enigma;
    }
    if lower.ends_with(".mapping") || lower.ends_with(".txt") {
        // still sniff
    }
    for line in text.lines().take(40) {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if t.starts_with("CLASS ") || t.starts_with("FIELD ") || t.starts_with("METHOD ") {
            return MappingFormat::Enigma;
        }
        if t.starts_with("c\t") || t.starts_with("v\t") || t.starts_with("tiny\t") {
            return MappingFormat::Tiny;
        }
        if t.contains(" -> ") {
            return MappingFormat::ProGuard;
        }
    }
    MappingFormat::ProGuard
}

/// ProGuard: `pkg.Class -> a.b:` then indented `type name -> obf` / `range:range:rettype name(args) -> obf`
fn parse_proguard(text: &str) -> RenameMap {
    let mut map = RenameMap::new();
    let mut current_orig: Option<String> = None;
    let mut current_obf: Option<String> = None;

    for line in text.lines() {
        let raw = line.trim_end();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let t = raw.trim();
        if !indented {
            // Class line: com.example.Foo -> a.b.c:
            if let Some((left, right)) = t.split_once(" -> ") {
                let orig = left.trim().to_string();
                let obf = right.trim().trim_end_matches(':').trim().to_string();
                if !orig.is_empty() && !obf.is_empty() && orig != obf {
                    map.class.insert(obf.clone(), orig.clone());
                }
                current_orig = Some(orig);
                current_obf = Some(obf);
            }
            continue;
        }
        let (Some(orig_class), Some(obf_class)) = (current_orig.as_ref(), current_obf.as_ref())
        else {
            continue;
        };
        // Member: [start:end:][type ]name[(args)] -> obfName
        if let Some((left, right)) = t.split_once(" -> ") {
            let obf_member = right.trim();
            if obf_member.is_empty() {
                continue;
            }
            let left = left.trim();
            // Strip optional line-number range `12:34:`
            let left = strip_pg_range(left);
            let orig_member = extract_pg_member_name(left);
            if orig_member.is_empty() || orig_member == obf_member {
                continue;
            }
            let is_method = left.contains('(');
            let key_obf = format!("{obf_class}#{obf_member}");
            let key_orig_side = format!("{orig_class}#{orig_member}");
            if is_method {
                // Map obfuscated Class#obfMethod -> original method name
                map.method.insert(key_obf, orig_member);
            } else {
                map.field.insert(key_obf, orig_member);
            }
            let _ = key_orig_side; // original FQN kept via class map
        }
    }
    map
}

fn strip_pg_range(s: &str) -> &str {
    // "12:34:void foo()" or "12:34:int bar"
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && bytes[i] == b':' {
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i > start && i < bytes.len() && bytes[i] == b':' {
            return s[i + 1..].trim();
        }
    }
    s
}

fn extract_pg_member_name(s: &str) -> String {
    // "void foo(int,java.lang.String)" or "int field"
    let s = s.trim();
    if let Some(paren) = s.find('(') {
        let before = s[..paren].trim();
        return before
            .split_whitespace()
            .last()
            .unwrap_or(before)
            .to_string();
    }
    s.split_whitespace().last().unwrap_or(s).to_string()
}

/// Tiny v1: `c\torig\tobfuscated`, `f\torig\tobfuscated\towner`, `m\torigDescName\tobfuscated\towner`
fn parse_tiny(text: &str) -> RenameMap {
    let mut map = RenameMap::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("tiny\t") || t.starts_with("v\t") {
            continue;
        }
        let parts: Vec<&str> = t.split('\t').collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "c" if parts.len() >= 3 => {
                let orig = parts[1].replace('/', ".");
                let obf = parts[2].replace('/', ".");
                if orig != obf {
                    map.class.insert(obf, orig);
                }
            }
            "f" if parts.len() >= 4 => {
                let orig = parts[1].to_string();
                let obf = parts[2].to_string();
                let owner_obf = parts[3].replace('/', ".");
                if orig != obf {
                    map.field.insert(format!("{owner_obf}#{obf}"), orig);
                }
            }
            "m" if parts.len() >= 4 => {
                // orig may be `name` or `name(desc)`
                let orig_raw = parts[1];
                let orig = orig_raw.split('(').next().unwrap_or(orig_raw).to_string();
                let obf = parts[2].to_string();
                let owner_obf = parts[3].replace('/', ".");
                if orig != obf {
                    map.method.insert(format!("{owner_obf}#{obf}"), orig);
                }
            }
            _ => {}
        }
    }
    map
}

/// Enigma: `CLASS a/b/c com/example/Foo` then nested FIELD/METHOD with indent.
fn parse_enigma(text: &str) -> RenameMap {
    let mut map = RenameMap::new();
    let mut current_obf: Option<String> = None;

    for line in text.lines() {
        let n_spaces = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = t.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        match parts[0] {
            "CLASS" if parts.len() >= 2 => {
                let obf = parts[1].replace('/', ".");
                let orig = if parts.len() >= 3 {
                    parts[2].replace('/', ".")
                } else {
                    obf.clone()
                };
                if orig != obf {
                    map.class.insert(obf.clone(), orig);
                }
                current_obf = Some(obf);
            }
            "FIELD" if parts.len() >= 3 && n_spaces > 0 => {
                if let Some(obf_class) = &current_obf {
                    let obf = parts[1];
                    let orig = parts[2];
                    if obf != orig {
                        map.field
                            .insert(format!("{obf_class}#{obf}"), orig.to_string());
                    }
                }
            }
            "METHOD" if parts.len() >= 3 && n_spaces > 0 => {
                if let Some(obf_class) = &current_obf {
                    let obf = parts[1];
                    let orig = parts[2];
                    if obf != orig {
                        map.method
                            .insert(format!("{obf_class}#{obf}"), orig.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    map
}

fn format_proguard(map: &RenameMap) -> String {
    let mut classes: Vec<(&String, &String)> = map.class.iter().collect();
    classes.sort_by(|a, b| a.1.cmp(b.1).then(a.0.cmp(b.0)));
    let mut out = String::from("# dex-decompiler mapping export (ProGuard/R8)\n");
    for (obf, orig) in classes {
        out.push_str(&format!("{orig} -> {obf}:\n"));
        let mut fields: Vec<_> = map
            .field
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{obf}#")))
            .collect();
        fields.sort_by(|a, b| a.1.cmp(b.1));
        for (key, orig_m) in fields {
            let obf_m = key.rsplit('#').next().unwrap_or(key);
            out.push_str(&format!("    {orig_m} -> {obf_m}\n"));
        }
        let mut methods: Vec<_> = map
            .method
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{obf}#")))
            .collect();
        methods.sort_by(|a, b| a.1.cmp(b.1));
        for (key, orig_m) in methods {
            let obf_m = key.rsplit('#').next().unwrap_or(key);
            out.push_str(&format!("    void {orig_m}() -> {obf_m}\n"));
        }
    }
    out
}

fn format_tiny(map: &RenameMap) -> String {
    let mut out = String::from("tiny\t2\t0\n");
    let mut classes: Vec<_> = map.class.iter().collect();
    classes.sort_by(|a, b| a.1.cmp(b.1));
    for (obf, orig) in &classes {
        out.push_str(&format!(
            "c\t{}\t{}\n",
            orig.replace('.', "/"),
            obf.replace('.', "/")
        ));
    }
    let mut fields: Vec<_> = map.field.iter().collect();
    fields.sort_by(|a, b| a.0.cmp(b.0));
    for (key, orig) in fields {
        if let Some((owner, obf_m)) = key.split_once('#') {
            out.push_str(&format!(
                "f\t{orig}\t{obf_m}\t{}\n",
                owner.replace('.', "/")
            ));
        }
    }
    let mut methods: Vec<_> = map.method.iter().collect();
    methods.sort_by(|a, b| a.0.cmp(b.0));
    for (key, orig) in methods {
        if let Some((owner, obf_m)) = key.split_once('#') {
            out.push_str(&format!(
                "m\t{orig}\t{obf_m}\t{}\n",
                owner.replace('.', "/")
            ));
        }
    }
    out
}

fn format_enigma(map: &RenameMap) -> String {
    let mut classes: Vec<_> = map.class.iter().collect();
    classes.sort_by(|a, b| a.1.cmp(b.1));
    let mut out = String::new();
    for (obf, orig) in classes {
        out.push_str(&format!(
            "CLASS {} {}\n",
            obf.replace('.', "/"),
            orig.replace('.', "/")
        ));
        let mut fields: Vec<_> = map
            .field
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{obf}#")))
            .collect();
        fields.sort_by(|a, b| a.1.cmp(b.1));
        for (key, orig_m) in fields {
            let obf_m = key.rsplit('#').next().unwrap_or(key);
            out.push_str(&format!("\tFIELD {obf_m} {orig_m}\n"));
        }
        let mut methods: Vec<_> = map
            .method
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{obf}#")))
            .collect();
        methods.sort_by(|a, b| a.1.cmp(b.1));
        for (key, orig_m) in methods {
            let obf_m = key.rsplit('#').next().unwrap_or(key);
            out.push_str(&format!("\tMETHOD {obf_m} {orig_m}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proguard_class_and_members() {
        let text = r#"
com.example.Hello -> a.a:
    java.lang.String name -> a
    1:3:void greet(java.lang.String) -> b
"#;
        let map = parse_proguard(text);
        assert_eq!(
            map.class.get("a.a").map(|s| s.as_str()),
            Some("com.example.Hello")
        );
        assert_eq!(map.field.get("a.a#a").map(|s| s.as_str()), Some("name"));
        assert_eq!(map.method.get("a.a#b").map(|s| s.as_str()), Some("greet"));
    }

    #[test]
    fn tiny_class() {
        let text = "tiny\t2\t0\nc\tcom/example/Hello\ta/a\nf\tname\ta\ta/a\nm\tgreet\tb\ta/a\n";
        let map = parse_tiny(text);
        assert_eq!(
            map.class.get("a.a").map(|s| s.as_str()),
            Some("com.example.Hello")
        );
        assert_eq!(map.field.get("a.a#a").map(|s| s.as_str()), Some("name"));
        assert_eq!(map.method.get("a.a#b").map(|s| s.as_str()), Some("greet"));
    }

    #[test]
    fn enigma_class() {
        let text = "CLASS a/a com/example/Hello\n\tFIELD a name\n\tMETHOD b greet\n";
        let map = parse_enigma(text);
        assert_eq!(
            map.class.get("a.a").map(|s| s.as_str()),
            Some("com.example.Hello")
        );
        assert_eq!(map.field.get("a.a#a").map(|s| s.as_str()), Some("name"));
        assert_eq!(map.method.get("a.a#b").map(|s| s.as_str()), Some("greet"));
    }

    #[test]
    fn proguard_roundtrip() {
        let text = r#"
com.example.Hello -> a.a:
    name -> a
    void greet() -> b
"#;
        let map = parse_proguard(text);
        let out = save_mapping_str(&map, MappingFormat::ProGuard);
        let again = parse_proguard(&out);
        assert_eq!(
            again.class.get("a.a").map(|s| s.as_str()),
            Some("com.example.Hello")
        );
        assert_eq!(again.field.get("a.a#a").map(|s| s.as_str()), Some("name"));
        assert_eq!(again.method.get("a.a#b").map(|s| s.as_str()), Some("greet"));
    }

    #[test]
    fn tiny_and_enigma_roundtrip() {
        let mut map = RenameMap::new();
        map.class.insert("a.a".into(), "com.example.Hello".into());
        map.field.insert("a.a#a".into(), "name".into());
        map.method.insert("a.a#b".into(), "greet".into());
        for fmt in [MappingFormat::Tiny, MappingFormat::Enigma] {
            let text = save_mapping_str(&map, fmt);
            let again = load_mapping_str(
                &text,
                match fmt {
                    MappingFormat::Tiny => "x.tiny",
                    MappingFormat::Enigma => "enigma",
                    MappingFormat::ProGuard => "x.mapping",
                },
            )
            .unwrap();
            assert_eq!(
                again.class.get("a.a").map(|s| s.as_str()),
                Some("com.example.Hello")
            );
            assert_eq!(again.field.get("a.a#a").map(|s| s.as_str()), Some("name"));
            assert_eq!(again.method.get("a.a#b").map(|s| s.as_str()), Some("greet"));
        }
    }
}
