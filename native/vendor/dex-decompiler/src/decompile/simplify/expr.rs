//! Cast, string-concat, and array-literal polish passes.

use std::collections::{HashMap, HashSet};

use super::util::*;

/// Strip obvious redundant casts: `(T) new T(...)`, `(T)(T)x`, typed assign forms.
pub(crate) fn strip_redundant_casts(body: &str) -> String {
    let mut out = String::new();
    let line_count = body.lines().count();
    for (idx, line) in body.lines().enumerate() {
        let binding = strip_trailing_comment(line);
        let stmt = binding.trim();
        let comment = line.get(binding.len()..).unwrap_or("");
        let indent = leading_indent(line);

        if let Some(new_stmt) = rewrite_redundant_casts_stmt(stmt) {
            out.push_str(indent);
            out.push_str(&new_stmt);
            out.push_str(comment);
        } else {
            out.push_str(line);
        }
        if idx < line_count.saturating_sub(1) {
            out.push('\n');
        }
    }
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn rewrite_redundant_casts_stmt(stmt: &str) -> Option<String> {
    if stmt.is_empty() {
        return None;
    }
    let ends_semi = stmt.ends_with(';');
    let core = stmt.trim_end_matches(';').trim();
    let rewritten = if let Some(eq) = core.find(" = ") {
        let lhs = core[..eq].trim();
        let rhs = core[eq + 3..].trim();
        let mut new_rhs = strip_redundant_casts_expr(rhs);
        new_rhs = strip_casts_anywhere(&new_rhs);
        // `Type x = (Type) …` when cast type matches declared type
        let new_rhs = if let Some(decl_ty) = lhs.rsplit_once(' ').map(|(t, _)| t.trim()) {
            strip_matching_decl_cast(decl_ty, &new_rhs)
        } else {
            new_rhs
        };
        if new_rhs == rhs {
            return None;
        }
        format!("{} = {}", lhs, new_rhs)
    } else {
        let mut new_core = strip_redundant_casts_expr(core);
        new_core = strip_casts_anywhere(&new_core);
        if new_core == core {
            return None;
        }
        new_core
    };
    Some(if ends_semi {
        format!("{};", rewritten)
    } else {
        rewritten
    })
}

/// Scan an expression/statement for `(T) new T(...)` / `(T)(T)x` / `(T) null` substrings.
/// Scan an expression/statement for `(T) new T(...)` / `(T)(T)x` / `(T) null` substrings.
pub(crate) fn strip_casts_anywhere(expr: &str) -> String {
    let mut e = expr.to_string();
    for _ in 0..16 {
        let next = strip_casts_anywhere_once(&e);
        if next == e {
            break;
        }
        e = next;
    }
    e
}

pub(crate) fn strip_casts_anywhere_once(expr: &str) -> String {
    // Find `(Type)` prefixes and try local strip rules at each site.
    let bytes = expr.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            if let Some((ty, rest_start)) = split_leading_cast_at(expr, i) {
                let after_cast = &expr[rest_start..];
                let trimmed = after_cast.trim_start();
                let rest_off = rest_start + (after_cast.len() - trimmed.len());
                // `(T)(T)…`
                if let Some((ty2, rest2_start_rel)) = split_leading_cast_at(expr, rest_off) {
                    if types_equal_for_cast(&ty, &ty2) {
                        let mut out = String::new();
                        out.push_str(&expr[..i]);
                        out.push('(');
                        out.push_str(&ty);
                        out.push(')');
                        out.push(' ');
                        out.push_str(expr[rest2_start_rel..].trim_start());
                        return out;
                    }
                }
                // `(T) new T(...)`
                if looks_like_new_of_type(&expr[rest_off..], &ty) {
                    let mut out = String::new();
                    out.push_str(&expr[..i]);
                    out.push_str(&expr[rest_off..]);
                    return out;
                }
                // `(T) null`
                if trimmed == "null"
                    || trimmed.starts_with("null;")
                    || trimmed.starts_with("null)")
                    || trimmed.starts_with("null,")
                    || trimmed.starts_with("null ")
                {
                    let mut out = String::new();
                    out.push_str(&expr[..i]);
                    out.push_str("null");
                    out.push_str(&trimmed["null".len()..]);
                    return out;
                }
                i = rest_start;
                continue;
            }
        }
        i += 1;
    }
    expr.to_string()
}

pub(crate) fn split_leading_cast_at(expr: &str, start: usize) -> Option<(String, usize)> {
    let e = &expr[start..];
    if !e.starts_with('(') {
        return None;
    }
    let mut depth = 0i32;
    let mut end_byte = None;
    for (i, c) in e.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end_byte = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end_byte = end_byte?;
    let ty = e[1..end_byte].trim();
    if ty.is_empty() || !looks_like_type_name(ty) {
        return None;
    }
    let rest_rel = end_byte + 1;
    if rest_rel >= e.len() {
        return None;
    }
    let rest = e[rest_rel..].trim_start();
    if rest.is_empty() {
        return None;
    }
    // Avoid `(a + b)` grouping
    let first = rest.chars().next()?;
    if first == '.' || first == '[' || first == ')' {
        return None;
    }
    // Cast must be followed by something that isn't an infix operator starting immediately
    // without a cast-like operand (new, ident, '(', null)
    if !(first.is_ascii_alphabetic()
        || first == '_'
        || first == '('
        || first == '"'
        || first == '\'')
    {
        return None;
    }
    Some((ty.to_string(), start + rest_rel))
}

pub(crate) fn strip_matching_decl_cast(decl_ty: &str, rhs: &str) -> String {
    if let Some((cast_ty, inner)) = split_leading_cast(rhs) {
        if types_equal_for_cast(decl_ty, &cast_ty) {
            // `Type x = (Type) new Type(...)` or `(Type) (Type) y` already partially stripped
            if inner.trim().starts_with("new ") || split_leading_cast(inner.trim()).is_none() {
                // Prefer dropping when new Type or simple expr; keep one cast if inner still cast of different type
                if let Some((inner_ty, _)) = split_leading_cast(inner.trim()) {
                    if types_equal_for_cast(decl_ty, &inner_ty) {
                        return strip_redundant_casts_expr(&inner);
                    }
                } else if looks_like_new_of_type(inner.trim(), decl_ty) || inner.trim() == "null" {
                    return strip_redundant_casts_expr(&inner);
                }
            }
        }
    }
    rhs.to_string()
}

pub(crate) fn types_equal_for_cast(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.trim()
            .replace(" ", "")
            .trim_start_matches("java.lang.")
            .to_string()
    };
    norm(a) == norm(b)
}

pub(crate) fn looks_like_new_of_type(expr: &str, ty: &str) -> bool {
    let e = expr.trim();
    let Some(rest) = e.strip_prefix("new ") else {
        return false;
    };
    let rest = rest.trim_start();
    let ty_simple = ty.rsplit('.').next().unwrap_or(ty).trim();
    rest.starts_with(ty) || rest.starts_with(ty_simple)
}

pub(crate) fn split_leading_cast(expr: &str) -> Option<(String, String)> {
    let e = expr.trim();
    if !e.starts_with('(') {
        return None;
    }
    let mut depth = 0i32;
    let mut end = None;
    for (i, c) in e.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let ty = e[1..end].trim();
    if ty.is_empty() || !looks_like_type_name(ty) {
        return None;
    }
    let rest = e[end + 1..].trim();
    if rest.is_empty() {
        return None;
    }
    // Avoid treating `(a + b)` as a cast
    if rest.starts_with('.') || rest.starts_with('[') || rest.starts_with(')') {
        return None;
    }
    Some((ty.to_string(), rest.to_string()))
}

pub(crate) fn looks_like_type_name(ty: &str) -> bool {
    let t = ty.trim();
    if t.is_empty() {
        return false;
    }
    // Reject operators / keywords used as casts accidentally
    if t.contains("==")
        || t.contains("&&")
        || t.contains("||")
        || t.contains('+')
        || t.contains(' ')
    {
        // Allow generic `List<String>` and arrays `int[]`
        if !(t.contains('<') || t.contains('[') || t.contains('.')) {
            return false;
        }
        // `Final String` is not a type cast
        if t.split_whitespace().count() > 1 && !t.contains('<') && !t.contains('[') {
            return false;
        }
    }
    let first = t.chars().next().unwrap();
    first.is_ascii_alphabetic() || first == '_'
}

pub(crate) fn strip_redundant_casts_expr(expr: &str) -> String {
    let mut e = expr.trim().to_string();
    for _ in 0..8 {
        let next = strip_redundant_casts_expr_once(&e);
        if next == e {
            break;
        }
        e = next;
    }
    e
}

pub(crate) fn strip_redundant_casts_expr_once(expr: &str) -> String {
    let e = expr.trim();
    // `(T)(T)x` / `(T) (T) x`
    if let Some((ty1, rest)) = split_leading_cast(e) {
        if let Some((ty2, inner)) = split_leading_cast(rest.trim()) {
            if types_equal_for_cast(&ty1, &ty2) {
                return format!("({}) {}", ty1, inner);
            }
        }
        // `(T) new T(...)`
        if looks_like_new_of_type(rest.trim(), &ty1) {
            return rest.trim().to_string();
        }
        // `(T) null` → `null`
        if rest.trim() == "null" {
            return "null".to_string();
        }
    }
    e.to_string()
}

/// Turn StringConcatFactory / invoke-custom makeConcat leftovers into `a + b + c`.
/// Turn StringConcatFactory / invoke-custom makeConcat leftovers into `a + b + c`.
pub(crate) fn restore_string_concat_indy(body: &str) -> String {
    let mut out = String::new();
    let line_count = body.lines().count();
    for (idx, line) in body.lines().enumerate() {
        let binding = strip_trailing_comment(line);
        let stmt = binding.trim();
        let comment = line.get(binding.len()..).unwrap_or("");
        let indent = leading_indent(line);
        if let Some(new_stmt) = rewrite_string_concat_indy_stmt(stmt) {
            out.push_str(indent);
            out.push_str(&new_stmt);
            out.push_str(comment);
        } else {
            out.push_str(line);
        }
        if idx < line_count.saturating_sub(1) {
            out.push('\n');
        }
    }
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn rewrite_string_concat_indy_stmt(stmt: &str) -> Option<String> {
    if stmt.is_empty() {
        return None;
    }
    let ends_semi = stmt.ends_with(';');
    let core = stmt.trim_end_matches(';').trim();
    let rewritten = if let Some(eq) = core.find(" = ") {
        let lhs = core[..eq].trim();
        let rhs = core[eq + 3..].trim();
        let new_rhs = rewrite_string_concat_indy_expr(rhs)?;
        format!("{} = {}", lhs, new_rhs)
    } else if let Some(ret) = core.strip_prefix("return ") {
        let new_e = rewrite_string_concat_indy_expr(ret.trim())?;
        format!("return {}", new_e)
    } else {
        rewrite_string_concat_indy_expr(core)?
    };
    Some(if ends_semi {
        format!("{};", rewritten)
    } else {
        rewritten
    })
}

pub(crate) fn rewrite_string_concat_indy_expr(expr: &str) -> Option<String> {
    let e = expr.trim();
    // `/* invoke-custom makeConcat… */ (a, b, c)`
    if let Some(idx) = e.find("/*") {
        if let Some(end) = e[idx..].find("*/") {
            let comment = &e[idx..idx + end + 2];
            let lower = comment.to_ascii_lowercase();
            if lower.contains("makeconcat") || lower.contains("stringconcatfactory") {
                let rest = e[idx + end + 2..].trim();
                if let Some(args) = extract_paren_arg_list(rest) {
                    if !args.is_empty() {
                        return Some(args.join(" + "));
                    }
                }
            }
        }
    }
    // `StringConcatFactory.makeConcat(…)` / `makeConcatWithConstants(…)`
    for prefix in [
        "java.lang.invoke.StringConcatFactory.makeConcatWithConstants(",
        "java.lang.invoke.StringConcatFactory.makeConcat(",
        "StringConcatFactory.makeConcatWithConstants(",
        "StringConcatFactory.makeConcat(",
    ] {
        if let Some(rest) = e.strip_prefix(prefix) {
            if !rest.ends_with(')') {
                continue;
            }
            let inner = &rest[..rest.len() - 1];
            let args = split_top_level_args(inner);
            if !args.is_empty() {
                // makeConcatWithConstants often has recipe string first — drop string literal recipe if present
                let parts: Vec<String> =
                    if prefix.contains("WithConstants") && args[0].starts_with('"') {
                        args[1..].to_vec()
                    } else {
                        args
                    };
                if !parts.is_empty() {
                    return Some(parts.join(" + "));
                }
            }
        }
    }
    None
}

/// Rewrite legacy `vN = { 1, 2 };` to `vN = new int[]{ 1, 2 };`.
pub(crate) fn polish_fill_array_initializers(body: &str) -> String {
    let mut out = String::new();
    let line_count = body.lines().count();
    for (idx, line) in body.lines().enumerate() {
        let binding = strip_trailing_comment(line);
        let stmt = binding.trim();
        let comment = line.get(binding.len()..).unwrap_or("");
        if let Some(new_stmt) = rewrite_bare_array_init(stmt) {
            out.push_str(leading_indent(line));
            out.push_str(&new_stmt);
            out.push_str(comment);
        } else {
            out.push_str(line);
        }
        if idx < line_count.saturating_sub(1) {
            out.push('\n');
        }
    }
    if body.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub(crate) fn rewrite_bare_array_init(stmt: &str) -> Option<String> {
    if !stmt.ends_with(';') {
        return None;
    }
    let eq = stmt.find(" = ")?;
    let lhs = stmt[..eq].trim();
    let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
    if !rhs.starts_with('{') || !rhs.ends_with('}') || rhs.contains("new ") {
        return None;
    }
    let inner = rhs[1..rhs.len() - 1].trim();
    let ty = if inner.split(',').any(|e| {
        let e = e.trim();
        e.ends_with('L') || e.ends_with('l')
    }) {
        "long"
    } else if inner.split(',').any(|e| {
        let e = e.trim();
        e.contains('.')
            || e.ends_with('f')
            || e.ends_with('F')
            || e.ends_with('d')
            || e.ends_with('D')
    }) {
        "double"
    } else {
        "int"
    };
    Some(format!("{} = new {}[]{{ {} }};", lhs, ty, inner))
}

pub(crate) fn parse_new_array_literal_rhs(rhs: &str) -> Option<(String, String)> {
    let rhs = rhs.trim();
    if !rhs.starts_with("new ") {
        return None;
    }
    let after_new = rhs.strip_prefix("new ")?.trim();
    let bracket = after_new.find("[]")?;
    let elem_ty = after_new[..bracket].trim();
    if elem_ty.is_empty() || elem_ty.contains(' ') {
        return None;
    }
    let rest = after_new[bracket + 2..].trim();
    let inner = rest.strip_prefix('{')?.trim();
    let inner = inner.strip_suffix('}')?.trim();
    Some((elem_ty.to_string(), inner.to_string()))
}

pub(crate) fn parse_1d_array_literal_init(line: &str) -> Option<(String, String, String)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    if !stmt.ends_with(';') {
        return None;
    }
    let eq = stmt.find(" = ")?;
    let lhs = stmt[..eq].trim();
    let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
    let pos = lhs.find("[]")?;
    if lhs[pos + 2..].contains("[]") {
        return None;
    }
    let elem_ty = lhs[..pos].trim();
    let var = lhs[pos + 2..].trim();
    if !is_java_ident(var) || elem_ty.is_empty() {
        return None;
    }
    let (ty2, literal) = parse_new_array_literal_rhs(rhs)?;
    if ty2 != elem_ty {
        return None;
    }
    Some((var.to_string(), elem_ty.to_string(), literal))
}

pub(crate) fn parse_2d_array_row_refs_init(line: &str) -> Option<(String, String, Vec<String>)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    if !stmt.ends_with(';') {
        return None;
    }
    let eq = stmt.find(" = ")?;
    let lhs = stmt[..eq].trim();
    let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
    let suffix = "[][]";
    let pos = lhs.rfind(suffix)?;
    let elem_ty = lhs[..pos].trim();
    let var = lhs[pos + suffix.len()..].trim();
    if !is_java_ident(var) || elem_ty.is_empty() {
        return None;
    }
    let rhs = rhs.trim();
    let prefix = format!("new {}[][]{{", elem_ty);
    let prefix_sp = format!("new {}[][] {{", elem_ty);
    let inner = rhs
        .strip_prefix(&prefix)
        .or_else(|| rhs.strip_prefix(&prefix_sp))?;
    let inner = inner.strip_suffix('}')?.trim();
    if inner.is_empty() {
        return None;
    }
    let refs: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).collect();
    if !refs.iter().all(|r| is_java_ident(r)) {
        return None;
    }
    Some((var.to_string(), elem_ty.to_string(), refs))
}

/// `int[] a = new int[]{1}; int[][] m = new int[][]{ a };` → nested literal rows when each
/// row array is a single-use literal initializer.
pub(crate) fn fold_2d_array_row_literals_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for idx in 0..lines.len() {
        let Some((dest_var, elem_ty, row_refs)) = parse_2d_array_row_refs_init(lines[idx]) else {
            continue;
        };
        let mut row_lits: Vec<(usize, String)> = Vec::new();
        let mut ok = true;
        for row_var in &row_refs {
            let mut matched = false;
            for j in (0..idx).rev() {
                let Some((v, ty, lit)) = parse_1d_array_literal_init(lines[j]) else {
                    continue;
                };
                if &v != row_var || ty != elem_ty {
                    continue;
                }
                if count_var_uses(&lines, j + 1, lines.len(), row_var) != 1 {
                    continue;
                }
                if !line_uses_ident_as_var(lines[idx], row_var) {
                    continue;
                }
                row_lits.push((j, lit));
                matched = true;
                break;
            }
            if !matched {
                ok = false;
                break;
            }
        }
        if !ok || row_lits.len() != row_refs.len() {
            continue;
        }
        let mut lit_by_var: HashMap<String, String> = HashMap::new();
        for (j, lit) in &row_lits {
            if let Some((v, _, _)) = parse_1d_array_literal_init(lines[*j]) {
                lit_by_var.insert(v, lit.clone());
            }
        }
        let rows: Vec<String> = row_refs
            .iter()
            .filter_map(|rv| lit_by_var.get(rv).map(|lit| format!("{{ {} }}", lit)))
            .collect();
        if rows.len() != row_refs.len() {
            continue;
        }
        let indent = leading_indent(lines[idx]);
        let remove: HashSet<usize> = row_lits.iter().map(|(j, _)| *j).collect();
        let new_line = format!(
            "{}{}[][] {} = new {}[][]{{ {} }};",
            indent,
            elem_ty,
            dest_var,
            elem_ty,
            rows.join(", ")
        );
        let mut out = String::new();
        for (i, line) in lines.iter().enumerate() {
            if remove.contains(&i) {
                continue;
            }
            if i == idx {
                out.push_str(&new_line);
            } else {
                out.push_str(line);
            }
            if i < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
        }
        return out;
    }
    body.to_string()
}

pub(crate) fn fold_2d_array_row_literals(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = fold_2d_array_row_literals_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

/// `Integer[] a = new Integer[2]; a[0]=Integer.valueOf(4); a[1]=Integer.valueOf(5); Arrays.asList(a)`
/// → `Arrays.asList(4, 5)` when `a` is only used by that `asList` call.
pub(crate) fn fold_arrays_as_list_filled_array(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = fold_arrays_as_list_filled_array_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn fold_arrays_as_list_filled_array_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some((arr, _elem_ty, size)) = parse_sized_new_array(lines[i]) else {
            continue;
        };
        if size == 0 || size > 32 {
            continue;
        }
        let mut elems: Vec<Option<String>> = vec![None; size];
        let mut store_idxs: Vec<usize> = Vec::new();
        let mut j = i + 1;
        while j < lines.len() && store_idxs.len() < size {
            let t = lines[j].trim();
            if t.is_empty() || t.starts_with("//") {
                j += 1;
                continue;
            }
            let Some((store_arr, idx_s, val)) = parse_array_elem_store(lines[j]) else {
                break;
            };
            if store_arr != arr {
                break;
            }
            let Ok(idx) = idx_s.parse::<usize>() else {
                break;
            };
            if idx >= size || elems[idx].is_some() {
                break;
            }
            elems[idx] = Some(strip_box_value_of(&val));
            store_idxs.push(j);
            j += 1;
        }
        if elems.iter().any(|e| e.is_none()) {
            continue;
        }
        // Single use: Arrays.asList(arr) somewhere after the stores.
        let mut use_line: Option<usize> = None;
        let mut use_count = 0usize;
        for (k, line) in lines.iter().enumerate() {
            if k == i || store_idxs.contains(&k) {
                continue;
            }
            if !ident_occurs(line, &arr) {
                continue;
            }
            use_count += 1;
            if use_count == 1 && line_is_arrays_as_list_of(line, &arr) {
                use_line = Some(k);
            }
        }
        let Some(use_i) = use_line else {
            continue;
        };
        if use_count != 1 {
            continue;
        }
        let args: Vec<String> = elems.into_iter().map(|e| e.unwrap()).collect();
        let as_list = format!("Arrays.asList({})", args.join(", "));
        let skip: HashSet<usize> = std::iter::once(i)
            .chain(store_idxs.iter().copied())
            .collect();
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if skip.contains(&idx) {
                continue;
            }
            if idx == use_i {
                let replaced = replace_arrays_as_list_arg(line, &arr, &as_list);
                out.push_str(&replaced);
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

fn parse_sized_new_array(line: &str) -> Option<(String, String, usize)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    let rhs = rhs.trim();
    let rest = rhs.strip_prefix("new ")?;
    let open = rest.find('[')?;
    let close = rest.rfind(']')?;
    if close != rest.len() - 1 {
        return None;
    }
    // Reject filled form `new T[]{…}`
    if rest[..open].trim().ends_with(']') {
        return None;
    }
    let ty = rest[..open].trim();
    if ty.is_empty() {
        return None;
    }
    let size: usize = rest[open + 1..close].trim().parse().ok()?;
    Some((var, ty.to_string(), size))
}

fn parse_array_elem_store(line: &str) -> Option<(String, String, String)> {
    let binding = strip_trailing_comment(line);
    let t = binding.trim().strip_suffix(';')?;
    let eq = t.find(" = ")?;
    let lhs = t[..eq].trim();
    let rhs = t[eq + 3..].trim();
    let open = lhs.find('[')?;
    let close = lhs.rfind(']')?;
    if close + 1 != lhs.len() {
        return None;
    }
    let arr = lhs[..open].trim();
    let idx = lhs[open + 1..close].trim();
    if !is_java_ident(arr) || arr.is_empty() || idx.is_empty() {
        return None;
    }
    Some((arr.to_string(), idx.to_string(), rhs.to_string()))
}

fn line_is_arrays_as_list_of(line: &str, arr: &str) -> bool {
    let binding = strip_trailing_comment(line);
    let needle = format!("Arrays.asList({})", arr);
    let needle_fq = format!("java.util.Arrays.asList({})", arr);
    binding.contains(&needle) || binding.contains(&needle_fq)
}

/// Replace `Arrays.asList(arr)` / FQN form with `replacement` (already an asList call).
fn replace_arrays_as_list_arg(line: &str, arr: &str, replacement: &str) -> String {
    let patterns = [
        format!("Arrays.asList({})", arr),
        format!("java.util.Arrays.asList({})", arr),
    ];
    let mut out = line.to_string();
    for p in &patterns {
        if out.contains(p.as_str()) {
            out = out.replacen(p.as_str(), replacement, 1);
            break;
        }
    }
    out
}

fn strip_box_value_of(expr: &str) -> String {
    let e = expr.trim();
    const PREFIXES: &[&str] = &[
        "Integer.valueOf(",
        "java.lang.Integer.valueOf(",
        "Long.valueOf(",
        "java.lang.Long.valueOf(",
        "Boolean.valueOf(",
        "java.lang.Boolean.valueOf(",
        "Double.valueOf(",
        "java.lang.Double.valueOf(",
        "Float.valueOf(",
        "java.lang.Float.valueOf(",
        "Short.valueOf(",
        "java.lang.Short.valueOf(",
        "Byte.valueOf(",
        "java.lang.Byte.valueOf(",
        "Character.valueOf(",
        "java.lang.Character.valueOf(",
    ];
    for p in PREFIXES {
        if let Some(rest) = e.strip_prefix(p) {
            if let Some(inner) = rest.strip_suffix(')') {
                let inner = inner.trim();
                if is_numeric_literal(inner) || matches!(inner, "true" | "false") {
                    return inner.to_string();
                }
            }
        }
    }
    e.to_string()
}

/// `int[] a = new int[]{1,2,3}; foo(a)` → `foo(new int[]{1,2,3})` when `a` is only used in calls
/// (not as a row of `new T[][]{ a, … }`, which fold_2d handles).
pub(crate) fn inline_filled_array_into_calls(body: &str) -> String {
    let mut current = body.to_string();
    for _ in 0..8 {
        let next = inline_filled_array_into_calls_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn inline_filled_array_into_calls_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    for i in 0..lines.len() {
        let Some((var, rhs)) = parse_simple_assign_line(lines[i]) else {
            continue;
        };
        if !is_temp_like_name(&var) || !is_filled_array_literal(&rhs) {
            continue;
        }
        let mut use_idxs: Vec<usize> = Vec::new();
        let mut ok = true;
        for (j, line) in lines.iter().enumerate() {
            if j == i || !ident_occurs(line, &var) {
                continue;
            }
            if assign_lhs_var(line).as_deref() == Some(var.as_str()) {
                ok = false;
                break;
            }
            // Do not inline into 2D array row lists — fold_2d needs the named rows.
            if line.contains("[][]") && line.contains('{') {
                ok = false;
                break;
            }
            if !line_contains_call(line) {
                ok = false;
                break;
            }
            use_idxs.push(j);
        }
        if !ok || use_idxs.len() != 1 {
            continue;
        }
        let use_i = use_idxs[0];
        let mut out = String::new();
        for (idx, line) in lines.iter().enumerate() {
            if idx == i {
                continue;
            }
            if idx == use_i {
                out.push_str(&replace_ident_as_expr(line, &var, &rhs));
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
