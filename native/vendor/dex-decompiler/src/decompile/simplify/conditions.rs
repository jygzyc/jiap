//! Condition folding, else-if chains, and boolean polish passes.

use std::collections::{HashMap, HashSet};

use super::util::*;

/// Fold string-literal equals temps into `if` conditions and collapse `!(!x)`.
pub(crate) fn fold_equals_into_conditions(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..6 {
        let next = fold_equals_into_conditions_once(&current);
        let next = collapse_double_not_conditions(&next);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn fold_equals_into_conditions_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut skip: HashSet<usize> = HashSet::new();
    let mut replacements: HashMap<usize, String> = HashMap::new();

    for i in 0..lines.len() {
        if skip.contains(&i) {
            continue;
        }
        // Pattern: lit = "…";  bool = lit.equals(x);  if (bool|/!bool|/!(!bool))
        // or: bool = expr.equals(x); if (…)
        let Some((bvar, equals_expr)) = parse_equals_assign(lines[i]) else {
            continue;
        };
        // Optional preceding lit assign used once in equals_expr.
        let mut lit_idx: Option<usize> = None;
        let mut equals_expr = equals_expr;
        if i > 0 {
            if let Some((lvar, lval)) = parse_simple_assign_line(lines[i - 1]) {
                if is_string_literal(&lval) && ident_occurs(&equals_expr, &lvar) {
                    equals_expr = replace_ident_as_expr(&equals_expr, &lvar, &lval);
                    lit_idx = Some(i - 1);
                }
            }
        }
        // Find following if that tests bvar.
        let mut if_idx = None;
        for j in i + 1..lines.len().min(i + 4) {
            if let Some(cond) = parse_if_condition(lines[j]) {
                if condition_tests_var(&cond, &bvar) {
                    if_idx = Some(j);
                    break;
                }
            }
            // Allow only blank / comment-ish gaps
            let t = lines[j].trim();
            if !t.is_empty() && !t.starts_with("//") {
                break;
            }
        }
        let Some(if_i) = if_idx else { continue };
        let cond = parse_if_condition(lines[if_i]).unwrap();
        let new_cond = rewrite_condition_with_equals(&cond, &bvar, &equals_expr);
        let indent = leading_indent(lines[if_i]);
        replacements.insert(if_i, format!("{}if ({}) {{", indent, new_cond));
        skip.insert(i);
        if let Some(li) = lit_idx {
            // Only skip lit if unused elsewhere before if.
            let used_elsewhere = lines.iter().enumerate().any(|(k, l)| {
                k != i
                    && k != li
                    && k < if_i
                    && ident_occurs(l, parse_simple_assign_line(lines[li]).unwrap().0.as_str())
            });
            if !used_elsewhere {
                skip.insert(li);
            }
        }
    }

    if replacements.is_empty() && skip.is_empty() {
        return body.to_string();
    }
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if skip.contains(&idx) {
            continue;
        }
        if let Some(rep) = replacements.get(&idx) {
            out.push_str(rep);
        } else {
            out.push_str(line);
        }
        if idx < lines.len().saturating_sub(1) {
            out.push('\n');
        }
    }
    out
}

fn parse_instanceof_assign(line: &str) -> Option<(String, String)> {
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    let eq = t.find(" = ")?;
    let lhs = t[..eq].trim();
    let var = lhs.rsplit(' ').next().unwrap_or(lhs);
    if !is_java_ident(var) {
        return None;
    }
    let rhs = t[eq + 3..].trim_end_matches(';').trim();
    if !rhs.contains(" instanceof ") {
        return None;
    }
    Some((var.to_string(), rhs.to_string()))
}

/// Fold `z = x instanceof T;` into a following `if` that tests `z`, or that tests a
/// leftover register name (`obj == null`) when `z` is otherwise unused.
pub(crate) fn fold_instanceof_into_conditions(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..6 {
        let next = fold_instanceof_into_conditions_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn fold_instanceof_into_conditions_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut skip: HashSet<usize> = HashSet::new();
    let mut replacements: HashMap<usize, String> = HashMap::new();

    for i in 0..lines.len() {
        if skip.contains(&i) {
            continue;
        }
        let Some((zvar, instanceof_expr)) = parse_instanceof_assign(lines[i]) else {
            continue;
        };
        let mut if_idx = None;
        for j in i + 1..lines.len().min(i + 4) {
            if parse_if_condition(lines[j]).is_some() {
                if_idx = Some(j);
                break;
            }
            let t = lines[j].trim();
            if !t.is_empty() && !t.starts_with("//") {
                break;
            }
        }
        let Some(if_i) = if_idx else { continue };
        let cond = parse_if_condition(lines[if_i]).unwrap();
        let new_cond = if condition_tests_var(&cond, &zvar) {
            rewrite_condition_with_equals(&cond, &zvar, &instanceof_expr)
        } else if !ident_occurs(&cond, &zvar)
            && dangling_false_instanceof_cond(&cond)
            && !lines
                .iter()
                .enumerate()
                .any(|(k, l)| k != i && k != if_i && ident_used_as_rvalue(l, &zvar))
        {
            format!("!({instanceof_expr})")
        } else {
            continue;
        };
        let new_cond = polish_instanceof_in_condition(&new_cond);
        replacements.insert(
            if_i,
            format!("{}if ({}) {{", leading_indent(lines[if_i]), new_cond),
        );
        skip.insert(i);
    }

    if replacements.is_empty() && skip.is_empty() {
        return body.to_string();
    }
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if skip.contains(&idx) {
            continue;
        }
        if let Some(rep) = replacements.get(&idx) {
            out.push_str(rep);
        } else {
            out.push_str(line);
        }
        if idx < lines.len().saturating_sub(1) {
            out.push('\n');
        }
    }
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn dangling_false_instanceof_cond(cond: &str) -> bool {
    let c = cond.trim();
    for suf in [" == 0", " == null", " == false"] {
        if let Some(name) = c.strip_suffix(suf) {
            let name = name.trim();
            return is_java_ident(name) && is_temp_like_name(name);
        }
    }
    if let Some(name) = c.strip_prefix('!') {
        let name = name.trim();
        return is_java_ident(name) && is_temp_like_name(name);
    }
    false
}

pub(crate) fn collapse_double_not_conditions(body: &str) -> String {
    let mut out = String::new();
    for (idx, line) in body.lines().enumerate() {
        if let Some(cond) = parse_if_condition(line) {
            let collapsed = collapse_double_not(&cond);
            if collapsed != cond {
                let indent = leading_indent(line);
                out.push_str(&format!("{}if ({}) {{", indent, collapsed));
            } else {
                out.push_str(line);
            }
        } else {
            out.push_str(line);
        }
        if idx < body.lines().count().saturating_sub(1) {
            out.push('\n');
        }
    }
    // Fix trailing newline parity
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn collapse_double_not(cond: &str) -> String {
    let mut c = cond.trim().to_string();
    for _ in 0..4 {
        if let Some(inner) = c.strip_prefix("!(").and_then(|s| s.strip_suffix(')')) {
            let inner = inner.trim();
            if let Some(inner2) = inner.strip_prefix('!') {
                let inner2 = inner2
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .trim();
                c = inner2.to_string();
                continue;
            }
        }
        if let Some(rest) = c.strip_prefix("!!") {
            c = rest.to_string();
            continue;
        }
        break;
    }
    c
}

/// True when condition is a (possibly negated) string/predicate call like `"x".equals(y)`.
/// Drop `name = "lit";` when the next statement is `if ("lit".equals(…))` / negated form.
pub(crate) fn drop_string_lits_folded_into_if(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut skip: HashSet<usize> = HashSet::new();
    for i in 0..lines.len().saturating_sub(1) {
        let Some((var, lit)) = parse_simple_assign_line(lines[i]) else {
            continue;
        };
        if !is_string_literal(&lit) {
            continue;
        }
        // Find next non-empty line
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
        }
        if j >= lines.len() {
            continue;
        }
        let Some(cond) = parse_if_condition(lines[j]) else {
            continue;
        };
        let Some(cond_lit) = string_lit_in_predicate_cond(&cond) else {
            continue;
        };
        if cond_lit != lit {
            continue;
        }
        // Keep if var is used in the condition (e.g. var.equals(...)) or later before reassignment.
        if ident_occurs(&cond, &var) {
            continue;
        }
        let used_later = lines.iter().enumerate().any(|(k, l)| {
            k > i
                && k != j
                && ident_occurs(l, &var)
                && assign_lhs_var(l).as_deref() != Some(var.as_str())
        });
        // If used later as a different value, still safe to drop THIS assign only when
        // the next if already has the literal inlined.
        if used_later {
            // Only drop if no use between assign and if.
            let used_between = (i + 1..j).any(|k| ident_occurs(lines[k], &var));
            if used_between {
                continue;
            }
        }
        skip.insert(i);
    }
    if skip.is_empty() {
        return body.to_string();
    }
    rebuild_lines(&lines, &skip, &HashMap::new(), body.ends_with('\n'))
}

/// Flip `if (!(pred)) { THEN } else { ELSE }` → `if (pred) { ELSE } else { THEN }`
/// when pred is a string equals/endsWith-style call. Repeat until stable.
pub(crate) fn flip_negated_equals_if_else(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..12 {
        let next = flip_negated_equals_if_else_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn flip_negated_equals_if_else_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some(cond) = parse_if_condition(lines[i]) else {
            continue;
        };
        let Some(inner) = strip_outer_not_parens(&cond) else {
            continue;
        };
        if !condition_has_string_predicate(&inner) && !condition_has_string_predicate(&cond) {
            continue;
        }
        // Require predicate form inside the not
        if !condition_has_string_predicate(&format!("({})", inner))
            && !inner.contains(".equals(")
            && !inner.contains(".endsWith(")
            && !inner.contains(".startsWith(")
            && !inner.contains(".contains(")
        {
            continue;
        }
        let Some(then_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        // Expect `} else {` on then_close
        let close_trim = lines[then_close].trim();
        if close_trim != "} else {" {
            continue;
        }
        let Some(else_close) = find_closing_brace_line(&lines, then_close) else {
            continue;
        };
        let indent = leading_indent(lines[i]);
        let then_body: Vec<&str> = lines[i + 1..then_close].to_vec();
        let else_body: Vec<&str> = lines[then_close + 1..else_close].to_vec();
        // Build flipped block
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                out.push_str(&format!("{}if ({}) {{\n", indent, inner));
                for l in &else_body {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}} else {{\n", indent));
                for l in &then_body {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if else_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > i && idx <= else_close {
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

/// Collapse `} else {\n    if (cond) {\n ... \n    }\n}` → `} else if (cond) {\n ... \n}`
/// Collapse `} else {\n    if (cond) {\n ... \n    }\n}` → `} else if (cond) {\n ... \n}`
pub(crate) fn flatten_else_if_chains(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..16 {
        let next = flatten_else_if_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn flatten_else_if_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        if lines[i].trim() != "} else {" {
            continue;
        }
        // Skip blanks
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
        }
        if j >= lines.len() {
            continue;
        }
        let Some(cond) = parse_if_condition(lines[j]) else {
            continue;
        };
        let Some(inner_end) = find_if_statement_end(&lines, j) else {
            continue;
        };
        // The `} else {` close must be followed only by this if (and its full if-else) until else-close
        let Some(else_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        // After inner if-statement end, only whitespace until else_close
        let mut k = inner_end + 1;
        let mut only_ws = true;
        while k < else_close {
            if !lines[k].trim().is_empty() {
                only_ws = false;
                break;
            }
            k += 1;
        }
        if !only_ws || inner_end >= else_close {
            continue;
        }
        let else_indent = leading_indent(lines[i]);
        // Find the then-close of the inner if (may be `}` or `} else {` / `} else if`)
        let Some(inner_then_close) = find_closing_brace_line(&lines, j) else {
            continue;
        };
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                out.push_str(&format!("{}}} else if ({}) {{\n", else_indent, cond));
                // Emit then-body of the inner if
                for l in &lines[j + 1..inner_then_close] {
                    out.push_str(l);
                    out.push('\n');
                }
                // If inner had else/else-if, keep those clauses through the if-statement end.
                if inner_then_close < inner_end {
                    for l in &lines[inner_then_close..=inner_end] {
                        let t = l.trim();
                        if t.starts_with("} else") {
                            out.push_str(&format!("{}{}\n", else_indent, t));
                        } else {
                            out.push_str(l);
                            out.push('\n');
                        }
                    }
                } else {
                    // Plain if with no else — need its closing brace
                    out.push_str(&format!("{}}}", else_indent));
                    if else_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                        out.push('\n');
                    }
                }
                continue;
            }
            // Drop the old else wrapper (`} else {` … closing `}`).
            if idx > i && idx <= else_close {
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

/// Merge:
/// ```text
/// path = uri.getScheme();
/// if ("a".equals(path)) {
///   path = uri.getHost();
///   if ("b".equals(path)) {
///     BODY
///   }
/// }
/// ```
/// into `if ("a".equals(uri.getScheme()) && "b".equals(uri.getHost())) { BODY }`
/// Merge:
/// ```text
/// path = uri.getScheme();
/// if ("a".equals(path)) {
///   path = uri.getHost();
///   if ("b".equals(path)) {
///     BODY
///   }
/// }
/// ```
/// into `if ("a".equals(uri.getScheme()) && "b".equals(uri.getHost())) { BODY }`
pub(crate) fn merge_nested_uri_component_checks(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..4 {
        let next = merge_nested_uri_component_checks_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn is_uri_component_getter(rhs: &str) -> bool {
    let r = rhs.trim();
    r.ends_with(".getScheme()")
        || r.ends_with(".getHost()")
        || r.ends_with(".getPath()")
        || r.ends_with(".getQuery()")
        || r.ends_with(".getFragment()")
}

pub(crate) fn merge_nested_uri_component_checks_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len().saturating_sub(1) {
        let Some((var1, getter1)) = parse_simple_assign_line(lines[i]) else {
            continue;
        };
        if !is_uri_component_getter(&getter1) {
            continue;
        }
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
        }
        if j >= lines.len() {
            continue;
        }
        let Some(cond1) = parse_if_condition(lines[j]) else {
            continue;
        };
        // "lit".equals(var1) or var1.equals("lit")
        if !ident_occurs(&cond1, &var1) || !condition_has_string_predicate(&cond1) {
            continue;
        }
        if strip_outer_not_parens(&cond1).is_some() {
            continue; // only merge positive checks
        }
        let Some(outer_close) = find_closing_brace_line(&lines, j) else {
            continue;
        };
        // Inside outer then: var1 = getter2; if (cond2) { body }  and nothing else
        let mut k = j + 1;
        while k < outer_close && lines[k].trim().is_empty() {
            k += 1;
        }
        if k >= outer_close {
            continue;
        }
        let Some((var2, getter2)) = parse_simple_assign_line(lines[k]) else {
            continue;
        };
        if var2 != var1 || !is_uri_component_getter(&getter2) {
            continue;
        }
        let mut m = k + 1;
        while m < outer_close && lines[m].trim().is_empty() {
            m += 1;
        }
        if m >= outer_close {
            continue;
        }
        let Some(cond2) = parse_if_condition(lines[m]) else {
            continue;
        };
        if !ident_occurs(&cond2, &var1) || !condition_has_string_predicate(&cond2) {
            continue;
        }
        if strip_outer_not_parens(&cond2).is_some() {
            continue;
        }
        let Some(inner_close) = find_closing_brace_line(&lines, m) else {
            continue;
        };
        if inner_close >= outer_close {
            continue;
        }
        // Only whitespace between inner_close and outer_close
        if (inner_close + 1..outer_close).any(|t| !lines[t].trim().is_empty()) {
            continue;
        }
        let cond1_inlined = replace_ident_as_expr(&cond1, &var1, &getter1);
        let cond2_inlined = replace_ident_as_expr(&cond2, &var1, &getter2);
        let indent = leading_indent(lines[j]);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                // skip getter1 assign
                continue;
            }
            if idx == j {
                out.push_str(&format!(
                    "{}if ({} && {}) {{\n",
                    indent, cond1_inlined, cond2_inlined
                ));
                for l in &lines[m + 1..inner_close] {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if outer_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > j && idx <= outer_close {
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

/// Inline `path = uri.getPath();` into a following equals-if when `path` is not used afterward
/// before reassignment (or only used in that if condition).
/// Inline `path = uri.getPath();` into a following equals-if when `path` is not used afterward
/// before reassignment (or only used in that if condition).
pub(crate) fn inline_single_use_into_equals_if(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut skip: HashSet<usize> = HashSet::new();
    let mut replacements: HashMap<usize, String> = HashMap::new();
    for i in 0..lines.len() {
        if skip.contains(&i) {
            continue;
        }
        let Some((var, getter)) = parse_simple_assign_line(lines[i]) else {
            continue;
        };
        if !is_uri_component_getter(&getter) && !getter.contains(".getQueryParameter(") {
            continue;
        }
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
        }
        if j >= lines.len() {
            continue;
        }
        let Some(cond) = parse_if_condition(lines[j]) else {
            continue;
        };
        if !ident_occurs(&cond, &var) || !condition_has_string_predicate(&cond) {
            continue;
        }
        // Count uses of var after the assign (excluding the if condition line's condition).
        let mut use_count = 0;
        let mut killed = false;
        for (k, l) in lines.iter().enumerate().skip(i + 1) {
            if assign_lhs_var(l).as_deref() == Some(var.as_str()) {
                killed = true;
                break;
            }
            if k == j {
                // condition use only — counted separately
                continue;
            }
            if ident_occurs(l, &var) {
                use_count += 1;
            }
        }
        // Allow inlining when the only use is the if condition (and optionally body doesn't use var).
        if use_count > 0 {
            continue;
        }
        let _ = killed;
        let new_cond = replace_ident_as_expr(&cond, &var, &getter);
        let indent = leading_indent(lines[j]);
        replacements.insert(j, format!("{}if ({}) {{", indent, new_cond));
        skip.insert(i);
    }
    if skip.is_empty() && replacements.is_empty() {
        return body.to_string();
    }
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if skip.contains(&idx) {
            continue;
        }
        if let Some(rep) = replacements.get(&idx) {
            out.push_str(rep);
        } else {
            out.push_str(line);
        }
        if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

/// Replace `name == 0` / `!= 0` with null checks for reference-looking names in statements.
/// `setJavaScriptEnabled`, `setAllowFileAccessFromFileURLs`, `setClickable`, …
/// Replace `name == 0` / `!= 0` with null checks for reference-looking names in statements.
/// `setJavaScriptEnabled`, `setAllowFileAccessFromFileURLs`, `setClickable`, …
pub(crate) fn looks_like_boolean_api_method(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("enabled")
        || n.contains("enable")
        || n.starts_with("setallow")
        || n.starts_with("setblock")
        || n.starts_with("setdeny")
        || n.contains("visible")
        || n.contains("hidden")
        || n.contains("clickable")
        || n.contains("focusable")
        || n.contains("checked")
        || n.contains("selected")
        || n.contains("activated")
        || n.contains("javascript")
}

/// `.setFooEnabled(1)` / `.setAllowX(0)` → `true` / `false`.
/// `.setFooEnabled(1)` / `.setAllowX(0)` → `true` / `false`.
pub(crate) fn polish_booleanish_int_args(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    while i < bytes.len() {
        // Match `.ident(0|1)` with optional spaces inside the parens.
        if bytes[i] == b'.' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            if j > i + 1 {
                let name = &body[i + 1..j];
                let mut k = j;
                while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                    k += 1;
                }
                if k < bytes.len() && bytes[k] == b'(' {
                    let mut a = k + 1;
                    while a < bytes.len() && bytes[a].is_ascii_whitespace() {
                        a += 1;
                    }
                    if a < bytes.len() && (bytes[a] == b'0' || bytes[a] == b'1') {
                        let digit = bytes[a] as char;
                        let mut b = a + 1;
                        while b < bytes.len() && bytes[b].is_ascii_whitespace() {
                            b += 1;
                        }
                        if b < bytes.len()
                            && bytes[b] == b')'
                            && looks_like_boolean_api_method(name)
                        {
                            out.push('.');
                            out.push_str(name);
                            out.push('(');
                            out.push_str(if digit == '1' { "true" } else { "false" });
                            out.push(')');
                            i = b + 1;
                            continue;
                        }
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub(crate) fn polish_null_comparisons_in_body(body: &str) -> String {
    let mut out = String::new();
    for (idx, line) in body.lines().enumerate() {
        let mut current = line.to_string();
        // if (x == 0) / if (x != 0)
        if let Some(cond) = parse_if_condition(line) {
            let new_cond = polish_null_in_condition(&cond);
            if new_cond != cond {
                current = format!("{}if ({}) {{", leading_indent(line), new_cond);
            }
        } else {
            // Also handle inline comparisons in assigns rarely needed; focus on if.
        }
        out.push_str(&current);
        if idx < body.lines().count().saturating_sub(1) {
            out.push('\n');
        }
    }
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn polish_null_in_condition(cond: &str) -> String {
    let c = cond.trim();
    for (suf, null_suf) in [(" == 0", " == null"), (" != 0", " != null")] {
        if let Some(name) = c.strip_suffix(suf) {
            let name = name.trim();
            if is_java_ident(name)
                && !looks_like_primitive_local(name)
                && !looks_like_boolean_local(name)
            {
                return format!("{}{}", name, null_suf);
            }
        }
    }
    c.to_string()
}

/// Dalvik `instance-of` + `if-eqz` becomes `x instanceof T == 0`, then null-polish
/// may turn that into `== null`. Fold back to a real instanceof test.
pub(crate) fn polish_instanceof_comparisons_in_body(body: &str) -> String {
    let mut out = String::new();
    let line_count = body.lines().count();
    for (idx, line) in body.lines().enumerate() {
        let mut current = line.to_string();
        if let Some(cond) = parse_if_condition(line) {
            let new_cond = polish_instanceof_in_condition(&cond);
            if new_cond != cond {
                current = format!("{}if ({}) {{", leading_indent(line), new_cond);
            }
        } else if let Some(cond) = parse_while_condition(line) {
            let new_cond = polish_instanceof_in_condition(&cond);
            if new_cond != cond {
                current = format!("{}while ({}) {{", leading_indent(line), new_cond);
            }
        }
        out.push_str(&current);
        if idx < line_count.saturating_sub(1) {
            out.push('\n');
        }
    }
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn polish_instanceof_in_condition(cond: &str) -> String {
    let c = cond.trim();
    // `!o instanceof Number` is `( !o ) instanceof Number` in Java; wrap the test.
    if let Some(rest) = c.strip_prefix('!') {
        let rest = rest.trim();
        if !rest.starts_with('(')
            && rest.contains(" instanceof ")
            && !rest.contains("==")
            && !rest.contains("!=")
        {
            return format!("!({rest})");
        }
    }
    const NEGATED: &[&str] = &[" == 0", " == null", " == false"];
    const POSITIVE: &[&str] = &[" != 0", " != null", " == true"];
    for suf in NEGATED {
        if let Some(left) = strip_instanceof_cmp_left(c, suf) {
            return format!("!({left})");
        }
    }
    for suf in POSITIVE {
        if let Some(left) = strip_instanceof_cmp_left(c, suf) {
            return left;
        }
    }
    c.to_string()
}

fn strip_instanceof_cmp_left(cond: &str, suffix: &str) -> Option<String> {
    let left = cond.strip_suffix(suffix)?.trim();
    if !left.contains(" instanceof ") {
        return None;
    }
    let left = left
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(left)
        .trim();
    if left.contains(" instanceof ") {
        Some(left.to_string())
    } else {
        None
    }
}

/// Flip `if (!(x instanceof T)) { THEN } else { ELSE }` so the true instanceof
/// path is the then-branch (Dalvik `if-eqz` skip layout).
pub(crate) fn flip_negated_instanceof_if_else(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..12 {
        let next = flip_negated_instanceof_if_else_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn flip_negated_instanceof_if_else_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some(cond) = parse_if_condition(lines[i]) else {
            continue;
        };
        let Some(inner) = strip_outer_not_parens(&cond) else {
            continue;
        };
        if !inner.contains(" instanceof ") {
            continue;
        }
        let Some(then_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        if lines[then_close].trim() != "} else {" {
            continue;
        }
        let Some(else_close) = find_closing_brace_line(&lines, then_close) else {
            continue;
        };
        let indent = leading_indent(lines[i]);
        let then_body: Vec<&str> = lines[i + 1..then_close].to_vec();
        let else_body: Vec<&str> = lines[then_close + 1..else_close].to_vec();
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                out.push_str(&format!("{}if ({}) {{\n", indent, inner));
                for l in &else_body {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}} else {{\n", indent));
                for l in &then_body {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if else_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > i && idx <= else_close {
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

/// When the then-branch always returns/throws, drop `else` so later tests sit at
/// the same level (`if (A) return; if (B) return; return;`).
pub(crate) fn flatten_returning_if_else(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..16 {
        let next = flatten_returning_if_else_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn flatten_returning_if_else_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        if parse_if_condition(lines[i]).is_none() {
            continue;
        }
        let Some(then_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        if !block_exits(&lines[i + 1..then_close]) {
            continue;
        }
        let close_trim = lines[then_close].trim();
        let indent = leading_indent(lines[i]);
        if let Some(cond) = parse_else_if_condition(lines[then_close]) {
            let mut out = String::new();
            for (idx, line) in lines.iter().enumerate() {
                if idx == then_close {
                    out.push_str(&format!("{}}}\n{}if ({}) {{", indent, indent, cond));
                } else {
                    out.push_str(line);
                }
                if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
            }
            return out;
        }
        if close_trim != "} else {" {
            continue;
        }
        let Some(else_close) = find_closing_brace_line(&lines, then_close) else {
            continue;
        };
        let else_body = &lines[then_close + 1..else_close];
        let else_first = else_body
            .iter()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim());
        // Keep `if { return } else { return }` as if/else. Only lift an else that
        // is itself an `if` (sequential instanceof-style tests).
        if else_first.is_none_or(|t| !t.starts_with("if (")) {
            continue;
        }
        let if_indent_len = indent.len();
        let extra = else_body
            .iter()
            .find(|l| !l.trim().is_empty())
            .map(|l| leading_indent(l).len().saturating_sub(if_indent_len))
            .unwrap_or(4);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == then_close {
                out.push_str(&format!("{}}}", indent));
                if else_close < lines.len().saturating_sub(1)
                    || body.ends_with('\n')
                    || !else_body.is_empty()
                {
                    out.push('\n');
                }
                continue;
            }
            if idx > then_close && idx < else_close {
                out.push_str(&dedent_line(line, extra));
                out.push('\n');
                continue;
            }
            if idx == else_close {
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

fn block_exits(body: &[&str]) -> bool {
    let Some(last) = body.iter().rev().find(|l| !l.trim().is_empty()) else {
        return false;
    };
    let binding = strip_trailing_comment(last);
    let t = binding.trim();
    is_return_line(last) || (t.starts_with("throw ") && t.ends_with(';'))
}

fn dedent_line(line: &str, spaces: usize) -> String {
    if spaces == 0 {
        return line.to_string();
    }
    let ind = leading_indent(line);
    if ind.len() >= spaces {
        format!("{}{}", &ind[spaces..], &line[ind.len()..])
    } else {
        line.trim_start().to_string()
    }
}

pub(crate) fn looks_like_boolean_local(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 2 && b[0] == b'z' && b[1..].iter().all(|c| c.is_ascii_digit())
}

pub(crate) fn looks_like_primitive_local(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 2
        && matches!(b[0], b'i' | b'j' | b'f' | b'd' | b'c' | b'b')
        && b[1..].iter().all(|c| c.is_ascii_digit())
}

/// Remove `continue;` that immediately precedes a closing `}` when it is redundant
/// (other statements already appear in the same block). Keep a lone `continue;` so
/// `while (…) { continue; }` back-edges still round-trip in tests / empty loop bodies.
/// Remove `continue;` that immediately precedes a closing `}` when it is redundant
/// (other statements already appear in the same block). Keep a lone `continue;` so
/// `while (…) { continue; }` back-edges still round-trip in tests / empty loop bodies.
pub(crate) fn remove_trailing_continues(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut skip = std::collections::HashSet::new();
    for i in 0..lines.len() {
        if lines[i].trim() != "continue;" {
            continue;
        }
        let next = lines[i + 1..].iter().find(|l| !l.trim().is_empty());
        if !next.is_some_and(|l| l.trim().starts_with('}')) {
            continue;
        }
        let cont_indent = leading_indent(lines[i]).len();
        let has_prior_sibling = lines[..i].iter().rev().any(|l| {
            let t = l.trim();
            if t.is_empty() {
                return false;
            }
            let ind = leading_indent(l).len();
            if ind < cont_indent {
                return false; // left the block
            }
            ind == cont_indent && t != "continue;"
        });
        if has_prior_sibling {
            skip.insert(i);
        }
    }
    if skip.is_empty() {
        return body.to_string();
    }
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if skip.contains(&i) {
            continue;
        }
        out.push_str(line);
        if i < lines.len().saturating_sub(1) {
            out.push('\n');
        }
    }
    out
}

/// Merge nested `if (A) { if (B) { BODY } }` (no else on either) into `if (A && B) { BODY }`.
pub(crate) fn merge_nested_if_and(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = merge_nested_if_and_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn merge_nested_if_and_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some(cond_a) = parse_if_condition(lines[i]) else {
            continue;
        };
        let Some(outer_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        if lines[outer_close].trim() != "}" {
            continue;
        }
        let mut j = i + 1;
        while j < outer_close && lines[j].trim().is_empty() {
            j += 1;
        }
        if j >= outer_close {
            continue;
        }
        let Some(cond_b) = parse_if_condition(lines[j]) else {
            continue;
        };
        let Some(inner_close) = find_closing_brace_line(&lines, j) else {
            continue;
        };
        if lines[inner_close].trim() != "}" {
            continue;
        }
        if (inner_close + 1..outer_close).any(|t| !lines[t].trim().is_empty()) {
            continue;
        }
        let indent = leading_indent(lines[i]);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                out.push_str(&format!("{}if ({} && {}) {{\n", indent, cond_a, cond_b));
                for l in &lines[j + 1..inner_close] {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if outer_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > i && idx <= outer_close {
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

/// Split a boolean condition on `&&` or `||` at parenthesis/string depth zero.
pub(crate) fn parse_boolean_return_literal(line: &str) -> Option<bool> {
    let expr = parse_return_expr(line)?;
    match expr.as_str() {
        "0" | "false" => Some(false),
        "1" | "true" => Some(true),
        _ => None,
    }
}

/// `if (!A && !B) { return false; } return true;` → `if (A || B) { return true; } return false;`
/// `if (!A && !B) { return false; } return true;` → `if (A || B) { return true; } return false;`
pub(crate) fn flip_negated_and_boolean_return(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = flip_negated_and_boolean_return_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn flip_negated_and_boolean_return_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some(cond) = parse_if_condition(lines[i]) else {
            continue;
        };
        let Some(pos_cond) = demorgan_or_from_negated_and(&cond) else {
            continue;
        };
        let Some(if_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        if lines[if_close].trim() != "}" {
            continue;
        }
        let then_stmts = normalized_block_stmts(&lines, i + 1, if_close);
        if then_stmts.len() != 1 {
            continue;
        }
        let Some(then_val) = parse_boolean_return_literal(&then_stmts[0]) else {
            continue;
        };
        if then_val {
            continue;
        }
        let mut j = if_close + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
        }
        if j >= lines.len() {
            continue;
        }
        let Some(tail_val) = parse_boolean_return_literal(lines[j]) else {
            continue;
        };
        if !tail_val {
            continue;
        }
        let mut k = j + 1;
        while k < lines.len() && lines[k].trim().is_empty() {
            k += 1;
        }
        if k < lines.len() {
            continue;
        }
        let indent = leading_indent(lines[i]);
        let inner = leading_indent(&then_stmts[0]);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                out.push_str(&format!("{}if ({}) {{\n", indent, pos_cond));
                out.push_str(&format!("{}return true;\n", inner));
                out.push_str(&format!("{}}}\n", indent));
                out.push_str(&format!("{}return false;\n", indent));
                continue;
            }
            if idx > i && idx <= j {
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

/// `if (A) { BODY } else if (B) { BODY }` (no further else) → `if (A || B) { BODY }`
pub(crate) fn merge_short_circuit_or(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = merge_short_circuit_or_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn merge_short_circuit_or_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some(cond_a) = parse_if_condition(lines[i]) else {
            continue;
        };
        let Some(then_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        let else_if_line = then_close;
        let Some(cond_b) = parse_else_if_condition(lines[else_if_line]) else {
            continue;
        };
        let Some(else_close) = find_closing_brace_line(&lines, else_if_line) else {
            continue;
        };
        // find_closing_brace_line on `} else if` returns the else-if body's close; ensure
        // that line isn't `} else {` / `} else if` (would mean a further chain arm).
        let close_trim = lines[else_close].trim();
        if close_trim != "}" {
            continue;
        }
        let then_body = normalized_block_stmts(&lines, i + 1, then_close);
        let else_body = normalized_block_stmts(&lines, else_if_line + 1, else_close);
        if then_body.is_empty() || then_body != else_body {
            continue;
        }
        let indent = leading_indent(lines[i]);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                out.push_str(&format!("{}if ({} || {}) {{\n", indent, cond_a, cond_b));
                for l in &lines[i + 1..then_close] {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if else_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > i && idx <= else_close {
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

/// `if (c) { x = a; } else { x = b; }` → `x = c ? a : b;`
/// `if (c) { x = a; } else { x = b; }` → `x = c ? a : b;`
pub(crate) fn fold_assign_ternary(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = fold_assign_ternary_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn fold_assign_ternary_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some(cond) = parse_if_condition(lines[i]) else {
            continue;
        };
        let Some(then_close) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        if lines[then_close].trim() != "} else {" {
            continue;
        }
        let Some(else_close) = find_closing_brace_line(&lines, then_close) else {
            continue;
        };
        let then_stmts: Vec<&str> = lines[i + 1..then_close]
            .iter()
            .copied()
            .filter(|l| !l.trim().is_empty())
            .collect();
        let else_stmts: Vec<&str> = lines[then_close + 1..else_close]
            .iter()
            .copied()
            .filter(|l| !l.trim().is_empty())
            .collect();
        if then_stmts.len() != 1 || else_stmts.len() != 1 {
            continue;
        }
        let Some((var_a, expr_a)) = parse_simple_assign_line(then_stmts[0]) else {
            continue;
        };
        let Some((var_b, expr_b)) = parse_simple_assign_line(else_stmts[0]) else {
            continue;
        };
        if var_a != var_b {
            continue;
        }
        let lhs = {
            let ta_owned = strip_trailing_comment(then_stmts[0]);
            let tb_owned = strip_trailing_comment(else_stmts[0]);
            let ta = ta_owned.trim();
            let tb = tb_owned.trim();
            let lhs_a = ta.split(" = ").next().unwrap_or(&var_a).trim();
            let lhs_b = tb.split(" = ").next().unwrap_or(&var_b).trim();
            if lhs_a.split_whitespace().count() > 1 {
                lhs_a.to_string()
            } else if lhs_b.split_whitespace().count() > 1 {
                lhs_b.to_string()
            } else {
                var_a.clone()
            }
        };
        let indent = leading_indent(lines[i]);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                out.push_str(&format!(
                    "{}{} = {} ? {} : {};",
                    indent, lhs, cond, expr_a, expr_b
                ));
                if else_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > i && idx <= else_close {
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
