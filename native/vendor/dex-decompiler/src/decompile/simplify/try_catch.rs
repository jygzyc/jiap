//! Try/catch/finally, do-while, and enum switch-map restoration.

use std::collections::{HashMap, HashSet};

use super::util::*;

/// Strip `Objects.requireNonNull` / Kotlin `Intrinsics.checkNotNull*` wrappers.
pub(crate) fn strip_require_non_null(body: &str) -> String {
    let mut out = String::new();
    let line_count = body.lines().count();
    for (idx, line) in body.lines().enumerate() {
        let binding = strip_trailing_comment(line);
        let stmt = binding.trim();
        let comment = line.get(binding.len()..).unwrap_or("");
        let indent = leading_indent(line);

        // Standalone call statement → drop
        if stmt.ends_with(';') {
            let expr = stmt.trim_end_matches(';').trim();
            if strip_null_check_expr(expr).is_some() {
                continue;
            }
        }

        // Assign: x = Objects.requireNonNull(y); → x = y;
        if let Some((var, rhs)) = parse_simple_assign_line(line) {
            if let Some(inner) = strip_null_check_expr(&rhs) {
                let lhs = {
                    let s = binding.trim();
                    s.split(" = ").next().unwrap_or(&var).trim()
                };
                out.push_str(&format!("{}{} = {};{}", indent, lhs, inner, comment));
                if idx < line_count.saturating_sub(1) {
                    out.push('\n');
                }
                continue;
            }
        }

        out.push_str(line);
        if idx < line_count.saturating_sub(1) {
            out.push('\n');
        }
    }
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn parse_catch_header(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let rest = t.strip_prefix("} catch (")?;
    let end = rest.find(") {")?;
    let inside = rest[..end].trim();
    let var = inside.split_whitespace().last()?.to_string();
    if !is_java_ident(&var) {
        return None;
    }
    let types = inside[..inside.len() - var.len()].trim().to_string();
    if types.is_empty() {
        return None;
    }
    Some((types, var))
}

/// Peel identical trailing cleanup from try + catch bodies into a single `finally`.
///
/// jadx-style duplicate-path finally: when every catch (and the try) ends with the same
/// non-empty cleanup statements, emit them once in `finally` (unless a `finally` already exists).
/// Peel identical trailing cleanup from try + catch bodies into a single `finally`.
///
/// jadx-style duplicate-path finally: when every catch (and the try) ends with the same
/// non-empty cleanup statements, emit them once in `finally` (unless a `finally` already exists).
pub fn merge_duplicate_finally(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = merge_duplicate_finally_once(&current);
        if next == current {
            return current;
        }
        current = next;
    }
    current
}

pub(crate) fn merge_duplicate_finally_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() < 6 {
        return body.to_string();
    }
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t == "try {" || t.ends_with(" try {") {
            if let Some(transformed) = try_merge_finally_at(&lines, i) {
                return transformed;
            }
        }
        i += 1;
    }
    body.to_string()
}

pub(crate) fn try_merge_finally_at(lines: &[&str], try_line: usize) -> Option<String> {
    let try_indent = leading_indent(lines[try_line]).to_string();
    let mut depth = 0i32;
    let try_body_start = try_line + 1;
    let mut try_body_end = None;
    let mut catches: Vec<(usize, usize)> = Vec::new();
    let mut j = try_line;
    let mut catch_body_start = None;
    while j < lines.len() {
        let t = lines[j].trim();
        let opens = t.chars().filter(|c| *c == '{').count() as i32;
        let closes = t.chars().filter(|c| *c == '}').count() as i32;
        if j == try_line {
            depth = 1;
            j += 1;
            continue;
        }
        if depth == 1 && t.starts_with("} catch (") {
            if let Some(cs) = catch_body_start {
                catches.push((cs, j));
            } else {
                try_body_end = Some(j);
            }
            catch_body_start = Some(j + 1);
            j += 1;
            continue;
        }
        if depth == 1 && (t == "} finally {" || t.starts_with("} finally {")) {
            return None;
        }
        if depth == 1 && t == "}" && catch_body_start.is_some() {
            if let Some(cs) = catch_body_start {
                catches.push((cs, j));
            }
            let try_end = try_body_end?;
            if catches.is_empty() {
                return None;
            }
            return peel_common_suffix(
                lines,
                try_line,
                &try_indent,
                try_body_start,
                try_end,
                &catches,
                j,
            );
        }
        depth += opens - closes;
        if depth <= 0 {
            return None;
        }
        j += 1;
    }
    None
}

pub(crate) fn peel_common_suffix(
    lines: &[&str],
    try_line: usize,
    try_indent: &str,
    try_body_start: usize,
    try_body_end: usize,
    catches: &[(usize, usize)],
    close_line: usize,
) -> Option<String> {
    let try_stmts = collect_top_level_stmts(lines, try_body_start, try_body_end);
    if try_stmts.is_empty() {
        return None;
    }
    let catch_stmts: Vec<Vec<String>> = catches
        .iter()
        .map(|&(s, e)| collect_top_level_stmts(lines, s, e))
        .collect();
    if catch_stmts.iter().any(|c| c.is_empty()) {
        return None;
    }
    let mut suffix_len = 0usize;
    let max_possible = try_stmts
        .len()
        .min(catch_stmts.iter().map(|c| c.len()).min().unwrap_or(0));
    while suffix_len < max_possible {
        let ti = try_stmts.len() - 1 - suffix_len;
        let want = try_stmts[ti].trim();
        if want.is_empty()
            || want == "return;"
            || want.starts_with("return ")
            || want.starts_with("throw ")
            || want.starts_with("if (")
            || want.starts_with("for ")
            || want.starts_with("while ")
            || want.starts_with("switch ")
        {
            break;
        }
        let ok = catch_stmts.iter().all(|c| {
            let ci = c.len() - 1 - suffix_len;
            c[ci].trim() == want
        });
        if !ok {
            break;
        }
        suffix_len += 1;
    }
    if suffix_len == 0 {
        return None;
    }
    let suffix = &try_stmts[try_stmts.len() - suffix_len..];
    let cleanup_ok = suffix.iter().any(|s| {
        let t = s.trim();
        t.contains(".close(")
            || t.contains(".unlock(")
            || t.contains(".recycle(")
            || t.contains(".release(")
            || t.contains(".disconnect(")
            || t.contains(".shutdown(")
    }) || suffix.iter().all(|s| {
        let t = s.trim().trim_end_matches(';');
        t.contains('.') && !t.contains('=') && !t.starts_with("throw")
    });
    if !cleanup_ok {
        return None;
    }

    let mut out: Vec<String> = lines[..try_line].iter().map(|s| (*s).to_string()).collect();
    out.push(lines[try_line].to_string());
    let try_keep = try_stmts.len() - suffix_len;
    for s in &try_stmts[..try_keep] {
        out.push(s.clone());
    }
    for &(cs, ce) in catches {
        let catch_hdr = cs - 1;
        out.push(lines[catch_hdr].to_string());
        let stmts = collect_top_level_stmts(lines, cs, ce);
        let keep = stmts.len() - suffix_len;
        for s in &stmts[..keep] {
            out.push(s.clone());
        }
    }
    let finally_indent = format!("{try_indent}    ");
    out.push(format!("{try_indent}}} finally {{"));
    for s in suffix {
        out.push(format!("{finally_indent}{}", s.trim()));
    }
    out.push(format!("{try_indent}}}"));
    for line in lines.iter().skip(close_line + 1) {
        out.push((*line).to_string());
    }
    Some(out.join("\n"))
}

/// Merge consecutive identical catch bodies into multi-catch `A | B`.
pub(crate) fn merge_multi_catch(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = merge_multi_catch_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn merge_multi_catch_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some((types_a, var_a)) = parse_catch_header(lines[i]) else {
            continue;
        };
        let Some(close_a) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        // `} catch (B …) {` often closes the previous catch on the same line.
        let (next_catch, body_a) = if parse_catch_header(lines[close_a]).is_some() {
            (close_a, lines[i + 1..close_a].to_vec())
        } else {
            let mut j = close_a + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j >= lines.len() || parse_catch_header(lines[j]).is_none() {
                continue;
            }
            (j, lines[i + 1..close_a].to_vec())
        };
        let Some((types_b, var_b)) = parse_catch_header(lines[next_catch]) else {
            continue;
        };
        if var_a != var_b {
            continue;
        }
        let Some(close_b) = find_closing_brace_line(&lines, next_catch) else {
            continue;
        };
        let (body_b, consume_end) = if parse_catch_header(lines[close_b]).is_some() {
            (lines[next_catch + 1..close_b].to_vec(), close_b - 1)
        } else {
            (lines[next_catch + 1..close_b].to_vec(), close_b)
        };
        let norm = |v: &[&str]| -> Vec<String> {
            v.iter()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        };
        if norm(&body_a) != norm(&body_b) {
            continue;
        }
        let indent = leading_indent(lines[i]);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                out.push_str(&format!(
                    "{}}} catch ({} | {} {}) {{\n",
                    indent, types_a, types_b, var_a
                ));
                for l in &body_a {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if consume_end < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > i && idx <= consume_end {
                continue;
            }
            out.push_str(line);
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
        }
        return out;
    }
    body.to_string()
}

/// Text-level do-while: `while (true) { BODY; if (!cond) break; }` → `do { BODY } while (cond);`
/// Text-level do-while: `while (true) { BODY; if (!cond) break; }` → `do { BODY } while (cond);`
pub(crate) fn restore_do_while_text(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..4 {
        let next = restore_do_while_text_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

/// Parse `if (cond) break;` (single-line) → Some(cond).
/// Parse `if (cond) break;` (single-line) → Some(cond).
pub(crate) fn parse_if_break_oneliner(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix("if (")?;
    // Find matching `)` before ` break;`
    let mut depth = 0i32;
    let mut close = None;
    for (i, c) in rest.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    close = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let close = close?;
    let cond = rest[..close].trim();
    let after = rest[close + 1..].trim();
    if after == "break;" {
        Some(cond.to_string())
    } else {
        None
    }
}

pub(crate) fn invert_while_cond(raw_cond: &str) -> String {
    if let Some(inner) = strip_outer_not_parens(raw_cond) {
        inner
    } else if let Some(inner) = raw_cond.strip_prefix('!').map(str::trim) {
        if is_java_ident(inner) {
            inner.to_string()
        } else {
            format!("!({})", raw_cond)
        }
    } else {
        format!("!({})", raw_cond)
    }
}

pub(crate) fn restore_do_while_text_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        if lines[i].trim() != "while (true) {" {
            continue;
        }
        let Some(while_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        if while_close <= i + 1 {
            continue;
        }
        // Find last non-empty line before while_close.
        let mut last = while_close;
        while last > i + 1 && lines[last - 1].trim().is_empty() {
            last -= 1;
        }
        let last_content = last - 1;
        if last_content <= i {
            continue;
        }

        // Pattern A: single-line `if (cond) break;` as last statement.
        if let Some(raw_cond) = parse_if_break_oneliner(lines[last_content]) {
            let while_cond = invert_while_cond(&raw_cond);
            return emit_do_while(
                &lines,
                body,
                i,
                while_close,
                last_content,
                last_content,
                &while_cond,
            );
        }

        // Pattern B/C: trailing `if (…) { … }` block ending at last_content.
        let if_close = last_content;
        if lines[if_close].trim() != "}" {
            continue;
        }
        let mut depth = 1i32;
        let mut if_idx = None;
        for t in (i + 1..if_close).rev() {
            let tr = lines[t].trim();
            depth += tr.matches('}').count() as i32;
            depth -= tr.matches('{').count() as i32;
            if depth == 0 {
                if parse_if_condition(lines[t]).is_some() {
                    if_idx = Some(t);
                }
                break;
            }
        }
        let Some(if_idx) = if_idx else {
            continue;
        };
        let Some(inner_close) = find_if_chain_end(&lines, if_idx) else {
            continue;
        };
        if inner_close != if_close {
            continue;
        }
        let Some(raw_cond) = parse_if_condition(lines[if_idx]) else {
            continue;
        };

        // Analyze then / else for break-only exit.
        let (then_start, then_end, else_start, else_end) =
            split_if_then_else(&lines, if_idx, inner_close);
        let then_body: Vec<&str> = lines[then_start..then_end]
            .iter()
            .copied()
            .filter(|l| !l.trim().is_empty())
            .collect();
        let else_body: Vec<&str> = if let (Some(es), Some(ee)) = (else_start, else_end) {
            lines[es..ee]
                .iter()
                .copied()
                .filter(|l| !l.trim().is_empty())
                .collect()
        } else {
            Vec::new()
        };

        let while_cond = if then_body.len() == 1
            && then_body[0].trim() == "break;"
            && else_body.is_empty()
        {
            // `if (cond) { break; }` → while (!cond)
            invert_while_cond(&raw_cond)
        } else if then_body.is_empty() && else_body.len() == 1 && else_body[0].trim() == "break;" {
            // `if (cond) { } else { break; }` → while (cond)
            raw_cond
        } else {
            continue;
        };

        return emit_do_while(&lines, body, i, while_close, if_idx, if_close, &while_cond);
    }
    body.to_string()
}

/// Returns (then_start, then_end, else_start?, else_end?) exclusive ranges inside if.
/// Returns (then_start, then_end, else_start?, else_end?) exclusive ranges inside if.
pub(crate) fn split_if_then_else(
    lines: &[&str],
    if_idx: usize,
    if_close: usize,
) -> (usize, usize, Option<usize>, Option<usize>) {
    // Look for `} else {` between if_idx and if_close.
    let mut depth = 0i32;
    for t in if_idx + 1..if_close {
        let tr = lines[t].trim();
        if depth == 0 && (tr == "} else {" || tr.starts_with("} else {")) {
            return (if_idx + 1, t, Some(t + 1), Some(if_close));
        }
        depth += tr.matches('{').count() as i32;
        depth -= tr.matches('}').count() as i32;
    }
    (if_idx + 1, if_close, None, None)
}

pub(crate) fn emit_do_while(
    lines: &[&str],
    body: &str,
    while_i: usize,
    while_close: usize,
    drop_from: usize,
    drop_to: usize,
    while_cond: &str,
) -> String {
    let indent = leading_indent(lines[while_i]);
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx == while_i {
            out.push_str(&format!("{}do {{\n", indent));
            for l in &lines[while_i + 1..drop_from] {
                out.push_str(l);
                out.push('\n');
            }
            out.push_str(&format!("{}}} while ({});", indent, while_cond));
            if while_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
            continue;
        }
        if idx > while_i && idx <= while_close {
            continue;
        }
        let _ = (drop_from, drop_to); // drops covered by while_close skip
        out.push_str(line);
        if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// Restore enum switches from `$SwitchMap$…[e.ordinal()]` / `e.ordinal()` forms.
/// Restore enum switches from `$SwitchMap$…[e.ordinal()]` / `e.ordinal()` forms.
pub(crate) fn restore_enum_switchmap(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = restore_enum_switchmap_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current = restore_enum_switchmap_cases(&current);
    current
}

pub(crate) fn parse_switch_header(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix("switch (")?;
    if !rest.ends_with(") {") {
        return None;
    }
    Some(rest[..rest.len() - 3].trim().to_string())
}

/// `…$SwitchMap$…[e.ordinal()]` → `e`
/// `…$SwitchMap$…[e.ordinal()]` → `e`
pub(crate) fn extract_switchmap_indexed_ordinal(expr: &str) -> Option<String> {
    let e = expr.trim();
    if !e.contains("$SwitchMap$") {
        return None;
    }
    let open = e.rfind('[')?;
    let close = e.rfind(']')?;
    if close != e.len() - 1 || close <= open {
        return None;
    }
    let before = e[..open].trim();
    if !before.contains("$SwitchMap$") {
        return None;
    }
    let index = e[open + 1..close].trim();
    let enum_expr = index.strip_suffix(".ordinal()")?.trim();
    if enum_expr.is_empty() {
        return None;
    }
    Some(enum_expr.to_string())
}

/// Extract `$SwitchMap$…` field key from an expression/assign RHS.
/// Extract `$SwitchMap$…` field key from an expression/assign RHS.
pub(crate) fn extract_switchmap_field_key(expr: &str) -> Option<String> {
    let e = expr.trim();
    let idx = e.find("$SwitchMap$")?;
    let rest = &e[idx..];
    let end = rest
        .find(|c: char| c == '[' || c == ';' || c == ' ' || c == ')' || c == ',')
        .unwrap_or(rest.len());
    let key = rest[..end].trim();
    if key.starts_with("$SwitchMap$") {
        Some(key.to_string())
    } else {
        None
    }
}

/// `$SwitchMap$com$example$Color` → (`com.example.Color`, `Color`)
/// `$SwitchMap$com$example$Color` → (`com.example.Color`, `Color`)
pub(crate) fn enum_names_from_switchmap_key(key: &str) -> Option<(String, String)> {
    let rest = key.strip_prefix("$SwitchMap$")?;
    if rest.is_empty() {
        return None;
    }
    let fq = rest.replace('$', ".");
    let simple = fq.rsplit('.').next()?.to_string();
    Some((fq, simple))
}

/// Map `(switchmap_field_key, slot) → (enum_type_simple_or_fq, const_name)`.
/// Map `(switchmap_field_key, slot) → (enum_type_simple_or_fq, const_name)`.
pub(crate) fn parse_switchmap_assignments(
    body: &str,
) -> std::collections::HashMap<(String, i32), (String, String)> {
    let mut map = std::collections::HashMap::new();
    // Track local vars assigned from a SwitchMap field.
    let mut local_to_key: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for line in body.lines() {
        let binding = strip_trailing_comment(line);
        let stmt = binding.trim();
        if let Some((var, rhs)) = parse_simple_assign_line(line) {
            if let Some(key) = extract_switchmap_field_key(&rhs) {
                if !rhs.contains('[') {
                    local_to_key.insert(var, key);
                }
            }
        }
        // `…$SwitchMap$…[Color.RED.ordinal()] = 1;` or `map[Color.RED.ordinal()] = 1;`
        if !stmt.ends_with(';') || !stmt.contains(".ordinal()]") {
            continue;
        }
        let eq = match stmt.rfind(" = ") {
            Some(p) => p,
            None => continue,
        };
        let lhs = stmt[..eq].trim();
        let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
        let Ok(slot) = rhs.parse::<i32>() else {
            continue;
        };
        let open = match lhs.rfind('[') {
            Some(p) => p,
            None => continue,
        };
        let close = match lhs.rfind(']') {
            Some(p) => p,
            None => continue,
        };
        if close != lhs.len() - 1 {
            continue;
        }
        let map_expr = lhs[..open].trim();
        let index = lhs[open + 1..close].trim();
        let enum_ord = match index.strip_suffix(".ordinal()") {
            Some(e) => e.trim(),
            None => continue,
        };
        // Color.RED or com.example.Color.GREEN
        let (enum_ty, const_name) = match enum_ord.rsplit_once('.') {
            Some((ty, name)) if is_java_ident(name) => (ty.to_string(), name.to_string()),
            _ => continue,
        };
        let key = if let Some(k) = extract_switchmap_field_key(map_expr) {
            k
        } else if is_java_ident(map_expr) {
            match local_to_key.get(map_expr) {
                Some(k) => k.clone(),
                None => continue,
            }
        } else {
            continue;
        };
        map.insert((key, slot), (enum_ty, const_name));
    }
    map
}

/// After switch selector restore, rewrite `case N:` using SwitchMap slot assignments.
/// After switch selector restore, rewrite `case N:` using SwitchMap slot assignments.
pub(crate) fn restore_enum_switchmap_cases(body: &str) -> String {
    let assignments = parse_switchmap_assignments(body);
    if assignments.is_empty() {
        return body.to_string();
    }
    let lines: Vec<&str> = body.lines().collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let Some(expr) = parse_switch_header(lines[i]) else {
            out_lines.push(lines[i].to_string());
            i += 1;
            continue;
        };
        // Find SwitchMap key associated with this switch (from prior map assign or leftover).
        let mut switchmap_key: Option<String> = None;
        // Prefer key from assignments that mention enums matching expr type — scan nearby.
        for j in (0..i).rev() {
            let t = lines[j].trim();
            if t.is_empty() {
                continue;
            }
            if let Some((var, rhs)) = parse_simple_assign_line(lines[j]) {
                if let Some(key) = extract_switchmap_field_key(&rhs) {
                    if !rhs.contains('[') {
                        // `int[] map = …$SwitchMap$…` — only useful if switch used map (already rewritten).
                        let _ = var;
                        switchmap_key = Some(key);
                        break;
                    }
                }
            }
            if let Some(key) = extract_switchmap_field_key(t) {
                switchmap_key = Some(key);
                break;
            }
            break;
        }
        // If body still has assignment lines for a key, use the key that has slots.
        if switchmap_key.is_none() {
            // Pick any key present in assignments (typical: one SwitchMap per method).
            if let Some(((k, _), _)) = assignments.iter().next() {
                switchmap_key = Some(k.clone());
            }
        }
        let key = switchmap_key.unwrap_or_default();
        let enum_simple = enum_names_from_switchmap_key(&key).map(|(_, s)| s);
        let indent = leading_indent(lines[i]);
        out_lines.push(format!("{}switch ({}) {{", indent, expr));
        let Some(sw_close) = find_closing_brace_line(&lines, i) else {
            i += 1;
            continue;
        };
        for j in i + 1..sw_close {
            let t = lines[j].trim();
            if let Some(rest) = t.strip_prefix("case ") {
                if let Some(num_s) = rest.strip_suffix(':') {
                    if let Ok(n) = num_s.trim().parse::<i32>() {
                        if let Some((enum_ty, const_name)) = assignments.get(&(key.clone(), n)) {
                            let label = match &enum_simple {
                                Some(s)
                                    if enum_ty == s || enum_ty.ends_with(&format!(".{}", s)) =>
                                {
                                    format!("{}.{}", s, const_name)
                                }
                                Some(s) if enum_ty.rsplit('.').next() == Some(s.as_str()) => {
                                    format!("{}.{}", s, const_name)
                                }
                                _ => {
                                    // Prefer simple name when ty is already simple.
                                    if enum_ty.contains('.') {
                                        format!("{}.{}", enum_ty, const_name)
                                    } else {
                                        format!("{}.{}", enum_ty, const_name)
                                    }
                                }
                            };
                            let case_indent = leading_indent(lines[j]);
                            out_lines.push(format!("{}case {}:", case_indent, label));
                            continue;
                        }
                    }
                }
            }
            out_lines.push(lines[j].to_string());
        }
        out_lines.push(lines[sw_close].to_string());
        i = sw_close + 1;
    }
    let mut out = out_lines.join("\n");
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    // Drop SwitchMap assignment lines used only for case tables (keep if still referenced).
    out = strip_consumed_switchmap_assignments(&out);
    out
}

pub(crate) fn strip_consumed_switchmap_assignments(body: &str) -> String {
    // Remove lines like `…$SwitchMap$…[Color.RED.ordinal()] = 1;` after cases rewritten.
    let mut out = String::new();
    let line_count = body.lines().count();
    for (idx, line) in body.lines().enumerate() {
        let binding = strip_trailing_comment(line);
        let t = binding.trim();
        let drop = t.contains("$SwitchMap$")
            && t.contains(".ordinal()]")
            && t.contains(" = ")
            && t.ends_with(';');
        let drop_local = {
            // `map[Color.RED.ordinal()] = 1;`
            if let Some(open) = t.find('[') {
                t.contains(".ordinal()]")
                    && t.contains(" = ")
                    && t.ends_with(';')
                    && is_java_ident(t[..open].trim())
            } else {
                false
            }
        };
        if drop || drop_local {
            // skip
        } else {
            out.push_str(line);
            if idx < line_count.saturating_sub(1) {
                out.push('\n');
            }
        }
    }
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// `map[e.ordinal()]` → `(map, e)`
/// `map[e.ordinal()]` → `(map, e)`
pub(crate) fn extract_map_var_ordinal(expr: &str) -> Option<(String, String)> {
    let e = expr.trim();
    let open = e.find('[')?;
    let close = e.rfind(']')?;
    if close != e.len() - 1 || close <= open {
        return None;
    }
    let map_var = e[..open].trim();
    if !is_java_ident(map_var) {
        return None;
    }
    let index = e[open + 1..close].trim();
    let enum_expr = index.strip_suffix(".ordinal()")?.trim();
    if enum_expr.is_empty() {
        return None;
    }
    Some((map_var.to_string(), enum_expr.to_string()))
}

pub(crate) fn extract_plain_ordinal(expr: &str) -> Option<String> {
    let e = expr.trim();
    let enum_expr = e.strip_suffix(".ordinal()")?.trim();
    if enum_expr.is_empty() || enum_expr.contains('[') {
        return None;
    }
    // Avoid `arr.length` style — require a receiver that looks like an expression/ident.
    Some(enum_expr.to_string())
}

pub(crate) fn line_assigns_switchmap(line: &str, map_var: &str) -> bool {
    let Some((var, rhs)) = parse_simple_assign_line(line) else {
        return false;
    };
    var == map_var && rhs.contains("$SwitchMap$")
}

pub(crate) fn restore_enum_switchmap_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some(expr) = parse_switch_header(lines[i]) else {
            continue;
        };
        let indent = leading_indent(lines[i]);
        let (new_expr, drop_map_line): (String, Option<usize>) =
            if let Some(e) = extract_switchmap_indexed_ordinal(&expr) {
                (e, None)
            } else if let Some((map_var, e)) = extract_map_var_ordinal(&expr) {
                let mut map_line = None;
                let mut k = i;
                while k > 0 {
                    k -= 1;
                    if lines[k].trim().is_empty() {
                        continue;
                    }
                    if line_assigns_switchmap(lines[k], &map_var) {
                        map_line = Some(k);
                    }
                    break;
                }
                if map_line.is_some() {
                    (e, map_line)
                } else {
                    continue;
                }
            } else if let Some(e) = extract_plain_ordinal(&expr) {
                (e, None)
            } else {
                continue;
            };

        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if drop_map_line == Some(idx) {
                continue;
            }
            if idx == i {
                out.push_str(&format!("{}switch ({}) {{", indent, new_expr));
            } else {
                out.push_str(line);
            }
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
        }
        return out;
    }
    body.to_string()
}

/// Restore try-with-resources when finally only closes the declared resource(s).
/// Restore try-with-resources when finally only closes the declared resource(s).
pub(crate) fn restore_try_with_resources(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = trim_trailing_stray_braces(&strip_orphan_twr_boilerplate(
            &restore_d8_try_with_resources_once(&restore_try_with_resources_once(&current)),
        ));
        if next == current {
            break;
        }
        current = next;
    }
    current
}

/// Remove leftover d8 TWR desugar fragments after restoration.
/// Remove leftover d8 TWR desugar fragments after restoration.
pub(crate) fn strip_orphan_twr_boilerplate(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < lines.len() {
        let t = lines[i].trim();
        if t == "try {" {
            if let Some(try_close) = find_closing_brace_line(&lines, i) {
                let try_body: Vec<&str> = lines[i + 1..try_close]
                    .iter()
                    .copied()
                    .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//"))
                    .collect();
                let throw_only = try_body.len() == 1 && try_body[0].trim().starts_with("throw ");
                let only_close = !try_body.is_empty()
                    && try_body.iter().all(|l| {
                        let s = l.trim().trim_end_matches(';');
                        s.ends_with(".close()")
                    });
                let duplicate_body = is_twr_duplicate_body_try(&try_body);
                if only_close || try_body.is_empty() || duplicate_body || throw_only {
                    let mut end = try_close + 1;
                    while end < lines.len() {
                        let t2 = lines[end].trim();
                        if t2.starts_with("} catch (")
                            || t2 == "} finally {"
                            || t2.starts_with("} finally {")
                            || t2 == "finally {"
                        {
                            if let Some(bclose) = block_body_is_empty(&lines, end) {
                                end = bclose + 1;
                                continue;
                            }
                            if let Some(bclose) = is_twr_suppression_tail(&lines, end) {
                                end = bclose + 1;
                                continue;
                            }
                        }
                        break;
                    }
                    i = end;
                    continue;
                }
            }
        }
        if t.contains("addSuppressed") || (t.starts_with("throw ") && t.ends_with(';')) {
            i += 1;
            continue;
        }
        out.push_str(lines[i]);
        if i < lines.len().saturating_sub(1) || body.ends_with('\n') {
            out.push('\n');
        }
        i += 1;
    }
    out
}

pub(crate) fn is_twr_duplicate_body_try(try_body: &[&str]) -> bool {
    let joined = try_body
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .collect::<Vec<_>>()
        .join(" ");
    joined.contains(".close()")
        && joined.contains("return ")
        && !joined.contains("new ")
        && (!joined.contains(".touch()") || joined.matches(".close()").count() >= 2)
}

pub(crate) fn collect_resource_decls_in_body(
    body_lines: &[&str],
) -> (Vec<(String, String, String)>, usize) {
    let mut decls = Vec::new();
    let mut i = 0usize;
    while i < body_lines.len() {
        if body_lines[i].trim().is_empty() {
            i += 1;
            continue;
        }
        if let Some((ty, var, rhs)) = parse_resource_decl(body_lines[i]) {
            decls.push((ty, var, rhs));
            i += 1;
        } else {
            break;
        }
    }
    (decls, i)
}

pub(crate) fn strip_resource_closes_from_body(
    body_lines: &[String],
    resources: &[&str],
) -> Vec<String> {
    body_lines
        .iter()
        .filter(|l| {
            let s = l.trim().trim_end_matches(';');
            !resources.iter().any(|r| s == format!("{r}.close()"))
        })
        .cloned()
        .collect()
}

pub(crate) fn trim_trailing_stray_braces(body: &str) -> String {
    let mut lines: Vec<&str> = body.lines().collect();
    while let Some(last) = lines.last() {
        if last.trim().is_empty() {
            lines.pop();
            continue;
        }
        // Method-body statements use 8-space indent; `}` above that are desugar leftovers.
        if last.trim() == "}" && leading_indent(last).len() < 8 {
            lines.pop();
        } else {
            break;
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut out = lines.join("\n");
    if body.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn strip_unreachable_after_return(body_lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen_return = false;
    for line in body_lines {
        if seen_return {
            continue;
        }
        out.push(line.to_string());
        if line.trim().starts_with("return ") {
            seen_return = true;
        }
    }
    out
}

pub(crate) fn try_body_only_closes_resources(body_lines: &[&str], resources: &[&str]) -> bool {
    let slice: Vec<&str> = body_lines
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if slice.is_empty() {
        return false;
    }
    let mut remaining: Vec<String> = resources.iter().map(|s| (*s).to_string()).collect();
    let mut pos = 0usize;
    while pos < slice.len() && !remaining.is_empty() {
        let Some((res, consumed)) = take_one_resource_close_unit(&slice[pos..], &remaining) else {
            return false;
        };
        remaining.retain(|r| r != &res);
        pos += consumed;
    }
    pos == slice.len()
}

pub(crate) fn block_body_is_empty(lines: &[&str], open: usize) -> Option<usize> {
    let close = find_closing_brace_line(lines, open)?;
    let nonempty: Vec<_> = lines[open + 1..close]
        .iter()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if nonempty.is_empty() {
        Some(close)
    } else {
        None
    }
}

pub(crate) fn is_twr_suppression_tail(lines: &[&str], open: usize) -> Option<usize> {
    let close = find_closing_brace_line(lines, open)?;
    let body = lines[open + 1..close]
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>();
    if body.is_empty() {
        return Some(close);
    }
    let joined = body.join(" ").to_ascii_lowercase();
    if joined.contains("addsuppressed") || (joined.contains("throw ") && body.len() <= 2) {
        Some(close)
    } else {
        None
    }
}

/// Skip d8/javac TWR desugar tails: empty catch/finally, close-only tries, addSuppressed blocks.
/// Skip d8/javac TWR desugar tails: empty catch/finally, close-only tries, addSuppressed blocks.
pub(crate) fn skip_twr_boilerplate_after(
    lines: &[&str],
    mut i: usize,
    resources: &[&str],
) -> usize {
    loop {
        while i < lines.len() && lines[i].trim().is_empty() {
            i += 1;
        }
        if i >= lines.len() {
            break;
        }
        let t = lines[i].trim();

        if parse_catch_header(lines[i]).is_some() || t.starts_with("} catch (") {
            if let Some(cclose) = block_body_is_empty(lines, i) {
                i = cclose + 1;
                continue;
            }
            break;
        }

        if t == "} finally {" || t.starts_with("} finally {") || t == "finally {" {
            if let Some(fclose) = block_body_is_empty(lines, i) {
                i = fclose + 1;
                continue;
            }
            if is_twr_suppression_tail(lines, i).is_some() {
                i = is_twr_suppression_tail(lines, i).unwrap() + 1;
                continue;
            }
            break;
        }

        if t == "try {" {
            let Some(try_close) = find_closing_brace_line(lines, i) else {
                break;
            };
            let try_body: Vec<&str> = lines[i + 1..try_close]
                .iter()
                .copied()
                .filter(|l| !l.trim().is_empty())
                .collect();
            if try_body_only_closes_resources(&try_body, resources)
                || try_body.iter().all(|l| {
                    let s = l.trim().trim_end_matches(';');
                    resources.iter().any(|r| s == format!("{r}.close()"))
                })
                || is_twr_duplicate_body_try(&try_body)
            {
                i = try_close;
                while i < lines.len() {
                    let t2 = lines[i].trim();
                    if parse_catch_header(lines[i]).is_some()
                        || t2.starts_with("} catch (")
                        || t2 == "} finally {"
                        || t2.starts_with("} finally {")
                        || t2 == "finally {"
                    {
                        if let Some(bclose) = block_body_is_empty(lines, i) {
                            i = bclose + 1;
                            continue;
                        }
                        if let Some(bclose) = is_twr_suppression_tail(lines, i) {
                            i = bclose + 1;
                            continue;
                        }
                    }
                    break;
                }
                continue;
            }
            break;
        }

        if t.contains("addSuppressed") {
            i += 1;
            continue;
        }

        break;
    }
    i
}

/// Restore d8-desugared try-with-resources: resource decl inside try, inline close on normal path,
/// and trailing close / addSuppressed helper tries.
/// Restore d8-desugared try-with-resources: resource decl inside try, inline close on normal path,
/// and trailing close / addSuppressed helper tries.
pub(crate) fn restore_d8_try_with_resources_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for try_i in 0..lines.len() {
        if lines[try_i].trim() != "try {" {
            continue;
        }
        let Some(try_close) = find_closing_brace_line(&lines, try_i) else {
            continue;
        };

        let decls_above = collect_resource_decls_above(&lines, try_i);
        let try_body_lines: Vec<&str> = lines[try_i + 1..try_close].to_vec();
        let (decls_inside, inside_end) = collect_resource_decls_in_body(&try_body_lines);

        if decls_above.is_empty() && decls_inside.is_empty() {
            continue;
        }

        let mut all_decls: Vec<(String, String, String)> = decls_above
            .iter()
            .map(|(_, ty, var, rhs)| (ty.clone(), var.clone(), rhs.clone()))
            .chain(decls_inside)
            .collect();
        let mut seen = std::collections::HashSet::new();
        all_decls.retain(|(_, v, _)| seen.insert(v.clone()));

        let resource_names: Vec<&str> = all_decls.iter().map(|(_, v, _)| v.as_str()).collect();
        let mut user_body: Vec<String> = try_body_lines[inside_end..]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        user_body = strip_resource_closes_from_body(&user_body, &resource_names);
        user_body = strip_unreachable_after_return(&user_body);

        let had_inline_close = try_body_lines[inside_end..]
            .iter()
            .any(|l| resource_names.iter().any(|r| is_close_call(l, r)));

        let mut cursor = try_close;
        loop {
            let t = lines.get(cursor).map(|l| l.trim()).unwrap_or("");
            if parse_catch_header(lines[cursor]).is_some() || t.starts_with("} catch (") {
                let Some(cclose) = find_closing_brace_line(&lines, cursor) else {
                    break;
                };
                cursor = cclose;
                continue;
            }
            break;
        }

        let boilerplate_end = skip_twr_boilerplate_after(&lines, cursor, &resource_names);
        let has_boilerplate = boilerplate_end > cursor + 1;
        if !had_inline_close && !has_boilerplate {
            continue;
        }

        let decl_idxs: std::collections::HashSet<usize> =
            decls_above.iter().map(|(i, _, _, _)| *i).collect();
        let indent = leading_indent(lines[try_i]);
        let resources_hdr: String = all_decls
            .iter()
            .map(|(ty, var, rhs)| format!("{ty} {var} = {rhs}"))
            .collect::<Vec<_>>()
            .join("; ");

        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if decl_idxs.contains(&idx) {
                continue;
            }
            if idx == try_i {
                out.push_str(&format!("{indent}try ({resources_hdr}) {{\n"));
                for l in &user_body {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{indent}}}"));
                if boilerplate_end < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > try_i && idx < boilerplate_end {
                continue;
            }
            if idx > try_i && idx <= boilerplate_end && line.trim().is_empty() {
                continue;
            }
            if idx > try_i && line.trim() == "}" && leading_indent(line).len() < indent.len() {
                continue;
            }
            out.push_str(line);
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
        }
        return out;
    }
    body.to_string()
}

pub(crate) fn parse_resource_decl(line: &str) -> Option<(String, String, String)> {
    // `Type r = new Type(...);` or `Type r = expr;`
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    if !stmt.ends_with(';') {
        return None;
    }
    let eq = stmt.find(" = ")?;
    let lhs = stmt[..eq].trim();
    let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
    if rhs.is_empty() {
        return None;
    }
    let parts: Vec<&str> = lhs.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let var = parts.last()?.to_string();
    if !is_java_ident(&var) {
        return None;
    }
    let ty = parts[..parts.len() - 1].join(" ");
    if ty.is_empty() || ty.starts_with("return") || ty.starts_with("if") {
        return None;
    }
    Some((ty, var, rhs.to_string()))
}

pub(crate) fn is_close_call(stmt: &str, resource: &str) -> bool {
    let t = stmt.trim().trim_end_matches(';').trim();
    t == format!("{}.close()", resource)
}

/// Collect consecutive resource decls immediately above `try_i` (blank lines allowed between).
/// Collect consecutive resource decls immediately above `try_i` (blank lines allowed between).
pub(crate) fn collect_resource_decls_above(
    lines: &[&str],
    try_i: usize,
) -> Vec<(usize, String, String, String)> {
    let mut decls: Vec<(usize, String, String, String)> = Vec::new();
    let mut i = try_i;
    while i > 0 {
        i -= 1;
        if lines[i].trim().is_empty() {
            continue;
        }
        if let Some((ty, var, rhs)) = parse_resource_decl(lines[i]) {
            decls.push((i, ty, var, rhs));
            continue;
        }
        break;
    }
    decls.reverse();
    decls
}

/// True if finally body only closes the given resources (any order; sequential close units).
/// True if finally body only closes the given resources (any order; sequential close units).
pub(crate) fn finally_body_only_closes_resources(
    lines: &[&str],
    body_start: usize,
    body_end: usize,
    resources: &[&str],
) -> bool {
    if resources.is_empty() {
        return false;
    }
    let mut slice: Vec<&str> = lines[body_start..body_end]
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if slice.is_empty() {
        return false;
    }
    let mut remaining: Vec<String> = resources.iter().map(|s| (*s).to_string()).collect();
    while !slice.is_empty() {
        let Some((res, consumed)) = take_one_resource_close_unit(&slice, &remaining) else {
            return false;
        };
        remaining.retain(|r| r != &res);
        slice = slice[consumed..].to_vec();
    }
    remaining.is_empty()
}

/// Consume one close-pattern unit for some resource in `remaining`.
/// Returns `(resource_name, lines_consumed)`.
/// Consume one close-pattern unit for some resource in `remaining`.
/// Returns `(resource_name, lines_consumed)`.
pub(crate) fn take_one_resource_close_unit(
    slice: &[&str],
    remaining: &[String],
) -> Option<(String, usize)> {
    if slice.is_empty() {
        return None;
    }
    // `r.close();`
    for res in remaining {
        if is_close_call(slice[0], res) {
            return Some((res.clone(), 1));
        }
    }
    // `if (r != null) { …close… }`
    if let Some(cond) = parse_if_condition(slice[0]) {
        let c = cond.replace(' ', "");
        let mut matched_res: Option<String> = None;
        for res in remaining {
            if c == format!("{}!=null", res) || c == format!("null!={}", res) {
                matched_res = Some(res.clone());
                break;
            }
        }
        let res = matched_res?;
        let if_close = find_closing_brace_line(slice, 0)?;
        if !close_block_only_closes(&slice[1..if_close], &res) {
            return None;
        }
        return Some((res, if_close + 1));
    }
    // `try { …close… } catch (…) { }` (empty catch)
    if slice[0].trim() == "try {" {
        let try_close = find_closing_brace_line(slice, 0)?;
        let try_body: Vec<&str> = slice[1..try_close]
            .iter()
            .copied()
            .filter(|l| !l.trim().is_empty())
            .collect();
        let (res, n) = take_one_resource_close_unit(&try_body, remaining)?;
        if n != try_body.len() {
            return None;
        }
        let mut i = try_close;
        let mut saw_catch = false;
        while i < slice.len() {
            let t = slice[i].trim();
            if parse_catch_header(slice[i]).is_some() || t.starts_with("} catch (") {
                let cclose = find_closing_brace_line(slice, i)?;
                let catch_body: Vec<&str> = slice[i + 1..cclose]
                    .iter()
                    .copied()
                    .filter(|l| !l.trim().is_empty())
                    .collect();
                if !catch_body.is_empty() {
                    return None;
                }
                saw_catch = true;
                i = cclose + 1;
                continue;
            }
            break;
        }
        if !saw_catch {
            return None;
        }
        return Some((res, i));
    }
    None
}

pub(crate) fn close_block_only_closes(lines: &[&str], resource: &str) -> bool {
    let slice: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if slice.is_empty() {
        return false;
    }
    if slice.len() == 1 && is_close_call(slice[0], resource) {
        return true;
    }
    // nested try { r.close(); } catch { }
    if slice[0].trim() == "try {" {
        let Some(try_close) = find_closing_brace_line(&slice, 0) else {
            return false;
        };
        let try_body: Vec<&str> = slice[1..try_close]
            .iter()
            .copied()
            .filter(|l| !l.trim().is_empty())
            .collect();
        if try_body.len() != 1 || !is_close_call(try_body[0], resource) {
            return false;
        }
        let mut i = try_close;
        while i < slice.len() {
            if parse_catch_header(slice[i]).is_some() || slice[i].trim().starts_with("} catch (") {
                let Some(cclose) = find_closing_brace_line(&slice, i) else {
                    return false;
                };
                let catch_body: Vec<&str> = slice[i + 1..cclose]
                    .iter()
                    .copied()
                    .filter(|l| !l.trim().is_empty())
                    .collect();
                if !catch_body.is_empty() {
                    return false;
                }
                i = cclose + 1;
                continue;
            }
            return false;
        }
        return true;
    }
    // `if (r != null) { r.close(); }`
    if let Some(cond) = parse_if_condition(slice[0]) {
        let c = cond.replace(' ', "");
        if c != format!("{}!=null", resource) && c != format!("null!={}", resource) {
            return false;
        }
        let Some(if_close) = find_closing_brace_line(&slice, 0) else {
            return false;
        };
        if if_close != slice.len() - 1 {
            return false;
        }
        let inner: Vec<&str> = slice[1..if_close]
            .iter()
            .copied()
            .filter(|l| !l.trim().is_empty())
            .collect();
        return inner.len() == 1 && is_close_call(inner[0], resource);
    }
    false
}

pub(crate) fn restore_try_with_resources_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for try_i in 0..lines.len() {
        if lines[try_i].trim() != "try {" {
            continue;
        }
        let decls = collect_resource_decls_above(&lines, try_i);
        if decls.is_empty() {
            continue;
        }
        let resource_names: Vec<&str> = decls.iter().map(|(_, _, v, _)| v.as_str()).collect();

        let Some(try_close) = find_closing_brace_line(&lines, try_i) else {
            continue;
        };

        // Walk optional catch clauses, then require finally
        let mut cursor = try_close;
        let mut catch_ranges: Vec<(usize, usize)> = Vec::new();
        loop {
            let t = lines.get(cursor).map(|l| l.trim()).unwrap_or("");
            if parse_catch_header(lines[cursor]).is_some() || t.starts_with("} catch (") {
                let Some(cclose) = find_closing_brace_line(&lines, cursor) else {
                    break;
                };
                catch_ranges.push((cursor, cclose));
                cursor = cclose;
                continue;
            }
            break;
        }

        let ft = lines.get(cursor).map(|l| l.trim()).unwrap_or("");
        let finally_open = if ft == "} finally {" || ft.starts_with("} finally {") {
            cursor
        } else if ft == "finally {" {
            cursor
        } else {
            let mut j = cursor;
            if j < lines.len() && lines[j].trim() == "}" {
                j += 1;
                while j < lines.len() && lines[j].trim().is_empty() {
                    j += 1;
                }
            }
            if j < lines.len() && lines[j].trim() == "finally {" {
                j
            } else {
                continue;
            }
        };

        let Some(finally_close) = find_closing_brace_line(&lines, finally_open) else {
            continue;
        };
        if !finally_body_only_closes_resources(
            &lines,
            finally_open + 1,
            finally_close,
            &resource_names,
        ) {
            continue;
        }

        let decl_idxs: std::collections::HashSet<usize> =
            decls.iter().map(|(i, _, _, _)| *i).collect();
        let indent = leading_indent(lines[try_i]);
        let resources_hdr: String = decls
            .iter()
            .map(|(_, ty, var, rhs)| format!("{} {} = {}", ty, var, rhs))
            .collect::<Vec<_>>()
            .join("; ");

        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if decl_idxs.contains(&idx) {
                continue;
            }
            if idx == try_i {
                out.push_str(&format!("{}try ({}) {{\n", indent, resources_hdr));
                for l in &lines[try_i + 1..try_close] {
                    out.push_str(l);
                    out.push('\n');
                }
                if catch_ranges.is_empty() {
                    out.push_str(&format!("{}}}", indent));
                    if finally_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                        out.push('\n');
                    }
                } else {
                    for (ci, &(ch, cc)) in catch_ranges.iter().enumerate() {
                        let header = lines[ch].trim();
                        let hdr = if header.starts_with('}') {
                            header.to_string()
                        } else {
                            format!("}} {}", header)
                        };
                        out.push_str(&format!("{}{}\n", indent, hdr));
                        let body_end = if ci + 1 < catch_ranges.len()
                            || lines[cc].trim().starts_with("} finally")
                            || lines[cc].trim().starts_with("} catch")
                        {
                            cc
                        } else {
                            cc
                        };
                        for l in &lines[ch + 1..body_end] {
                            out.push_str(l);
                            out.push('\n');
                        }
                        if ci + 1 == catch_ranges.len() {
                            if lines[cc].trim().starts_with("} finally")
                                || (ci + 1 < catch_ranges.len())
                            {
                                out.push_str(&format!("{}}}", indent));
                            } else if lines[cc].trim() == "}" {
                                out.push_str(lines[cc]);
                            } else {
                                out.push_str(&format!("{}}}", indent));
                            }
                            if finally_close < lines.len().saturating_sub(1) || body.ends_with('\n')
                            {
                                out.push('\n');
                            }
                        }
                    }
                }
                continue;
            }
            if idx > try_i && idx <= finally_close {
                continue;
            }
            out.push_str(line);
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
        }
        return out;
    }
    body.to_string()
}
