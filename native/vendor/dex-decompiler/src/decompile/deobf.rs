//! Automatic deobfuscation: rename short / invalid / non-printable identifiers.

use crate::decompile::rename::RenameMap;
use crate::java::descriptor_to_java;
use dex_parser::DexFile;

/// Options for heuristic deobfuscation (jadx-inspired).
#[derive(Clone, Debug)]
pub struct DeobfuscateOptions {
    /// Rename identifiers shorter than this (default 3, like jadx `--deobf-min`).
    pub min_len: usize,
    /// Rename identifiers longer than this (default 64).
    pub max_len: usize,
    /// Package prefixes to leave alone (e.g. `android.`, `java.`).
    pub whitelist_prefixes: Vec<String>,
    /// Fix invalid Java identifiers.
    pub rename_invalid: bool,
    /// Strip / replace non-printable characters.
    pub rename_non_printable: bool,
}

impl Default for DeobfuscateOptions {
    fn default() -> Self {
        Self {
            min_len: 3,
            max_len: 64,
            whitelist_prefixes: vec![
                "android.".into(),
                "androidx.".into(),
                "java.".into(),
                "javax.".into(),
                "kotlin.".into(),
                "kotlinx.".into(),
                "dalvik.".into(),
                "com.android.".into(),
                "com.google.android.".into(),
            ],
            rename_invalid: true,
            rename_non_printable: true,
        }
    }
}

/// Build a [`RenameMap`] for obfuscated class/method/field names across `dexes`.
pub fn build_deobf_rename_map(dexes: &[&DexFile], opts: &DeobfuscateOptions) -> RenameMap {
    let mut map = RenameMap::new();
    let mut class_n = 0u32;
    let mut method_n = 0u32;
    let mut field_n = 0u32;

    for dex in dexes {
        for class_result in dex.class_defs() {
            let Ok(class_def) = class_result else {
                continue;
            };
            let Ok(class_type) = dex.get_type(class_def.class_idx) else {
                continue;
            };
            let class_name = descriptor_to_java(&class_type);
            if is_whitelisted(&class_name, opts) {
                continue;
            }

            let simple = class_name.rsplit('.').next().unwrap_or(class_name.as_str());
            if needs_rename(simple, opts) {
                class_n += 1;
                let pkg = class_name.rsplit_once('.').map(|(p, _)| p).unwrap_or("");
                let new_simple = format!("C{class_n:04}");
                let out_class = if pkg.is_empty() {
                    new_simple
                } else {
                    format!("{pkg}.{new_simple}")
                };
                map.class.insert(class_name.clone(), out_class);
            }

            let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
                continue;
            };

            for encoded in class_data
                .direct_methods
                .iter()
                .chain(class_data.virtual_methods.iter())
            {
                let Ok(info) = dex.get_method_info(encoded.method_idx) else {
                    continue;
                };
                if info.name == "<init>" || info.name == "<clinit>" {
                    continue;
                }
                if needs_rename(&info.name, opts) {
                    method_n += 1;
                    let key = format!("{class_name}#{}", info.name);
                    map.method.insert(key, format!("m{method_n:04}"));
                }
            }

            for encoded in class_data
                .static_fields
                .iter()
                .chain(class_data.instance_fields.iter())
            {
                let Ok(info) = dex.get_field_info(encoded.field_idx) else {
                    continue;
                };
                if needs_rename(&info.name, opts) {
                    field_n += 1;
                    let key = format!("{class_name}#{}", info.name);
                    map.field.insert(key, format!("f{field_n:04}"));
                }
            }
        }
    }
    map
}

fn is_whitelisted(class_name: &str, opts: &DeobfuscateOptions) -> bool {
    opts.whitelist_prefixes
        .iter()
        .any(|p| class_name.starts_with(p.as_str()))
}

fn needs_rename(name: &str, opts: &DeobfuscateOptions) -> bool {
    if name.is_empty() {
        return true;
    }
    let len = name.chars().count();
    if len < opts.min_len || len > opts.max_len {
        return true;
    }
    if opts.rename_non_printable
        && name
            .chars()
            .any(|c| c.is_control() || !is_printable_ascii_or_ident(c))
    {
        // Allow Unicode letters in identifiers; flag control chars and spaces.
        if name.chars().any(|c| c.is_control() || c == ' ') {
            return true;
        }
    }
    if opts.rename_invalid && !is_valid_java_ident(name) {
        return true;
    }
    false
}

fn is_printable_ascii_or_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || !c.is_ascii()
}

fn is_valid_java_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_alphabetic() || first == '_' || first == '$') {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// Merge `extra` into `base` (extra wins on key conflicts).
pub fn merge_rename_maps(mut base: RenameMap, extra: RenameMap) -> RenameMap {
    base.package.extend(extra.package);
    base.class.extend(extra.class);
    base.method.extend(extra.method);
    base.field.extend(extra.field);
    for (k, v) in extra.variable {
        base.variable.entry(k).or_default().extend(v);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_need_rename() {
        let opts = DeobfuscateOptions::default();
        assert!(needs_rename("a", &opts));
        assert!(needs_rename("ab", &opts));
        assert!(!needs_rename("get", &opts));
        assert!(!needs_rename("onCreate", &opts));
    }

    #[test]
    fn invalid_ident() {
        let opts = DeobfuscateOptions::default();
        assert!(needs_rename("1abc", &opts));
        assert!(needs_rename("foo-bar", &opts));
    }
}
