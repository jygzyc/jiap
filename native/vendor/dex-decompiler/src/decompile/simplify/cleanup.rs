//! Inline temps and remove decompiler artifacts.

use std::collections::{HashMap, HashSet};

use super::util::*;

/// Inline static field references: "var = System.out;" followed by "var.println(x)" → "System.out.println(x)".
/// Handles SSA aliasing where "local2 = System.out;" and "v2.println(x)" refer to the same register.
pub(crate) fn inline_static_field_refs(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut aliases: Vec<(String, String, Option<String>)> = Vec::new();
    for line in &lines {
        let stmt = strip_trailing_comment(line);
        let t = stmt.trim();
        if let Some(eq) = t.find(" = ") {
            let lhs = t[..eq].trim();
            // Use the identifier only (`String s0` → `s0`), not the full typed LHS.
            let var = lhs.rsplit(' ').next().unwrap_or(lhs);
            let val = t[eq + 3..].trim_end_matches(';').trim();
            // Only static/instance field paths (e.g. System.out), never string literals
            // like "android.permission.READ_…" which also contain '.', and never
            // const-class literals (`Context.class`) which look like dotted paths.
            if is_java_ident(var) && is_simple_field_path(val) && !is_class_literal(val) {
                let reg_num = extract_reg_number(var).map(String::from);
                aliases.push((var.to_string(), val.to_string(), reg_num));
            }
        }
    }
    if aliases.is_empty() {
        return body.to_string();
    }
    let match_vars_for = |alias_var: &str, reg_num: &Option<String>| -> Vec<String> {
        let mut vars = vec![alias_var.to_string()];
        if let Some(n) = reg_num {
            let v_form = format!("v{}", n);
            let local_form = format!("local{}", n);
            if v_form != alias_var {
                vars.push(v_form);
            }
            if local_form != alias_var {
                vars.push(local_form);
            }
        }
        vars
    };
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let stmt = strip_trailing_comment(line);
        let t = stmt.trim();
        let mut skip = false;
        for (var, val, reg_num) in &aliases {
            let assign_stmt = format!("{} = {};", var, val);
            if t == assign_stmt {
                let candidate_vars = match_vars_for(var, reg_num);
                let non_print_use = lines.iter().enumerate().any(|(j, l)| {
                    j != idx
                        && candidate_vars.iter().any(|cv| {
                            let prefix = format!("{}.", cv);
                            l.contains(&prefix)
                                && !l.contains(&format!("{}.println(", cv))
                                && !l.contains(&format!("{}.print(", cv))
                        })
                });
                if !non_print_use {
                    skip = true;
                    break;
                }
            }
        }
        if skip {
            continue;
        }
        let mut current_line = line.to_string();
        for (var, val, reg_num) in &aliases {
            let candidate_vars = match_vars_for(var, reg_num);
            for cv in &candidate_vars {
                for method in &["println", "print"] {
                    let from = format!("{}.{}(", cv, method);
                    let to = format!("{}.{}(", val, method);
                    current_line = current_line.replace(&from, &to);
                }
            }
        }
        out.push_str(&current_line);
        if idx < lines.len().saturating_sub(1) {
            out.push('\n');
        }
    }
    out
}

/// Remove bare "var; /* move-exception */" statements and inline single-use temps
/// (literals, copies, casts, single-use calls) into their only use.
///
/// Live ranges stop at the next assignment to the same name, so reused temps like
/// `s0 = "…"; …; s0 = this.foo;` are handled correctly.
pub(crate) fn cleanup_decompiler_artifacts(body: &str) -> String {
    let mut current = body.to_string();
    // Cast/call chains need multiple rounds: view→cast→getText→toString.
    for _ in 0..8 {
        let next = cleanup_decompiler_artifacts_once(&current);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(crate) fn cleanup_decompiler_artifacts_once(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();

    struct InlineCand {
        def_idx: usize,
        var: String,
        val: String,
        /// Exclusive end of live range (next kill or EOF).
        end_idx: usize,
        use_count: usize,
    }

    let mut cands: Vec<InlineCand> = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let binding = strip_trailing_comment(line);
        let t = binding.trim();
        let Some(eq) = t.find(" = ") else { continue };
        let lhs = t[..eq].trim();
        let val = t[eq + 3..].trim_end_matches(';').trim();
        let var = lhs.rsplit(' ').next().unwrap_or(lhs);
        if var.is_empty() || !is_java_ident(var) || !is_inlineable_rhs(val) {
            continue;
        }
        // Never inline a self-referential RHS (`email = email.findViewById(...)`).
        // Allow `length = arr.length` (field access named like the dest).
        if ident_occurs(val, var) && !val.ends_with(&format!(".{}", var)) {
            continue;
        }
        let mut end_idx = lines.len();
        for (j, later) in lines.iter().enumerate().skip(idx + 1) {
            if line_kills_var(later, var) {
                end_idx = j;
                break;
            }
        }
        let use_count = {
            let mut c = lines[idx + 1..end_idx]
                .iter()
                .filter(|l| ident_occurs(l, var))
                .count();
            // Reassignment that *reads* the old value: `view0 = (TextView) view0`
            // Do NOT count a pure kill `i0 = 123` — the LHS matches but the old value is dead.
            if end_idx < lines.len() {
                let kill = lines[end_idx];
                if assign_lhs_var(kill).as_deref() == Some(var) {
                    if let Some(eq) = kill.find(" = ") {
                        if ident_occurs(&kill[eq + 3..], var) {
                            c += 1;
                        }
                    }
                } else if ident_occurs(kill, var) {
                    c += 1;
                }
            }
            c
        };
        cands.push(InlineCand {
            def_idx: idx,
            var: var.to_string(),
            val: val.to_string(),
            end_idx,
            use_count,
        });
    }

    // Inline single-use *temps*, and multi-use temps whose RHS is a cheap literal
    // (`int i0 = 1; foo(i0); bar(i0);` → `foo(1); bar(1);`).
    // Keep meaningful names (email, password, requestCode) as locals.
    // When the same temp name is reused (`i0 = layout; …; i0 = button;`), only inline the
    // *latest* live range in this pass — otherwise skipping both defs leaves a dangling use.
    // When the same temp name is reused (`i0 = layout; …; i0 = button;`), only inline the
    // *latest* live range in this pass — otherwise skipping both defs leaves a dangling use.
    let mut latest_inline_by_var: HashMap<String, usize> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        if !is_temp_like_name(&c.var) || c.use_count == 0 {
            continue;
        }
        if has_index_increment_after(&lines, c.def_idx, &c.var) {
            continue;
        }
        if matches!(c.var.as_str(), "i" | "j" | "k") {
            let used_in_condition = lines[c.def_idx + 1..].iter().any(|l| {
                let binding = strip_trailing_comment(l);
                let t = binding.trim();
                (t.starts_with("if (") || t.starts_with("while (") || t.starts_with("for ("))
                    && ident_occurs(l, &c.var)
            });
            // `int k = 0` with uses in `k + 1` and `out[k]` must not become `out[0]`.
            if c.use_count > 1
                || used_in_condition
                || has_index_increment_after(&lines, c.def_idx, &c.var)
            {
                continue;
            }
        }
        if all_uses_are_length_member(&lines, &c.var, c.def_idx, c.end_idx) {
            continue;
        }
        let want = c.use_count == 1 || is_cheap_literal_rhs(&c.val);
        if !want {
            continue;
        }
        match latest_inline_by_var.get(&c.var) {
            Some(&prev) if cands[prev].def_idx > c.def_idx => {}
            _ => {
                latest_inline_by_var.insert(c.var.clone(), i);
            }
        }
    }
    let inline_set: HashSet<usize> = latest_inline_by_var.values().copied().collect();
    let skip_indices: HashSet<usize> = cands
        .iter()
        .enumerate()
        .filter(|(i, c)| {
            let temp = is_temp_like_name(&c.var);
            let used_after_def = lines[c.def_idx + 1..]
                .iter()
                .any(|l| ident_used_as_rvalue(l, &c.var));
            (inline_set.contains(i) && temp)
                || (temp
                    && c.use_count == 0
                    && !used_after_def
                    && !is_simple_call_expr(&c.val)
                    && !is_cast_expr(&c.val)
                    && !is_loop_body_slot_update(&c.var, c.def_idx, &lines)
                    && !is_index_increment(lines[c.def_idx], &c.var))
        })
        .map(|(i, _)| cands[i].def_idx)
        .collect();
    let inlines: Vec<&InlineCand> = {
        let mut v: Vec<&InlineCand> = inline_set.iter().map(|&i| &cands[i]).collect();
        // Earlier defs first so nested temps (button id → findViewById → listener) resolve.
        v.sort_by_key(|c| c.def_idx);
        v
    };

    // Substitute other single-use inlines that are live at `at_idx` into `val`.
    let materialize = |val: &str, at_idx: usize| -> String {
        let mut out = val.to_string();
        for other in &inlines {
            let live = at_idx > other.def_idx && at_idx < other.end_idx;
            let on_kill =
                at_idx == other.end_idx && at_idx > other.def_idx && ident_occurs(val, &other.var);
            if (live || on_kill) && ident_occurs(&out, &other.var) {
                out = replace_ident_as_expr(&out, &other.var, &other.val);
            }
        }
        out
    };

    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let binding = strip_trailing_comment(line);
        let t = binding.trim();
        if t.ends_with("; /* move-exception */") || t.ends_with("/* move-exception */;") {
            continue;
        }
        if skip_indices.contains(&idx) {
            continue;
        }
        let mut current = line.to_string();
        for cand in &inlines {
            // Include the killing reassignment line (end_idx) when it reads the old value.
            let in_range = idx > cand.def_idx && idx < cand.end_idx;
            let on_kill_use =
                idx == cand.end_idx && idx > cand.def_idx && ident_occurs(line, &cand.var);
            if (in_range || on_kill_use) && ident_occurs(&current, &cand.var) {
                let replacement = materialize(&cand.val, idx);
                // Never rewrite the assignment target (`i4 = 20` must not become `18 = 20`).
                if assign_lhs_var(&current).as_deref() == Some(cand.var.as_str()) {
                    if let Some(eq) = current.find(" = ") {
                        let (lhs, rhs) = current.split_at(eq + 3);
                        current = format!(
                            "{}{}",
                            lhs,
                            replace_ident_as_expr(rhs, &cand.var, &replacement)
                        );
                    }
                } else {
                    current = replace_ident_as_expr(&current, &cand.var, &replacement);
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

/// Inline temps assigned exactly once to a cheap literal across the whole method body.
/// Inline temps assigned exactly once to a cheap literal across the whole method body.
pub(crate) fn inline_global_literal_temps(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut assigns: HashMap<String, (usize, String)> = HashMap::new();
    let mut assign_counts: HashMap<String, usize> = HashMap::new();
    for (idx, line) in lines.iter().enumerate() {
        let Some((var, val)) = parse_simple_assign_line(line) else {
            continue;
        };
        if !is_temp_like_name(&var) || !is_cheap_literal_rhs(&val) {
            continue;
        }
        // Loop indices (`j = 0` in bubble sort) must stay — inlining breaks bound checks.
        if matches!(var.as_str(), "i" | "j" | "k") {
            continue;
        }
        if lines.iter().any(|l| is_index_increment(l, &var)) {
            continue;
        }
        // Register reuse (`s0 = "x"; …; s0 = receiver; s0.method()`) — cleanup inlines per live range.
        if lines
            .iter()
            .enumerate()
            .skip(idx + 1)
            .any(|(_, l)| assign_lhs_var(l).as_deref() == Some(var.as_str()))
        {
            continue;
        }
        *assign_counts.entry(var.clone()).or_insert(0) += 1;
        assigns.insert(var, (idx, val));
    }
    let mut inline_vars: Vec<String> = assigns
        .keys()
        .filter(|v| assign_counts.get(*v).copied() == Some(1))
        .cloned()
        .collect();
    inline_vars.sort();
    let inline_vars: HashSet<String> = inline_vars.into_iter().collect();
    if inline_vars.is_empty() {
        return body.to_string();
    }
    let skip: HashSet<usize> = assigns
        .iter()
        .filter(|(v, _)| inline_vars.contains(*v))
        .map(|(_, (idx, _))| *idx)
        .collect();
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if skip.contains(&idx) {
            continue;
        }
        let mut current = line.to_string();
        for var in {
            let mut vars: Vec<_> = inline_vars.iter().cloned().collect();
            vars.sort();
            vars
        } {
            if let Some((_, lit)) = assigns.get(&var) {
                if ident_occurs(&current, &var) {
                    if assign_lhs_var(&current).as_deref() == Some(var.as_str()) {
                        continue;
                    }
                    current = replace_ident_as_expr(&current, &var, lit);
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

/// `i = i0 + 1` / `int j = i1 + 1` after SSA unify → `i++` / `j++`.
/// `i0 = i * i;` without a prior declaration → `int i0 = i * i;` (when inlining did not apply).
pub(crate) fn fix_undeclared_temp_assigns(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let mut declared: HashSet<String> = HashSet::new();
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let binding = strip_trailing_comment(line);
        let t = binding.trim();
        let mut current = line.to_string();
        if let Some((var, rhs)) = parse_simple_assign_line(line) {
            if is_temp_like_name(&var) && !declared.contains(&var) && !is_simple_call_expr(&rhs) {
                if let Some(eq) = t.find(" = ") {
                    let lhs = t[..eq].trim();
                    if !lhs.contains(' ') {
                        let indent = leading_indent(line);
                        let comment = line.get(binding.len()..).unwrap_or("");
                        let stmt = t.trim_end_matches(';');
                        current = format!("{}int {};", indent, stmt);
                        current.push_str(comment);
                    }
                }
            }
        }
        if let Some(eq) = t.find(" = ") {
            let lhs = t[..eq].trim();
            if let Some(var) = lhs.split_whitespace().last() {
                if is_java_ident(var) {
                    declared.insert(var.to_string());
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

/// Merge `x = new Foo();` + `x.<init>(args);` (and typed forms) into `x = new Foo(args);`.
/// Merge `x = new Foo();` + `x.<init>(args);` (and typed forms) into `x = new Foo(args);`.
pub(crate) fn merge_constructor_calls(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() < 2 {
        return body.to_string();
    }
    let mut skip: HashSet<usize> = HashSet::new();
    let mut replacements: HashMap<usize, String> = HashMap::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        let binding = strip_trailing_comment(lines[i]);
        let t = binding.trim();
        let Some(eq) = t.find(" = ") else {
            i += 1;
            continue;
        };
        let lhs = t[..eq].trim();
        let rhs = t[eq + 3..].trim_end_matches(';').trim();
        let var = lhs.rsplit(' ').next().unwrap_or(lhs);
        let Some(class) = rhs.strip_prefix("new ").and_then(|s| s.strip_suffix("()")) else {
            i += 1;
            continue;
        };
        // Skip intervening simple assigns that don't touch `var`.
        let mut j = i + 1;
        while j < lines.len() {
            let jb = strip_trailing_comment(lines[j]);
            let jt = jb.trim();
            if let Some((init_var, args)) = parse_init_call(lines[j]) {
                if init_var == var {
                    let indent = leading_indent(lines[i]);
                    let typed = if lhs.contains(' ') {
                        format!(
                            "{}{} = new {}({});",
                            indent,
                            lhs,
                            class,
                            args.unwrap_or_default()
                        )
                    } else {
                        format!(
                            "{}{} = new {}({});",
                            indent,
                            var,
                            class,
                            args.unwrap_or_default()
                        )
                    };
                    replacements.insert(i, typed);
                    skip.insert(j);
                    i = j + 1;
                    break;
                }
            }
            if assign_lhs_var(lines[j]).as_deref() == Some(var) {
                i += 1;
                break;
            }
            // Allow simple temps between new and init.
            if parse_simple_assign_line(lines[j]).is_some() || jt.is_empty() {
                j += 1;
                continue;
            }
            i += 1;
            break;
        }
        if j >= lines.len() {
            i += 1;
        }
    }
    if replacements.is_empty() {
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
