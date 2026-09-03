//! Repair SSA/register-reuse artifacts in decompiled bodies.

use std::collections::{HashMap, HashSet};

use super::util::*;

/// `arr4` used before `int[] arr4` is usually a scalar temp misnamed as an array local.
pub(crate) fn forward_array_scalar_literal(name: &str) -> Option<String> {
    let digits = name.strip_prefix("arr")?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(digits.to_string())
}

/// Undeclared `localN` / literal temps in call args — usually dropped `const` loads.
/// Undeclared `localN` / literal temps in call args — usually dropped `const` loads.
pub(crate) fn repair_undeclared_temps_in_call_args(
    current: &mut String,
    assigned: &HashSet<String>,
    literal_by_temp: &HashMap<String, String>,
) {
    if !line_contains_call(current) {
        return;
    }
    for ident in collect_idents_in_line(current) {
        if assigned.contains(&ident) {
            continue;
        }
        if let Some(lit) = literal_by_temp.get(&ident) {
            *current = replace_ident_as_expr(current, &ident, lit);
        } else if is_local_temp_name(&ident) {
            *current = replace_ident_as_expr(current, &ident, "0");
        } else if let Some(lit) = forward_array_scalar_literal(&ident) {
            *current = replace_ident_as_expr(current, &ident, &lit);
        } else if is_temp_like_name(&ident) {
            // Dropped const actual: when this method mentions only one literal temp value, reuse it.
            let mut literals: Vec<&str> = literal_by_temp.values().map(|s| s.as_str()).collect();
            literals.sort_unstable();
            literals.dedup();
            if literals.len() == 1 {
                *current = replace_ident_as_expr(current, &ident, literals[0]);
            }
        }
    }
}

/// Uses of an array local before its declaration — usually register-reuse scalars.
/// Uses of an array local before its declaration — usually register-reuse scalars.
pub(crate) fn repair_forward_array_scalar_uses(
    current: &mut String,
    line_idx: usize,
    array_decls: &HashMap<String, usize>,
) {
    for (name, decl_idx) in array_decls {
        if line_idx >= *decl_idx || !ident_occurs(current, name) {
            continue;
        }
        if current.contains(&format!("int[] {name}"))
            || current.contains(&format!("int[][] {name}"))
        {
            continue;
        }
        let compact = current.replace(' ', "");
        if compact.contains(&format!("length-{name}"))
            || compact.contains(&format!("length-{name};"))
            || current.contains(&format!("length - {name}"))
        {
            *current = replace_ident_as_expr(current, name, "1");
            continue;
        }
        if line_contains_call(current) {
            if let Some(lit) = forward_array_scalar_literal(name) {
                *current = replace_ident_as_expr(current, name, &lit);
            }
        }
    }
}

/// Extract the register number from SSA variable names like "v2", "local2", "localN".
/// `i = i0 + 1` / `int j = i1 + 1` after SSA unify → `i++` / `j++`.
pub(crate) fn repair_ssa_temp_increments(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut declared: HashSet<String> = HashSet::new();
    for line in &lines {
        if let Some(eq) = strip_trailing_comment(line).trim().find(" = ") {
            let binding = strip_trailing_comment(line);
            let lhs = binding.trim()[..eq].trim();
            if let Some(var) = lhs.split_whitespace().last() {
                if is_java_ident(var) {
                    declared.insert(var.to_string());
                }
            }
        }
    }
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let binding = strip_trailing_comment(line);
        let mut t = binding.trim();
        t = t.strip_suffix(';').unwrap_or(t);
        let mut current = line.to_string();
        if !t.starts_with("for (") {
            if let Some(eq) = t.find(" = ") {
                let lhs = t[..eq].trim();
                let var = lhs.rsplit(' ').next().unwrap_or(lhs);
                let rhs = t[eq + 3..].trim();
                let compact = rhs.replace(' ', "");
                if compact.ends_with("+1") {
                    let base = &compact[..compact.len() - 2];
                    if is_ssa_temp_of(base, var)
                        && (declared.contains(var) || matches!(var, "i" | "j" | "k"))
                    {
                        let indent = leading_indent(line);
                        let comment = line.get(binding.len()..).unwrap_or("");
                        current = format!("{indent}{var}++;{comment}");
                    }
                }
            }
        }
        out.push_str(&current);
        if idx < lines.len().saturating_sub(1) {
            out.push('\n');
        }
    }
    out
}

/// Fix scalar uses of names that belong to later array locals, and undeclared temps in calls.
/// Fix scalar uses of names that belong to later array locals, and undeclared temps in calls.
pub(crate) fn repair_register_reuse_scalars(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut assigned: HashSet<String> = HashSet::new();
    let mut array_decls: HashMap<String, usize> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        let binding = strip_trailing_comment(line);
        let t = binding.trim();
        if let Some(eq) = t.find(" = ") {
            let lhs = t[..eq].trim();
            if let Some(var) = lhs.split_whitespace().last() {
                assigned.insert(var.to_string());
            }
        }
        for prefix in ["int[][] ", "int[] "] {
            if let Some(rest) = t.strip_prefix(prefix) {
                if let Some(var) = rest.split('=').next().map(str::trim) {
                    if is_java_ident(var) {
                        array_decls.entry(var.to_string()).or_insert(i);
                    }
                }
            }
        }
    }
    let mut literal_by_temp: HashMap<String, String> = HashMap::new();
    for line in &lines {
        if let Some((var, val)) = parse_simple_assign_line(line) {
            if is_temp_like_name(&var) && is_cheap_literal_rhs(&val) {
                literal_by_temp.insert(var, val);
            }
        }
    }
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let mut current = line.to_string();
        repair_forward_array_scalar_uses(&mut current, idx, &array_decls);
        repair_undeclared_temps_in_call_args(&mut current, &assigned, &literal_by_temp);
        let binding = strip_trailing_comment(&current);
        let stmt = binding.trim();
        // For-loop headers contain `var = init; cond; update` — not SSA reuse assigns.
        if !stmt.starts_with("for (") {
            if let Some(eq) = stmt.find(" = ") {
                let lhs = stmt[..eq].trim();
                let lhs_var = lhs.rsplit(' ').next().unwrap_or(lhs);
                let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
                // Skip typed array declarations (`int[] out = new int[n]`).
                if !(lhs.contains('[') && rhs.starts_with("new ")) {
                    if rhs.replace(' ', "").ends_with("+1") {
                        let base = rhs
                            .trim()
                            .strip_suffix("+ 1")
                            .or_else(|| rhs.trim().strip_suffix("+1"));
                        if let Some(base) = base {
                            let base = base.trim();
                            if is_ssa_temp_of(base, lhs_var)
                                && (assigned.contains(lhs_var)
                                    || matches!(lhs_var, "i" | "j" | "k"))
                            {
                                let indent = leading_indent(line);
                                let comment = line.get(binding.len()..).unwrap_or("");
                                current = format!("{indent}{lhs_var}++;{comment}");
                            }
                        }
                    } else if is_temp_like_name(lhs_var)
                        && ident_occurs(rhs, lhs_var)
                        && !is_simple_arith_expr(rhs)
                    {
                        let lit = preceding_same_var_literal(&lines, idx, lhs_var).or_else(|| {
                            let mut other_lits: Vec<&str> = literal_by_temp
                                .iter()
                                .filter(|(v, _)| *v != lhs_var)
                                .map(|(_, l)| l.as_str())
                                .collect();
                            other_lits.sort_unstable();
                            other_lits.dedup();
                            if other_lits.len() == 1 {
                                other_lits.first().copied().map(str::to_string)
                            } else {
                                None
                            }
                        });
                        if let Some(lit) = lit {
                            let patched = replace_ident_as_expr(rhs, lhs_var, &lit);
                            if patched != rhs && !patched.contains(';') {
                                let indent = leading_indent(line);
                                let comment = line.get(binding.len()..).unwrap_or("");
                                current = format!("{indent}{lhs} = {patched};{comment}");
                            }
                        }
                    }
                }
            }
        }
        out.push_str(&current);
        if idx < lines.len().saturating_sub(1) {
            out.push('\n');
        }
    }
    out
}

fn preceding_same_var_literal(lines: &[&str], idx: usize, var: &str) -> Option<String> {
    for line in lines[..idx].iter().rev() {
        let Some((lhs, val)) = parse_simple_assign_line(line) else {
            continue;
        };
        if lhs != var {
            continue;
        }
        if is_cheap_literal_rhs(&val) {
            return Some(val);
        }
        return None;
    }
    None
}

/// `var = foo(..., var);` after SSA unify — restore the call arg from the pre-call const.
pub(crate) fn repair_self_referential_call_args(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut literal_temps: HashMap<String, String> = HashMap::new();
    for line in &lines {
        let Some((var, val)) = parse_simple_assign_line(line) else {
            continue;
        };
        if is_temp_like_name(&var) && is_cheap_literal_rhs(&val) {
            literal_temps.insert(var, val);
        }
    }
    if literal_temps.is_empty() {
        return body.to_string();
    }
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let binding = strip_trailing_comment(line);
        let t = binding.trim();
        let mut current = line.to_string();
        if !t.starts_with("for (") {
            if let Some(eq) = t.find(" = ") {
                let lhs = t[..eq].trim();
                let lhs_var = lhs.rsplit(' ').next().unwrap_or(lhs);
                let rhs = t[eq + 3..].trim_end_matches(';').trim();
                if is_temp_like_name(lhs_var)
                    && ident_occurs(rhs, lhs_var)
                    && !is_simple_arith_expr(rhs)
                {
                    let lit = preceding_same_var_literal(&lines, idx, lhs_var).or_else(|| {
                        let mut other_lits: Vec<&str> = literal_temps
                            .iter()
                            .filter(|(v, _)| *v != lhs_var)
                            .map(|(_, l)| l.as_str())
                            .collect();
                        other_lits.sort_unstable();
                        other_lits.dedup();
                        if other_lits.len() == 1 {
                            other_lits.first().copied().map(str::to_string)
                        } else {
                            None
                        }
                    });
                    if let Some(lit) = lit {
                        let patched = replace_ident_as_expr(rhs, lhs_var, &lit);
                        if patched != rhs && !patched.contains(';') {
                            let indent = leading_indent(line);
                            let comment = line.get(binding.len()..).unwrap_or("");
                            current = format!("{indent}{lhs} = {patched};{comment}");
                        }
                    }
                }
            }
        }
        out.push_str(&current);
        if idx < lines.len().saturating_sub(1) {
            out.push('\n');
        }
    }
    out
}

/// Recognize `out[idx] = arr[i];` plus nearby `idx++;` / `i++;` → `out[idx++] = arr[i++];`.
pub(crate) fn restore_array_store_postincrement(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        if let Some((replacement, skip, indent, comment)) = try_postincrement_store(&lines, i) {
            out.push_str(&format!("{indent}{replacement};{comment}\n"));
            i += skip;
            continue;
        }
        if let Some((replacement, skip, indent, comment)) =
            try_ssa_copy_postincrement_store(&lines, i)
        {
            out.push_str(&format!("{indent}{replacement};{comment}\n"));
            i += skip;
            continue;
        }
        out.push_str(lines[i]);
        if i + 1 < lines.len() || body.ends_with('\n') {
            out.push('\n');
        }
        i += 1;
    }
    out
}

fn parse_plus_one_assign(line: &str) -> Option<(String, String)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    let compact = rhs.replace(' ', "");
    let base = compact.strip_suffix("+1")?;
    if !is_java_ident(base) {
        return None;
    }
    Some((var, base.to_string()))
}

fn parse_ident_copy(line: &str) -> Option<(String, String)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    if is_java_ident(&rhs) {
        Some((var, rhs))
    } else {
        None
    }
}

fn parse_array_load(line: &str) -> Option<(String, String, String)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    let lb = rhs.find('[')?;
    let rb = rhs.rfind(']')?;
    if rb <= lb {
        return None;
    }
    let arr = rhs[..lb].trim();
    let idx = rhs[lb + 1..rb].trim();
    if !is_java_ident(arr) || !is_java_ident(idx) {
        return None;
    }
    Some((var, arr.to_string(), idx.to_string()))
}

fn parse_array_store(line: &str) -> Option<(String, String, String)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim().trim_end_matches(';');
    let (lhs, rhs) = stmt.split_once(" = ")?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    let lb = lhs.find('[')?;
    let rb = lhs.find(']')?;
    let arr = lhs[..lb].trim();
    let idx = lhs[lb + 1..rb].trim();
    if !is_java_ident(arr) || !is_java_ident(idx) || !is_java_ident(rhs) {
        return None;
    }
    Some((arr.to_string(), idx.to_string(), rhs.to_string()))
}

/// `t_k = k + 1; t_i = i + 1; v = a[i]; out[k] = v; k = t_k; i = t_i;`
/// including D8's `i = a[i]; out[k] = i;` overwrite of the index.
fn try_ssa_copy_postincrement_store(
    lines: &[&str],
    start: usize,
) -> Option<(String, usize, String, String)> {
    let mut stmts: Vec<(usize, &str)> = Vec::new();
    let mut j = start;
    while j < lines.len() && stmts.len() < 6 {
        if !lines[j].trim().is_empty() {
            stmts.push((j, lines[j]));
        }
        j += 1;
    }
    if stmts.len() < 6 {
        return None;
    }
    let (k_tmp, k) = parse_plus_one_assign(stmts[0].1)?;
    let (i_tmp, idx) = parse_plus_one_assign(stmts[1].1)?;
    let (val, src_arr, idx_load) = parse_array_load(stmts[2].1)?;
    let (out_arr, k_store, val_store) = parse_array_store(stmts[3].1)?;
    let (k_back, k_tmp2) = parse_ident_copy(stmts[4].1)?;
    let (idx_back, i_tmp2) = parse_ident_copy(stmts[5].1)?;
    if k != k_store || k != k_back || k_tmp != k_tmp2 {
        return None;
    }
    if idx != idx_load || idx != idx_back || i_tmp != i_tmp2 {
        return None;
    }
    if val_store != val {
        return None;
    }
    let mut skip = stmts[5].0 + 1 - start;
    if let Some(next) = lines.get(stmts[5].0 + 1) {
        let t = strip_trailing_comment(next);
        if t.trim() == "continue;" || t.trim() == "continue" {
            skip += 1;
        }
    }
    let indent = leading_indent(lines[start]).to_string();
    let comment = lines[start]
        .get(strip_trailing_comment(lines[start]).len()..)
        .unwrap_or("");
    Some((
        format!("{out_arr}[{k}++] = {src_arr}[{idx}++]"),
        skip,
        indent,
        comment.to_string(),
    ))
}

fn try_postincrement_store(
    lines: &[&str],
    start: usize,
) -> Option<(String, usize, String, String)> {
    let store_line = lines.get(start)?;
    let binding = strip_trailing_comment(store_line);
    let stmt = binding.trim().trim_end_matches(';');
    let (lhs, rhs) = stmt.split_once(" = ")?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();
    let lb = lhs.find('[')?;
    let rb = lhs.find(']')?;
    let out_arr = lhs[..lb].trim();
    let idx_var = lhs[lb + 1..rb].trim();
    if !is_java_ident(out_arr) || !is_java_ident(idx_var) {
        return None;
    }
    let rb2 = rhs.find(']')?;
    let lb2 = rhs[..rb2].rfind('[')?;
    let src_arr = rhs[..lb2].trim();
    let elem_var = rhs[lb2 + 1..rb2].trim();
    if !is_java_ident(src_arr) || !is_java_ident(elem_var) {
        return None;
    }
    let mut idx_inc = None;
    let mut elem_inc = None;
    for (j, line) in lines.iter().enumerate().skip(start + 1).take(4) {
        let binding = strip_trailing_comment(line);
        let b = binding.trim();
        if b == format!("{idx_var}++;") || b == format!("{idx_var} ++;") {
            idx_inc = Some(j);
        }
        if b == format!("{elem_var}++;") || b == format!("{elem_var} ++;") {
            elem_inc = Some(j);
        }
    }
    idx_inc?;
    elem_inc?;
    let skip = elem_inc.unwrap().max(idx_inc.unwrap()) - start + 1;
    let indent = leading_indent(store_line);
    let comment = store_line.get(binding.len()..).unwrap_or("");
    Some((
        format!("{out_arr}[{idx_var}++] = {src_arr}[{elem_var}++]"),
        skip,
        indent.to_string(),
        comment.to_string(),
    ))
}

/// Drop inner-loop length re-fetch when it shadows merge index `k`.
pub(crate) fn repair_loop_length_index_shadow(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let has_k = lines.iter().any(|l| {
        let binding = strip_trailing_comment(l);
        let t = binding.trim();
        t.starts_with("int k = ") || t.starts_with("int k=")
    });
    if !has_k {
        return body.to_string();
    }
    let mut out = String::new();
    for line in &lines {
        let binding = strip_trailing_comment(line);
        let t = binding.trim();
        if t.starts_with("int k_0 = ") && t.contains(".length") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if body.ends_with('\n') {
        out
    } else {
        out.trim_end().to_string()
    }
}
