//! Resolve Android resource ids (`0x7f…`) to `R.type.name` in decompiled Java.
//!
//! Sources:
//! - Optional external map (from `resources.arsc`)
//! - DEX `R$type` static final int fields (when present)

use std::collections::HashMap;

use dex_parser::DexFile;

use super::const_fields::{self, apply_const_field_replacements};
use crate::java;

/// Merge ARSC / caller map with constants discovered from DEX `R$*` classes.
pub fn build_resource_name_map(
    dex: &DexFile,
    external: Option<&HashMap<u32, String>>,
) -> HashMap<u32, String> {
    let mut map = collect_dex_r_constants(dex);
    if let Some(ext) = external {
        for (id, name) in ext {
            map.insert(*id, name.clone());
        }
    }
    map
}

/// Turn id → `R.type.name` into literal-token replacements for Java source.
pub fn resource_replacements(map: &HashMap<u32, String>) -> Vec<(String, String)> {
    let mut out = Vec::with_capacity(map.len().saturating_mul(4));
    for (&id, name) in map {
        if name.is_empty() {
            continue;
        }
        for lit in resource_literal_keys(id) {
            out.push((lit, name.clone()));
        }
    }
    out
}

pub fn apply_resource_replacements(body: &str, replacements: &[(String, String)]) -> String {
    apply_const_field_replacements(body, replacements)
}

fn resource_literal_keys(id: u32) -> Vec<String> {
    let signed = id as i32;
    let mut keys = vec![
        signed.to_string(),
        format!("0x{:x}", id),
        format!("0x{:X}", id),
        format!("0x{:08x}", id),
        format!("0x{:08X}", id),
    ];
    // Bytecode dumps sometimes use `#int N`
    keys.push(format!("#int {}", signed));
    keys.push(format!("#int 0x{:x}", id));
    keys
}

/// Scan DEX for `*.R$*` (and nested `R` type) static final int fields.
pub fn collect_dex_r_constants(dex: &DexFile) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    for class_result in dex.class_defs() {
        let Ok(class_def) = class_result else {
            continue;
        };
        let Ok(class_type) = dex.get_type(class_def.class_idx) else {
            continue;
        };
        let class_name = java::descriptor_to_java(&class_type);
        let Some(type_name) = r_inner_type_name(&class_name) else {
            continue;
        };
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            continue;
        };
        let inits = const_fields::parse_static_inits(dex, &class_def, &class_data);
        let access: Vec<(u32, u32)> = class_data
            .static_fields
            .iter()
            .map(|f| (f.field_idx, f.access_flags))
            .collect();
        for init in &inits {
            let is_final = access
                .iter()
                .find(|(idx, _)| *idx == init.field_idx)
                .map(|(_, f)| (f & 0x10) != 0)
                .unwrap_or(false);
            if !is_final {
                continue;
            }
            if init.java_type != "int" && init.java_type != "java.lang.Integer" {
                continue;
            }
            let Some(id) = parse_int_literal(&init.literal_key)
                .or_else(|| parse_int_literal(&init.value_java))
            else {
                continue;
            };
            // App package resources are typically 0x7f……; also accept other pkg ids.
            if (id >> 24) == 0 {
                continue;
            }
            let java_name = if type_name.is_empty() {
                format!("R.{}", init.name)
            } else {
                format!("R.{}.{}", type_name, init.name)
            };
            map.entry(id).or_insert(java_name);
        }
    }
    map
}

/// `com.foo.R$string` → `string`; `com.foo.R` → `""`.
fn r_inner_type_name(class_name: &str) -> Option<String> {
    if let Some(rest) = class_name.rsplit_once(".R$") {
        let t = rest.1;
        if t.is_empty() || t.contains('.') {
            return None;
        }
        return Some(t.to_string());
    }
    if class_name.ends_with(".R") {
        return Some(String::new());
    }
    // Obfuscated single-segment rarely matches; skip.
    None
}

fn parse_int_literal(s: &str) -> Option<u32> {
    let t = s.trim().trim_end_matches('L').trim_end_matches('l');
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16).ok();
    }
    if let Ok(v) = t.parse::<i32>() {
        return Some(v as u32);
    }
    if let Ok(v) = t.parse::<u32>() {
        return Some(v);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_type_parse() {
        assert_eq!(
            r_inner_type_name("com.example.R$string").as_deref(),
            Some("string")
        );
        assert_eq!(r_inner_type_name("com.example.R").as_deref(), Some(""));
        assert_eq!(r_inner_type_name("com.example.Main"), None);
    }

    #[test]
    fn literal_keys_include_signed_and_hex() {
        let id = 0x7f0b0001u32;
        let keys = resource_literal_keys(id);
        assert!(keys
            .iter()
            .any(|k| k == "2131427329" || k == &(id as i32).to_string()));
        assert!(keys.iter().any(|k| k.eq_ignore_ascii_case("0x7f0b0001")));
    }

    #[test]
    fn apply_replaces_resource_id() {
        let reps = resource_replacements(&HashMap::from([(
            0x7f0b0001u32,
            "R.layout.activity_main".to_string(),
        )]));
        let body = "        setContentView(2131427329);\n";
        let out = apply_resource_replacements(body, &reps);
        assert!(out.contains("R.layout.activity_main"), "got: {}", out);
    }
}
