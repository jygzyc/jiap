//! Restore string switch from hashCode/equals chains.

use super::util::*;

/// Best-effort restore of `switch (str.hashCode())` + equals cases into `switch (str)` with string labels.
///
/// Looks for the common javac pattern:
/// ```text
/// switch (s.hashCode()) {
/// case 97:
///     if (!s.equals("a")) break;
///     ...
/// ```
/// and rewrites case labels to the string when equals is found in the case body.
/// Also folds `int h = s.hashCode(); switch (h)` and accepts `"lit".equals(s)`.
pub fn restore_string_switch(body: &str) -> String {
    if !body.contains("hashCode()") || !body.contains("switch (") {
        return body.to_string();
    }
    let mut lines: Vec<String> = body.lines().map(|l| l.to_string()).collect();
    fold_hashcode_temp_into_switch(&mut lines);
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim().to_string();
        if let Some(inner) = trimmed
            .strip_prefix("switch (")
            .and_then(|s| s.strip_suffix(") {"))
        {
            let inner = inner.trim().to_string();
            if inner.ends_with(".hashCode()") {
                let expr = inner.trim_end_matches(".hashCode()").trim().to_string();
                let indent = leading_indent(&lines[i]).to_string();
                lines[i] = format!("{}switch ({}) {{", indent, expr);
                let mut j = i + 1;
                while j < lines.len() {
                    let t = lines[j].trim().to_string();
                    if t == "}" {
                        break;
                    }
                    if let Some(rest) = t.strip_prefix("case ") {
                        let case_num = rest.trim().trim_end_matches(':').trim().to_string();
                        let case_hash = parse_java_int_literal(&case_num);
                        let mut k = j + 1;
                        let mut found: Vec<String> = Vec::new();
                        while k < lines.len() {
                            let tk = lines[k].trim().to_string();
                            if tk.starts_with("case ") || tk.starts_with("default:") || tk == "}" {
                                break;
                            }
                            if let Some(eq) = extract_string_equals(&tk, &expr) {
                                if !found.iter().any(|s| s == &eq) {
                                    found.push(eq);
                                }
                            }
                            k += 1;
                        }
                        // Prefer equals whose Java hashCode matches the case int when available.
                        let chosen = if found.len() == 1 {
                            Some(found[0].clone())
                        } else if found.len() > 1 {
                            if let Some(h) = case_hash {
                                found
                                    .iter()
                                    .find(|s| java_string_hash_code(s) == h)
                                    .cloned()
                                    .or_else(|| Some(found[0].clone()))
                            } else {
                                Some(found[0].clone())
                            }
                        } else {
                            None
                        };
                        if let Some(s) = chosen {
                            // Reject when hash is present and clearly wrong (keep numeric case).
                            let hash_ok = case_hash
                                .map(|h| java_string_hash_code(&s) == h)
                                .unwrap_or(true);
                            if hash_ok {
                                let cindent = leading_indent(&lines[j]).to_string();
                                lines[j] = format!(
                                    "{}case \"{}\": // was {}",
                                    cindent,
                                    escape_switch_str(&s),
                                    case_num
                                );
                                clear_string_equals_guards(&mut lines, j + 1, &expr);
                            }
                        }
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    // Drop blank lines left by guard removal (preserve indent structure lightly).
    let mut cleaned = Vec::with_capacity(lines.len());
    for (idx, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            let prev_empty = cleaned
                .last()
                .map(|l: &String| l.trim().is_empty())
                .unwrap_or(true);
            let next_case = lines
                .get(idx + 1)
                .map(|l| {
                    let t = l.trim();
                    t.starts_with("case ") || t.starts_with("default:") || t == "}"
                })
                .unwrap_or(false);
            if prev_empty || next_case {
                continue;
            }
        }
        cleaned.push(line.clone());
    }
    cleaned.join("\n")
}

/// Fold `int h = expr.hashCode();` / `h = expr.hashCode();` immediately before `switch (h)`.
/// Fold `int h = expr.hashCode();` / `h = expr.hashCode();` immediately before `switch (h)`.
pub(crate) fn fold_hashcode_temp_into_switch(lines: &mut [String]) {
    let mut i = 1;
    while i < lines.len() {
        let sw = lines[i].trim().to_string();
        if let Some(inner) = sw
            .strip_prefix("switch (")
            .and_then(|s| s.strip_suffix(") {"))
        {
            let disc = inner.trim();
            if is_java_ident(disc) {
                // Look back over blank lines for `… disc = expr.hashCode();`
                let mut p = i;
                while p > 0 {
                    p -= 1;
                    if lines[p].trim().is_empty() {
                        continue;
                    }
                    if let Some(expr) = parse_hashcode_assign(lines[p].trim(), disc) {
                        let indent = leading_indent(&lines[i]).to_string();
                        lines[i] = format!("{}switch ({}.hashCode()) {{", indent, expr);
                        lines[p] = String::new();
                    }
                    break;
                }
            }
        }
        i += 1;
    }
}

pub(crate) fn clear_string_equals_guards(lines: &mut [String], start: usize, expr: &str) {
    let mut k = start;
    while k < lines.len() {
        let tk = lines[k].trim().to_string();
        if tk.starts_with("case ") || tk.starts_with("default:") || tk == "}" {
            break;
        }
        let is_guard = extract_string_equals(&tk, expr).is_some()
            || (tk.contains(".equals(")
                && (tk.contains(expr) || tk.contains(&format!("\"{expr}"))));
        if is_guard && tk.starts_with("if (") {
            lines[k] = String::new();
            if k + 1 < lines.len() && lines[k + 1].trim() == "break;" {
                lines[k + 1] = String::new();
            }
        } else if tk == "break;"
            && lines
                .get(k.wrapping_sub(1))
                .map(|l| l.trim().is_empty() || l.trim().contains(".equals("))
                .unwrap_or(false)
        {
            // orphaned break after cleared multi-line if — leave alone unless previous was equals
            if lines
                .get(k.wrapping_sub(1))
                .map(|l| l.trim().contains(".equals("))
                .unwrap_or(false)
            {
                lines[k] = String::new();
            }
        }
        k += 1;
    }
}

/// Java `String.hashCode()` (31-bit polynomial over UTF-16 code units; BMP chars = Rust chars).
pub fn java_string_hash_code(s: &str) -> i32 {
    let mut h: i32 = 0;
    for c in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(c as i32);
    }
    h
}

/// Extract string literal from `recv.equals("…")` or `"…".equals(recv)` (optional `!` / `if`).
/// Extract string literal from `recv.equals("…")` or `"…".equals(recv)` (optional `!` / `if`).
pub(crate) fn extract_string_equals(line: &str, recv: &str) -> Option<String> {
    if let Some(s) = extract_equals_string(line, recv) {
        return Some(s);
    }
    // "lit".equals(recv) / !"lit".equals(recv)
    let needle = format!(".equals({recv})");
    let idx = line.find(&needle)?;
    let before = &line[..idx];
    let q = before.rfind('"')?;
    // walk back to opening quote
    let mut i = q;
    let bytes = before.as_bytes();
    while i > 0 {
        i -= 1;
        if bytes[i] == b'"' {
            // check not escaped
            let mut bs = 0;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                bs += 1;
                j -= 1;
            }
            if bs % 2 == 0 {
                let lit = &before[i + 1..q];
                return Some(unescape_java_str(lit));
            }
        }
    }
    None
}

pub(crate) fn extract_equals_string(line: &str, recv: &str) -> Option<String> {
    // Patterns: s.equals("foo") / !s.equals("foo")
    let needle = format!("{}.equals(", recv);
    let idx = line.find(&needle)?;
    let after = &line[idx + needle.len()..];
    let after = after.trim_start();
    parse_java_string_literal(after)
}

pub(crate) fn unescape_java_str(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(match n {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                });
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn escape_switch_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
