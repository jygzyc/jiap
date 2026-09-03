//! Main simplify pipeline: invoke patterns + pass orchestration.

use std::fmt::Write;

use super::cleanup::*;
use super::conditions::*;
use super::expr::*;
use super::format::*;
use super::loops::*;
use super::repair::*;
use super::try_catch::*;
use super::util::*;

/// Replace whole-identifier occurrences of `var` with `replacement`.
/// When the use is a receiver (`var.` / `var[`), wrap casts/complex exprs in parentheses.
/// and "invoke(...); vN = <result>;" into "vN = method(args);".
/// Also collapses "if (cond) { return a; } else { return b; }" into "return cond ? a : b;" (JADX-style).
/// When `is_constructor` is true, "receiver.<init>();" (no args) is simplified to "super();".
pub fn simplify_method_body(body: &str, is_constructor: bool) -> String {
    let body = fold_array_length_assigns(body);
    let body = fold_array_alloc_length_sum(&body);
    let body = fold_length_sum_return(&body);
    let lines: Vec<String> = body.lines().map(String::from).collect();
    if lines.len() < 2 {
        return body.to_string();
    }
    let mut i = 0usize;
    let mut out = String::new();
    let mut skip_unreachable_indent: Option<usize> = None;
    while i < lines.len() {
        let line = &lines[i];

        // Skip unreachable code after "return ...;": skip only lines with indent > return's indent until we see "}" at same or less indent.
        if let Some(return_indent) = skip_unreachable_indent {
            let line_indent = leading_indent(line).len();
            let t = strip_trailing_comment(line);
            let stmt = t.trim();
            // Never strip braces or exception handlers — they may follow a return in try/catch.
            if stmt.starts_with('}')
                || stmt.starts_with("} catch")
                || stmt.starts_with("} finally")
                || stmt.starts_with("catch (")
            {
                skip_unreachable_indent = None;
            } else if line_indent > return_indent {
                i += 1;
                continue;
            } else {
                out.push_str(line);
                if i < lines.len().saturating_sub(1) {
                    out.push('\n');
                }
                skip_unreachable_indent = None;
                i += 1;
                continue;
            }
        }

        // Try: var = expr; return var;  (or "Type var = expr;") → return expr;
        if i + 1 < lines.len() {
            if let (Some((var, expr)), Some(ret_var)) = (
                parse_simple_assign_line(line),
                parse_return_ident_line(&lines[i + 1]),
            ) {
                if var == ret_var {
                    let indent = leading_indent(line);
                    writeln!(out, "{}return {};", indent, expr).ok();
                    skip_unreachable_indent = Some(indent.len());
                    i += 2;
                    continue;
                }
            }
        }

        // Try: if (cond) { return a; } else { return b; } → return cond ? a : b;
        if i + 4 < lines.len() {
            if let Some(cond) = parse_if_condition(line) {
                let then_line = lines[i + 1].trim();
                let else_line = lines[i + 2].trim();
                let return_b_line = lines[i + 3].trim();
                let close_line = lines[i + 4].trim();
                if parse_return_expr(then_line).is_some()
                    && else_line.contains("} else {")
                    && parse_return_expr(return_b_line).is_some()
                    && close_line.trim() == "}"
                {
                    let then_expr = parse_return_expr(then_line).unwrap();
                    let else_expr = parse_return_expr(return_b_line).unwrap();
                    let indent = leading_indent(line);
                    writeln!(
                        out,
                        "{}return {} ? {} : {};",
                        indent, cond, then_expr, else_expr
                    )
                    .ok();
                    skip_unreachable_indent = Some(indent.len());
                    i += 5;
                    continue;
                }
            }
        }

        // Try: invoke + move-result + return (same reg)
        if is_invoke_line(line)
            && i + 2 < lines.len()
            && parse_move_result_line(&lines[i + 1]).is_some()
            && parse_return_reg_line(&lines[i + 2]).is_some()
        {
            let move_reg = parse_move_result_line(&lines[i + 1]).unwrap();
            let return_reg = parse_return_reg_line(&lines[i + 2]).unwrap();
            if move_reg == return_reg {
                if let Some((args, method_ref)) = parse_invoke_args_and_method(line) {
                    let indent = leading_indent(line);
                    let call = format!("{}({});", method_ref, args);
                    writeln!(out, "{}return {}", indent, call).ok();
                    skip_unreachable_indent = Some(indent.len());
                    i += 3;
                    continue;
                }
            }
        }

        // Try: invoke + move-result (no return)
        if is_invoke_line(line)
            && i + 1 < lines.len()
            && parse_move_result_line(&lines[i + 1]).is_some()
        {
            let move_reg = parse_move_result_line(&lines[i + 1]).unwrap();
            if let Some((args, method_ref)) = parse_invoke_args_and_method(line) {
                let indent = leading_indent(line);
                let call = format!("{}({});", method_ref, args);
                writeln!(out, "{}{} = {}", indent, move_reg, call).ok();
                i += 2;
                continue;
            }
        }

        // Try: invoke + return; (void call) → emit as normal Java call "method(args);" then "return;"
        if is_invoke_line(line) && i + 1 < lines.len() && is_return_void_line(&lines[i + 1]) {
            if let Some((args, method_ref)) = parse_invoke_args_and_method(line) {
                let indent = leading_indent(line);
                let call = format!("{}({});", method_ref, args);
                writeln!(out, "{}{}", indent, call).ok();
                writeln!(out, "{}", lines[i + 1]).ok();
                i += 2;
                continue;
            }
        }

        // Try: StringBuilder chain → a + b + c
        // Handles SSA-aliased variables: new StringBuilder() on sb0 may be used as v3 in
        // <init>, append, toString. Also folds into println if the toString result is used there.
        if let Some((_sb_var, first_opt)) = parse_new_stringbuilder(line) {
            let mut parts: Vec<String> = if let Some(a) = first_opt {
                vec![a]
            } else {
                vec![]
            };
            let mut j = i + 1;
            let chain_start = i;
            let mut const_assigns: Vec<(String, String)> = Vec::new();

            while j < lines.len() {
                let jline = lines[j].trim();
                if let Some((_init_var, init_arg)) = parse_init_call(&lines[j]) {
                    if let Some(arg) = init_arg {
                        parts.push(arg);
                    }
                    j += 1;
                    continue;
                }
                if jline.contains(" = ")
                    && !jline.contains(".append(")
                    && !jline.contains(".toString(")
                    && !jline.contains("new ")
                    && !jline.contains(".println(")
                {
                    if let Some(eq) = jline.find(" = ") {
                        let var = jline[..eq].trim();
                        let val = jline[eq + 3..].trim_end_matches(';').trim();
                        if !var.is_empty() && !val.is_empty() && !val.contains('(') {
                            const_assigns.push((var.to_string(), val.to_string()));
                            j += 1;
                            continue;
                        }
                    }
                }
                break;
            }

            while j < lines.len() {
                if let Some((_v, arg)) = parse_append(&lines[j]) {
                    parts.push(arg);
                    j += 1;
                    continue;
                }
                break;
            }

            if j < lines.len() && !parts.is_empty() {
                if let Some((dest, _to_str_var)) = parse_to_string(&lines[j]) {
                    let indent = leading_indent(line);
                    let inline_const = |s: &str| -> String {
                        for (cvar, cval) in &const_assigns {
                            if s == cvar {
                                return cval.clone();
                            }
                        }
                        s.to_string()
                    };
                    let parts_inlined: Vec<String> =
                        parts.iter().map(|p| inline_const(p)).collect();
                    let concat = parts_inlined.join(" + ");

                    if dest == "return" {
                        writeln!(out, "{}return {};", indent, concat).ok();
                        skip_unreachable_indent = Some(indent.len());
                        i = j + 1;
                        continue;
                    }
                    if j + 1 < lines.len() {
                        if let Some((print_obj, print_arg)) = parse_println(&lines[j + 1]) {
                            if print_arg == dest {
                                let receiver = const_assigns
                                    .iter()
                                    .rfind(|(_, v)| v == "System.out")
                                    .map(|(k, _)| k.as_str());
                                let obj = if receiver.is_some_and(|r| {
                                    r == print_obj
                                        || print_obj.starts_with("local")
                                        || print_obj.starts_with("v")
                                }) {
                                    "System.out"
                                } else {
                                    &print_obj
                                };
                                writeln!(out, "{}{}.println({});", indent, obj, concat).ok();
                                i = j + 2;
                                continue;
                            }
                        }
                    }
                    writeln!(out, "{}{} = {};", indent, dest, concat).ok();
                    i = j + 1;
                    continue;
                }
            }
            if j > chain_start + 1 {
                // partial chain matched but no toString found; emit original lines
            }
        }

        out.push_str(line);
        if i < lines.len().saturating_sub(1) {
            out.push('\n');
        }
        if is_return_line(line) {
            skip_unreachable_indent = Some(leading_indent(line).len());
        }
        i += 1;
    }
    // Simplify "x + -N" to "x - N" (e.g. "local0 + -3" → "local0 - 3").
    out = out.replace(" + -", " - ");
    // Fold assign ternary before dead-assign cleanup removes unused x=a / x=b arms.
    out = fold_assign_ternary(&out);
    // Inline "var = System.out;" → replace var.println(x) with System.out.println(x) and remove the assignment.
    out = inline_static_field_refs(&out);
    // Restore for-each before dead-code cleanup drops loop-bound length temps (e.g. length_0 = row.length).
    out = remove_trailing_continues(&out);
    out = restore_while_true_nested_for(&out);
    out = restore_foreach_iterator(&out);
    out = restore_foreach_array(&out);
    out = restore_counting_for_loop(&out);
    // Remove bare "var; /* move-exception */" lines and inline single-use temps (consts / simple copies).
    out = inline_global_literal_temps(&out);
    out = repair_register_reuse_scalars(&out);
    out = repair_loop_length_index_shadow(&out);
    out = restore_array_store_postincrement(&out);
    out = restore_d8_merge_copy_loops(&out);
    // Fold instanceof into `if` before cleanup drops unused `z0 = x instanceof T`.
    out = fold_instanceof_into_conditions(&out);
    out = cleanup_decompiler_artifacts(&out);
    out = repair_self_referential_call_args(&out);
    out = fix_undeclared_temp_assigns(&out);
    // Merge `x = new T(); x.<init>(args);` → `x = new T(args);`
    out = merge_constructor_calls(&out);
    // Fold `lit; x = lit.equals(y); if (x)` and collapse `!(!z)` conditions.
    out = fold_equals_into_conditions(&out);
    // Drop leftover `url = "/login";` before `if ("/login".equals(…))`.
    out = drop_string_lits_folded_into_if(&out);
    // `if (!(lit.equals(x))) { A } else { B }` → `if (lit.equals(x)) { B } else { A }`
    out = flip_negated_equals_if_else(&out);
    // `} else { if (c) {` → `} else if (c) {`
    out = flatten_else_if_chains(&out);
    // Nested scheme/host Uri checks → `a && b`
    out = merge_nested_uri_component_checks(&out);
    // General nested `if (A) { if (B) { BODY } }` → `if (A && B) { BODY }`
    out = merge_nested_if_and(&out);
    // `if (A) { BODY } else if (B) { BODY }` → `if (A || B) { BODY }`
    out = merge_short_circuit_or(&out);
    // `if (!A && !B) { return false; } return true;` → `if (A || B) { return true; } return false;`
    out = flip_negated_and_boolean_return(&out);
    // `continue;` before `}` blocks for-each restore.
    out = remove_trailing_continues(&out);
    // Iterator while → for-each
    out = restore_foreach_iterator(&out);
    // Array index-while → for-each (including nested 2D)
    out = restore_foreach_array(&out);
    // Objects.requireNonNull / Intrinsics.checkNotNull* strip
    out = strip_require_non_null(&out);
    // Identical consecutive catches → multi-catch
    out = merge_multi_catch(&out);
    // Duplicate cleanup at end of try + each catch → finally
    out = merge_duplicate_finally(&out);
    // `while (true) { …; if (!c) break; }` → do-while
    out = restore_do_while_text(&out);
    // `$SwitchMap$[e.ordinal()]` / `e.ordinal()` → `switch (e)`
    out = restore_enum_switchmap(&out);
    // try + finally-close → try-with-resources
    out = restore_try_with_resources(&out);
    // `(T) new T(...)` / `(T)(T)x` strip
    out = strip_redundant_casts(&out);
    // StringConcatFactory / makeConcat indy → `a + b`
    out = restore_string_concat_indy(&out);
    // Legacy `vN = { … }` → `new int[]{ … }`
    out = polish_fill_array_initializers(&out);
    // `int[] a = new int[]{…}; int[][] m = new int[][]{ a, … }` → nested literal rows
    out = fold_2d_array_row_literals(&out);
    // `Integer[] a = new Integer[2]; a[0]=…; Arrays.asList(a)` → `Arrays.asList(…)`
    out = fold_arrays_as_list_filled_array(&out);
    // `int[] a = new int[]{…}; foo(a)` → `foo(new int[]{…})` (not into `new T[][]{ a }`)
    out = inline_filled_array_into_calls(&out);
    // `path = uri.getScheme(); if ("x".equals(path))` → inline getter into if when single-use.
    out = inline_single_use_into_equals_if(&out);
    // `.setJavaScriptEnabled(1)` → `.setJavaScriptEnabled(true)` for boolean-looking APIs.
    out = polish_booleanish_int_args(&out);
    // `obj == 0` / `!= 0` → null when the name looks like a reference.
    out = polish_null_comparisons_in_body(&out);
    // `x instanceof T == 0/null` → `!(x instanceof T)`, then flip if-eqz skip layout.
    out = polish_instanceof_comparisons_in_body(&out);
    out = flip_negated_instanceof_if_else(&out);
    out = flatten_else_if_chains(&out);
    out = flatten_returning_if_else(&out);
    // Cleanup may simplify array-index temps after the first loop-restore pass.
    out = restore_while_true_nested_for(&out);
    out = restore_counting_for_loop(&out);
    out = repair_register_reuse_scalars(&out);
    // Only in constructors: simplify "receiver.<init>();" (no args) to "super();".
    if is_constructor {
        let mut simplified = String::new();
        for line in out.lines() {
            let binding = strip_trailing_comment(line);
            let stmt = binding.trim();
            let ind = leading_indent(line);
            let comment_part = line.get(binding.len()..).unwrap_or("");
            if stmt.ends_with(".<init>();") {
                let prefix = stmt.trim_end_matches(".<init>();");
                if prefix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && !prefix.is_empty()
                {
                    writeln!(simplified, "{}super();{}", ind, comment_part).ok();
                    continue;
                }
            }
            simplified.push_str(line);
            if !line.is_empty() {
                simplified.push('\n');
            }
        }
        if simplified.ends_with('\n') && !out.ends_with('\n') {
            simplified.pop();
        }
        out = simplified;
    }
    // Rebase indentation to consistent 4-space levels (fixes over-indented bodies).
    out = normalize_java_indent(&out);
    out = repair_register_reuse_scalars(&out);
    out = repair_loop_length_index_shadow(&out);
    out = restore_array_store_postincrement(&out);
    out = restore_d8_merge_copy_loops(&out);
    out = cleanup_decompiler_artifacts(&out);
    out = repair_ssa_temp_increments(&out);
    out
}
