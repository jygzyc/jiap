//! decx-json — minimal JSON value / parser / serializer (std-only).
//!
//! Object preserves insertion order (matches envelope key ordering needs).
//! No serde, no external crates. Good enough for the DECX API envelope.

use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn obj(kv: Vec<(&str, Json)>) -> Json {
        Json::Obj(kv.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
    /// empty object
    pub fn obj0() -> Json {
        Json::Obj(Vec::new())
    }
    pub fn set(&mut self, key: &str, val: Json) -> &mut Self {
        if let Json::Obj(pairs) = self {
            if let Some(p) = pairs.iter_mut().find(|(k, _)| k == key) {
                p.1 = val;
            } else {
                pairs.push((key.to_string(), val));
            }
        }
        self
    }
    pub fn get(&self, key: &str) -> Option<&Json> {
        if let Json::Obj(pairs) = self {
            pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Json::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Int(i) => Some(*i),
            Json::Float(f) => Some(*f as i64),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Json::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    pub fn as_arr(&self) -> Option<&[Json]> {
        if let Json::Arr(a) = self {
            Some(a)
        } else {
            None
        }
    }
    pub fn str(s: impl Into<String>) -> Json {
        Json::Str(s.into())
    }
    pub fn int(i: i64) -> Json {
        Json::Int(i)
    }

    pub fn to_string(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Json::Float(f) => {
                if f.is_finite() {
                    if f.fract() == 0.0 && f.abs() < 1e15 {
                        let _ = write!(out, "{}", *f as i64);
                    } else {
                        let _ = write!(out, "{f}");
                    }
                } else {
                    out.push_str("null");
                }
            }
            Json::Str(s) => write_escaped(out, s),
            Json::Arr(items) => {
                out.push('[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Obj(pairs) => {
                out.push('{');
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_escaped(out, k);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_escaped(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn parse(input: &str) -> Result<Json, String> {
    let mut p = Parser {
        bytes: input.as_bytes(),
        pos: 0,
        depth: 0,
    };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(format!("trailing data at byte {}", p.pos));
    }
    Ok(v)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(b) = self.bytes.get(self.pos) {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "expected '{}' at byte {} (found {:?})",
                b as char,
                self.pos,
                self.peek().map(|c| c as char)
            ))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        if self.depth > 128 {
            return Err("nesting too deep".into());
        }
        match self.peek() {
            Some(b'{') => {
                self.depth += 1;
                self.pos += 1;
                let mut pairs = Vec::new();
                self.skip_ws();
                if self.peek() == Some(b'}') {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Json::Obj(pairs));
                }
                loop {
                    self.skip_ws();
                    let key = self.string()?;
                    self.skip_ws();
                    self.expect(b':')?;
                    self.skip_ws();
                    let val = self.value()?;
                    pairs.push((key, val));
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {
                            self.pos += 1;
                            break;
                        }
                        _ => return Err(format!("expected ',' or '}}' at byte {}", self.pos)),
                    }
                }
                self.depth -= 1;
                Ok(Json::Obj(pairs))
            }
            Some(b'[') => {
                self.depth += 1;
                self.pos += 1;
                let mut items = Vec::new();
                self.skip_ws();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Json::Arr(items));
                }
                loop {
                    self.skip_ws();
                    items.push(self.value()?);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            break;
                        }
                        _ => return Err(format!("expected ',' or ']' at byte {}", self.pos)),
                    }
                }
                self.depth -= 1;
                Ok(Json::Arr(items))
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.lit("true", Json::Bool(true)),
            Some(b'f') => self.lit("false", Json::Bool(false)),
            Some(b'n') => self.lit("null", Json::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => Err(format!("unexpected byte at {}", self.pos)),
        }
    }

    fn lit(&mut self, word: &str, val: Json) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(val)
        } else {
            Err(format!("bad literal at byte {}", self.pos))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let mut is_float = false;
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => self.pos += 1,
                b'.' | b'e' | b'E' | b'+' | b'-' => {
                    is_float = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| "bad number".to_string())?;
        if !is_float {
            if let Ok(i) = text.parse::<i64>() {
                return Ok(Json::Int(i));
            }
        }
        text.parse::<f64>()
            .map(Json::Float)
            .map_err(|_| format!("bad number '{text}'"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err("unterminated string".into());
            };
            self.pos += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(esc) = self.peek() else {
                        return Err("unterminated escape".into());
                    };
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self
                                .bytes
                                .get(self.pos..self.pos + 4)
                                .ok_or("bad \\u escape")?;
                            self.pos += 4;
                            let cp = u32::from_str_radix(
                                std::str::from_utf8(hex).map_err(|_| "bad \\u hex")?,
                                16,
                            )
                            .map_err(|_| "bad \\u hex")?;
                            // surrogate pair handling
                            if (0xD800..0xDC00).contains(&cp) {
                                if self.bytes.get(self.pos) == Some(&b'\\')
                                    && self.bytes.get(self.pos + 1) == Some(&b'u')
                                {
                                    let hex2 = self
                                        .bytes
                                        .get(self.pos + 2..self.pos + 6)
                                        .ok_or("bad surrogate")?;
                                    let low = u32::from_str_radix(
                                        std::str::from_utf8(hex2).map_err(|_| "bad surrogate")?,
                                        16,
                                    )
                                    .map_err(|_| "bad surrogate")?;
                                    self.pos += 6;
                                    let combined =
                                        0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                                    out.push(char::from_u32(combined).unwrap_or('\u{FFFD}'));
                                } else {
                                    out.push('\u{FFFD}');
                                }
                            } else {
                                out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                            }
                        }
                        _ => return Err("bad escape".into()),
                    }
                }
                c if c < 0x80 => out.push(c as char),
                // raw UTF-8 continuation
                c => {
                    let len = if c >= 0xF0 {
                        4
                    } else if c >= 0xE0 {
                        3
                    } else {
                        2
                    };
                    let start = self.pos - 1;
                    let end = start + len;
                    let slice = self.bytes.get(start..end).ok_or("bad utf8")?;
                    out.push_str(std::str::from_utf8(slice).map_err(|_| "bad utf8")?);
                    self.pos = end;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let src = r#"{"ok":true,"n":-42,"f":3.5,"s":"a\nbé","arr":[1,null,"x"],"nested":{"k":false}}"#;
        let v = parse(src).unwrap();
        let out = v.to_string();
        let v2 = parse(&out).unwrap();
        assert_eq!(v, v2);
        assert_eq!(v.get("n").unwrap().as_i64(), Some(-42));
        assert_eq!(v.get("s").unwrap().as_str().unwrap(), "a\nbé");
    }

    #[test]
    fn set_overwrites() {
        let mut o = Json::obj0();
        o.set("a", Json::int(1)).set("b", Json::str("x"));
        o.set("a", Json::int(2));
        assert_eq!(o.to_string(), r#"{"a":2,"b":"x"}"#);
    }
}
