//! Binary XML (AXML) decoder: chunk stream -> text XML.
//! Covers string pool, namespaces, start/end elements, attributes, cdata.

fn u16le(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn u32le(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

pub struct StringPool {
    strings: Vec<String>,
}

impl StringPool {
    fn parse(d: &[u8], off: usize) -> Result<StringPool, String> {
        // chunk header at off: type(2) size(2) chunkSize(4) strCount(4) ...
        let str_count = u32le(d, off + 8) as usize;
        let flags = u32le(d, off + 16);
        let utf8 = flags & (1 << 8) != 0;
        let strings_start = off as usize + u32le(d, off + 20) as usize;
        let mut strings = Vec::with_capacity(str_count);
        for i in 0..str_count {
            let idx_off = off + 28 + i * 4;
            if idx_off + 4 > d.len() {
                return Err("axml: string index oob".into());
            }
            let mut p = strings_start + u32le(d, idx_off) as usize;
            if utf8 {
                // two lengths (chars, bytes) as u8-or-u16leb
                let (_nchars, adv1) = read_len8(d, p)?;
                p += adv1;
                let (nbytes, adv2) = read_len8(d, p)?;
                p += adv2;
                let end = p + nbytes as usize;
                let s = d.get(p..end).ok_or("axml: utf8 str oob")?;
                strings.push(String::from_utf8_lossy(s).into_owned());
            } else {
                let (nchars, adv) = read_len16(d, p)?;
                p += adv;
                let bytes = nchars as usize * 2;
                let s = d.get(p..p + bytes).ok_or("axml: utf16 str oob")?;
                let mut out = String::with_capacity(nchars as usize);
                let mut i = 0usize;
                while i + 2 <= s.len() {
                    let c = u16::from_le_bytes([s[i], s[i + 1]]);
                    if c == 0 {
                        break;
                    }
                    out.push(char::from_u32(c as u32).unwrap_or('\u{FFFD}'));
                    i += 2;
                }
                strings.push(out);
            }
        }
        Ok(StringPool { strings })
    }

    pub fn get(&self, idx: i32) -> &str {
        if idx >= 0 && (idx as usize) < self.strings.len() {
            &self.strings[idx as usize]
        } else {
            ""
        }
    }
}

fn read_len8(d: &[u8], p: usize) -> Result<(u32, usize), String> {
    let b = *d.get(p).ok_or("axml: len8 oob")?;
    if b & 0x80 != 0 {
        let b2 = *d.get(p + 1).ok_or("axml: len8 oob")?;
        Ok(((((b & 0x7f) as u32) << 8) | b2 as u32, 2))
    } else {
        Ok((b as u32, 1))
    }
}

fn read_len16(d: &[u8], p: usize) -> Result<(u32, usize), String> {
    let w = u16le(d, p);
    if w & 0x8000 != 0 {
        let w2 = u16le(d, p + 2);
        Ok(((((w & 0x7fff) as u32) << 16) | w2 as u32, 4))
    } else {
        Ok((w as u32, 2))
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// Decode an AXML buffer into text XML.
pub fn decode(d: &[u8]) -> Result<String, String> {
    if d.len() < 8 || u16le(d, 0) != 0x0003 {
        return Err("axml: not a binary xml document".into());
    }
    let file_size = u32le(d, 4) as usize;
    let end = file_size.min(d.len());
    let mut pool = StringPool { strings: Vec::new() };
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let mut indent = 0usize;
    let mut ns_prefixes: Vec<(u32, String)> = Vec::new(); // (uri idx, prefix)

    let mut pos = 8usize;
    while pos + 8 <= end {
        let ctype = u16le(d, pos);
        let csize = u32le(d, pos + 4) as usize;
        if csize < 8 || pos + csize > end {
            break;
        }
        match ctype {
            0x0001 => {
                pool = StringPool::parse(d, pos)?;
            }
            0x0100 => {
                // start namespace: prefix(idx@16), uri(idx@20)
                let prefix = u32le(d, pos + 16) as i32;
                let uri = u32le(d, pos + 20) as i32;
                ns_prefixes.push((uri as u32, pool.get(prefix).to_string()));
            }
            0x0102 => {
                // start element: header(8) line(4) comment(4) ns(4) name(4) attrStart(2) attrSize(2) attrCount(2)...
                let name_idx = u32le(d, pos + 20) as i32;
                let ns_idx = u32le(d, pos + 16) as i32;
                let attr_start = u16le(d, pos + 24) as usize;
                let attr_count = u16le(d, pos + 28) as usize;
                let tag = pool.get(name_idx);
                for _ in 0..indent {
                    out.push_str("  ");
                }
                out.push('<');
                if ns_idx >= 0 {
                    if let Some((_, pfx)) = ns_prefixes.iter().rev().find(|(u, _)| *u == ns_idx as u32) {
                        out.push_str(pfx);
                        out.push(':');
                    }
                }
                out.push_str(tag);
                for a in 0..attr_count {
                    // attrExt starts at +16; attributeStart is relative to it
                    let ao = pos + 16 + attr_start + a * 20;
                    if ao + 20 > pos + csize {
                        break;
                    }
                    let ans = u32le(d, ao) as i32;
                    let aname = u32le(d, ao + 4) as i32;
                    let raw = u32le(d, ao + 8) as i32;
                    let vtype = u8::from_le_bytes([d[ao + 15]]);
                    let vdata = u32le(d, ao + 16);
                    out.push(' ');
                    if ans >= 0 {
                        if let Some((_, pfx)) = ns_prefixes.iter().rev().find(|(u, _)| *u == ans as u32) {
                            out.push_str(pfx);
                            out.push(':');
                        }
                    }
                    out.push_str(pool.get(aname));
                    out.push_str("=\"");
                    out.push_str(&xml_escape(&attr_value(&pool, raw, vtype, vdata)));
                    out.push('"');
                }
                out.push_str(">\n");
                indent += 1;
            }
            0x0103 => {
                // end element
                indent = indent.saturating_sub(1);
                let name_idx = u32le(d, pos + 20) as i32;
                let ns_idx = u32le(d, pos + 16) as i32;
                for _ in 0..indent {
                    out.push_str("  ");
                }
                out.push_str("</");
                if ns_idx >= 0 {
                    if let Some((_, pfx)) = ns_prefixes.iter().rev().find(|(u, _)| *u == ns_idx as u32) {
                        out.push_str(pfx);
                        out.push(':');
                    }
                }
                out.push_str(pool.get(name_idx));
                out.push_str(">\n");
            }
            0x0104 => {
                // cdata
                let idx = u32le(d, pos + 8) as i32;
                for _ in 0..indent {
                    out.push_str("  ");
                }
                out.push_str(&xml_escape(pool.get(idx)));
                out.push('\n');
            }
            _ => {}
        }
        pos += csize;
    }
    Ok(out)
}

fn attr_value(pool: &StringPool, raw: i32, vtype: u8, vdata: u32) -> String {
    if raw >= 0 {
        return pool.get(raw).to_string();
    }
    match vtype {
        0x03 => String::new(), // string handled by raw
        0x10 => (vdata as i32).to_string(),
        0x11 => format!("0x{vdata:x}"),
        0x12 => {
            if vdata != 0 {
                "true".into()
            } else {
                "false".into()
            }
        }
        0x01 => format!("#{}", vdata),
        0x02 => format!("0x{:x} * 0.5^{}", (vdata & 0xffffff00) >> 8, vdata & 0xff),
        _ => format!("0x{vdata:x}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_garbage() {
        assert!(super::decode(b"not-axml-at-all").is_err());
    }
}
