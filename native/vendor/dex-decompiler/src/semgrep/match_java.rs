//! Lightweight Semgrep-like Java pattern matcher (token + metavariable + ellipsis).
//!
//! Enough for common Android call patterns on decompiled Java; not a full Semgrep clone.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Ident(String),
    /// `$NAME` metavariable
    Meta(String),
    /// `...` ellipsis
    Ellipsis,
    Punct(char),
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

fn is_ident_cont(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

fn tokenize(src: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // Ellipsis
        if c == '.' && i + 2 < chars.len() && chars[i + 1] == '.' && chars[i + 2] == '.' {
            out.push(Tok::Ellipsis);
            i += 3;
            continue;
        }
        // String / char literal → treat as opaque Ident so `$X` can still match loosely
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut buf = String::from(quote);
            while i < chars.len() {
                let ch = chars[i];
                buf.push(ch);
                i += 1;
                if ch == '\\' && i < chars.len() {
                    buf.push(chars[i]);
                    i += 1;
                    continue;
                }
                if ch == quote {
                    break;
                }
            }
            out.push(Tok::Ident(buf));
            continue;
        }
        if is_ident_start(c) {
            let start = i;
            i += 1;
            while i < chars.len() && is_ident_cont(chars[i]) {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            if s.starts_with('$')
                && s.len() > 1
                && s.chars()
                    .nth(1)
                    .map(|x| x.is_ascii_alphabetic() || x == '_')
                    .unwrap_or(false)
            {
                out.push(Tok::Meta(s[1..].to_string()));
            } else {
                out.push(Tok::Ident(s));
            }
            continue;
        }
        // Digits as Ident
        if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
            continue;
        }
        out.push(Tok::Punct(c));
        i += 1;
    }
    out
}

/// True if `pattern` (Semgrep-ish Java snippet) matches somewhere in `java_src`.
pub fn java_matches_pattern(java_src: &str, pattern: &str) -> bool {
    let hay = TokenizedSource::new(java_src);
    let pat = PreparedPattern::new(pattern);
    pat.matches(&hay)
}

/// Pre-tokenized source (Java or XML text) for repeated pattern matching.
#[derive(Debug, Clone)]
pub struct TokenizedSource {
    tokens: Vec<Tok>,
}

impl TokenizedSource {
    pub fn new(src: &str) -> Self {
        Self {
            tokens: tokenize(src),
        }
    }
}

/// Pre-tokenized Semgrep pattern for reuse across many methods.
#[derive(Debug, Clone)]
pub struct PreparedPattern {
    tokens: Vec<Tok>,
}

impl PreparedPattern {
    pub fn new(pattern: &str) -> Self {
        Self {
            tokens: tokenize(pattern),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn matches(&self, hay: &TokenizedSource) -> bool {
        if self.tokens.is_empty() {
            return false;
        }
        for start in 0..=hay.tokens.len() {
            let mut binds = HashMap::new();
            if match_from(&hay.tokens, start, &self.tokens, 0, &mut binds).is_some() {
                return true;
            }
        }
        false
    }
}

/// `"$FOO"` / `'$FOO'` — Semgrep metavariable inside a string literal (common in XML rules).
fn quoted_meta_name(ident: &str) -> Option<&str> {
    let bytes = ident.as_bytes();
    if bytes.len() < 4 {
        return None;
    }
    let quote = bytes[0];
    if !(quote == b'"' || quote == b'\'') || bytes[bytes.len() - 1] != quote {
        return None;
    }
    let inner = &ident[1..ident.len() - 1];
    if inner.starts_with('$') && inner.len() > 1 {
        let name = &inner[1..];
        if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Some(name);
        }
    }
    None
}

fn is_quoted_string_literal(ident: &str) -> bool {
    let b = ident.as_bytes();
    b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0]
}

/// Match pattern tokens against haystack starting at `hi` / `pi`. Returns next hay index on success.
fn match_from(
    hay: &[Tok],
    hi: usize,
    pat: &[Tok],
    pi: usize,
    binds: &mut HashMap<String, String>,
) -> Option<usize> {
    if pi >= pat.len() {
        return Some(hi);
    }
    match &pat[pi] {
        Tok::Ellipsis => {
            // Match zero or more tokens until the rest of the pattern matches.
            if pi + 1 >= pat.len() {
                return Some(hay.len());
            }
            for skip in hi..=hay.len() {
                if let Some(end) = match_from(hay, skip, pat, pi + 1, binds) {
                    return Some(end);
                }
            }
            None
        }
        Tok::Meta(name) => {
            if hi >= hay.len() {
                return None;
            }
            let wildcard = name == "_";
            // Shortest-first: bind the smallest prefix so `$WV.foo()` keeps `.foo()` for the pattern.
            for end in (hi + 1)..=hay.len() {
                if !looks_like_expr_span(hay, hi, end) {
                    continue;
                }
                let text = tokens_to_text(&hay[hi..end]);
                let mut local = binds.clone();
                if !wildcard {
                    if let Some(prev) = local.get(name) {
                        if prev != &text {
                            continue;
                        }
                    } else {
                        local.insert(name.clone(), text);
                    }
                }
                if let Some(final_hi) = match_from(hay, end, pat, pi + 1, &mut local) {
                    *binds = local;
                    return Some(final_hi);
                }
            }
            None
        }
        Tok::Ident(want) => {
            if hi >= hay.len() {
                return None;
            }
            // XML/Java: `"$ARG"` matches any string literal and binds ARG.
            if let Some(meta) = quoted_meta_name(want) {
                match &hay[hi] {
                    Tok::Ident(got) if is_quoted_string_literal(got) => {
                        let wildcard = meta == "_";
                        let mut local = binds.clone();
                        if !wildcard {
                            if let Some(prev) = local.get(meta) {
                                if prev != got {
                                    return None;
                                }
                            } else {
                                local.insert(meta.to_string(), got.clone());
                            }
                        }
                        let end = match_from(hay, hi + 1, pat, pi + 1, &mut local)?;
                        *binds = local;
                        Some(end)
                    }
                    _ => None,
                }
            } else {
                match &hay[hi] {
                    Tok::Ident(got) if got == want => match_from(hay, hi + 1, pat, pi + 1, binds),
                    _ => None,
                }
            }
        }
        Tok::Punct(want) => {
            if hi >= hay.len() {
                return None;
            }
            match &hay[hi] {
                Tok::Punct(got) if got == want => match_from(hay, hi + 1, pat, pi + 1, binds),
                _ => None,
            }
        }
    }
}

/// Whether `hay[start..end]` is a plausible expression span (idents, dots, balanced parens).
fn looks_like_expr_span(hay: &[Tok], start: usize, end: usize) -> bool {
    if start >= end || end > hay.len() {
        return false;
    }
    // Must start with Ident or '('
    match &hay[start] {
        Tok::Ident(_) | Tok::Punct('(') => {}
        _ => return false,
    }
    let mut depth = 0i32;
    for t in &hay[start..end] {
        match t {
            Tok::Punct('(') => depth += 1,
            Tok::Punct(')') => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Tok::Punct(c) if depth == 0 && matches!(c, ';' | '{' | '}' | ',') => return false,
            _ => {}
        }
    }
    depth == 0
}

fn tokens_to_text(toks: &[Tok]) -> String {
    let mut buf = String::new();
    for t in toks {
        match t {
            Tok::Ident(s) => buf.push_str(s),
            Tok::Meta(s) => {
                buf.push('$');
                buf.push_str(s);
            }
            Tok::Ellipsis => buf.push_str("..."),
            Tok::Punct(c) => buf.push(*c),
        }
    }
    buf
}

/// Consume a simple expression: `foo`, `foo.bar`, `foo.bar(...)`, nested parens.
#[allow(dead_code)]
fn consume_expr(hay: &[Tok], start: usize) -> Option<(usize, String)> {
    if start >= hay.len() {
        return None;
    }
    let mut i = start;
    let mut parts = Vec::new();
    match &hay[i] {
        Tok::Ident(s) => {
            parts.push(s.clone());
            i += 1;
        }
        Tok::Punct('(') => {
            let (ni, inner) = consume_balanced(hay, i, '(', ')')?;
            parts.push(inner);
            i = ni;
        }
        _ => return None,
    }
    loop {
        if i < hay.len() {
            if let Tok::Punct('.') = hay[i] {
                parts.push(".".into());
                i += 1;
                if i >= hay.len() {
                    break;
                }
                if let Tok::Ident(s) = &hay[i] {
                    parts.push(s.clone());
                    i += 1;
                } else {
                    break;
                }
                continue;
            }
            if let Tok::Punct('(') = hay[i] {
                let (ni, inner) = consume_balanced(hay, i, '(', ')')?;
                parts.push(inner);
                i = ni;
                continue;
            }
        }
        break;
    }
    Some((i, parts.join("")))
}

fn consume_balanced(hay: &[Tok], start: usize, open: char, close: char) -> Option<(usize, String)> {
    if start >= hay.len() {
        return None;
    }
    match &hay[start] {
        Tok::Punct(c) if *c == open => {}
        _ => return None,
    }
    let mut depth = 0;
    let mut buf = String::new();
    let mut i = start;
    while i < hay.len() {
        match &hay[i] {
            Tok::Punct(c) if *c == open => {
                depth += 1;
                buf.push(*c);
            }
            Tok::Punct(c) if *c == close => {
                depth -= 1;
                buf.push(*c);
                i += 1;
                if depth == 0 {
                    return Some((i, buf));
                }
                continue;
            }
            Tok::Ident(s) => buf.push_str(s),
            Tok::Meta(s) => {
                buf.push('$');
                buf.push_str(s);
            }
            Tok::Ellipsis => buf.push_str("..."),
            Tok::Punct(c) => buf.push(*c),
        }
        // space between tokens for readability in bind text
        if !matches!(&hay[i], Tok::Punct(_)) {
            buf.push(' ');
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_add_javascript_interface() {
        let java = r#"
            public void setup(WebView wv) {
                wv.addJavascriptInterface(bridge, "Android");
            }
        "#;
        assert!(java_matches_pattern(
            java,
            "$WV.addJavascriptInterface($OBJ, $NAME);"
        ));
    }

    #[test]
    fn matches_loadurl_from_intent_extra() {
        let java = r#"
            void onCreate() {
                webView.loadUrl((String) getIntent().getStringExtra("url"));
            }
        "#;
        assert!(java_matches_pattern(
            java,
            "$WV.loadUrl((String) $INTENT.getStringExtra(...));"
        ));
    }

    #[test]
    fn matches_ssl_bypass_with_ellipsis() {
        let java = r#"
            public void onReceivedSslError(WebView v, SslErrorHandler h, SslError e) {
                Log.d("x", "bad");
                h.proceed();
            }
        "#;
        assert!(java_matches_pattern(
            java,
            r#"public void onReceivedSslError(WebView $V, SslErrorHandler $H, SslError $E) {
                ...
                $H.proceed();
                ...
            }"#
        ));
    }

    #[test]
    fn no_false_positive_partial_name() {
        let java = "void foo() { bar.addJavascriptInterfaceX(a, b); }";
        assert!(!java_matches_pattern(
            java,
            "$WV.addJavascriptInterface($OBJ, $NAME);"
        ));
    }

    #[test]
    fn matches_xml_attribute_quoted_meta() {
        let xml = r#"<application android:debuggable="true" android:allowBackup="false"/>"#;
        assert!(java_matches_pattern(xml, r#"android:debuggable="$ARG""#));
        assert!(java_matches_pattern(xml, r#"android:allowBackup="$ARG""#));
    }
}
