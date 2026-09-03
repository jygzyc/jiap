//! Restore for/foreach/do-while from lowered Dalvik loop shapes.

use std::collections::{HashMap, HashSet};

use super::util::*;

pub(crate) fn parse_while_has_next(line: &str, it: &str) -> bool {
    parse_while_condition(line).is_some_and(|cond| cond == format!("{}.hasNext()", it))
}

/// `Type x = (Type) it.next();` / `x = (Type) it.next();` / `x = it.next();`
/// `Type x = (Type) it.next();` / `x = (Type) it.next();` / `x = it.next();`
pub(crate) fn parse_iterator_next_line(line: &str, it: &str) -> Option<(Option<String>, String)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    let rhs = rhs.trim();
    let next_call = format!("{}.next()", it);
    let ty_opt = if let Some(rest) = rhs.strip_prefix('(') {
        let close = rest.find(')')?;
        let ty = rest[..close].trim();
        let after = rest[close + 1..].trim();
        if after != next_call {
            return None;
        }
        Some(ty.to_string())
    } else if rhs == next_call {
        None
    } else {
        return None;
    };
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let lhs = stmt.split(" = ").next()?.trim();
    let declared_ty = if lhs.split_whitespace().count() > 1 {
        let parts: Vec<&str> = lhs.split_whitespace().collect();
        Some(parts[..parts.len() - 1].join(" "))
    } else {
        ty_opt.clone()
    };
    Some((declared_ty.or(ty_opt), var))
}

/// Restore `for (Type x : coll)` from iterator while-loops.
/// Restore `for (Type x : coll)` from iterator while-loops.
pub(crate) fn restore_foreach_iterator(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = restore_foreach_iterator_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn restore_foreach_iterator_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some((it, coll)) = parse_iterator_assign(lines[i]) else {
            continue;
        };
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
        }
        if j >= lines.len() || !parse_while_has_next(lines[j], &it) {
            continue;
        }
        let Some(while_close) = find_closing_brace_line(&lines, j) else {
            continue;
        };
        let mut k = j + 1;
        while k < while_close && lines[k].trim().is_empty() {
            k += 1;
        }
        if k >= while_close {
            continue;
        }
        let Some((ty_opt, elem_var)) = parse_iterator_next_line(lines[k], &it) else {
            continue;
        };
        let indent = leading_indent(lines[j]);
        let ty = ty_opt.unwrap_or_else(|| "Object".to_string());
        let coll_expr = if coll.is_empty() {
            "/* iterator */".to_string()
        } else {
            coll
        };
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                continue;
            }
            if idx == j {
                out.push_str(&format!(
                    "{}for ({} {} : {}) {{\n",
                    indent, ty, elem_var, coll_expr
                ));
                for l in &lines[k + 1..while_close] {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if while_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > j && idx <= while_close {
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

pub(crate) fn looks_like_index_use(line: &str, var: &str) -> bool {
    if !ident_occurs(line, var) {
        return false;
    }
    let t = strip_trailing_comment(line);
    t.contains(&format!("{} <", var))
        || t.contains(&format!("{} >", var))
        || t.contains(&format!("{} <=", var))
        || t.contains(&format!("{} >=", var))
        || t.contains(&format!("< {}", var))
        || t.contains(&format!("> {}", var))
        || t.contains(&format!("<= {}", var))
        || t.contains(&format!(">= {}", var))
}

pub(crate) fn parse_length_temp_assign(line: &str) -> Option<(String, String)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    let expr = rhs.strip_suffix(".length")?.trim();
    if expr.is_empty() {
        return None;
    }
    Some((var, expr.to_string()))
}

pub(crate) fn parse_array_length_assign(line: &str) -> Option<(String, String)> {
    let (len_var, rhs) = parse_simple_assign_line(line)?;
    let arr = rhs.strip_suffix(".length")?.trim();
    if arr.is_empty() || arr.contains('(') {
        return None;
    }
    if !(is_java_ident(arr) || is_simple_field_path(arr)) {
        return None;
    }
    Some((len_var, arr.to_string()))
}

/// `i = right.length; i = left.length + i; int[] out = new int[i]; i = 0;` →
/// `int[] out = new int[left.length + right.length]; i = 0;`
pub(crate) fn fold_array_alloc_length_sum(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut remove: HashSet<usize> = HashSet::new();
    let mut replacements: HashMap<usize, String> = HashMap::new();
    for (alloc_idx, line) in lines.iter().enumerate() {
        let binding = strip_trailing_comment(line);
        let stmt = binding.trim();
        let Some(eq) = stmt.find(" = ") else { continue };
        let lhs = stmt[..eq].trim();
        let Some(arr_var) = lhs.strip_prefix("int[] ") else {
            continue;
        };
        let arr_var = arr_var.trim();
        if !is_java_ident(arr_var) {
            continue;
        }
        let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
        let Some(size_var) = rhs
            .strip_prefix("new int[")
            .and_then(|s| s.strip_suffix(']'))
        else {
            continue;
        };
        let size_var = size_var.trim();
        if !is_java_ident(size_var) {
            continue;
        }
        let Some(sum_idx) = (0..alloc_idx).rev().find(|&i| {
            parse_simple_assign_line(lines[i]).is_some_and(|(v, r)| {
                v == size_var
                    && r.split('+')
                        .map(str::trim)
                        .filter(|p| !p.is_empty())
                        .count()
                        == 2
                    && r.contains(size_var)
            })
        }) else {
            continue;
        };
        let Some((_, sum_rhs)) = parse_simple_assign_line(lines[sum_idx]) else {
            continue;
        };
        let parts: Vec<&str> = sum_rhs.split('+').map(str::trim).collect();
        if parts.len() != 2 {
            continue;
        }
        let addend = if parts[1] == size_var {
            parts[0]
        } else if parts[0] == size_var {
            parts[1]
        } else {
            continue;
        };
        let Some(base_idx) = (0..sum_idx)
            .rev()
            .find(|&i| parse_simple_assign_line(lines[i]).is_some_and(|(v, _)| v == size_var))
        else {
            continue;
        };
        let Some((_, base_rhs)) = parse_simple_assign_line(lines[base_idx]) else {
            continue;
        };
        let reset_ok = lines.get(alloc_idx + 1).is_some_and(|l| {
            parse_simple_assign_line(l)
                .is_some_and(|(v, r)| v == size_var && matches!(r.trim(), "0" | "0L"))
        });
        if !reset_ok {
            continue;
        }
        let indent = leading_indent(line);
        replacements.insert(
            alloc_idx,
            format!(
                "{}int[] {} = new int[{} + {}];",
                indent, arr_var, addend, base_rhs
            ),
        );
        remove.insert(base_idx);
        remove.insert(sum_idx);
    }
    if remove.is_empty() && replacements.is_empty() {
        return body.to_string();
    }
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if remove.contains(&idx) {
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

pub(crate) fn fold_array_length_assigns(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut length_init_count: HashMap<String, usize> = HashMap::new();
    for line in &lines {
        if let Some((var, _)) = parse_array_length_assign(line) {
            *length_init_count.entry(var).or_insert(0) += 1;
        }
    }
    let mut folds: Vec<(String, String)> = Vec::new();
    for line in &lines {
        if let Some((var, arr)) = parse_array_length_assign(line) {
            folds.push((var, arr));
        }
    }
    if folds.is_empty() {
        return body.to_string();
    }
    let mut remove: HashSet<usize> = HashSet::new();
    let mut subs: HashMap<String, String> = HashMap::new();
    for (var, arr) in folds {
        if subs.contains_key(&var) {
            continue;
        }
        // D8 reuses `length` for successive array-length temps — never fold those.
        if length_init_count.get(&var).copied().unwrap_or(0) != 1 {
            continue;
        }
        let init_idx = lines
            .iter()
            .position(|line| parse_array_length_assign(line).is_some_and(|(v, _)| v == var));
        let Some(init_idx) = init_idx else {
            continue;
        };
        if lines
            .iter()
            .enumerate()
            .any(|(idx, line)| idx != init_idx && line_kills_var(line, &var))
        {
            continue;
        }
        let use_lines: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_idx, line)| {
                !parse_array_length_assign(line).is_some_and(|(v, _)| v == var)
                    && line_uses_ident_as_var(line, &var)
            })
            .map(|(i, _)| i)
            .collect();
        if use_lines.is_empty() {
            continue;
        }
        if use_lines
            .iter()
            .any(|&idx| looks_like_index_use(lines[idx], &var))
        {
            continue;
        }
        subs.insert(var.clone(), format!("{}.length", arr));
        for (idx, line) in lines.iter().enumerate() {
            if parse_array_length_assign(line).is_some_and(|(v, _)| v == var) {
                remove.insert(idx);
            }
        }
    }
    if subs.is_empty() {
        return body.to_string();
    }
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if remove.contains(&idx) {
            continue;
        }
        let mut current = line.to_string();
        for (var, repl) in &subs {
            if assign_lhs_var(&current).as_deref() == Some(var.as_str()) {
                continue;
            }
            if line_uses_ident_as_var(&current, var) {
                current = replace_ident_as_expr(&current, var, repl);
            }
        }
        out.push_str(&current);
        if idx < lines.len().saturating_sub(1) {
            out.push('\n');
        }
    }
    out
}

pub(crate) fn parse_self_add_assign(line: &str) -> Option<(String, String)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    let parts = split_top_level_bool_op(&rhs, "+");
    if parts.len() != 2 || parts[0].trim() != var {
        return None;
    }
    Some((var, parts[1].trim().to_string()))
}

/// `i6 = arr1.length; length = mac.length; i6 = i6 + length; …; return i6 + same;`
/// → `return arr1.length + mac.length + … + (same ? 1 : 0);`
pub(crate) fn fold_length_sum_return(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let Some(ret_idx) = lines
        .iter()
        .enumerate()
        .rposition(|(_, l)| parse_return_expr(l).is_some())
    else {
        return body.to_string();
    };
    let ret_expr = parse_return_expr(lines[ret_idx]).unwrap_or_default();
    let ret_parts = split_top_level_bool_op(&ret_expr, "+");
    let (acc_var, tail) = match ret_parts.as_slice() {
        [acc] => (acc.trim().to_string(), None),
        [acc, tail] => {
            let acc = acc.trim().to_string();
            let tail = tail.trim().to_string();
            if tail == acc {
                (acc, None)
            } else {
                (acc, Some(tail))
            }
        }
        _ => return body.to_string(),
    };
    if !is_java_ident(&acc_var) {
        return body.to_string();
    }

    let mut terms: Vec<String> = Vec::new();
    let mut i = ret_idx;
    loop {
        if i == 0 {
            return body.to_string();
        }
        i -= 1;
        while i > 0 && lines[i].trim().is_empty() {
            i -= 1;
        }
        let line = lines[i];

        if let Some((v, rhs)) = parse_self_add_assign(line) {
            if v != acc_var {
                return body.to_string();
            }
            if i > 0 {
                if let Some((temp_var, expr)) = parse_length_temp_assign(lines[i - 1]) {
                    if temp_var == rhs {
                        terms.push(format!("{}.length", expr));
                        i -= 1;
                        continue;
                    }
                }
            }
            terms.push(rhs);
            continue;
        }

        if parse_self_add_assign(line).is_some() {
            return body.to_string();
        }
        let Some((v, _rhs)) = parse_simple_assign_line(line) else {
            return body.to_string();
        };
        if v != acc_var {
            // Skip unrelated locals between the accumulator chain and return.
            if parse_length_temp_assign(line).is_none() {
                continue;
            }
            return body.to_string();
        }
        terms.push(_rhs);
        break;
    }

    if terms.is_empty() {
        return body.to_string();
    }
    terms.reverse();

    let mut sum = terms.join(" + ");
    if let Some(tail) = tail {
        sum = format!("{} + {}", sum, format_int_add_tail(&lines, &tail));
    }

    let indent = leading_indent(lines[ret_idx]);
    let start_idx = i;
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx < start_idx || idx > ret_idx {
            out.push_str(line);
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
        } else if idx == start_idx {
            out.push_str(&format!("{}return {};", indent, sum));
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

pub(crate) fn parse_array_elem_assign_line(line: &str) -> Option<(String, String, String, String)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let eq = stmt.find(" = ")?;
    let lhs = stmt[..eq].trim();
    let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
    let open = rhs.find('[')?;
    let arr = rhs[..open].trim();
    if arr.is_empty() || arr.contains('(') {
        return None;
    }
    let rest = &rhs[open + 1..];
    let close = rest.find(']')?;
    let idx = rest[..close].trim();
    if !is_java_ident(idx) {
        return None;
    }
    let var = lhs.split_whitespace().last()?.to_string();
    if !is_java_ident(&var) {
        return None;
    }
    let ty = if lhs.split_whitespace().count() > 1 {
        lhs[..lhs.len() - var.len()].trim().to_string()
    } else {
        "int".to_string()
    };
    Some((ty, var, arr.to_string(), idx.to_string()))
}

pub(crate) fn parse_index_lt_bound(cond: &str) -> Option<(String, String)> {
    let c = cond.trim();
    let lt = c.find(" < ")?;
    let idx = c[..lt].trim();
    let bound = c[lt + 3..].trim();
    if !is_java_ident(idx) {
        return None;
    }
    if !(is_java_ident(bound) || is_simple_field_path(bound)) {
        return None;
    }
    Some((idx.to_string(), bound.to_string()))
}

pub(crate) fn resolve_length_bound(
    lines: &[&str],
    before: usize,
    bound: &str,
) -> Option<(String, String)> {
    if bound.ends_with(".length") {
        let arr = bound.strip_suffix(".length")?.trim();
        return Some((bound.to_string(), arr.to_string()));
    }
    for line in lines[..before].iter().rev() {
        if line.trim().starts_with("while (") {
            break;
        }
        if let Some((var, arr)) = parse_array_length_assign(line) {
            if var == bound {
                return Some((var, arr));
            }
        }
    }
    None
}

pub(crate) fn length_var_for_array_before(
    lines: &[&str],
    before: usize,
    arr: &str,
) -> Option<String> {
    for line in lines[..before].iter().rev() {
        if line.trim().starts_with("while (") {
            break;
        }
        if let Some((lv, a)) = parse_array_length_assign(line) {
            if a == arr {
                return Some(lv);
            }
        }
    }
    None
}

pub(crate) fn repair_while_loop_bounds(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..16 {
        let next = repair_while_loop_bounds_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn repair_while_loop_bounds_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut best: Option<(usize, usize)> = None;
    for i in 0..lines.len() {
        if !lines[i].trim().starts_with("while (") {
            continue;
        }
        let Some(end) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        let size = end - i;
        if best.map(|(_, s)| size < s).unwrap_or(true) {
            best = Some((i, end));
        }
    }
    let Some((w, end)) = best else {
        return body.to_string();
    };
    let Some(cond) = parse_while_condition(lines[w]) else {
        return body.to_string();
    };
    let Some((mut idx, mut bound)) = parse_index_lt_bound(&cond) else {
        return body.to_string();
    };
    let mut loop_arr: Option<String> = None;
    for line in &lines[w + 1..end] {
        if let Some((_, _, arr, idx2)) = parse_array_elem_assign_line(line) {
            idx = idx2;
            loop_arr = Some(arr);
            break;
        }
    }
    if let Some(ref arr) = loop_arr {
        if let Some(lv) = length_var_for_array_before(&lines, w, arr) {
            bound = lv;
        }
    }
    for line in &lines[w + 1..end] {
        if let Some((lv, arr)) = parse_array_length_assign(line) {
            if loop_arr.as_ref().is_some_and(|a| a == &arr) {
                bound = lv;
            }
        }
    }
    let new_cond = format!("{} < {}", idx, bound);
    if new_cond == cond {
        return body.to_string();
    }
    let indent = leading_indent(lines[w]);
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i == w {
            out.push_str(&format!("{}while ({}) {{", indent, new_cond));
        } else {
            out.push_str(line);
        }
        if i < lines.len().saturating_sub(1) || body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

pub(crate) fn parse_zero_init_ident(line: &str) -> Option<String> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    if rhs == "0" {
        Some(var)
    } else {
        None
    }
}

pub(crate) fn parse_index_compare_bound(cond: &str) -> Option<(String, String, &'static str)> {
    let c = cond.trim();
    if let Some(pos) = c.find(" <= ") {
        let idx = c[..pos].trim();
        let bound = c[pos + 4..].trim();
        if is_java_ident(idx) && (is_java_ident(bound) || is_simple_field_path(bound)) {
            return Some((idx.to_string(), bound.to_string(), "<="));
        }
    }
    if let Some(pos) = c.find(" < ") {
        let idx = c[..pos].trim();
        let bound = c[pos + 3..].trim();
        if is_java_ident(idx) && (is_java_ident(bound) || is_simple_field_path(bound)) {
            return Some((idx.to_string(), bound.to_string(), "<"));
        }
    }
    if let Some(pos) = c.find(" >= ") {
        let bound = c[..pos].trim();
        let idx = c[pos + 4..].trim();
        if is_java_ident(idx) && (is_java_ident(bound) || is_simple_field_path(bound)) {
            return Some((idx.to_string(), bound.to_string(), "<="));
        }
    }
    if let Some(pos) = c.find(" > ") {
        let bound = c[..pos].trim();
        let idx = c[pos + 3..].trim();
        if is_java_ident(idx) && (is_java_ident(bound) || is_simple_field_path(bound)) {
            return Some((idx.to_string(), bound.to_string(), "<"));
        }
    }
    None
}

pub(crate) fn parse_int_literal_init(line: &str) -> Option<(String, String)> {
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    let rest = t.strip_prefix("int ")?;
    let rest = rest.strip_suffix(';')?.trim();
    let eq = rest.find(" = ")?;
    let var = rest[..eq].trim();
    let val = rest[eq + 3..].trim();
    if is_java_ident(var) && is_cheap_literal_rhs(val) {
        Some((var.to_string(), val.to_string()))
    } else {
        None
    }
}

pub(crate) fn find_int_init_before(
    lines: &[&str],
    while_idx: usize,
    idx_var: &str,
) -> Option<(usize, String)> {
    for i in (0..while_idx).rev() {
        let line = lines[i].trim();
        if line.is_empty() || line == "}" || line == "} else {" {
            continue;
        }
        if line.starts_with("while (") || line.starts_with("for (") || line.starts_with("if (") {
            break;
        }
        if let Some((var, val)) = parse_int_literal_init(lines[i]) {
            if var == idx_var {
                return Some((i, val));
            }
        }
        if let Some((var, rhs)) = parse_int_assign_rhs_line(lines[i]) {
            if var == idx_var && is_java_ident(rhs.trim()) {
                return Some((i, rhs.trim().to_string()));
            }
        }
        if let Some((var, rhs)) = parse_simple_assign_line(lines[i]) {
            if var == idx_var && is_java_ident(rhs.trim()) {
                return Some((i, rhs.trim().to_string()));
            }
        }
        if line_uses_ident_as_var(lines[i], idx_var) {
            break;
        }
    }
    None
}

/// When loop init was dropped (e.g. `j = lo` before `while (j < hi)`), infer from `i = lo - 1`.
/// When loop init was dropped (e.g. `j = lo` before `while (j < hi)`), infer from `i = lo - 1`.
pub(crate) fn infer_counting_loop_init(
    lines: &[&str],
    while_idx: usize,
    idx_var: &str,
    bound: &str,
) -> Option<String> {
    for i in (0..while_idx).rev() {
        let Some(rhs) = parse_int_assign_rhs_line(lines[i])
            .map(|(_, rhs)| rhs)
            .or_else(|| parse_simple_assign_line(lines[i]).map(|(_, rhs)| rhs))
        else {
            continue;
        };
        let Some(base) = rhs
            .trim()
            .strip_suffix(" - 1")
            .or_else(|| rhs.trim().strip_suffix("- 1"))
        else {
            continue;
        };
        let base = base.trim();
        if is_java_ident(base) && base != idx_var && base != bound {
            return Some(base.to_string());
        }
    }
    None
}

pub(crate) fn restore_counting_for_loop_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut whiles: Vec<(usize, usize, usize)> = Vec::new();
    for i in 0..lines.len() {
        if !lines[i].trim().starts_with("while (") {
            continue;
        }
        let Some(end) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        whiles.push((i, end, end - i));
    }
    whiles.sort_by_key(|(_, _, size)| *size);
    for (w, while_close, _) in whiles {
        let Some(cond) = parse_while_condition(lines[w]) else {
            continue;
        };
        let Some((idx_var, bound, cmp)) = parse_index_compare_bound(&cond) else {
            continue;
        };
        let (init_idx, init_val) = if let Some(found) = find_int_init_before(&lines, w, &idx_var) {
            found
        } else if lines[..w]
            .iter()
            .all(|l| assign_lhs_var(l).as_deref() != Some(idx_var.as_str()))
        {
            let Some(inferred) = infer_counting_loop_init(&lines, w, &idx_var, &bound) else {
                continue;
            };
            (w, inferred)
        } else {
            continue;
        };
        let mut last = while_close;
        while last > w + 1 {
            let t = lines[last - 1].trim();
            if t.is_empty() || t == "continue;" || t == "continue" {
                last -= 1;
            } else {
                break;
            }
        }
        let update_idx = last.saturating_sub(1);
        if update_idx <= w || !is_index_increment(lines[update_idx], &idx_var) {
            continue;
        }
        let body_end = update_idx;
        let other_assign = lines[w + 1..update_idx]
            .iter()
            .any(|l| assign_lhs_var(l).as_deref() == Some(idx_var.as_str()));
        if other_assign {
            continue;
        }
        let indent = leading_indent(lines[w]);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == init_idx && init_idx < w {
                continue;
            }
            if idx == w {
                out.push_str(&format!(
                    "{}for (int {} = {}; {} {} {}; {}++) {{\n",
                    indent, idx_var, init_val, idx_var, cmp, bound, idx_var
                ));
                for l in &lines[w + 1..body_end] {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if while_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > w && idx <= while_close {
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

pub(crate) fn restore_counting_for_loop(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = restore_counting_for_loop_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

/// `int v = arr.length - 1;` → `(v, arr)`
/// `int v = arr.length - 1;` → `(v, arr)`
pub(crate) fn parse_length_minus_one_assign(line: &str) -> Option<(String, String)> {
    let (var, rhs) = parse_int_assign_rhs_line(line)?;
    let compact = rhs.replace(' ', "");
    if let Some(arr) = compact.strip_suffix(".length-1") {
        if is_simple_field_path(arr) || is_java_ident(arr) {
            return Some((var, arr.to_string()));
        }
    }
    None
}

pub(crate) fn parse_int_assign_rhs_line(line: &str) -> Option<(String, String)> {
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    let rest = t.strip_prefix("int ")?;
    let rest = rest.strip_suffix(';')?.trim();
    let eq = rest.find(" = ")?;
    let var = rest[..eq].trim();
    let rhs = rest[eq + 3..].trim();
    if is_java_ident(var) {
        Some((var.to_string(), rhs.to_string()))
    } else {
        None
    }
}

pub(crate) fn parse_zero_assign_to(line: &str, var: &str) -> bool {
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    t == format!("{var} = 0;")
        || t == format!("int {var} = 0;")
        || t == format!("{var} = 0")
        || t == format!("int {var} = 0")
}

pub(crate) fn is_bound_computation_do_while(
    lines: &[&str],
    do_open: usize,
    do_close: usize,
) -> bool {
    if lines.get(do_open).is_none_or(|l| l.trim() != "do {") {
        return false;
    }
    let body: Vec<&str> = lines[do_open + 1..do_close]
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if body.is_empty() || body.len() > 4 {
        return false;
    }
    body.iter().all(|l| {
        let t = l.trim();
        t.contains(".length")
            || t.ends_with(" - 1;")
            || t.ends_with(" - 0;")
            || parse_simple_assign_line(l).is_some()
    })
}

pub(crate) fn polish_adjacent_array_swap_block(
    lines: &[&str],
    idx_var: &str,
    arr: &str,
) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let indent = leading_indent(lines[0]);
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if !t.starts_with("if (") || !t.contains(" > ") {
            continue;
        }
        let if_open = i;
        let Some(if_close) = find_closing_brace_line(lines, if_open) else {
            continue;
        };
        let then_body: String = lines[if_open + 1..if_close].join("\n");
        if !then_body.contains('[') || !then_body.contains('=') {
            continue;
        }
        let inner_indent = format!("{indent}    ");
        let mut out = String::new();
        out.push_str(&format!(
            "{inner_indent}if ({arr}[{idx_var}] > {arr}[{idx_var} + 1]) {{\n"
        ));
        out.push_str(&format!(
            "{inner_indent}    int tmp = {arr}[{idx_var}];\n\
             {inner_indent}    {arr}[{idx_var}] = {arr}[{idx_var} + 1];\n\
             {inner_indent}    {arr}[{idx_var} + 1] = tmp;\n\
             {inner_indent}}}\n"
        ));
        return Some(out);
    }
    None
}

/// Recover nested counting for-loops from d8's `while (true) { … if (0 >= n-1) return; else … }`.
/// Recover nested counting for-loops from d8's `while (true) { … if (0 >= n-1) return; else … }`.
pub(crate) fn restore_while_true_nested_for_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for w in 0..lines.len() {
        if lines[w].trim() != "while (true) {" {
            continue;
        }
        let Some(while_close) = find_closing_brace_line(&lines, w) else {
            continue;
        };
        if w + 4 >= while_close {
            continue;
        }
        let (if_line, arr, outer_tmp, has_init_line) =
            if let Some((outer_tmp, arr)) = parse_length_minus_one_assign(lines[w + 1]) {
                (w + 2, arr, outer_tmp, true)
            } else if lines[w + 1].trim().starts_with("if (0 >= ") {
                let if_trim = lines[w + 1].trim();
                let inner = if_trim
                    .strip_prefix("if (0 >= ")
                    .and_then(|s| s.strip_suffix(") {"))
                    .unwrap_or("")
                    .trim();
                let Some(arr_name) = inner.strip_suffix(".length - 1") else {
                    continue;
                };
                if !is_java_ident(arr_name) && !is_simple_field_path(arr_name) {
                    continue;
                }
                (w + 1, arr_name.to_string(), "i".to_string(), false)
            } else {
                continue;
            };
        let if_trim = lines[if_line].trim();
        if !if_trim.starts_with("if (") || !if_trim.ends_with(") {") {
            continue;
        }
        let cond = if_trim
            .strip_prefix("if (")
            .and_then(|s| s.strip_suffix(") {"))
            .unwrap_or("")
            .trim();
        let bound_var = if has_init_line {
            if let Some(inner) = cond.strip_prefix("0 >= ") {
                inner.trim().to_string()
            } else if let Some((_, rhs)) = cond.split_once(" >= ") {
                rhs.trim().to_string()
            } else {
                continue;
            }
        } else {
            outer_tmp.clone()
        };
        if has_init_line && bound_var != outer_tmp {
            continue;
        }
        if !has_init_line && bound_var != "i" && bound_var != outer_tmp {
            continue;
        }
        let Some(then_close) = find_closing_brace_line(&lines, if_line) else {
            continue;
        };
        if then_close >= while_close {
            continue;
        }
        // `find_closing_brace_line` returns the line containing the then-branch `}`,
        // which is usually `} else {` rather than a standalone `}`.
        if lines[then_close].trim() != "} else {" {
            continue;
        }
        let else_open = then_close;
        let Some(else_close) = find_closing_brace_line(&lines, else_open) else {
            continue;
        };
        if else_close + 1 != while_close {
            continue;
        }
        let inner_idx = "j";
        let mut k = else_open + 1;
        while k < else_close && lines[k].trim().is_empty() {
            k += 1;
        }
        if !parse_zero_assign_to(lines[k], inner_idx) && !parse_zero_assign_to(lines[k], &outer_tmp)
        {
            continue;
        }
        k += 1;
        if k < else_close && lines[k].trim() == "do {" {
            if let Some(do_close) = find_closing_brace_line(&lines, k) {
                if do_close + 1 < else_close && lines[do_close + 1].trim().starts_with("} while (")
                {
                    if is_bound_computation_do_while(&lines, k, do_close) {
                        k = do_close + 2;
                    }
                }
            }
        }
        let mut last = else_close;
        while last > k && lines[last - 1].trim().is_empty() {
            last -= 1;
        }
        if last <= k {
            continue;
        }
        let inc_ok = is_index_increment(lines[last - 1], &outer_tmp)
            || is_index_increment(lines[last - 1], inner_idx);
        if !inc_ok {
            continue;
        }
        let inner_body_lines = &lines[k..last - 1];
        let inner_body = polish_adjacent_array_swap_block(inner_body_lines, inner_idx, &arr)
            .unwrap_or_else(|| {
                inner_body_lines
                    .iter()
                    .map(|l| l.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n"
            });
        let indent = leading_indent(lines[w]);
        let inner_indent = format!("{indent}    ");
        let body_indent = format!("{inner_indent}    ");
        let mut out = String::new();
        let skip_init = w > 0
            && (parse_zero_assign_to(lines[w - 1], &outer_tmp)
                || (has_init_line
                    && cond.contains("i >= ")
                    && parse_zero_assign_to(lines[w - 1], "i")));
        for (idx, line) in lines.iter().enumerate() {
            if skip_init && idx == w - 1 {
                continue;
            }
            if idx == w {
                out.push_str(&format!("{indent}int n = {arr}.length;\n"));
                out.push_str(&format!("{indent}for (int i = 0; i < n - 1; i++) {{\n"));
                out.push_str(&format!(
                    "{inner_indent}for (int {inner_idx} = 0; {inner_idx} < n - i - 1; {inner_idx}++) {{\n"
                ));
                for l in inner_body.lines() {
                    if l.trim().is_empty() {
                        out.push('\n');
                    } else {
                        out.push_str(&format!("{body_indent}{}\n", l.trim()));
                    }
                }
                out.push_str(&format!("{inner_indent}}}\n"));
                out.push_str(&format!("{indent}}}\n"));
                continue;
            }
            if idx > w && idx <= while_close {
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

pub(crate) fn restore_while_true_nested_for(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..4 {
        let next = restore_while_true_nested_for_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn restore_foreach_array_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut whiles: Vec<(usize, usize, usize)> = Vec::new();
    for i in 0..lines.len() {
        if !lines[i].trim().starts_with("while (") {
            continue;
        }
        let Some(end) = find_closing_brace_line(&lines, i) else {
            continue;
        };
        whiles.push((i, end, end - i));
    }
    whiles.sort_by_key(|(_, _, size)| *size);
    for (w, while_close, _) in whiles {
        let Some(cond) = parse_while_condition(lines[w]) else {
            continue;
        };
        let Some((idx_var, bound)) = parse_index_lt_bound(&cond) else {
            continue;
        };
        let mut k = w + 1;
        while k < while_close && lines[k].trim().is_empty() {
            k += 1;
        }
        if k >= while_close {
            continue;
        }
        let Some((elem_ty, elem_var, arr2, idx2)) = parse_array_elem_assign_line(lines[k]) else {
            continue;
        };
        if idx2 != idx_var {
            continue;
        }
        let bound_resolved = resolve_length_bound(&lines, w, &bound)
            .or_else(|| length_var_for_array_before(&lines, w, &arr2).map(|lv| (lv, arr2.clone())));
        let Some((_len_var, arr)) = bound_resolved else {
            continue;
        };
        if arr2 != arr {
            continue;
        }
        let mut last = while_close;
        while last > k + 1 {
            let t = lines[last - 1].trim();
            if t.is_empty() || t == "continue;" || t == "continue" {
                last -= 1;
            } else {
                break;
            }
        }
        let mut update_idx = last - 1;
        let has_inc = update_idx > k && is_index_increment(lines[update_idx], &idx_var);
        if !has_inc {
            update_idx = while_close.saturating_sub(1);
            while update_idx > k && lines[update_idx].trim().is_empty() {
                update_idx -= 1;
            }
        }
        if update_idx <= k {
            continue;
        }
        let body_end = if has_inc { update_idx } else { update_idx + 1 };
        let other_use = lines[k + 1..update_idx]
            .iter()
            .any(|l| line_uses_ident_as_var(l, &idx_var) || line_uses_ident_as_var(l, &bound));
        if other_use {
            continue;
        }
        let mut remove: HashSet<usize> = HashSet::new();
        for (idx, line) in lines.iter().enumerate().take(w) {
            if parse_array_length_assign(line).is_some_and(|(v, a)| v == bound || a == arr) {
                remove.insert(idx);
            }
            if parse_zero_init_ident(line).is_some_and(|v| v == idx_var) {
                remove.insert(idx);
            }
        }
        let indent = leading_indent(lines[w]);
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if remove.contains(&idx) {
                continue;
            }
            if idx == w {
                out.push_str(&format!(
                    "{}for ({} {} : {}) {{\n",
                    indent, elem_ty, elem_var, arr
                ));
                for l in &lines[k + 1..body_end] {
                    out.push_str(l);
                    out.push('\n');
                }
                out.push_str(&format!("{}}}", indent));
                if while_close < lines.len().saturating_sub(1) || body.ends_with('\n') {
                    out.push('\n');
                }
                continue;
            }
            if idx > w && idx <= while_close {
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

pub(crate) fn restore_foreach_array(body: &str) -> String {
    let mut current = repair_while_loop_bounds(body);
    for _ in 0..8 {
        let next = restore_foreach_array_once(&current);
        if next == current {
            break;
        }
        current = next;
        current = repair_while_loop_bounds(&current);
    }
    current
}

/// D8 merge: `while (true) { len = a.length; if (i >= len) DRAIN; else if (j < b.length) BODY }`
/// → `while (i < a.length && j < b.length) { BODY } DRAIN`
pub(crate) fn restore_d8_merge_copy_loops(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..6 {
        let next = restore_d8_merge_loop_once(&current);
        let next = restore_d8_merge_drain_once(&next);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn skip_blank(lines: &[&str], mut i: usize, end: usize) -> usize {
    while i < end && lines[i].trim().is_empty() {
        i += 1;
    }
    i
}

struct IfElseBodies {
    then_lo: usize,
    then_hi: usize,
    else_lo: usize,
    else_hi: usize,
    _close: usize,
}

fn find_if_else_bodies(lines: &[&str], if_idx: usize) -> Option<IfElseBodies> {
    let mut depth = 0i32;
    let mut seen_open = false;
    let mut else_line = None;
    let mut close = None;
    for (i, line) in lines.iter().enumerate().skip(if_idx) {
        let t = line.trim();
        let opens = t.chars().filter(|c| *c == '{').count() as i32;
        let closes = t.chars().filter(|c| *c == '}').count() as i32;
        if !seen_open && opens > 0 {
            seen_open = true;
        }
        if t.contains("} else") && seen_open && depth == 1 && else_line.is_none() {
            else_line = Some(i);
        }
        if seen_open {
            depth += opens - closes;
            if depth <= 0 {
                close = Some(i);
                break;
            }
        }
    }
    let close = close?;
    let else_line = else_line?;
    Some(IfElseBodies {
        then_lo: if_idx + 1,
        then_hi: else_line,
        else_lo: else_line + 1,
        else_hi: close,
        _close: close,
    })
}

fn strip_trailing_continues_in_range(lines: &[&str], lo: usize, hi: usize) -> String {
    let mut body_lines: Vec<&str> = lines[lo..hi].iter().copied().collect();
    while let Some(last) = body_lines.last() {
        let t = last.trim();
        if t.is_empty() || t == "continue;" || t == "continue" {
            body_lines.pop();
        } else {
            break;
        }
    }
    body_lines.join("\n")
}

fn restore_d8_merge_loop_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for w in 0..lines.len() {
        if lines[w].trim() != "while (true) {" {
            continue;
        }
        let Some(while_close) = find_closing_brace_line(&lines, w) else {
            continue;
        };
        let mut i = skip_blank(&lines, w + 1, while_close);
        if i >= while_close {
            continue;
        }
        let mut left_arr = "left".to_string();
        let mut len_var: Option<String> = None;
        if let Some((lv, arr)) = parse_array_length_assign(lines[i]) {
            left_arr = arr;
            len_var = Some(lv);
            i = skip_blank(&lines, i + 1, while_close);
            if i >= while_close {
                continue;
            }
        }
        let Some(cond) = parse_if_condition(lines[i]) else {
            continue;
        };
        let Some((idx_i, bound)) = cond.split_once(" >= ").map(|(a, b)| (a.trim(), b.trim()))
        else {
            continue;
        };
        if let Some(arr) = bound.strip_suffix(".length") {
            left_arr = arr.trim().to_string();
        } else if len_var.as_deref() != Some(bound) && bound != left_arr.as_str() {
            // reused length temp after the assign was dropped — still a merge-shaped exit test
            if !is_java_ident(bound) {
                continue;
            }
        }
        let Some(bodies) = find_if_else_bodies(&lines, i) else {
            continue;
        };
        let else_line = lines.get(bodies.then_hi).copied().unwrap_or("");
        let (idx_j, right_arr, merge_lo, merge_hi) =
            if let Some(cond_j) = parse_else_if_condition(else_line) {
                let Some((idx_j, bound_j)) =
                    cond_j.split_once(" < ").map(|(a, b)| (a.trim(), b.trim()))
                else {
                    continue;
                };
                let right_arr = bound_j
                    .strip_suffix(".length")
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "right".to_string());
                (idx_j.to_string(), right_arr, bodies.else_lo, bodies.else_hi)
            } else {
                let else_start = skip_blank(&lines, bodies.else_lo, bodies.else_hi);
                if else_start >= bodies.else_hi {
                    continue;
                }
                let mut right_arr = "right".to_string();
                let mut j_if = else_start;
                if let Some((lv2, arr)) = parse_array_length_assign(lines[else_start]) {
                    if len_var.as_deref().is_some_and(|v| v != lv2) && len_var.is_some() {
                        // still ok — D8 reuses the same temp
                    }
                    right_arr = arr;
                    j_if = skip_blank(&lines, else_start + 1, bodies.else_hi);
                }
                if j_if >= bodies.else_hi {
                    continue;
                }
                let Some(cond_j) = parse_if_condition(lines[j_if]) else {
                    continue;
                };
                let Some((idx_j, bound_j)) =
                    cond_j.split_once(" < ").map(|(a, b)| (a.trim(), b.trim()))
                else {
                    continue;
                };
                if let Some(arr) = bound_j.strip_suffix(".length") {
                    right_arr = arr.trim().to_string();
                }
                let inner = find_if_else_bodies(&lines, j_if).or_else(|| {
                    let close = find_closing_brace_line(&lines, j_if)?;
                    Some(IfElseBodies {
                        then_lo: j_if + 1,
                        then_hi: close,
                        else_lo: close,
                        else_hi: close,
                        _close: close,
                    })
                });
                let Some(inner) = inner else {
                    continue;
                };
                (idx_j.to_string(), right_arr, inner.then_lo, inner.then_hi)
            };
        let merge_body = strip_trailing_continues_in_range(&lines, merge_lo, merge_hi);
        if merge_body.trim().is_empty() {
            continue;
        }
        let drain = strip_trailing_continues_in_range(&lines, bodies.then_lo, bodies.then_hi);
        let indent = leading_indent(lines[w]);
        let mut rebuilt = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == w {
                rebuilt.push_str(&format!(
                    "{indent}while ({idx_i} < {left_arr}.length && {idx_j} < {right_arr}.length) {{\n"
                ));
                for bl in merge_body.lines() {
                    rebuilt.push_str(bl);
                    rebuilt.push('\n');
                }
                rebuilt.push_str(&format!("{indent}}}\n"));
                if !drain.trim().is_empty() {
                    for bl in drain.lines() {
                        rebuilt.push_str(bl);
                        rebuilt.push('\n');
                    }
                }
                continue;
            }
            if idx > w && idx <= while_close {
                continue;
            }
            rebuilt.push_str(line);
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                rebuilt.push('\n');
            }
        }
        return rebuilt;
    }
    body.to_string()
}

fn restore_d8_merge_drain_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for w in 0..lines.len() {
        if lines[w].trim() != "while (true) {" {
            continue;
        }
        let Some(while_close) = find_closing_brace_line(&lines, w) else {
            continue;
        };
        let mut i = skip_blank(&lines, w + 1, while_close);
        if i >= while_close {
            continue;
        }
        let mut left_arr = "left".to_string();
        let mut len_var: Option<String> = None;
        if let Some((lv, arr)) = parse_array_length_assign(lines[i]) {
            left_arr = arr;
            len_var = Some(lv);
            i = skip_blank(&lines, i + 1, while_close);
            if i >= while_close {
                continue;
            }
        }
        let Some(cond) = parse_if_condition(lines[i]) else {
            continue;
        };
        let Some((idx_i, bound)) = cond.split_once(" >= ").map(|(a, b)| (a.trim(), b.trim()))
        else {
            continue;
        };
        if let Some(arr) = bound.strip_suffix(".length") {
            left_arr = arr.trim().to_string();
        } else if len_var.as_deref() != Some(bound) && !is_java_ident(bound) {
            continue;
        }
        let Some(bodies) = find_if_else_bodies(&lines, i) else {
            continue;
        };
        let left_store = strip_trailing_continues_in_range(&lines, bodies.else_lo, bodies.else_hi);
        if !left_store.contains(&format!("{left_arr}[")) {
            continue;
        }
        let then = skip_blank(&lines, bodies.then_lo, bodies.then_hi);
        if then >= bodies.then_hi || !lines[then].trim().starts_with("while (") {
            continue;
        }
        let Some(inner_close) = find_closing_brace_line(&lines, then) else {
            continue;
        };
        let inner_body = strip_trailing_continues_in_range(&lines, then + 1, inner_close);
        if !inner_body.contains("right[") {
            continue;
        }
        let cond_j = parse_while_condition(lines[then]).unwrap_or_default();
        let idx_j = cond_j
            .split_once(" < ")
            .map(|(a, _)| a.trim().to_string())
            .unwrap_or_else(|| "j".to_string());
        let mut ret = String::new();
        for line in lines[inner_close + 1..bodies.then_hi].iter() {
            if line.trim().starts_with("return ") {
                ret = (*line).to_string();
                break;
            }
        }
        let indent = leading_indent(lines[w]);
        let mut rebuilt = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == w {
                rebuilt.push_str(&format!("{indent}while ({idx_i} < {left_arr}.length) {{\n"));
                for bl in left_store.lines() {
                    rebuilt.push_str(bl);
                    rebuilt.push('\n');
                }
                rebuilt.push_str(&format!("{indent}}}\n"));
                rebuilt.push_str(&format!("{indent}while ({idx_j} < right.length) {{\n"));
                for bl in inner_body.lines() {
                    if parse_array_length_assign(bl).is_some() {
                        continue;
                    }
                    rebuilt.push_str(bl);
                    rebuilt.push('\n');
                }
                rebuilt.push_str(&format!("{indent}}}\n"));
                if !ret.is_empty() {
                    rebuilt.push_str(&ret);
                    rebuilt.push('\n');
                }
                continue;
            }
            if idx > w && idx <= while_close {
                continue;
            }
            rebuilt.push_str(line);
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                rebuilt.push('\n');
            }
        }
        return rebuilt;
    }
    body.to_string()
}
