//! Kotlin `@Metadata` heuristics + protobuf decode (no prost).
//!
//! Uses kind + naming heuristics for companions / data / DefaultImpls.
//! When `d1`/`d2` are present, decodes via Kotlin JVM BitEncoding (or base64)
//! and walks Class protobuf fields for flags, fq name, properties, functions.

use dex_parser::ClassDef;
use regex::Regex;

use super::annotations::{class_annotations, EncodedValue, ParsedAnnotation};

/// Kotlin class kind from `@kotlin.Metadata(k=…)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KotlinKind {
    Class = 1,
    File = 2,
    SyntheticClass = 3,
    MultiFileClassFacade = 4,
    MultiFileClassPart = 5,
}

impl KotlinKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Class),
            2 => Some(Self::File),
            3 => Some(Self::SyntheticClass),
            4 => Some(Self::MultiFileClassFacade),
            5 => Some(Self::MultiFileClassPart),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::File => "file facade",
            Self::SyntheticClass => "synthetic class",
            Self::MultiFileClassFacade => "multi-file facade",
            Self::MultiFileClassPart => "multi-file part",
        }
    }
}

/// Heuristic + optional protobuf Kotlin class info for decompile comments / naming.
#[derive(Clone, Debug, Default)]
pub struct KotlinClassInfo {
    pub kind: Option<KotlinKind>,
    pub is_companion: bool,
    pub is_data: bool,
    pub is_default_impls: bool,
    /// Extends ContinuationImpl / SuspendLambda / has invokeSuspend.
    pub is_coroutine: bool,
    pub metadata_version: Option<(u32, u32, u32)>,
    pub package_name: Option<String>,
    pub fq_name: Option<String>,
    pub property_names: Vec<String>,
    pub function_names: Vec<String>,
    /// Local / anonymous class names from StringTableTypes.local_name.
    pub local_class_names: Vec<String>,
    pub protobuf_decoded: bool,
}

impl KotlinClassInfo {
    pub fn comment_prefix(&self) -> Option<String> {
        let mut parts = Vec::new();
        if self.is_companion {
            parts.push("companion object".to_string());
        }
        if self.is_data {
            parts.push("data class".to_string());
        }
        if self.is_default_impls {
            parts.push("DefaultImpls".to_string());
        }
        if self.is_coroutine {
            parts.push("coroutine / suspend".to_string());
        }
        if let Some(k) = self.kind {
            if !self.is_companion && !self.is_data {
                parts.push(format!("Kotlin {}", k.label()));
            } else if matches!(
                k,
                KotlinKind::File
                    | KotlinKind::MultiFileClassFacade
                    | KotlinKind::MultiFileClassPart
            ) {
                parts.push(format!("Kotlin {}", k.label()));
            }
        }
        if let Some(ref fq) = self.fq_name {
            parts.push(format!("fq={fq}"));
        }
        if let Some(ref pn) = self.package_name {
            if self
                .fq_name
                .as_ref()
                .map(|f| !f.starts_with(pn.as_str()))
                .unwrap_or(true)
            {
                parts.push(format!("pn={pn}"));
            }
        }
        if !self.property_names.is_empty() {
            let n = self.property_names.len().min(6);
            parts.push(format!("props=[{}]", self.property_names[..n].join(", ")));
        }
        if !self.local_class_names.is_empty() {
            let n = self.local_class_names.len().min(4);
            parts.push(format!(
                "local=[{}]",
                self.local_class_names[..n].join(", ")
            ));
        }
        if parts.is_empty() {
            None
        } else {
            Some(format!("/* {} */", parts.join(", ")))
        }
    }
}

/// True if this annotation type should be omitted from Java output (noise).
pub fn is_kotlin_noise_annotation(java_type: &str) -> bool {
    matches!(
        java_type,
        "kotlin.Metadata"
            | "kotlin.jvm.internal.SourceDebugExtension"
            | "kotlin.coroutines.jvm.internal.DebugMetadata"
    )
}

/// Parse `@kotlin.Metadata` kind from class annotations.
pub fn kotlin_metadata_kind(
    anns: &[ParsedAnnotation],
    get_type: &dyn Fn(u32) -> Option<String>,
) -> Option<KotlinKind> {
    for ann in anns {
        let ty = get_type(ann.type_idx)?;
        let java = crate::java::descriptor_to_java(&ty);
        if java != "kotlin.Metadata" {
            continue;
        }
        for (name, val) in &ann.elements {
            if name == "k" {
                if let EncodedValue::Int(v) = val {
                    return KotlinKind::from_i32(*v);
                }
            }
        }
    }
    None
}

/// Resolved `@kotlin.Metadata` element values.
#[derive(Clone, Debug, Default)]
pub struct MetadataElements {
    pub kind: Option<KotlinKind>,
    pub metadata_version: Option<(u32, u32, u32)>,
    pub d1: Vec<String>,
    pub d2: Vec<String>,
    pub package_name: Option<String>,
    pub extra_int: Option<i32>,
}

fn encoded_int_array(val: &EncodedValue) -> Option<Vec<i32>> {
    match val {
        EncodedValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                match it {
                    EncodedValue::Int(v) => out.push(*v),
                    EncodedValue::Byte(v) => out.push(*v as i32),
                    EncodedValue::Short(v) => out.push(*v as i32),
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn encoded_string_array(
    val: &EncodedValue,
    get_string: &dyn Fn(u32) -> Option<String>,
) -> Option<Vec<String>> {
    match val {
        EncodedValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                match it {
                    EncodedValue::String(idx) => out.push(get_string(*idx).unwrap_or_default()),
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}

/// Extract Metadata annotation elements (k/mv/d1/d2/pn/xi).
pub fn parse_metadata_elements(
    anns: &[ParsedAnnotation],
    get_string: &dyn Fn(u32) -> Option<String>,
    get_type: &dyn Fn(u32) -> Option<String>,
) -> Option<MetadataElements> {
    for ann in anns {
        let ty = get_type(ann.type_idx)?;
        let java = crate::java::descriptor_to_java(&ty);
        if java != "kotlin.Metadata" {
            continue;
        }
        let mut out = MetadataElements::default();
        for (name, val) in &ann.elements {
            match name.as_str() {
                "k" => {
                    if let EncodedValue::Int(v) = val {
                        out.kind = KotlinKind::from_i32(*v);
                    }
                }
                "mv" => {
                    if let Some(arr) = encoded_int_array(val) {
                        if arr.len() >= 3 {
                            out.metadata_version =
                                Some((arr[0] as u32, arr[1] as u32, arr[2] as u32));
                        } else if arr.len() == 2 {
                            out.metadata_version = Some((arr[0] as u32, arr[1] as u32, 0));
                        }
                    }
                }
                "d1" => {
                    if let Some(arr) = encoded_string_array(val, get_string) {
                        out.d1 = arr;
                    }
                }
                "d2" => {
                    if let Some(arr) = encoded_string_array(val, get_string) {
                        out.d2 = arr;
                    }
                }
                "pn" => {
                    if let EncodedValue::String(idx) = val {
                        out.package_name = get_string(*idx);
                    }
                }
                "xi" => {
                    if let EncodedValue::Int(v) = val {
                        out.extra_int = Some(*v);
                    }
                }
                _ => {}
            }
        }
        return Some(out);
    }
    None
}

/// Standard base64 decode (std only).
pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn dig(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let cleaned: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if cleaned.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::with_capacity(cleaned.len() * 3 / 4);
    let mut i = 0;
    while i < cleaned.len() {
        let chunk: Vec<u8> = cleaned[i..].iter().copied().take(4).collect();
        if chunk.len() < 2 {
            return None;
        }
        let a = dig(chunk[0])?;
        let b = dig(chunk[1])?;
        out.push((a << 2) | (b >> 4));
        if chunk.len() >= 3 && chunk[2] != b'=' {
            let c = dig(chunk[2])?;
            out.push(((b & 0x0f) << 4) | (c >> 2));
            if chunk.len() >= 4 && chunk[3] != b'=' {
                let d = dig(chunk[3])?;
                out.push(((c & 0x03) << 6) | d);
            }
        }
        i += 4;
    }
    Some(out)
}

/// Kotlin JVM `BitEncoding` mode marker (`(char) -1` / U+FFFF).
const BIT_ENCODING_8TO7_MARKER: char = '\u{FFFF}';
/// Modern Kotlin UTF-8 embedding mode marker.
const BIT_ENCODING_UTF8_MARKER: char = '\u{0000}';

fn d1_looks_like_base64(d1: &[String]) -> bool {
    let s: String = d1.iter().map(|x| x.as_str()).collect();
    if s.is_empty() {
        return false;
    }
    // Real BitEncoding payloads use code units 0..=0x7f after packing and often include
    // non-base64 chars; synthetic tests use standard base64.
    s.chars().all(
        |c| matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '+' | '/' | '=' | '\n' | '\r' | ' '),
    )
}

/// Kotlin `BitEncoding.decodeBytes` — 8↔7 bit packing used by `@Metadata(d1=…)`.
/// See `org.jetbrains.kotlin.metadata.jvm.deserialization.BitEncoding`.
pub fn bit_encoding_decode(d1: &[String]) -> Option<Vec<u8>> {
    if d1.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = d1.to_vec();
    if let Some(first) = parts.first() {
        if !first.is_empty() {
            let marker = first.chars().next()?;
            if marker == BIT_ENCODING_UTF8_MARKER {
                // UTF-8 mode: drop marker, map each char to a byte.
                parts[0] = first.chars().skip(1).collect();
                return Some(combine_string_array_into_bytes(&parts));
            }
            if marker == BIT_ENCODING_8TO7_MARKER {
                parts[0] = first.chars().skip(1).collect();
            }
        }
    }
    let mut bytes = combine_string_array_into_bytes(&parts);
    add_modulo_byte(&mut bytes, 0x7f);
    Some(decode_7to8(&bytes))
}

fn combine_string_array_into_bytes(data: &[String]) -> Vec<u8> {
    let mut out = Vec::new();
    for s in data {
        for c in s.chars() {
            out.push((c as u32 & 0xff) as u8);
        }
    }
    out
}

fn add_modulo_byte(data: &mut [u8], increment: u8) {
    for b in data.iter_mut() {
        *b = b.wrapping_add(increment) & 0x7f;
    }
}

fn decode_7to8(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    let result_len = 7 * data.len() / 8;
    let mut result = vec![0u8; result_len];
    let mut byte_index = 0usize;
    let mut bit = 0u32;
    for i in 0..result_len {
        let first_part = (data[byte_index] as u32) >> bit;
        byte_index += 1;
        if byte_index >= data.len() {
            // Padding / truncated — stop with what we have.
            result.truncate(i);
            break;
        }
        let second_part = (data[byte_index] as u32 & ((1 << (bit + 1)) - 1)) << (7 - bit);
        result[i] = (first_part + second_part) as u8;
        if bit == 6 {
            byte_index += 1;
            bit = 0;
        } else {
            bit += 1;
        }
    }
    result
}

/// Encode bytes with Kotlin BitEncoding (for tests). Inverse of [`bit_encoding_decode`].
pub fn bit_encoding_encode(data: &[u8]) -> Vec<String> {
    let mut packed = encode_8to7(data);
    add_modulo_byte(&mut packed, 1);
    // Single string; prepend 8to7 marker like the Kotlin compiler.
    let mut s = String::new();
    s.push(BIT_ENCODING_8TO7_MARKER);
    for b in packed {
        s.push(char::from_u32(b as u32).unwrap_or('\0'));
    }
    vec![s]
}

fn encode_8to7(data: &[u8]) -> Vec<u8> {
    // Kotlin BitEncoding.encode8to7 — ceil(n * 8 / 7)
    if data.is_empty() {
        return Vec::new();
    }
    let result_len = (data.len() * 8 + 6) / 7;
    let mut result = vec![0u8; result_len];
    let mut byte_index = 0usize;
    let mut bit: u32 = 0;
    for i in 0..result_len.saturating_sub(1) {
        if bit == 0 {
            result[i] = data[byte_index] & 0x7f;
            bit = 7;
            continue;
        }
        let first_part = (data[byte_index] as u32) >> bit;
        let new_bit = (bit + 7) & 7;
        byte_index += 1;
        let second_part = (data[byte_index] as u32 & ((1 << new_bit) - 1)) << (8 - bit);
        result[i] = (first_part + second_part) as u8;
        bit = new_bit;
    }
    if result_len > 0 {
        result[result_len - 1] = ((data[byte_index] as u32) >> bit) as u8;
    }
    result
}

/// Decode `@Metadata` `d1` into protobuf bytes.
/// Real Kotlin uses [`bit_encoding_decode`]; tests/tools may use standard base64.
pub fn decode_d1_bytes(d1: &[String]) -> Option<Vec<u8>> {
    if d1.is_empty() {
        return None;
    }
    if d1_looks_like_base64(d1) {
        let mut out = Vec::new();
        for s in d1 {
            let part = base64_decode(s)?;
            out.extend_from_slice(&part);
        }
        return Some(out);
    }
    bit_encoding_decode(d1)
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProtoVal {
    Varint(u64),
    Len(Vec<u8>),
    Fixed64(u64),
    Fixed32(u32),
}

/// Minimal protobuf wire field walker (varint + length-delimited + skip others).
pub fn read_protobuf_fields(bytes: &[u8]) -> Vec<(u32, ProtoVal)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let (key, ni) = match read_varint(bytes, i) {
            Some(v) => v,
            None => break,
        };
        i = ni;
        let field = (key >> 3) as u32;
        let wire = (key & 7) as u32;
        match wire {
            0 => {
                let (v, ni) = match read_varint(bytes, i) {
                    Some(v) => v,
                    None => break,
                };
                i = ni;
                out.push((field, ProtoVal::Varint(v)));
            }
            1 => {
                if i + 8 > bytes.len() {
                    break;
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&bytes[i..i + 8]);
                i += 8;
                out.push((field, ProtoVal::Fixed64(u64::from_le_bytes(buf))));
            }
            2 => {
                let (len, ni) = match read_varint(bytes, i) {
                    Some(v) => v,
                    None => break,
                };
                i = ni;
                let len = len as usize;
                if i + len > bytes.len() {
                    break;
                }
                out.push((field, ProtoVal::Len(bytes[i..i + len].to_vec())));
                i += len;
            }
            5 => {
                if i + 4 > bytes.len() {
                    break;
                }
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&bytes[i..i + 4]);
                i += 4;
                out.push((field, ProtoVal::Fixed32(u32::from_le_bytes(buf))));
            }
            _ => break,
        }
    }
    out
}

fn read_varint(bytes: &[u8], mut i: usize) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    loop {
        if i >= bytes.len() || shift >= 64 {
            return None;
        }
        let b = bytes[i];
        i += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some((result, i));
        }
        shift += 7;
    }
}

/// Kotlin `Flags.Class.IS_DATA` bit (bit 10).
const CLASS_FLAG_IS_DATA: u32 = 1 << 10;

/// Kotlin JVM predefined string table (JvmNameResolverBase.PREDEFINED_STRINGS).
const PREDEFINED_STRINGS: &[&str] = &[
    "kotlin/Any",
    "kotlin/Nothing",
    "kotlin/Unit",
    "kotlin/Throwable",
    "kotlin/Number",
    "kotlin/Byte",
    "kotlin/Double",
    "kotlin/Float",
    "kotlin/Int",
    "kotlin/Long",
    "kotlin/Short",
    "kotlin/Boolean",
    "kotlin/Char",
    "kotlin/CharSequence",
    "kotlin/String",
    "kotlin/Comparable",
    "kotlin/Enum",
    "kotlin/Array",
    "kotlin/ByteArray",
    "kotlin/DoubleArray",
    "kotlin/FloatArray",
    "kotlin/IntArray",
    "kotlin/LongArray",
    "kotlin/ShortArray",
    "kotlin/BooleanArray",
    "kotlin/CharArray",
    "kotlin/Cloneable",
    "kotlin/Annotation",
    "kotlin/collections/Iterable",
    "kotlin/collections/MutableIterable",
    "kotlin/collections/Collection",
    "kotlin/collections/MutableCollection",
    "kotlin/collections/List",
    "kotlin/collections/MutableList",
    "kotlin/collections/Set",
    "kotlin/collections/MutableSet",
    "kotlin/collections/Map",
    "kotlin/collections/MutableMap",
    "kotlin/collections/Map.Entry",
    "kotlin/collections/MutableMap.MutableEntry",
    "kotlin/collections/Iterator",
    "kotlin/collections/MutableIterator",
    "kotlin/collections/ListIterator",
    "kotlin/collections/MutableListIterator",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum StringOp {
    #[default]
    None = 0,
    InternalToClassId = 1,
    DescToClassId = 2,
}

#[derive(Clone, Debug, Default)]
struct StringTableRecord {
    range: i32,
    predefined_index: Option<i32>,
    string: Option<String>,
    operation: StringOp,
    substring_index: Vec<i32>,
    replace_char: Vec<i32>,
}

/// JVM NameResolver: `d2` strings + StringTableTypes records + localName set.
#[derive(Clone, Debug)]
pub struct JvmNameResolver {
    strings: Vec<String>,
    records: Vec<StringTableRecord>,
    local_name_indices: std::collections::HashSet<i32>,
}

impl JvmNameResolver {
    /// Flat `d2`-only resolver (tests / metadata without StringTableTypes).
    pub fn from_d2(d2: &[String]) -> Self {
        Self {
            strings: d2.to_vec(),
            records: Vec::new(),
            local_name_indices: std::collections::HashSet::new(),
        }
    }

    pub fn get_string(&self, index: usize) -> Option<String> {
        let record = if index < self.records.len() {
            Some(&self.records[index])
        } else {
            None
        };
        let mut string = if let Some(r) = record {
            if let Some(ref s) = r.string {
                s.clone()
            } else if let Some(pi) = r.predefined_index {
                let pi = pi as usize;
                if pi < PREDEFINED_STRINGS.len() {
                    PREDEFINED_STRINGS[pi].to_string()
                } else if index < self.strings.len() {
                    self.strings[index].clone()
                } else {
                    return None;
                }
            } else if index < self.strings.len() {
                self.strings[index].clone()
            } else {
                return None;
            }
        } else if index < self.strings.len() {
            self.strings[index].clone()
        } else {
            return None;
        };

        if let Some(r) = record {
            if r.substring_index.len() >= 2 {
                let begin = r.substring_index[0] as usize;
                let end = r.substring_index[1] as usize;
                if begin <= end && end <= string.len() {
                    // Kotlin uses UTF-16 indices; for BMP-only names byte/char indices match.
                    let chars: Vec<char> = string.chars().collect();
                    if end <= chars.len() {
                        string = chars[begin..end].iter().collect();
                    }
                }
            }
            if r.replace_char.len() >= 2 {
                let from = char::from_u32(r.replace_char[0] as u32).unwrap_or('\0');
                let to = char::from_u32(r.replace_char[1] as u32).unwrap_or('\0');
                string = string.replace(from, &to.to_string());
            }
            match r.operation {
                StringOp::None => {}
                StringOp::InternalToClassId => {
                    string = string.replace('$', ".");
                }
                StringOp::DescToClassId => {
                    if string.len() >= 2 {
                        string = string[1..string.len() - 1].to_string();
                    }
                    string = string.replace('$', ".");
                }
            }
        }
        Some(normalize_kotlin_name(&string))
    }

    pub fn is_local_class_name(&self, index: i32) -> bool {
        self.local_name_indices.contains(&index)
    }

    pub fn local_class_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut idxs: Vec<_> = self.local_name_indices.iter().copied().collect();
        idxs.sort_unstable();
        for i in idxs {
            if let Some(s) = self.get_string(i as usize) {
                out.push(s);
            }
        }
        out
    }
}

fn resolve_name(resolver: &JvmNameResolver, idx: u64) -> Option<String> {
    let i0 = idx as usize;
    if let Some(s) = resolver.get_string(i0) {
        return Some(s);
    }
    // Legacy 1-based fallback for flat d2-only fixtures.
    if idx >= 1 {
        return resolver.get_string((idx as usize) - 1);
    }
    None
}

fn normalize_kotlin_name(s: &str) -> String {
    // Kotlin internal names use `/`; present as Java-ish `.` for comments.
    s.replace('/', ".")
}

fn looks_like_name(s: &str) -> bool {
    if s.is_empty() || s.len() > 200 {
        return false;
    }
    let bytes = s.as_bytes();
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '/' || c == '$')
}

fn parse_string_table_record(bytes: &[u8]) -> StringTableRecord {
    let mut rec = StringTableRecord {
        range: 1,
        ..Default::default()
    };
    for (num, val) in read_protobuf_fields(bytes) {
        match (num, val) {
            (1, ProtoVal::Varint(v)) => rec.range = v as i32,
            (2, ProtoVal::Varint(v)) => rec.predefined_index = Some(v as i32),
            (3, ProtoVal::Varint(v)) => {
                rec.operation = match v {
                    1 => StringOp::InternalToClassId,
                    2 => StringOp::DescToClassId,
                    _ => StringOp::None,
                };
            }
            (4, ProtoVal::Len(raw)) => {
                // packed repeated int32
                rec.substring_index.extend(read_packed_int32s(&raw));
            }
            (4, ProtoVal::Varint(v)) => rec.substring_index.push(v as i32),
            (5, ProtoVal::Len(raw)) => {
                rec.replace_char.extend(read_packed_int32s(&raw));
            }
            (5, ProtoVal::Varint(v)) => rec.replace_char.push(v as i32),
            (6, ProtoVal::Len(raw)) => {
                if let Ok(s) = std::str::from_utf8(&raw) {
                    rec.string = Some(s.to_string());
                }
            }
            _ => {}
        }
    }
    rec
}

fn read_packed_int32s(bytes: &[u8]) -> Vec<i32> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let (v, ni) = match read_varint(bytes, i) {
            Some(v) => v,
            None => break,
        };
        out.push(v as i32);
        i = ni;
    }
    out
}

/// Parse `JvmProtoBuf.StringTableTypes` and expand `range` like Kotlin `toExpandedRecordsList`.
pub fn parse_string_table_types(bytes: &[u8], d2: &[String]) -> JvmNameResolver {
    let mut records_raw = Vec::new();
    let mut local_name = std::collections::HashSet::new();
    for (num, val) in read_protobuf_fields(bytes) {
        match (num, val) {
            (1, ProtoVal::Len(raw)) => {
                records_raw.push(parse_string_table_record(&raw));
            }
            (5, ProtoVal::Len(raw)) => {
                for v in read_packed_int32s(&raw) {
                    local_name.insert(v);
                }
            }
            (5, ProtoVal::Varint(v)) => {
                local_name.insert(v as i32);
            }
            _ => {}
        }
    }
    let mut expanded = Vec::new();
    for rec in records_raw {
        let range = if rec.range <= 0 { 1 } else { rec.range };
        for _ in 0..range {
            expanded.push(rec.clone());
        }
    }
    JvmNameResolver {
        strings: d2.to_vec(),
        records: expanded,
        local_name_indices: local_name,
    }
}

/// Decode Class-like protobuf payload using a [`JvmNameResolver`].
pub fn decode_class_protobuf(bytes: &[u8], resolver: &JvmNameResolver) -> DecodedClassMeta {
    let mut meta = DecodedClassMeta::default();
    let fields = read_protobuf_fields(bytes);
    if fields.is_empty() {
        return meta;
    }
    meta.decoded = true;
    for (num, val) in &fields {
        match (*num, val) {
            (1, ProtoVal::Varint(v)) => {
                meta.flags = Some(*v as u32);
                if (*v as u32) & CLASS_FLAG_IS_DATA != 0 {
                    meta.is_data = true;
                }
            }
            // ProtoBuf.Class.fq_name = 3 (string table index)
            (3, ProtoVal::Varint(v)) => {
                meta.fq_name = resolve_name(resolver, *v);
            }
            (3, ProtoVal::Len(raw)) => {
                if let Ok(s) = std::str::from_utf8(raw) {
                    if looks_like_name(s) {
                        meta.fq_name = Some(normalize_kotlin_name(s));
                    }
                }
            }
            // function = 9
            (9, ProtoVal::Len(raw)) => {
                if let Some(n) = nested_name_field(raw, resolver) {
                    meta.function_names.push(n);
                }
            }
            // property = 10
            (10, ProtoVal::Len(raw)) => {
                if let Some(n) = nested_name_field(raw, resolver) {
                    meta.property_names.push(n);
                }
            }
            (_, ProtoVal::Len(raw)) => {
                // Collect obvious UTF-8 name-like strings.
                if let Ok(s) = std::str::from_utf8(raw) {
                    if looks_like_name(s) && s.contains('/') {
                        if meta.fq_name.is_none() {
                            meta.fq_name = Some(normalize_kotlin_name(s));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    meta
}

/// Convenience for tests that only have a flat `d2` table.
pub fn decode_class_protobuf_d2(bytes: &[u8], d2: &[String]) -> DecodedClassMeta {
    decode_class_protobuf(bytes, &JvmNameResolver::from_d2(d2))
}

fn nested_name_field(bytes: &[u8], resolver: &JvmNameResolver) -> Option<String> {
    let fields = read_protobuf_fields(bytes);
    for (num, val) in fields {
        // Function/Property.name = 2
        if num == 2 {
            match val {
                ProtoVal::Varint(v) => return resolve_name(resolver, v),
                ProtoVal::Len(raw) => {
                    if let Ok(s) = std::str::from_utf8(&raw) {
                        if looks_like_name(s) {
                            return Some(normalize_kotlin_name(s));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    None
}

#[derive(Clone, Debug, Default)]
pub struct DecodedClassMeta {
    pub decoded: bool,
    pub flags: Option<u32>,
    pub is_data: bool,
    pub fq_name: Option<String>,
    pub property_names: Vec<String>,
    pub function_names: Vec<String>,
}

fn apply_metadata_decode(info: &mut KotlinClassInfo, elems: &MetadataElements) {
    info.kind = elems.kind.or(info.kind);
    info.metadata_version = elems.metadata_version;
    info.package_name = elems.package_name.clone();
    let Some(bytes) = decode_d1_bytes(&elems.d1) else {
        return;
    };
    // Real JVM metadata: length-delimited StringTableTypes then Class protobuf
    // (see JvmProtoBufUtil.readDataFrom / parseDelimitedFrom).
    let (resolver, class_bytes) =
        if let Some((types_payload, class_rest)) = take_length_delimited(&bytes) {
            let resolver = parse_string_table_types(types_payload, &elems.d2);
            (resolver, class_rest.to_vec())
        } else {
            (JvmNameResolver::from_d2(&elems.d2), bytes.clone())
        };

    let mut best = decode_class_protobuf(&class_bytes, &resolver);
    if !best.decoded || (best.fq_name.is_none() && best.property_names.is_empty()) {
        // Fallback: whole buffer as Class with flat d2 (unit tests / no types prefix).
        let flat = JvmNameResolver::from_d2(&elems.d2);
        let alt = decode_class_protobuf(&bytes, &flat);
        if alt.decoded
            && (alt.fq_name.is_some()
                || !alt.property_names.is_empty()
                || !alt.function_names.is_empty())
        {
            best = alt;
        } else if let Some((_, after)) = take_length_delimited(&bytes) {
            let alt = decode_class_protobuf(after, &flat);
            if alt.decoded
                && (alt.fq_name.is_some()
                    || !alt.property_names.is_empty()
                    || !alt.function_names.is_empty())
            {
                best = alt;
            }
        } else if let Some((_, after)) = skip_field_delimited_message(&bytes) {
            let alt = decode_class_protobuf(after, &flat);
            if alt.decoded
                && (alt.fq_name.is_some()
                    || !alt.property_names.is_empty()
                    || !alt.function_names.is_empty())
            {
                best = alt;
            }
        }
    }
    if best.decoded {
        info.protobuf_decoded = true;
        if best.is_data {
            info.is_data = true;
        }
        if let Some(fq) = best.fq_name {
            info.fq_name = Some(fq);
        }
        info.property_names = best.property_names;
        info.function_names = best.function_names;
        let locals = resolver.local_class_names();
        if !locals.is_empty() {
            info.local_class_names = locals;
        }
    }
}

/// Protobuf `writeDelimitedTo` / `parseDelimitedFrom`: varint length + message.
/// Only succeeds when the payload looks like StringTableTypes **and** Class bytes remain.
fn take_length_delimited(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let (len, i) = read_varint(bytes, 0)?;
    let end = i + len as usize;
    if len == 0 || end >= bytes.len() {
        // Need trailing Class message; pure Class buffers must not match.
        return None;
    }
    let payload = &bytes[i..end];
    let fields = read_protobuf_fields(payload);
    let looks_like_types = fields
        .iter()
        .any(|(n, v)| (*n == 1 && matches!(v, ProtoVal::Len(_))) || *n == 5);
    if !looks_like_types {
        return None;
    }
    Some((payload, &bytes[end..]))
}

fn skip_field_delimited_message(bytes: &[u8]) -> Option<(usize, &[u8])> {
    // Parse first field; if length-delimited, return rest after it.
    let (key, i) = read_varint(bytes, 0)?;
    let wire = (key & 7) as u32;
    if wire != 2 {
        return None;
    }
    let (len, i) = read_varint(bytes, i)?;
    let end = i + len as usize;
    if end > bytes.len() {
        return None;
    }
    Some((end, &bytes[end..]))
}

/// Build heuristic Kotlin info for a class.
pub fn analyze_kotlin_class(
    data: &[u8],
    class_def: &ClassDef,
    class_name: &str,
    method_names: &[String],
    get_string: &dyn Fn(u32) -> Option<String>,
    get_type: &dyn Fn(u32) -> Option<String>,
) -> KotlinClassInfo {
    let mut info = KotlinClassInfo::default();
    if let Some(anns) = class_annotations(data, class_def, get_string) {
        info.kind = kotlin_metadata_kind(&anns, get_type);
        if let Some(elems) = parse_metadata_elements(&anns, get_string, get_type) {
            apply_metadata_decode(&mut info, &elems);
        }
    }

    let simple = class_name.rsplit('.').next().unwrap_or(class_name);
    info.is_companion = simple == "Companion" || class_name.ends_with("$Companion");
    info.is_default_impls = simple == "DefaultImpls" || class_name.ends_with("$DefaultImpls");

    let has_invoke_suspend = method_names.iter().any(|n| n == "invokeSuspend");
    let looks_continuation = class_name.contains("Continuation")
        || method_names.iter().any(|n| n == "create" || n == "invoke");
    info.is_coroutine =
        has_invoke_suspend || (info.kind == Some(KotlinKind::SyntheticClass) && looks_continuation);

    // Data class: prefer protobuf flag; else component1 + copy heuristic.
    if !info.is_data {
        let has_component1 = method_names.iter().any(|n| n == "component1");
        let has_copy = method_names.iter().any(|n| n == "copy");
        let has_equals = method_names.iter().any(|n| n == "equals");
        info.is_data = has_component1
            && has_copy
            && (has_equals || method_names.iter().any(|n| n == "toString"));
    }

    info
}

/// Best-effort coroutine state-machine cleanup for `invokeSuspend` bodies.
/// Adds a readable header and normalizes common `COROUTINE_SUSPENDED` checks.
pub fn restore_coroutine_invoke_suspend(body: &str) -> String {
    let mut out = String::new();
    if !body.contains("/* coroutine") && !body.contains("coroutine state machine") {
        out.push_str("        // coroutine state machine (invokeSuspend)\n");
    }
    let mut cleaned = body.to_string();
    // Common Intrinsics / Result patterns → clearer markers.
    for (from, to) in [
        (
            "kotlin.coroutines.intrinsics.IntrinsicsKt.getCOROUTINE_SUSPENDED()",
            "/* COROUTINE_SUSPENDED */ null",
        ),
        (
            "kotlin.coroutines.intrinsics.IntrinsicsKt.COROUTINE_SUSPENDED",
            "/* COROUTINE_SUSPENDED */",
        ),
        (
            "kotlin.coroutines.intrinsics.IntrinsicsKt$COROUTINE_SUSPENDED",
            "/* COROUTINE_SUSPENDED */",
        ),
        (
            "kotlin.coroutines.intrinsics.CoroutineSingletons.COROUTINE_SUSPENDED",
            "/* COROUTINE_SUSPENDED */",
        ),
    ] {
        cleaned = cleaned.replace(from, to);
    }
    // Label switch comment when we see this.label / label field.
    if (cleaned.contains("this.label") || cleaned.contains(".label"))
        && !cleaned.contains("coroutine label")
        && !out.contains("dispatches on coroutine label")
    {
        out.push_str("        // dispatches on coroutine label\n");
    }
    out.push_str(&cleaned);
    out
}

/// Strip / rewrite common Kotlin → Java boilerplate toward more idiomatic Java.
pub fn restore_kotlin_idioms(body: &str) -> String {
    let mut cleaned = body.to_string();

    // Null-check intrinsics are noise in Java restores.
    let null_check_re = Regex::new(
        r#"(?m)^\s*kotlin\.jvm\.internal\.Intrinsics\.(?:checkNotNullParameter|checkNotNullExpressionValue|checkParameterIsNotNull|checkExpressionValueIsNotNull)\([^;]*\);\s*\n?"#,
    );
    if let Ok(re) = null_check_re {
        cleaned = re.replace_all(&cleaned, "").into_owned();
    }
    cleaned = cleaned.replace(
        "kotlin.jvm.internal.Intrinsics.throwNpe()",
        "throw new NullPointerException()",
    );
    cleaned = cleaned.replace(
        "kotlin.jvm.internal.Intrinsics.throwJavaNpe()",
        "throw new NullPointerException()",
    );

    // Companion.INSTANCE.foo → Companion.foo (common R8/Kotlin emit).
    let companion_re = Regex::new(r"([A-Za-z0-9_$.]+)\.Companion\.INSTANCE\.");
    if let Ok(re) = companion_re {
        cleaned = re.replace_all(&cleaned, "$1.Companion.").into_owned();
    }
    let companion_inst_re = Regex::new(r"([A-Za-z0-9_$.]+)\.Companion\.INSTANCE\b");
    if let Ok(re) = companion_inst_re {
        cleaned = re.replace_all(&cleaned, "$1.Companion").into_owned();
    }

    // Foo.DefaultImpls.bar(receiver, args) → /* DefaultImpls */ receiver.bar(args)
    let default_impls_re = Regex::new(
        r"([A-Za-z0-9_$.]+)\.DefaultImpls\.([A-Za-z0-9_$]+)\(([A-Za-z0-9_$.]+)(?:,\s*)?",
    );
    if let Ok(re) = default_impls_re {
        cleaned = re
            .replace_all(&cleaned, "/* DefaultImpls */ $3.$2(")
            .into_owned();
    }

    // Unit.INSTANCE → /* Unit */ null (void-ish sentinel in Kotlin).
    cleaned = cleaned.replace("kotlin.Unit.INSTANCE", "/* Unit */ null");

    cleaned
}

/// Encode protobuf fields for tests (varint + length-delimited only).
#[cfg(test)]
pub fn encode_protobuf_fields(fields: &[(u32, ProtoVal)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (num, val) in fields {
        match val {
            ProtoVal::Varint(v) => {
                write_varint(&mut out, ((*num as u64) << 3) | 0);
                write_varint(&mut out, *v);
            }
            ProtoVal::Len(raw) => {
                write_varint(&mut out, ((*num as u64) << 3) | 2);
                write_varint(&mut out, raw.len() as u64);
                out.extend_from_slice(raw);
            }
            ProtoVal::Fixed32(v) => {
                write_varint(&mut out, ((*num as u64) << 3) | 5);
                out.extend_from_slice(&v.to_le_bytes());
            }
            ProtoVal::Fixed64(v) => {
                write_varint(&mut out, ((*num as u64) << 3) | 1);
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    out
}

#[cfg(test)]
fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
            out.push(b);
        } else {
            out.push(b);
            break;
        }
    }
}

#[cfg(test)]
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i];
        let b1 = if i + 1 < data.len() { data[i + 1] } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] } else { 0 };
        out.push(T[(b0 >> 2) as usize] as char);
        out.push(T[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if i + 1 < data.len() {
            out.push(T[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if i + 2 < data.len() {
            out.push(T[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        i += 3;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_from_i32() {
        assert_eq!(KotlinKind::from_i32(1), Some(KotlinKind::Class));
        assert_eq!(KotlinKind::from_i32(2), Some(KotlinKind::File));
        assert_eq!(KotlinKind::from_i32(99), None);
    }

    #[test]
    fn companion_and_data_heuristics() {
        let info = analyze_kotlin_class(
            &[],
            &ClassDef {
                class_idx: 0,
                access_flags: 0,
                superclass_idx: 0xffff_ffff,
                interfaces_off: 0,
                source_file_idx: 0xffff_ffff,
                annotations_off: 0,
                class_data_off: 0,
                static_values_off: 0,
            },
            "com.example.Foo$Companion",
            &["equals".into(), "hashCode".into()],
            &|_| None,
            &|_| None,
        );
        assert!(info.is_companion);
        assert!(!info.is_data);

        let data = analyze_kotlin_class(
            &[],
            &ClassDef {
                class_idx: 0,
                access_flags: 0,
                superclass_idx: 0xffff_ffff,
                interfaces_off: 0,
                source_file_idx: 0xffff_ffff,
                annotations_off: 0,
                class_data_off: 0,
                static_values_off: 0,
            },
            "com.example.User",
            &[
                "component1".into(),
                "copy".into(),
                "equals".into(),
                "toString".into(),
            ],
            &|_| None,
            &|_| None,
        );
        assert!(data.is_data);
    }

    #[test]
    fn base64_roundtrip_and_protobuf_walk() {
        let raw = b"hello";
        let enc = base64_encode(raw);
        assert_eq!(base64_decode(&enc).unwrap(), raw);

        let prop = encode_protobuf_fields(&[(2, ProtoVal::Varint(1))]); // name idx 1
        let class_bytes = encode_protobuf_fields(&[
            (1, ProtoVal::Varint((CLASS_FLAG_IS_DATA as u64) | 6)),
            (3, ProtoVal::Varint(0)), // fq idx 0
            (10, ProtoVal::Len(prop)),
        ]);
        let fields = read_protobuf_fields(&class_bytes);
        assert!(fields.iter().any(|(n, _)| *n == 1));
        assert!(fields.iter().any(|(n, _)| *n == 3));
        assert!(fields.iter().any(|(n, _)| *n == 10));

        let d2 = vec!["com/example/User".into(), "name".into()];
        let meta = decode_class_protobuf_d2(&class_bytes, &d2);
        assert!(meta.decoded);
        assert!(meta.is_data);
        assert_eq!(meta.fq_name.as_deref(), Some("com.example.User"));
        assert_eq!(meta.property_names, vec!["name".to_string()]);
    }

    #[test]
    fn bit_encoding_roundtrip() {
        for raw in &[b"" as &[u8], b"a", b"hello", b"\x00\x01\x7f\xff\x80"] {
            let enc = bit_encoding_encode(raw);
            let dec = bit_encoding_decode(&enc).expect("decode");
            assert_eq!(&dec[..], *raw, "roundtrip failed for {:?}", raw);
        }
        let long: Vec<u8> = (0..200u8).collect();
        let enc = bit_encoding_encode(&long);
        let dec = bit_encoding_decode(&enc).expect("decode long");
        assert_eq!(dec, long);

        // decode_d1_bytes should use BitEncoding for non-base64 payloads
        let class_bytes = encode_protobuf_fields(&[
            (1, ProtoVal::Varint((CLASS_FLAG_IS_DATA as u64) | 6)),
            (3, ProtoVal::Varint(0)),
        ]);
        let d1 = bit_encoding_encode(&class_bytes);
        let got = decode_d1_bytes(&d1).expect("d1");
        assert_eq!(got, class_bytes);
        let d2 = vec!["com/example/User".into()];
        let meta = decode_class_protobuf_d2(&got, &d2);
        assert!(meta.is_data);
        assert_eq!(meta.fq_name.as_deref(), Some("com.example.User"));
    }

    #[test]
    fn metadata_elements_decode_into_comment() {
        // Build synthetic strings table for get_string.
        let strings = vec![
            "com/example/User".to_string(),
            "name".to_string(),
            "age".to_string(),
            "hello".to_string(),
            "com.example".to_string(),
        ];
        let get_type = |idx: u32| {
            if idx == 0 {
                Some("Lkotlin/Metadata;".into())
            } else {
                None
            }
        };

        let prop_name = encode_protobuf_fields(&[(2, ProtoVal::Varint(1))]);
        let prop_age = encode_protobuf_fields(&[(2, ProtoVal::Varint(2))]);
        let fun_hello = encode_protobuf_fields(&[(2, ProtoVal::Varint(3))]);
        let class_bytes = encode_protobuf_fields(&[
            (1, ProtoVal::Varint((CLASS_FLAG_IS_DATA as u64) | 6)),
            (3, ProtoVal::Varint(0)),
            (9, ProtoVal::Len(fun_hello)),
            (10, ProtoVal::Len(prop_name)),
            (10, ProtoVal::Len(prop_age)),
        ]);
        let d1_b64 = base64_encode(&class_bytes);

        // d1/d2 string indices in annotation point into DEX string pool.
        let mut pool = strings.clone();
        let d1_idx = pool.len() as u32;
        pool.push(d1_b64);
        let get_string = move |idx: u32| pool.get(idx as usize).cloned();

        let ann = ParsedAnnotation {
            type_idx: 0,
            elements: vec![
                ("k".into(), EncodedValue::Int(1)),
                (
                    "mv".into(),
                    EncodedValue::Array(vec![
                        EncodedValue::Int(1),
                        EncodedValue::Int(8),
                        EncodedValue::Int(0),
                    ]),
                ),
                (
                    "d1".into(),
                    EncodedValue::Array(vec![EncodedValue::String(d1_idx)]),
                ),
                (
                    "d2".into(),
                    EncodedValue::Array(vec![
                        EncodedValue::String(0),
                        EncodedValue::String(1),
                        EncodedValue::String(2),
                        EncodedValue::String(3),
                    ]),
                ),
                ("pn".into(), EncodedValue::String(4)),
            ],
        };

        let elems = parse_metadata_elements(&[ann], &get_string, &get_type).expect("elems");
        assert_eq!(elems.kind, Some(KotlinKind::Class));
        assert_eq!(elems.metadata_version, Some((1, 8, 0)));
        assert_eq!(elems.package_name.as_deref(), Some("com.example"));

        let mut info = KotlinClassInfo::default();
        apply_metadata_decode(&mut info, &elems);
        assert!(info.protobuf_decoded);
        assert!(info.is_data);
        assert_eq!(info.fq_name.as_deref(), Some("com.example.User"));
        assert!(info.property_names.contains(&"name".to_string()));
        assert!(info.function_names.contains(&"hello".to_string()));

        let comment = info.comment_prefix().expect("comment");
        assert!(comment.contains("data class"), "{comment}");
        assert!(comment.contains("fq=com.example.User"), "{comment}");
        assert!(comment.contains("props=["), "{comment}");
    }

    #[test]
    fn name_resolver_predefined_and_operations() {
        // Record 0: predefined kotlin/String (index 14)
        let rec0 = encode_protobuf_fields(&[(2, ProtoVal::Varint(14))]);
        // Record 1: string "pkg/Outer$Inner" + INTERNAL_TO_CLASS_ID
        let rec1 = encode_protobuf_fields(&[
            (6, ProtoVal::Len(b"pkg/Outer$Inner".to_vec())),
            (3, ProtoVal::Varint(1)),
        ]);
        // Record 2: range=2 expands to two slots sharing "x"
        let rec2 = encode_protobuf_fields(&[
            (1, ProtoVal::Varint(2)),
            (6, ProtoVal::Len(b"dup".to_vec())),
        ]);
        let types = encode_protobuf_fields(&[
            (1, ProtoVal::Len(rec0)),
            (1, ProtoVal::Len(rec1)),
            (1, ProtoVal::Len(rec2)),
            (5, ProtoVal::Varint(1)), // local_name index 1
        ]);
        let d2 = vec!["fallback0".into(), "fallback1".into()];
        let resolver = parse_string_table_types(&types, &d2);
        assert_eq!(resolver.get_string(0).as_deref(), Some("kotlin.String"));
        assert_eq!(resolver.get_string(1).as_deref(), Some("pkg.Outer.Inner"));
        assert!(resolver.is_local_class_name(1));
        assert_eq!(resolver.get_string(2).as_deref(), Some("dup"));
        assert_eq!(resolver.get_string(3).as_deref(), Some("dup")); // range expansion

        // Delimited types + class → apply_metadata_decode path
        let mut buf = Vec::new();
        write_varint(&mut buf, types.len() as u64);
        buf.extend_from_slice(&types);
        let class_bytes = encode_protobuf_fields(&[
            (1, ProtoVal::Varint(CLASS_FLAG_IS_DATA as u64)),
            (3, ProtoVal::Varint(1)), // fq via resolver index 1
        ]);
        buf.extend_from_slice(&class_bytes);
        let d1 = vec![base64_encode(&buf)];
        let elems = MetadataElements {
            kind: Some(KotlinKind::Class),
            d1,
            d2,
            ..Default::default()
        };
        let mut info = KotlinClassInfo::default();
        apply_metadata_decode(&mut info, &elems);
        assert!(info.protobuf_decoded);
        assert_eq!(info.fq_name.as_deref(), Some("pkg.Outer.Inner"));
        assert!(info
            .local_class_names
            .iter()
            .any(|s| s.contains("Outer.Inner")));
    }

    #[test]
    fn kotlin_idiom_restore_companion_and_intrinsics() {
        let body = r#"
        kotlin.jvm.internal.Intrinsics.checkNotNullParameter(x, "x");
        Foo.Companion.INSTANCE.bar();
        Foo.DefaultImpls.qux(this, 1);
        return kotlin.Unit.INSTANCE;
"#;
        let out = restore_kotlin_idioms(body);
        assert!(!out.contains("checkNotNullParameter"), "{out}");
        assert!(out.contains("Foo.Companion.bar()"), "{out}");
        assert!(out.contains("/* DefaultImpls */ this.qux("), "{out}");
        assert!(out.contains("/* Unit */ null"), "{out}");
    }
}
