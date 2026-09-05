//! Minimal Android binary XML (AXML) decoder — enough for AndroidManifest.xml
//! and simple res XML files: string pools, namespaces, elements, attributes
//! with typed values, and text-XML rendering back to what jadx exposes.

pub const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";
const DEFAULT_ANDROID_PREFIX: &str = "android";

const CHUNK_STRING_POOL: u16 = 0x0001;
const CHUNK_START_NAMESPACE: u16 = 0x0100;
const CHUNK_END_NAMESPACE: u16 = 0x0101;
const CHUNK_START_ELEMENT: u16 = 0x0102;
const CHUNK_END_ELEMENT: u16 = 0x0103;
const CHUNK_CDATA: u16 = 0x0104;

pub struct AxmlError(pub String);

impl std::fmt::Display for AxmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "axml: {}", self.0)
    }
}

type Result<T> = std::result::Result<T, AxmlError>;

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(AxmlError(msg.into()))
}

fn u16_at(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn u32_at(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// Parsed string pool (UTF-16 or UTF-8).
#[derive(Debug, Clone, Default)]
pub struct StringPool {
    pub strings: Vec<String>,
}

impl StringPool {
    pub fn get(&self, idx: usize) -> Option<&str> {
        self.strings.get(idx).map(|s| s.as_str())
    }

    /// Decode a string-pool chunk starting at `off` (chunk header included).
    pub fn parse(buf: &[u8], off: usize) -> Result<StringPool> {
        if off + 28 > buf.len() {
            return err("string pool header out of bounds");
        }
        let string_count = u32_at(buf, off + 8) as usize;
        let flags = u32_at(buf, off + 16);
        let strings_start = u32_at(buf, off + 20) as usize;
        let is_utf8 = flags & 0x100 != 0;
        let data_base = off + strings_start;
        let mut strings = Vec::with_capacity(string_count.min(1 << 20));
        for i in 0..string_count {
            let off_pos = off + 28 + i * 4;
            if off_pos + 4 > buf.len() {
                strings.push(String::new());
                continue;
            }
            let str_off = data_base + u32_at(buf, off_pos) as usize;
            strings.push(if is_utf8 {
                read_utf8(buf, str_off)
            } else {
                read_utf16(buf, str_off)
            });
        }
        Ok(StringPool { strings })
    }
}

fn read_utf16(buf: &[u8], mut pos: usize) -> String {
    if pos + 2 > buf.len() {
        return String::new();
    }
    let mut len = u16_at(buf, pos) as usize;
    pos += 2;
    if len & 0x8000 != 0 {
        if pos + 2 > buf.len() {
            return String::new();
        }
        len = ((len & 0x7FFF) << 16) | u16_at(buf, pos) as usize;
        pos += 2;
    }
    let mut units = Vec::with_capacity(len.min(1 << 20));
    for _ in 0..len {
        if pos + 2 > buf.len() {
            break;
        }
        units.push(u16_at(buf, pos));
        pos += 2;
    }
    String::from_utf16_lossy(&units)
}

fn read_utf8(buf: &[u8], pos: usize) -> String {
    // UTF-8 string-pool entries: two length prefixes — charLen and byteLen —
    // each encoded as one byte below 0x80 or two bytes ((len >> 8) | 0x80,
    // len & 0x7F) — followed by the UTF-8 bytes and a NUL terminator.
    let (char_len, pos) = read_len8(buf, pos);
    let _ = char_len; // the byte length below is what bounds the slice
    let (byte_len, pos) = read_len8(buf, pos);
    let end = (pos + byte_len).min(buf.len());
    String::from_utf8_lossy(&buf[pos..end]).into_owned()
}

fn read_len8(buf: &[u8], mut pos: usize) -> (usize, usize) {
    if pos >= buf.len() {
        return (0, pos);
    }
    let mut n = buf[pos] as usize;
    pos += 1;
    if n & 0x80 != 0 {
        n = ((n & 0x7F) << 8) | buf.get(pos).copied().unwrap_or(0) as usize;
        pos += 1;
    }
    (n, pos)
}

/// Typed attribute value (android Res_value).
#[derive(Debug, Clone, Copy)]
pub struct TypedValue {
    pub data_type: u8,
    pub data: u32,
}

impl TypedValue {
    pub fn as_bool(&self) -> Option<bool> {
        (self.data_type == 0x12).then(|| self.data != 0)
    }

    pub fn as_int(&self) -> Option<i32> {
        matches!(self.data_type, 0x10 | 0x11).then_some(self.data as i32)
    }
}

/// Text form of a typed value used when no raw string exists.
pub fn typed_value_text(tv: &TypedValue) -> String {
    match tv.data_type {
        0x03 => format!("@string/{}", tv.data),
        0x10 => format!("{}", tv.data as i32),
        0x11 => format!("0x{:x}", tv.data),
        0x12 => if tv.data != 0 { "true".into() } else { "false".into() },
        0x01 => format!("@0x{:08x}", tv.data),
        0x02 => format!("?0x{:08x}", tv.data),
        0x04 => format!("{}", f32::from_bits(tv.data)),
        _ => format!("0x{:08x}", tv.data),
    }
}

#[derive(Debug, Clone)]
pub struct XmlAttr {
    pub ns: Option<String>,
    pub prefix: Option<String>,
    pub name: String,
    /// Raw string value (string-pool reference) when present.
    pub raw: Option<String>,
    /// Decoded typed value.
    pub typed: TypedValue,
}

#[derive(Debug)]
pub struct XmlNode {
    pub line: u32,
    pub name: String,
    pub ns: Option<String>,
    pub prefix: Option<String>,
    pub attrs: Vec<XmlAttr>,
    pub children: Vec<XmlNode>,
}

impl XmlNode {
    fn attr(&self, ns: Option<&str>, name: &str) -> Option<&XmlAttr> {
        self.attrs.iter().find(|a| {
            a.name == name
                && match (ns, &a.ns) {
                    (Some(want), Some(got)) => want == got,
                    (None, None) => true,
                    _ => false,
                }
        })
    }

    /// Attribute value as text: raw string when present, else typed decode.
    pub fn attr_str(&self, ns: Option<&str>, name: &str) -> Option<String> {
        self.attr(ns, name).map(|a| {
            a.raw
                .clone()
                .unwrap_or_else(|| typed_value_text(&a.typed))
        })
    }

    /// String value resolved against a string pool (type 0x03 → pool lookup).
    pub fn attr_str_resolved(&self, ns: Option<&str>, name: &str, pool: &StringPool) -> Option<String> {
        self.attr(ns, name).map(|a| match (&a.raw, a.typed.data_type) {
            (Some(raw), _) => raw.clone(),
            (None, 0x03) => pool.get(a.typed.data as usize).unwrap_or("").to_string(),
            _ => typed_value_text(&a.typed),
        })
    }

    pub fn attr_bool(&self, ns: Option<&str>, name: &str) -> Option<bool> {
        self.attr(ns, name).and_then(|a| a.typed.as_bool())
    }

    pub fn attr_int(&self, ns: Option<&str>, name: &str) -> Option<i32> {
        self.attr(ns, name).and_then(|a| a.typed.as_int())
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlNode> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    pub fn first_child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.name == name)
    }
}

/// Full AXML document: tree + string pool.
pub struct AxmlDocument {
    pub pool: StringPool,
    pub root: XmlNode,
}

struct NsScope {
    prefix: String,
    uri: String,
}

struct Parser<'a> {
    buf: &'a [u8],
    pool: StringPool,
}

impl<'a> Parser<'a> {
    fn pool_str(&self, idx: u32) -> String {
        self.pool.get(idx as usize).unwrap_or("").to_string()
    }

    fn resolve_ns_prefix(scopes: &[NsScope], uri: &str) -> Option<String> {
        scopes
            .iter()
            .rev()
            .find(|s| s.uri == uri)
            .map(|s| s.prefix.clone())
            .or(if uri == ANDROID_NS {
                Some(DEFAULT_ANDROID_PREFIX.to_string())
            } else {
                None
            })
    }

    /// Parse one START_ELEMENT chunk into a node.
    fn parse_element(&mut self, off: usize, scopes: &[NsScope]) -> Result<XmlNode> {
        if off + 36 > self.buf.len() {
            return err("start element out of bounds");
        }
        let line = u32_at(self.buf, off + 8);
        let ns_idx = u32_at(self.buf, off + 16);
        let name = self.pool_str(u32_at(self.buf, off + 20));
        let attr_start = u16_at(self.buf, off + 24) as usize; // relative to attrExt (@16)
        let attr_size = u16_at(self.buf, off + 26) as usize;
        let attr_count = u16_at(self.buf, off + 28) as usize;
        let (ns, prefix) = if ns_idx == 0xFFFF_FFFF {
            (None, None)
        } else {
            let uri = self.pool_str(ns_idx);
            let prefix = Self::resolve_ns_prefix(scopes, &uri);
            (Some(uri), prefix)
        };
        let mut node = XmlNode {
            line,
            name,
            ns,
            prefix,
            attrs: Vec::with_capacity(attr_count.min(256)),
            children: Vec::new(),
        };
        let base = off + 16 + attr_start;
        for i in 0..attr_count {
            let a = base + i * attr_size.max(20);
            if a + 20 > self.buf.len() {
                break;
            }
            let ans_idx = u32_at(self.buf, a);
            let aname = self.pool_str(u32_at(self.buf, a + 4));
            let raw_idx = u32_at(self.buf, a + 8);
            let data_type = self.buf[a + 15];
            let data = u32_at(self.buf, a + 16);
            let (ans, aprefix) = if ans_idx == 0xFFFF_FFFF {
                (None, None)
            } else {
                let uri = self.pool_str(ans_idx);
                let prefix = Self::resolve_ns_prefix(scopes, &uri);
                (Some(uri), prefix)
            };
            let raw = (raw_idx != 0xFFFF_FFFF).then(|| self.pool_str(raw_idx));
            node.attrs.push(XmlAttr {
                ns: ans,
                prefix: aprefix,
                name: aname,
                raw,
                typed: TypedValue { data_type, data },
            });
        }
        Ok(node)
    }
}

/// Parse an AXML buffer into a document.
pub fn parse(buf: &[u8]) -> Result<AxmlDocument> {
    if buf.len() < 8 {
        return err("buffer too small");
    }
    // First pass: locate the string pool (first chunk after the file header).
    // File header: u16 type, u16 headerSize, u32 size.
    let mut pool: Option<StringPool> = None;
    let mut off = u16_at(buf, 2) as usize;
    while off + 8 <= buf.len() {
        let ctype = u16_at(buf, off);
        let csize = u32_at(buf, off + 4) as usize;
        if ctype == CHUNK_STRING_POOL && pool.is_none() {
            pool = Some(StringPool::parse(buf, off)?);
        }
        if csize == 0 {
            break;
        }
        off += csize;
    }
    let pool = pool.ok_or_else(|| AxmlError("no string pool".into()))?;

    let mut parser = Parser { buf, pool };
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    let mut ns_scopes: Vec<NsScope> = Vec::new();

    let mut off = u16_at(buf, 2) as usize;
    while off + 8 <= buf.len() {
        let ctype = u16_at(buf, off);
        let csize = u32_at(buf, off + 4) as usize;
        match ctype {
            CHUNK_START_ELEMENT => {
                let node = parser.parse_element(off, &ns_scopes)?;
                stack.push(node);
            }
            CHUNK_END_ELEMENT => {
                if let Some(node) = stack.pop() {
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(node),
                        None => root = Some(node),
                    }
                }
            }
            CHUNK_START_NAMESPACE => {
                if off + 24 <= buf.len() {
                    let prefix = parser.pool_str(u32_at(buf, off + 16));
                    let uri = parser.pool_str(u32_at(buf, off + 20));
                    ns_scopes.push(NsScope { prefix, uri });
                }
            }
            CHUNK_END_NAMESPACE => {
                ns_scopes.pop();
            }
            CHUNK_CDATA => {
                // text nodes — not needed for manifest analysis
            }
            _ => {}
        }
        if csize == 0 {
            break;
        }
        off += csize;
    }

    // Unbalanced document (truncated manifest): keep the outermost open element.
    while let Some(node) = stack.pop() {
        match stack.last_mut() {
            Some(parent) => parent.children.push(node),
            None => root = Some(node),
        }
    }

    let root = root.ok_or_else(|| AxmlError("no root element".into()))?;
    Ok(AxmlDocument { pool: parser.pool, root })
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render an AXML tree to indented text XML, emitting `xmlns:` declarations on
/// the root for every namespace used in the document.
pub fn to_text_xml(doc: &AxmlDocument) -> String {
    let mut used: Vec<(String, String)> = Vec::new(); // (prefix, uri)
    collect_ns(&doc.root, &mut used);
    let mut out = String::new();
    render_node(&doc.root, 0, &doc.pool, Some(&used), &mut out);
    out
}

fn collect_ns(node: &XmlNode, used: &mut Vec<(String, String)>) {
    let mut push = |prefix: &str, uri: &str| {
        if !prefix.is_empty() && !used.iter().any(|(p, _)| p == prefix) {
            used.push((prefix.to_string(), uri.to_string()));
        }
    };
    if let (Some(p), Some(u)) = (&node.prefix, &node.ns) {
        push(p, u);
    }
    for attr in &node.attrs {
        if let (Some(p), Some(u)) = (&attr.prefix, &attr.ns) {
            push(p, u);
        }
    }
    for child in &node.children {
        collect_ns(child, used);
    }
}

fn render_node(node: &XmlNode, depth: usize, pool: &StringPool, root_ns: Option<&[(String, String)]>, out: &mut String) {
    let indent = "    ".repeat(depth);
    out.push_str(&indent);
    out.push('<');
    push_qualified(out, node.prefix.as_deref(), &node.name);
    if depth == 0 {
        if let Some(decls) = root_ns {
            for (prefix, uri) in decls {
                out.push_str(&format!(" xmlns:{prefix}=\"{}\"", xml_escape(uri)));
            }
        }
    }
    for attr in &node.attrs {
        out.push(' ');
        push_qualified(out, attr.prefix.as_deref(), &attr.name);
        out.push_str("=\"");
        let value = match (&attr.raw, attr.typed.data_type) {
            (Some(raw), _) => raw.clone(),
            (None, 0x03) => pool.get(attr.typed.data as usize).unwrap_or("").to_string(),
            _ => typed_value_text(&attr.typed),
        };
        out.push_str(&xml_escape(&value));
        out.push('"');
    }
    if node.children.is_empty() {
        out.push_str(" />\n");
        return;
    }
    out.push_str(">\n");
    for child in &node.children {
        render_node(child, depth + 1, pool, None, out);
    }
    out.push_str(&indent);
    out.push_str("</");
    push_qualified(out, node.prefix.as_deref(), &node.name);
    out.push_str(">\n");
}

fn push_qualified(out: &mut String, prefix: Option<&str>, name: &str) {
    if let Some(p) = prefix {
        out.push_str(p);
        out.push(':');
    }
    out.push_str(name);
}
