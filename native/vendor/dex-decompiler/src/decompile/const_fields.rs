//! Parse `static_values` (encoded_array) and replace matching literals with static final fields.

use dex_parser::{ClassData, ClassDef, DexFile};

use super::annotations::EncodedValue;
use crate::java;

/// One static field initializer from `class_def.static_values_off`.
#[derive(Clone, Debug)]
pub struct StaticInit {
    pub field_idx: u32,
    pub name: String,
    pub java_type: String,
    pub value_java: String,
    /// Canonical literal form used for matching (e.g. `42`, `"hi"`, `true`).
    pub literal_key: String,
}

/// Parse static initializers for a class (aligned with static_fields order).
pub fn parse_static_inits(
    dex: &DexFile,
    class_def: &ClassDef,
    class_data: &ClassData,
) -> Vec<StaticInit> {
    if class_def.static_values_off == 0 {
        return vec![];
    }
    let values =
        parse_encoded_array(&*dex.data, class_def.static_values_off as usize).unwrap_or_default();
    let mut out = Vec::new();
    for (i, field) in class_data.static_fields.iter().enumerate() {
        let Some(val) = values.get(i) else { break };
        let Ok(fi) = dex.get_field_info(field.field_idx) else {
            continue;
        };
        let java_type = java::descriptor_to_java(&fi.typ);
        let Some((value_java, literal_key)) = encoded_to_java_literal(val, dex) else {
            continue;
        };
        out.push(StaticInit {
            field_idx: field.field_idx,
            name: fi.name.to_string(),
            java_type,
            value_java,
            literal_key,
        });
    }
    out
}

/// Build a map of literal string → `ClassName.field` for const replacement in method bodies.
pub fn const_field_replacements(
    class_name: &str,
    inits: &[StaticInit],
    access_flags: &[(u32, u32)], // (field_idx, flags)
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let simple = class_name.rsplit('.').next().unwrap_or(class_name);
    for init in inits {
        let is_final = access_flags
            .iter()
            .find(|(idx, _)| *idx == init.field_idx)
            .map(|(_, f)| (f & 0x10) != 0) // ACC_FINAL
            .unwrap_or(false);
        if !is_final {
            continue;
        }
        // Prefer Class.FIELD for external readability
        let repl = format!("{}.{}", simple, init.name);
        out.push((init.literal_key.clone(), repl));
    }
    out
}

/// Replace whole-token literals in Java source with static field refs (longest first).
pub fn apply_const_field_replacements(body: &str, replacements: &[(String, String)]) -> String {
    if replacements.is_empty() {
        return body.to_string();
    }
    let mut sorted: Vec<_> = replacements.iter().collect();
    sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    let mut out = body.to_string();
    for (lit, field) in sorted {
        out = replace_literal_token(&out, lit, field);
    }
    out
}

fn replace_literal_token(src: &str, lit: &str, repl: &str) -> String {
    if lit.is_empty() {
        return src.to_string();
    }
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let lit_b = lit.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + lit_b.len() <= bytes.len() && &bytes[i..i + lit_b.len()] == lit_b {
            let before_ok = i == 0 || !is_token_char(bytes[i - 1]);
            let after = i + lit_b.len();
            let after_ok = after >= bytes.len() || !is_token_char(bytes[after]);
            // Don't replace inside string literals (naive: odd number of quotes before).
            let prefix = &src[..i];
            let in_string = prefix.matches('"').count() % 2 == 1;
            if before_ok && after_ok && !in_string {
                out.push_str(repl);
                i = after;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn is_token_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b == b'.' || b == b'"' || b == b'\''
}

fn encoded_to_java_literal(val: &EncodedValue, dex: &DexFile) -> Option<(String, String)> {
    match val {
        EncodedValue::Boolean(b) => {
            let s = if *b { "true" } else { "false" };
            Some((s.into(), s.into()))
        }
        EncodedValue::Byte(v) => {
            let s = v.to_string();
            Some((s.clone(), s))
        }
        EncodedValue::Short(v) => {
            let s = v.to_string();
            Some((s.clone(), s))
        }
        EncodedValue::Char(v) => {
            let ch = char::from_u32(*v as u32).unwrap_or('?');
            let s = format!("'{}'", ch);
            Some((s.clone(), s))
        }
        EncodedValue::Int(v) => {
            let s = v.to_string();
            Some((s.clone(), s))
        }
        EncodedValue::Long(v) => {
            let s = format!("{}L", v);
            Some((s.clone(), s.trim_end_matches('L').to_string()))
        }
        EncodedValue::Float(v) => {
            let s = format!("{}f", v);
            Some((s.clone(), s))
        }
        EncodedValue::Double(v) => {
            let s = v.to_string();
            Some((s.clone(), s))
        }
        EncodedValue::String(idx) => {
            let raw = dex.get_string(*idx).ok()?;
            let escaped = escape(&raw);
            let java = format!("\"{}\"", escaped);
            Some((java.clone(), java))
        }
        EncodedValue::Null => Some(("null".into(), "null".into())),
        _ => None,
    }
}

fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

fn parse_encoded_array(data: &[u8], off: usize) -> Option<Vec<EncodedValue>> {
    let mut pos = off;
    let size = read_uleb128(data, &mut pos)? as usize;
    let mut items = Vec::with_capacity(size);
    for _ in 0..size {
        items.push(read_encoded_value(data, &mut pos)?);
    }
    Some(items)
}

fn read_uleb128(data: &[u8], pos: &mut usize) -> Option<u32> {
    let mut result: u32 = 0;
    let mut shift = 0;
    loop {
        if *pos >= data.len() {
            return None;
        }
        let b = data[*pos];
        *pos += 1;
        result |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            return None;
        }
    }
    Some(result)
}

fn read_encoded_value(data: &[u8], pos: &mut usize) -> Option<EncodedValue> {
    if *pos >= data.len() {
        return None;
    }
    let header = data[*pos];
    *pos += 1;
    let value_type = header & 0x1f;
    let value_arg = (header >> 5) as usize;
    match value_type {
        0x00 => {
            let v = data.get(*pos).copied()? as i8;
            *pos += 1;
            Some(EncodedValue::Byte(v))
        }
        0x02 => {
            let mut buf = [0u8; 2];
            for i in 0..=value_arg.min(1) {
                buf[i] = *data.get(*pos)?;
                *pos += 1;
            }
            Some(EncodedValue::Short(i16::from_le_bytes(buf)))
        }
        0x03 => {
            let mut buf = [0u8; 2];
            for i in 0..=value_arg.min(1) {
                buf[i] = *data.get(*pos)?;
                *pos += 1;
            }
            Some(EncodedValue::Char(u16::from_le_bytes(buf)))
        }
        0x04 => {
            let mut buf = [0u8; 4];
            for i in 0..=value_arg.min(3) {
                buf[i] = *data.get(*pos)?;
                *pos += 1;
            }
            Some(EncodedValue::Int(i32::from_le_bytes(buf)))
        }
        0x06 => {
            let mut buf = [0u8; 8];
            for i in 0..=value_arg.min(7) {
                buf[i] = *data.get(*pos)?;
                *pos += 1;
            }
            Some(EncodedValue::Long(i64::from_le_bytes(buf)))
        }
        0x10 => {
            let mut buf = [0u8; 4];
            for i in 0..=value_arg.min(3) {
                buf[i] = *data.get(*pos)?;
                *pos += 1;
            }
            Some(EncodedValue::Float(f32::from_bits(u32::from_le_bytes(buf))))
        }
        0x11 => {
            let mut buf = [0u8; 8];
            for i in 0..=value_arg.min(7) {
                buf[i] = *data.get(*pos)?;
                *pos += 1;
            }
            Some(EncodedValue::Double(f64::from_bits(u64::from_le_bytes(
                buf,
            ))))
        }
        0x17 => {
            let mut v: u32 = 0;
            for i in 0..=value_arg {
                let b = *data.get(*pos)? as u32;
                *pos += 1;
                v |= b << (8 * i);
            }
            Some(EncodedValue::String(v))
        }
        0x18 => {
            let mut v: u32 = 0;
            for i in 0..=value_arg {
                let b = *data.get(*pos)? as u32;
                *pos += 1;
                v |= b << (8 * i);
            }
            Some(EncodedValue::Type(v))
        }
        0x1e => Some(EncodedValue::Null),
        0x1f => Some(EncodedValue::Boolean(value_arg != 0)),
        _ => {
            *pos = (*pos).saturating_add(value_arg + 1);
            Some(EncodedValue::Other)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_literal_int() {
        let body = "        int x = 42;\n        return 42;\n";
        let out = apply_const_field_replacements(body, &[("42".into(), "Foo.ANSWER".into())]);
        assert!(out.contains("Foo.ANSWER"));
        assert!(!out.contains("= 42"));
    }
}
