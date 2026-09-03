//! Parse DEX annotations (annotation_set_item, annotation_item) for class/method/field.
//! class_def.annotations_off points to annotations_directory_item; we read class_annotations_off.
//! Also extracts `dalvik.annotation.Signature` string values for generics.

use dex_parser::ClassDef;

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

fn read_u32(data: &[u8], pos: usize) -> Option<u32> {
    if pos + 4 > data.len() {
        return None;
    }
    Some(u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?))
}

/// One annotation element value (subset needed for Signature / InnerClass).
#[derive(Clone, Debug)]
pub enum EncodedValue {
    Byte(i8),
    Short(i16),
    Char(u16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(u32),
    Type(u32),
    Array(Vec<EncodedValue>),
    Annotation,
    Null,
    Boolean(bool),
    Other,
}

/// Parsed annotation: type_idx + named elements.
#[derive(Clone, Debug)]
pub struct ParsedAnnotation {
    pub type_idx: u32,
    pub elements: Vec<(String, EncodedValue)>,
}

/// Parse class_def.annotations_off: points to annotations_directory_item.
/// Return list of type_idx for the class's direct annotations (for @Override etc.).
pub fn class_annotation_type_ids(data: &[u8], class_def: &ClassDef) -> Option<Vec<u32>> {
    class_annotations(data, class_def, &|_| None)
        .map(|anns| anns.into_iter().map(|a| a.type_idx).collect())
}

/// Full class annotations with element values.
/// `get_string` resolves string_id → text (for element names and Signature pieces).
pub fn class_annotations(
    data: &[u8],
    class_def: &ClassDef,
    get_string: &dyn Fn(u32) -> Option<String>,
) -> Option<Vec<ParsedAnnotation>> {
    if class_def.annotations_off == 0 {
        return Some(vec![]);
    }
    let off = class_def.annotations_off as usize;
    if off + 4 > data.len() {
        return None;
    }
    let class_annotations_off = read_u32(data, off)?;
    if class_annotations_off == 0 {
        return Some(vec![]);
    }
    annotation_set(data, class_annotations_off as usize, get_string)
}

/// Look up a field's `dalvik.annotation.Signature` string (if annotated).
pub fn field_generic_signature(
    data: &[u8],
    class_def: &ClassDef,
    field_idx: u32,
    get_string: &dyn Fn(u32) -> Option<String>,
    get_type: &dyn Fn(u32) -> Option<String>,
) -> Option<String> {
    let set_off = field_annotation_set_off(data, class_def, field_idx)?;
    let anns = annotation_set(data, set_off, get_string)?;
    signature_from_annotations(&anns, get_string, get_type)
}

/// Look up a method's `dalvik.annotation.Signature` string (if annotated).
pub fn method_generic_signature(
    data: &[u8],
    class_def: &ClassDef,
    method_idx: u32,
    get_string: &dyn Fn(u32) -> Option<String>,
    get_type: &dyn Fn(u32) -> Option<String>,
) -> Option<String> {
    let set_off = method_annotation_set_off(data, class_def, method_idx)?;
    let anns = annotation_set(data, set_off, get_string)?;
    signature_from_annotations(&anns, get_string, get_type)
}

fn annotations_directory_sizes(data: &[u8], class_def: &ClassDef) -> Option<(usize, usize, usize)> {
    if class_def.annotations_off == 0 {
        return None;
    }
    let off = class_def.annotations_off as usize;
    if off + 16 > data.len() {
        return None;
    }
    let fields_size = read_u32(data, off + 4)? as usize;
    let methods_size = read_u32(data, off + 8)? as usize;
    let params_size = read_u32(data, off + 12)? as usize;
    Some((fields_size, methods_size, params_size))
}

fn field_annotation_set_off(data: &[u8], class_def: &ClassDef, field_idx: u32) -> Option<usize> {
    let (fields_size, _, _) = annotations_directory_sizes(data, class_def)?;
    let off = class_def.annotations_off as usize;
    let mut pos = off + 16;
    for _ in 0..fields_size {
        if pos + 8 > data.len() {
            return None;
        }
        let fidx = read_u32(data, pos)?;
        let ann_off = read_u32(data, pos + 4)?;
        pos += 8;
        if fidx == field_idx {
            return if ann_off == 0 {
                None
            } else {
                Some(ann_off as usize)
            };
        }
    }
    None
}

fn method_annotation_set_off(data: &[u8], class_def: &ClassDef, method_idx: u32) -> Option<usize> {
    let (fields_size, methods_size, _) = annotations_directory_sizes(data, class_def)?;
    let off = class_def.annotations_off as usize;
    let mut pos = off + 16 + fields_size * 8;
    for _ in 0..methods_size {
        if pos + 8 > data.len() {
            return None;
        }
        let midx = read_u32(data, pos)?;
        let ann_off = read_u32(data, pos + 4)?;
        pos += 8;
        if midx == method_idx {
            return if ann_off == 0 {
                None
            } else {
                Some(ann_off as usize)
            };
        }
    }
    None
}

fn signature_from_annotations(
    anns: &[ParsedAnnotation],
    get_string: &dyn Fn(u32) -> Option<String>,
    get_type: &dyn Fn(u32) -> Option<String>,
) -> Option<String> {
    for ann in anns {
        let ty = get_type(ann.type_idx)?;
        let java = crate::java::descriptor_to_java(&ty);
        if java != "dalvik.annotation.Signature" {
            continue;
        }
        for (name, val) in &ann.elements {
            if name != "value" {
                continue;
            }
            if let EncodedValue::Array(items) = val {
                let mut s = String::new();
                for it in items {
                    if let EncodedValue::String(idx) = it {
                        if let Some(piece) = get_string(*idx) {
                            s.push_str(&piece);
                        }
                    }
                }
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

fn annotation_set(
    data: &[u8],
    set_off: usize,
    get_string: &dyn Fn(u32) -> Option<String>,
) -> Option<Vec<ParsedAnnotation>> {
    if set_off + 4 > data.len() {
        return None;
    }
    let size = read_u32(data, set_off)? as usize;
    let mut out = Vec::with_capacity(size);
    for i in 0..size {
        let entry_off = set_off + 4 + i * 4;
        if entry_off + 4 > data.len() {
            break;
        }
        let annotation_off = read_u32(data, entry_off)? as usize;
        if let Some(ann) = parse_annotation_item(data, annotation_off, get_string) {
            out.push(ann);
        }
    }
    Some(out)
}

fn parse_annotation_item(
    data: &[u8],
    annotation_off: usize,
    get_string: &dyn Fn(u32) -> Option<String>,
) -> Option<ParsedAnnotation> {
    if annotation_off >= data.len() {
        return None;
    }
    let mut pos = annotation_off;
    pos += 1; // visibility
    let type_idx = read_uleb128(data, &mut pos)?;
    let size = read_uleb128(data, &mut pos)? as usize;
    let mut elements = Vec::with_capacity(size);
    for _ in 0..size {
        let name_idx = read_uleb128(data, &mut pos)?;
        let name = get_string(name_idx).unwrap_or_default();
        let value = read_encoded_value(data, &mut pos)?;
        elements.push((name, value));
    }
    Some(ParsedAnnotation { type_idx, elements })
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
            // byte
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
            // string (uleb128 index, size = value_arg+1 bytes as unsigned)
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
        0x1c => {
            // array
            let size = read_uleb128(data, pos)? as usize;
            let mut items = Vec::with_capacity(size);
            for _ in 0..size {
                items.push(read_encoded_value(data, pos)?);
            }
            Some(EncodedValue::Array(items))
        }
        0x1d => {
            // annotation — skip nested
            let _type_idx = read_uleb128(data, pos)?;
            let size = read_uleb128(data, pos)? as usize;
            for _ in 0..size {
                let _ = read_uleb128(data, pos)?;
                let _ = read_encoded_value(data, pos)?;
            }
            Some(EncodedValue::Annotation)
        }
        0x1e => Some(EncodedValue::Null),
        0x1f => Some(EncodedValue::Boolean(value_arg != 0)),
        _ => {
            // Skip unknown payload bytes
            *pos = (*pos).saturating_add(value_arg + 1);
            Some(EncodedValue::Other)
        }
    }
}

/// Extract the Java generic signature string from `dalvik.annotation.Signature` if present.
///
/// Signature value is an array of strings that concatenate to the JVMS signature.
pub fn class_generic_signature(
    data: &[u8],
    class_def: &ClassDef,
    get_string: &dyn Fn(u32) -> Option<String>,
    get_type: &dyn Fn(u32) -> Option<String>,
) -> Option<String> {
    let anns = class_annotations(data, class_def, get_string)?;
    signature_from_annotations(&anns, get_string, get_type)
}

/// Format a class annotation as Java source (`@Name` or `@Name(a=1, b="x")`).
pub fn format_annotation_java(
    ann: &ParsedAnnotation,
    get_string: &dyn Fn(u32) -> Option<String>,
    get_type: &dyn Fn(u32) -> Option<String>,
) -> Option<String> {
    let ty = get_type(ann.type_idx)?;
    let java = crate::java::descriptor_to_java(&ty);
    if java == "dalvik.annotation.Signature"
        || java == "dalvik.annotation.InnerClass"
        || java == "dalvik.annotation.EnclosingMethod"
        || java == "dalvik.annotation.MemberClasses"
        || super::kotlin::is_kotlin_noise_annotation(&java)
    {
        return None;
    }
    let short = java.rsplit('.').next().unwrap_or(&java);
    if ann.elements.is_empty() {
        return Some(format!("@{}", short));
    }
    let mut parts = Vec::new();
    for (name, val) in &ann.elements {
        parts.push(format!(
            "{}={}",
            name,
            format_encoded_value_java(val, get_string, get_type)
        ));
    }
    if parts.len() == 1 && ann.elements[0].0 == "value" {
        let v = format_encoded_value_java(&ann.elements[0].1, get_string, get_type);
        return Some(format!("@{}({})", short, v));
    }
    Some(format!("@{}({})", short, parts.join(", ")))
}

fn format_encoded_value_java(
    val: &EncodedValue,
    get_string: &dyn Fn(u32) -> Option<String>,
    get_type: &dyn Fn(u32) -> Option<String>,
) -> String {
    match val {
        EncodedValue::Byte(v) => v.to_string(),
        EncodedValue::Short(v) => v.to_string(),
        EncodedValue::Char(v) => format!("'{}'", escape_ann_char(*v)),
        EncodedValue::Int(v) => v.to_string(),
        EncodedValue::Long(v) => format!("{}L", v),
        EncodedValue::Float(v) => format!("{}f", v),
        EncodedValue::Double(v) => v.to_string(),
        EncodedValue::String(idx) => {
            let s = get_string(*idx).unwrap_or_default();
            format!("\"{}\"", escape_ann_str(&s))
        }
        EncodedValue::Type(idx) => {
            let t = get_type(*idx).unwrap_or_else(|| "?".into());
            format!("{}.class", crate::java::descriptor_to_java(&t))
        }
        EncodedValue::Boolean(b) => if *b { "true" } else { "false" }.into(),
        EncodedValue::Null => "null".into(),
        EncodedValue::Array(items) => {
            let inner: Vec<String> = items
                .iter()
                .map(|i| format_encoded_value_java(i, get_string, get_type))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        EncodedValue::Annotation | EncodedValue::Other => "/* … */".into(),
    }
}

fn escape_ann_str(s: &str) -> String {
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

fn escape_ann_char(c: u16) -> String {
    match char::from_u32(c as u32) {
        Some('\\') => "\\\\".into(),
        Some('\'') => "\\'".into(),
        Some('\n') => "\\n".into(),
        Some(ch) if ch.is_control() => format!("\\u{:04x}", c),
        Some(ch) => ch.to_string(),
        None => format!("\\u{:04x}", c),
    }
}

/// Parsed class Signature: type params (with bounds), superclass, interfaces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassJavaSignature {
    /// e.g. `<T extends Foo>`
    pub type_params: Option<String>,
    pub superclass: Option<String>,
    pub interfaces: Vec<String>,
}

/// Convert a JVMS Signature's leading formal type parameters to Java (`<T>`, `<T extends Foo>`).
/// Works for both class and method signatures (stops at the closing `>`).
pub fn signature_to_java_generics(sig: &str) -> Option<String> {
    let sig = sig.trim();
    let rest = sig.strip_prefix('<')?;
    let end = find_matching_gt(rest)?;
    Some(format_formal_type_params(&rest[..end]))
}

/// Full parse of a class Signature: `<T:LBound;>LSuper;LIface;…`
pub fn parse_class_signature(sig: &str) -> Option<ClassJavaSignature> {
    let sig = sig.trim();
    if sig.is_empty() {
        return None;
    }
    let mut out = ClassJavaSignature::default();
    let mut s = sig;
    if let Some(tp) = signature_to_java_generics(sig) {
        out.type_params = Some(tp);
    }
    if let Some(rest) = s.strip_prefix('<') {
        if let Some(end) = find_matching_gt(rest) {
            s = &rest[end + 1..];
        }
    }
    // Superclass then interfaces.
    let mut i = 0;
    let mut first = true;
    while i < s.len() {
        let (ty, n) = parse_type_sig(&s[i..])?;
        if first {
            out.superclass = Some(ty);
            first = false;
        } else {
            out.interfaces.push(ty);
        }
        i += n;
    }
    if out.type_params.is_none() && out.superclass.is_none() && out.interfaces.is_empty() {
        return None;
    }
    Some(out)
}

fn format_formal_type_params(params: &str) -> String {
    let mut parts = Vec::new();
    let mut i = 0;
    let bytes = params.as_bytes();
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && bytes[i] != b':' {
            i += 1;
        }
        if start >= i {
            break;
        }
        let name = &params[start..i];
        let mut bounds = Vec::new();
        while i < bytes.len() && bytes[i] == b':' {
            i += 1;
            if i >= bytes.len() {
                break;
            }
            if bytes[i] == b':' {
                // empty class bound, next is interface
                continue;
            }
            if let Some((ty, n)) = parse_type_sig(&params[i..]) {
                // Skip trivial Object bound for readability.
                if ty != "java.lang.Object" && ty != "Object" {
                    bounds.push(ty);
                }
                i += n;
            } else {
                break;
            }
        }
        if bounds.is_empty() {
            parts.push(name.to_string());
        } else {
            parts.push(format!("{} extends {}", name, bounds.join(" & ")));
        }
    }
    format!("<{}>", parts.join(", "))
}

/// Convert a JVMS type signature (field / return) to a Java type with generics.
///
/// Example: `Ljava/util/List<Ljava/lang/String;>;` → `java.util.List<java.lang.String>`
pub fn signature_type_to_java(sig: &str) -> Option<String> {
    let sig = sig.trim();
    if sig.is_empty() {
        return None;
    }
    // Skip leading formal type params if this is a method/class signature.
    let mut s = sig;
    if let Some(rest) = s.strip_prefix('<') {
        if let Some(end) = find_matching_gt(rest) {
            s = &rest[end + 1..];
        }
    }
    parse_type_sig(s).map(|(ty, _)| ty)
}

/// Parsed method Signature: optional type params, parameter types, return type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodJavaSignature {
    pub type_params: Option<String>,
    pub params: Vec<String>,
    pub return_type: String,
}

/// Convert a JVMS method Signature to Java types with generics.
///
/// Example: `<T:Ljava/lang/Object;>(Ljava/util/List<TT;>;)TT;` →
/// type_params=`<T>`, params=`[List<T>]`, return=`T`
pub fn signature_method_to_java(sig: &str) -> Option<MethodJavaSignature> {
    let sig = sig.trim();
    if sig.is_empty() {
        return None;
    }
    let type_params = signature_to_java_generics(sig);
    let mut s = sig;
    if let Some(rest) = s.strip_prefix('<') {
        if let Some(end) = find_matching_gt(rest) {
            s = &rest[end + 1..];
        }
    }
    let s = s.strip_prefix('(')?;
    let mut params = Vec::new();
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() && bytes[i] != b')' {
        let (ty, n) = parse_type_sig(&s[i..])?;
        params.push(ty);
        i += n;
    }
    if i >= bytes.len() || bytes[i] != b')' {
        return None;
    }
    i += 1;
    let (return_type, _) = parse_type_sig(&s[i..])?;
    Some(MethodJavaSignature {
        type_params,
        params,
        return_type,
    })
}

fn parse_type_sig(s: &str) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    match bytes[0] {
        b'B' => Some(("byte".into(), 1)),
        b'C' => Some(("char".into(), 1)),
        b'D' => Some(("double".into(), 1)),
        b'F' => Some(("float".into(), 1)),
        b'I' => Some(("int".into(), 1)),
        b'J' => Some(("long".into(), 1)),
        b'S' => Some(("short".into(), 1)),
        b'Z' => Some(("boolean".into(), 1)),
        b'V' => Some(("void".into(), 1)),
        b'[' => {
            let (inner, n) = parse_type_sig(&s[1..])?;
            Some((format!("{}[]", inner), 1 + n))
        }
        b'T' => {
            // Type variable: TName;
            let end = s.find(';')?;
            Some((s[1..end].to_string(), end + 1))
        }
        b'L' => {
            let mut i = 1usize;
            let mut name = String::new();
            while i < bytes.len() {
                match bytes[i] {
                    b';' => {
                        return Some((name.replace('/', "."), i + 1));
                    }
                    b'<' => {
                        // generic args
                        i += 1;
                        let mut args = Vec::new();
                        while i < bytes.len() && bytes[i] != b'>' {
                            if bytes[i] == b'*' {
                                args.push("?".into());
                                i += 1;
                                continue;
                            }
                            if bytes[i] == b'+' || bytes[i] == b'-' {
                                let bound_kind = if bytes[i] == b'+' {
                                    " extends "
                                } else {
                                    " super "
                                };
                                i += 1;
                                let (inner, n) = parse_type_sig(&s[i..])?;
                                args.push(format!("?{}{}", bound_kind, inner));
                                i += n;
                                continue;
                            }
                            let (inner, n) = parse_type_sig(&s[i..])?;
                            args.push(inner);
                            i += n;
                        }
                        if i < bytes.len() && bytes[i] == b'>' {
                            i += 1;
                        }
                        if i < bytes.len() && bytes[i] == b';' {
                            i += 1;
                        }
                        let base = name.replace('/', ".");
                        return Some((format!("{}<{}>", base, args.join(", ")), i));
                    }
                    b'.' => {
                        // inner class → Outer.Inner in Java source
                        name.push('.');
                        i += 1;
                    }
                    c => {
                        name.push(c as char);
                        i += 1;
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn find_matching_gt(s: &str) -> Option<usize> {
    let mut depth = 1i32;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_type_params() {
        let s = signature_to_java_generics("<T:Ljava/lang/Object;>Ljava/lang/Object;").unwrap();
        assert_eq!(s, "<T>");
        let s2 = signature_to_java_generics(
            "<K:Ljava/lang/Object;V:Ljava/lang/Object;>Ljava/lang/Object;",
        )
        .unwrap();
        assert_eq!(s2, "<K, V>");
    }

    #[test]
    fn signature_list_of_string() {
        let s = signature_type_to_java("Ljava/util/List<Ljava/lang/String;>;").unwrap();
        assert_eq!(s, "java.util.List<java.lang.String>");
    }

    #[test]
    fn signature_method_with_type_param() {
        let m =
            signature_method_to_java("<T:Ljava/lang/Object;>(Ljava/util/List<TT;>;)TT;").unwrap();
        assert_eq!(m.type_params.as_deref(), Some("<T>"));
        assert_eq!(m.params, vec!["java.util.List<T>"]);
        assert_eq!(m.return_type, "T");
    }

    /// jadx `generics/TestGenerics` — wildcard / extends / super.
    #[test]
    fn jadx_signature_wildcards() {
        assert_eq!(
            signature_type_to_java("Ljava/util/List<*>;").as_deref(),
            Some("java.util.List<?>")
        );
        assert_eq!(
            signature_type_to_java("Ljava/util/List<+LA;>;").as_deref(),
            Some("java.util.List<? extends A>")
        );
        assert_eq!(
            signature_type_to_java("Ljava/util/List<-LA;>;").as_deref(),
            Some("java.util.List<? super A>")
        );
    }

    /// jadx-style class Signature with bounds.
    #[test]
    fn jadx_class_signature_with_bounds() {
        let s = signature_to_java_generics("<T:Ljava/lang/Number;>Ljava/lang/Object;").unwrap();
        assert_eq!(s, "<T extends java.lang.Number>");
        let cls = parse_class_signature(
            "<K:Ljava/lang/Object;V:Ljava/lang/Object;>Ljava/util/HashMap<TK;TV;>;",
        )
        .unwrap();
        assert_eq!(cls.type_params.as_deref(), Some("<K, V>"));
        assert!(cls.superclass.unwrap().contains("HashMap"));
    }
}
