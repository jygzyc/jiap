//! Shared parsing, identifier, and structural helpers.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// Strip trailing "  // ..." comment from a line to get the statement part.
pub(crate) fn strip_trailing_comment(line: &str) -> String {
    let line = line.trim_end();
    if let Some(idx) = line.find("  // ") {
        line[..idx].trim_end().to_string()
    } else {
        line.to_string()
    }
}

/// Extract indent (leading spaces) from a line.
/// Extract indent (leading spaces) from a line.
pub(crate) fn leading_indent(line: &str) -> &str {
    let trimmed = line.trim_start();
    let n = line.len() - trimmed.len();
    &line[..n]
}

/// From invoke line content like "invoke-static( v2, v3, Class.method(A, B) );",
/// extract the inner " v2, v3, Class.method(A, B) " and split into (args, method_ref)
/// where method_ref is the last comma-separated token (method ref may contain commas inside parens).
/// Also handles no-arg forms: "invoke-static( Runtime.getRuntime() );".
/// From invoke line content like "invoke-static( v2, v3, Class.method(A, B) );",
/// extract the inner " v2, v3, Class.method(A, B) " and split into (args, method_ref)
/// where method_ref is the last comma-separated token (method ref may contain commas inside parens).
/// Also handles no-arg forms: "invoke-static( Runtime.getRuntime() );".
pub(crate) fn parse_invoke_args_and_method(line: &str) -> Option<(String, String)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let start = stmt.find('(')?;
    let end = stmt.rfind(");")?;
    let inner = stmt[start + 1..end].trim();
    if inner.is_empty() {
        return None;
    }
    // Find the last comma at paren depth 0 (method ref can contain commas in param types).
    let mut depth = 0u32;
    let mut last_comma_at = None;
    for (i, c) in inner.chars().enumerate() {
        match c {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => last_comma_at = Some(i),
            _ => {}
        }
    }
    let (args, method_ref) = if let Some(split_at) = last_comma_at {
        (
            inner[..split_at].trim().to_string(),
            inner[split_at + 1..].trim().to_string(),
        )
    } else {
        (String::new(), inner.to_string())
    };
    if method_ref.is_empty() {
        return None;
    }
    // Method ref from DEX is "Class.method(ParamTypes)" - use only "Class.method" for the call.
    let method_name = method_ref
        .find('(')
        .map(|i| method_ref[..i].trim_end())
        .unwrap_or(method_ref.as_str())
        .to_string();
    if method_name.is_empty() {
        return None;
    }
    Some((args, method_name))
}

/// Check if line is an invoke statement (invoke-xxx( ... );).
/// Check if line is an invoke statement (invoke-xxx( ... );).
pub(crate) fn is_invoke_line(line: &str) -> bool {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    stmt.starts_with("invoke-")
        && stmt.contains('(')
        && (stmt.ends_with(" );") || stmt.ends_with(");"))
}

/// Match "Type? var = expr;" → Some((var, expr)). Type is optional (e.g. `String result = "x";`).
/// Match "Type? var = expr;" → Some((var, expr)). Type is optional (e.g. `String result = "x";`).
pub(crate) fn parse_simple_assign_line(line: &str) -> Option<(String, String)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    if !stmt.ends_with(';') {
        return None;
    }
    let eq = stmt.find(" = ")?;
    let lhs = stmt[..eq].trim();
    let rest = stmt[eq + 3..].trim();
    let rhs = rhs_until_top_level_semicolon(rest)?;
    if rhs.is_empty() || rhs == "<result>" {
        return None;
    }
    // Reject control-flow / declarations that aren't simple assigns
    if lhs.contains('(') || lhs.starts_with("return") || lhs.starts_with("if") {
        return None;
    }
    let var = lhs.split_whitespace().last()?.to_string();
    if var.is_empty() || !is_java_ident(&var) {
        return None;
    }
    Some((var, rhs.to_string()))
}

/// First statement's RHS: `foo(a, b); leftover)` → `foo(a, b)`.
fn rhs_until_top_level_semicolon(rest: &str) -> Option<String> {
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'(' | b'{' | b'[' => depth += 1,
            b')' | b'}' | b']' => depth -= 1,
            b';' if depth <= 0 => {
                let rhs = rest[..i].trim();
                return if rhs.is_empty() {
                    None
                } else {
                    Some(rhs.to_string())
                };
            }
            _ => {}
        }
    }
    Some(rest.trim_end_matches(';').trim().to_string())
}

/// Match "return ident;" → Some(ident).
/// Match "return ident;" → Some(ident).
pub(crate) fn parse_return_ident_line(line: &str) -> Option<String> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let rest = stmt.strip_prefix("return ")?.trim_end_matches(';').trim();
    if rest.is_empty() || !is_java_ident(rest) {
        return None;
    }
    Some(rest.to_string())
}

pub(crate) fn is_java_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// True if `var` appears as a whole identifier in `text` (not as a substring of another ident).
/// True if `var` appears as a whole identifier in `text` (not as a substring of another ident).
pub(crate) fn ident_occurs(text: &str, var: &str) -> bool {
    if var.is_empty() || !text.contains(var) {
        return false;
    }
    let bytes = text.as_bytes();
    let v = var.as_bytes();
    let mut i = 0;
    while i + v.len() <= bytes.len() {
        if &bytes[i..i + v.len()] == v {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok = i + v.len() == bytes.len() || !is_ident_byte(bytes[i + v.len()]);
            if before_ok && after_ok {
                return true;
            }
            i += v.len();
        } else {
            i += 1;
        }
    }
    false
}

pub(crate) fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Check if line is "vN = <result>;" or "ident = <result>;" and return the LHS name.
/// Check if line is "vN = <result>;" or "ident = <result>;" and return the LHS name.
pub(crate) fn parse_move_result_line(line: &str) -> Option<String> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let eq = stmt.find(" = <result>;")?;
    let lhs = stmt[..eq].trim();
    if lhs.is_empty() {
        return None;
    }
    // vN / vN_k register forms
    if let Some(rest) = lhs.strip_prefix('v') {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '_') {
            return Some(lhs.to_string());
        }
    }
    if is_java_ident(lhs) {
        return Some(lhs.to_string());
    }
    None
}

/// Check if line is "return;" (return-void).
/// Check if line is "return;" (return-void).
pub(crate) fn is_return_void_line(line: &str) -> bool {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    stmt == "return;"
}

/// Check if line is "return vN;" / "return ident;" and return the returned name.
/// Check if line is "return vN;" / "return ident;" and return the returned name.
pub(crate) fn parse_return_reg_line(line: &str) -> Option<String> {
    parse_return_ident_line(line).or_else(|| {
        let binding = strip_trailing_comment(line);
        let stmt = binding.trim();
        let rest = stmt.strip_prefix("return ")?.trim_end_matches(';').trim();
        if rest.starts_with('v')
            && rest.len() > 1
            && rest[1..].chars().all(|c| c.is_ascii_digit() || c == '_')
        {
            return Some(rest.to_string());
        }
        None
    })
}

/// Match "if (cond) {" line: return Some(cond), else None.
/// Match "if (cond) {" line: return Some(cond), else None.
pub(crate) fn parse_if_condition(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix("if (")?;
    let end = rest.find(") {")?;
    Some(rest[..end].trim().to_string())
}

/// True if `var` is read in `line` (not merely assigned to).
pub(crate) fn ident_used_as_rvalue(line: &str, var: &str) -> bool {
    let binding = strip_trailing_comment(line);
    if assign_lhs_var(&binding).as_deref() == Some(var) {
        if let Some(eq) = binding.find(" = ") {
            return ident_occurs(&binding[eq + 3..], var);
        }
        return false;
    }
    ident_occurs(&binding, var)
}

/// Match "return expr;" line: return Some(expr), else None.
/// Match "return expr;" line: return Some(expr), else None.
pub(crate) fn parse_return_expr(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix("return ")?.trim_end_matches(';').trim();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

/// True if the line (after stripping comment) is "return;" or "return expr;".
/// True if the line (after stripping comment) is "return;" or "return expr;".
pub(crate) fn is_return_line(line: &str) -> bool {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    stmt == "return;" || (stmt.starts_with("return ") && stmt.ends_with(';'))
}

/// Match "var = new StringBuilder();" or "var = new StringBuilder(arg);", return (var, None) or (var, Some(arg)).
/// Match "var = new StringBuilder();" or "var = new StringBuilder(arg);", return (var, None) or (var, Some(arg)).
pub(crate) fn parse_new_stringbuilder(line: &str) -> Option<(String, Option<String>)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let eq = stmt.find(" = ")?;
    let var = stmt[..eq].trim().to_string();
    let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
    if !rhs.contains("new StringBuilder(") {
        return None;
    }
    let start = rhs.find('(')?;
    let end = rhs.rfind(')')?;
    let inner = rhs[start + 1..end].trim();
    let first_arg = if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    };
    Some((var, first_arg))
}

/// Match "var.append(arg);" or "dest = var.append(arg);", return Some((var, arg)).
/// Match "var.append(arg);" or "dest = var.append(arg);", return Some((var, arg)).
pub(crate) fn parse_append(line: &str) -> Option<(String, String)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let dot = stmt.find(".append(")?;
    let before_dot = &stmt[..dot];
    let var = if let Some(eq) = before_dot.find(" = ") {
        before_dot[eq + 3..].trim().to_string()
    } else {
        before_dot.trim().to_string()
    };
    let start = dot + ".append(".len();
    let end = stmt.rfind(");")?;
    let arg = stmt[start..end].trim().to_string();
    Some((var, arg))
}

/// Match "var.<init>();" or "var.<init>(arg);", return Some((var, optional_arg)).
/// Match "var.<init>();" or "var.<init>(arg);", return Some((var, optional_arg)).
pub(crate) fn parse_init_call(line: &str) -> Option<(String, Option<String>)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let init_pos = stmt.find(".<init>(")?;
    let var = stmt[..init_pos].trim().to_string();
    let start = init_pos + ".<init>(".len();
    let end = stmt.rfind(");")?;
    let inner = stmt[start..end].trim();
    let arg = if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    };
    Some((var, arg))
}

/// Match "var.println(arg);" or "dest = var.println(arg);", return Some((var, arg)).
/// Match "var.println(arg);" or "dest = var.println(arg);", return Some((var, arg)).
pub(crate) fn parse_println(line: &str) -> Option<(String, String)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    let dot = stmt.find(".println(")?;
    let before_dot = &stmt[..dot];
    let var = if let Some(eq) = before_dot.find(" = ") {
        before_dot[eq + 3..].trim().to_string()
    } else {
        before_dot.trim().to_string()
    };
    let start = dot + ".println(".len();
    let end = stmt.rfind(");")?;
    let arg = stmt[start..end].trim().to_string();
    Some((var, arg))
}

/// Match "dest = var.toString();" or "return var.toString();", return Some((dest_var, sb_var)) or Some(("return", sb_var)).
/// Match "dest = var.toString();" or "return var.toString();", return Some((dest_var, sb_var)) or Some(("return", sb_var)).
pub(crate) fn parse_to_string(line: &str) -> Option<(String, String)> {
    let binding = strip_trailing_comment(line);
    let stmt = binding.trim();
    if let Some(rest) = stmt.strip_prefix("return ") {
        let rest = rest.trim_end_matches(';').trim();
        if let Some(var) = rest.strip_suffix(".toString()") {
            return Some(("return".to_string(), var.trim().to_string()));
        }
    }
    let eq = stmt.find(" = ")?;
    let dest = stmt[..eq].trim().to_string();
    let rhs = stmt[eq + 3..].trim_end_matches(';').trim();
    if let Some(var) = rhs.strip_suffix(".toString()") {
        return Some((dest, var.trim().to_string()));
    }
    None
}

pub(crate) fn is_local_temp_name(var: &str) -> bool {
    var.starts_with("local")
        && var.len() > 5
        && var.as_bytes()[5..].iter().all(|b| b.is_ascii_digit())
}

/// Line contains an invoke (possibly on the RHS of an assignment).
/// Line contains an invoke (possibly on the RHS of an assignment).
pub(crate) fn line_contains_call(line: &str) -> bool {
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    if t.is_empty() {
        return false;
    }
    let expr = if let Some(eq) = t.find(" = ") {
        t[eq + 3..].trim()
    } else {
        t
    };
    if expr.starts_with("if ")
        || expr.starts_with("while ")
        || expr.starts_with("for ")
        || expr.starts_with("switch ")
        || expr.starts_with("return new ")
        || expr.contains(" = new ")
    {
        return false;
    }
    expr.contains('(') && expr.contains(')')
}

/// Collect whole Java identifiers appearing in `line` (declaration order, deduped).
/// Collect whole Java identifiers appearing in `line` (declaration order, deduped).
pub(crate) fn collect_idents_in_line(line: &str) -> Vec<String> {
    let t = strip_trailing_comment(line);
    let mut out = Vec::new();
    let mut i = 0usize;
    let bytes = t.as_bytes();
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let c = bytes[i];
                if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                    i += 1;
                } else {
                    break;
                }
            }
            let ident = t[start..i].to_string();
            if is_java_ident(&ident) && !out.contains(&ident) {
                out.push(ident);
            }
        } else {
            i += 1;
        }
    }
    out
}

/// `arr4` used before `int[] arr4` is usually a scalar temp misnamed as an array local.
/// Extract the register number from SSA variable names like "v2", "local2", "localN".
pub(crate) fn extract_reg_number(var: &str) -> Option<&str> {
    if let Some(n) = var.strip_prefix("local") {
        if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
            return Some(n);
        }
    }
    if let Some(n) = var.strip_prefix('v') {
        if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
            return Some(n);
        }
    }
    None
}

/// Inline static field references: "var = System.out;" followed by "var.println(x)" → "System.out.println(x)".
/// Handles SSA aliasing where "local2 = System.out;" and "v2.println(x)" refer to the same register.
/// `Foo.class` / `com.example.Foo.class` (DEX const-class).
pub(crate) fn is_class_literal(s: &str) -> bool {
    let s = s.trim();
    let Some(ty) = s.strip_suffix(".class") else {
        return false;
    };
    let ty = ty.trim();
    if ty.is_empty() || ty.contains('(') || ty.contains(' ') {
        return false;
    }
    ty.split('.').all(|p| !p.is_empty() && is_java_ident(p))
}

/// `(Type) expr` — Type starts with uppercase letter / package.
pub(crate) fn is_numeric_literal(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    if bytes[0] == b'-' {
        if bytes.len() == 1 {
            return false;
        }
        i = 1;
    }
    if i + 1 < bytes.len() && bytes[i] == b'0' && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
        i += 2;
        if i >= bytes.len() {
            return false;
        }
        let mut saw_digit = false;
        while i < bytes.len() {
            let b = bytes[i];
            if b.is_ascii_hexdigit() {
                saw_digit = true;
                i += 1;
            } else if matches!(b, b'l' | b'L') && i + 1 == bytes.len() {
                return saw_digit;
            } else {
                return false;
            }
        }
        return saw_digit;
    }
    let mut saw_digit = false;
    let mut saw_dot = false;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_digit() {
            saw_digit = true;
            i += 1;
        } else if b == b'.' && !saw_dot {
            saw_dot = true;
            i += 1;
        } else if matches!(b, b'f' | b'F' | b'd' | b'D' | b'l' | b'L') && i + 1 == bytes.len() {
            return saw_digit;
        } else {
            return false;
        }
    }
    saw_digit
}

/// Literals cheap enough to duplicate at every use (`1`, `"x"`, `true`, …).
/// Literals cheap enough to duplicate at every use (`1`, `"x"`, `true`, …).
pub(crate) fn is_cheap_literal_rhs(rhs: &str) -> bool {
    let rhs = rhs.trim();
    if rhs.is_empty() {
        return false;
    }
    if matches!(rhs, "null" | "true" | "false") {
        return true;
    }
    if is_numeric_literal(rhs) {
        return true;
    }
    if rhs.starts_with('"') && rhs.ends_with('"') && rhs.len() >= 2 {
        return true;
    }
    if rhs.starts_with('\'') && rhs.ends_with('\'') && rhs.len() >= 3 {
        return true;
    }
    // Class literals are immutable / cheap to repeat at each use site.
    if is_class_literal(rhs) {
        return true;
    }
    false
}

/// `this.foo`, `Foo.BAR`, `a.b.c` — identifiers separated by dots only.
/// Not a class literal (`Foo.class` / `pkg.Foo.class`).
/// `this.foo`, `Foo.BAR`, `a.b.c` — identifiers separated by dots only.
/// Not a class literal (`Foo.class` / `pkg.Foo.class`).
pub(crate) fn is_simple_field_path(s: &str) -> bool {
    let s = s.trim();
    if !s.contains('.') || s.contains('(') || s.contains(')') || s.contains(' ') || s.contains('[')
    {
        return false;
    }
    // `Type.class` is a const-class literal, not a field access.
    if s.ends_with(".class") {
        return false;
    }
    s.split('.').all(|p| !p.is_empty() && is_java_ident(p))
}

/// True for decompiler temps (`i0`, `s0`, `view0`, `local0`) — safe to inline away.
/// Meaningful names (`email`, `password`) are kept even when single-use.
/// True for decompiler temps (`i0`, `s0`, `view0`, `local0`) — safe to inline away.
/// Meaningful names (`email`, `password`) are kept even when single-use.
pub(crate) fn is_temp_like_name(var: &str) -> bool {
    if var.is_empty() {
        return false;
    }
    // Array-index temps from SemanticRole::Index (often hold a literal 0/1).
    if matches!(var, "i" | "j" | "k") {
        return true;
    }
    if var.starts_with("local") && var.bytes().skip(5).all(|b| b.is_ascii_digit()) {
        return true;
    }
    let b = var.as_bytes();
    // i0 / s0 / v0 / o0 / …
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1..].iter().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // k_0 / j_0 (SSA copies of loop indexes)
    if b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1..].iter().all(|c| *c == b'_' || c.is_ascii_digit())
        && b[1..].iter().any(|c| c.is_ascii_digit())
    {
        return true;
    }
    // view0, textView0, arr0, cls0, …
    if let Some(i) = var.bytes().rposition(|c| c.is_ascii_alphabetic()) {
        let (prefix, digits) = var.split_at(i + 1);
        if !digits.is_empty()
            && digits.bytes().all(|c| c.is_ascii_digit())
            && prefix
                .chars()
                .next()
                .map(|c| c.is_ascii_lowercase())
                .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Variable assigned on this line (`var = …` / `int var = …`), if any.
/// Variable assigned on this line (`var = …` / `int var = …`), if any.
pub(crate) fn assign_lhs_var(line: &str) -> Option<String> {
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    let eq = t.find(" = ")?;
    let lhs = t[..eq].trim();
    let var = lhs.rsplit(' ').next().unwrap_or("");
    if !var.is_empty() && is_java_ident(var) {
        Some(var.to_string())
    } else {
        None
    }
}

/// True if `line` reassigns or mutates `var` (ends the previous value's live range).
/// True if `line` reassigns or mutates `var` (ends the previous value's live range).
pub(crate) fn line_kills_var(line: &str, var: &str) -> bool {
    if assign_lhs_var(line).as_deref() == Some(var) {
        return true;
    }
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    // Compound assigns / increments: i0 += 1; ++i0; i0++;
    let compounds = [
        format!("{} += ", var),
        format!("{} -= ", var),
        format!("{} *= ", var),
        format!("{} /= ", var),
        format!("{} %= ", var),
        format!("{} &= ", var),
        format!("{} |= ", var),
        format!("{} ^= ", var),
        format!("{} <<= ", var),
        format!("{} >>= ", var),
    ];
    if compounds.iter().any(|p| t.starts_with(p.as_str())) {
        return true;
    }
    if t == format!("++{};", var)
        || t == format!("--{};", var)
        || t == format!("{}++;", var)
        || t == format!("{}--;", var)
    {
        return true;
    }
    false
}

pub(crate) fn is_loop_body_slot_update(var: &str, def_idx: usize, lines: &[&str]) -> bool {
    def_idx > 0
        && lines[..def_idx]
            .iter()
            .any(|l| line_uses_ident_as_var(l, var))
}

pub(crate) fn has_index_increment_after(lines: &[&str], after: usize, var: &str) -> bool {
    lines
        .iter()
        .skip(after + 1)
        .any(|l| is_index_increment(l, var))
}

/// Remove bare "var; /* move-exception */" statements and inline single-use temps
/// (literals, copies, casts, single-use calls) into their only use.
///
/// Live ranges stop at the next assignment to the same name, so reused temps like
/// `s0 = "…"; …; s0 = this.foo;` are handled correctly.
pub(crate) fn parse_equals_assign(line: &str) -> Option<(String, String)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    let rhs = rhs.trim();
    // x.equals(y) or x.endsWith(y) / startsWith / contains
    for meth in [
        "equals",
        "endsWith",
        "startsWith",
        "contains",
        "equalsIgnoreCase",
    ] {
        if let Some(dot) = rhs.find(&format!(".{}(", meth)) {
            if rhs.ends_with(')') {
                return Some((var, rhs.to_string()));
            }
            let _ = dot;
        }
    }
    None
}

pub(crate) fn is_string_literal(s: &str) -> bool {
    let s = s.trim();
    s.len() >= 2 && s.starts_with('"') && s.ends_with('"')
}

pub(crate) fn condition_tests_var(cond: &str, var: &str) -> bool {
    let c = cond.trim();
    c == var
        || c == format!("!{}", var)
        || c == format!("!(!{})", var)
        || c == format!("{} == 0", var)
        || c == format!("{} != 0", var)
        || c == format!("{} == null", var)
        || c == format!("{} != null", var)
}

pub(crate) fn rewrite_condition_with_equals(cond: &str, var: &str, equals_expr: &str) -> String {
    let c = cond.trim();
    // Positive: var / var != 0 / var != null / !(!var)
    if c == var
        || c == format!("{} != 0", var)
        || c == format!("{} != null", var)
        || c == format!("!(!{})", var)
    {
        return equals_expr.to_string();
    }
    // Negative: !var / var == 0 / var == null
    if c == format!("!{}", var) || c == format!("{} == 0", var) || c == format!("{} == null", var) {
        return format!("!({})", equals_expr);
    }
    // Fallback: replace ident
    replace_ident_as_expr(c, var, equals_expr)
}

/// True when condition is a (possibly negated) string/predicate call like `"x".equals(y)`.
pub(crate) fn condition_has_string_predicate(cond: &str) -> bool {
    let c = cond.trim();
    let inner = c
        .strip_prefix("!(")
        .and_then(|s| s.strip_suffix(')'))
        .map(|s| s.trim())
        .unwrap_or(c);
    for meth in [
        "equals",
        "endsWith",
        "startsWith",
        "contains",
        "equalsIgnoreCase",
    ] {
        if inner.contains(&format!(".{}(", meth)) {
            return true;
        }
    }
    false
}

pub(crate) fn strip_outer_not_parens(cond: &str) -> Option<String> {
    let c = cond.trim();
    let inner = c.strip_prefix("!(")?.strip_suffix(')')?;
    Some(inner.trim().to_string())
}

/// Extract string literal from `"lit".equals(…)` / `"lit".endsWith(…)` style conditions.
/// Extract string literal from `"lit".equals(…)` / `"lit".endsWith(…)` style conditions.
pub(crate) fn string_lit_in_predicate_cond(cond: &str) -> Option<String> {
    let c = strip_outer_not_parens(cond).unwrap_or_else(|| cond.trim().to_string());
    let c = c.trim();
    if !c.starts_with('"') {
        return None;
    }
    let end = c[1..].find('"')? + 1;
    Some(c[..=end].to_string())
}

/// Drop `name = "lit";` when the next statement is `if ("lit".equals(…))` / negated form.
pub(crate) fn rebuild_lines(
    lines: &[&str],
    skip: &HashSet<usize>,
    replacements: &HashMap<usize, String>,
    trailing_nl: bool,
) -> String {
    let mut out = String::new();
    let mut wrote = false;
    for (idx, line) in lines.iter().enumerate() {
        if skip.contains(&idx) {
            continue;
        }
        if wrote {
            out.push('\n');
        }
        if let Some(rep) = replacements.get(&idx) {
            out.push_str(rep);
        } else {
            out.push_str(line);
        }
        wrote = true;
        let _ = idx;
    }
    if trailing_nl && wrote {
        out.push('\n');
    } else if trailing_nl && !wrote {
        // empty
    }
    out
}

/// Find closing `}` for the block opened by the first `{` on `open_idx`.
/// Handles `} else {` by ignoring the leading `}` and matching the else-body `{`.
/// Find closing `}` for the block opened by the first `{` on `open_idx`.
/// Handles `} else {` by ignoring the leading `}` and matching the else-body `{`.
pub(crate) fn find_closing_brace_line(lines: &[&str], open_idx: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut seen_open = false;
    for (i, line) in lines.iter().enumerate().skip(open_idx) {
        let t = line.trim();
        let mut in_str = false;
        let mut escape = false;
        for c in t.chars() {
            if in_str {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_str = false;
                }
                continue;
            }
            match c {
                '"' => in_str = true,
                '{' => {
                    depth += 1;
                    seen_open = true;
                }
                '}' => {
                    if !seen_open {
                        // Leading `}` on `} else {` — not part of the block we are matching.
                        continue;
                    }
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// End line of `if (…) { … } [else if …] [else { … }]`, past any `} else {` continuations.
/// End line of `if (…) { … } [else if …] [else { … }]`, past any `} else {` continuations.
pub(crate) fn find_if_chain_end(lines: &[&str], if_idx: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut seen_open = false;
    for (i, line) in lines.iter().enumerate().skip(if_idx) {
        let t = line.trim();
        let mut in_str = false;
        let mut escape = false;
        let chars: Vec<char> = t.chars().collect();
        let mut ci = 0;
        while ci < chars.len() {
            let c = chars[ci];
            if in_str {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_str = false;
                }
                ci += 1;
                continue;
            }
            match c {
                '"' => in_str = true,
                '{' => {
                    depth += 1;
                    seen_open = true;
                }
                '}' => {
                    if !seen_open {
                        ci += 1;
                        continue;
                    }
                    depth -= 1;
                    if depth == 0 {
                        // `} else {` / `} else if (` continues the chain.
                        let rest: String = chars[ci + 1..].iter().collect();
                        let rest_t = rest.trim_start();
                        if rest_t.starts_with("else") {
                            // Keep scanning; following `{` re-opens.
                            ci += 1;
                            continue;
                        }
                        return Some(i);
                    }
                }
                _ => {}
            }
            ci += 1;
        }
    }
    None
}

/// End line of a full `if` / `if-else` / `if-else if` statement starting at `if_idx`.
/// End line of a full `if` / `if-else` / `if-else if` statement starting at `if_idx`.
pub(crate) fn find_if_statement_end(lines: &[&str], if_idx: usize) -> Option<usize> {
    let mut close = find_closing_brace_line(lines, if_idx)?;
    loop {
        let t = lines[close].trim();
        if t == "} else {" || t.starts_with("} else if (") {
            close = find_closing_brace_line(lines, close)?;
            continue;
        }
        return Some(close);
    }
}

/// Flip `if (!(pred)) { THEN } else { ELSE }` → `if (pred) { ELSE } else { THEN }`
/// when pred is a string equals/endsWith-style call. Repeat until stable.
pub(crate) fn replace_ident_as_expr(text: &str, var: &str, replacement: &str) -> String {
    if var.is_empty() || !text.contains(var) {
        return text.to_string();
    }
    let needs_wrap = replacement.contains(' ')
        || (replacement.starts_with('(') && !replacement.ends_with(')'))
        || is_cast_expr(replacement);
    let bytes = text.as_bytes();
    let v = var.as_bytes();
    let mut out = String::with_capacity(text.len() + replacement.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + v.len() <= bytes.len() && &bytes[i..i + v.len()] == v {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok = i + v.len() == bytes.len() || !is_ident_byte(bytes[i + v.len()]);
            if before_ok && after_ok {
                // Do not substitute into field names (`arr.length` is not variable `length`).
                if i > 0 && bytes[i - 1] == b'.' {
                    out.push(bytes[i] as char);
                    i += 1;
                    continue;
                }
                let next = bytes.get(i + v.len()).copied();
                let as_receiver = matches!(next, Some(b'.') | Some(b'['));
                let unary_not = i > 0 && bytes[i - 1] == b'!';
                if needs_wrap && (as_receiver || unary_not) {
                    out.push('(');
                    out.push_str(replacement);
                    out.push(')');
                } else {
                    out.push_str(replacement);
                }
                i += v.len();
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Merge nested `if (A) { if (B) { BODY } }` (no else on either) into `if (A && B) { BODY }`.
/// Split a boolean condition on `&&` or `||` at parenthesis/string depth zero.
pub(crate) fn split_top_level_bool_op(cond: &str, op: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    let mut start = 0usize;
    let chars: Vec<char> = cond.chars().collect();
    let oplen = op.len();
    let ochars: Vec<char> = op.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ if depth == 0 && i + oplen <= chars.len() => {
                if chars[i..i + oplen]
                    .iter()
                    .copied()
                    .eq(ochars.iter().copied())
                {
                    let piece = chars[start..i]
                        .iter()
                        .collect::<String>()
                        .trim()
                        .to_string();
                    if !piece.is_empty() {
                        out.push(piece);
                    }
                    start = i + oplen;
                    i += oplen;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let piece = chars[start..].iter().collect::<String>().trim().to_string();
    if !piece.is_empty() {
        out.push(piece);
    }
    out
}

pub(crate) fn strip_balanced_outer_parens(s: &str) -> Option<String> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, c) in s.char_indices() {
        if in_str {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' => {
                depth += 1;
                if depth == 1 && i != 0 {
                    return None;
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 && i != s.len() - 1 {
                    return None;
                }
            }
            _ => {}
        }
    }
    Some(s[1..s.len() - 1].trim().to_string())
}

/// Strip outer `!` / `!(…)` from a boolean operand.
/// Strip outer `!` / `!(…)` from a boolean operand.
pub(crate) fn strip_negated_operand(part: &str) -> Option<String> {
    let rest = part.trim().strip_prefix('!')?.trim();
    if rest.starts_with('(') {
        strip_balanced_outer_parens(rest)
    } else if !rest.is_empty() {
        Some(rest.to_string())
    } else {
        None
    }
}

/// De Morgan: `!A && !B && …` → `A || B || …`
/// De Morgan: `!A && !B && …` → `A || B || …`
pub(crate) fn demorgan_or_from_negated_and(cond: &str) -> Option<String> {
    let parts = split_top_level_bool_op(cond, "&&");
    if parts.len() < 1 {
        return None;
    }
    let mut positive = Vec::new();
    for p in parts {
        positive.push(strip_negated_operand(&p)?);
    }
    Some(positive.join(" || "))
}

pub(crate) fn parse_else_if_condition(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix("} else if (")?;
    let end = rest.find(") {")?;
    Some(rest[..end].trim().to_string())
}

pub(crate) fn normalized_block_stmts(lines: &[&str], start: usize, end: usize) -> Vec<String> {
    lines[start..end]
        .iter()
        .map(|l| l.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `if (A) { BODY } else if (B) { BODY }` (no further else) → `if (A || B) { BODY }`
pub(crate) fn parse_iterator_assign(line: &str) -> Option<(String, String)> {
    let (var, rhs) = parse_simple_assign_line(line)?;
    let coll = rhs.strip_suffix(".iterator()")?.trim();
    if coll.is_empty() {
        return None;
    }
    Some((var, coll.to_string()))
}

pub(crate) fn parse_while_condition(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix("while (")?;
    let end = rest.find(") {")?;
    Some(rest[..end].trim().to_string())
}

pub(crate) fn line_uses_ident_as_var(line: &str, var: &str) -> bool {
    replace_ident_as_expr(line, var, "__X__") != line
}

pub(crate) fn format_int_add_tail(body_lines: &[&str], tail: &str) -> String {
    if tail == "0" || tail == "1" {
        return tail.to_string();
    }
    if body_lines
        .iter()
        .any(|l| line_declares_boolean_var(l, tail))
    {
        return format!("({} ? 1 : 0)", tail);
    }
    tail.to_string()
}

/// `i6 = arr1.length; length = mac.length; i6 = i6 + length; …; return i6 + same;`
/// → `return arr1.length + mac.length + … + (same ? 1 : 0);`
pub(crate) fn split_first_arg(args: &str) -> Option<String> {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, c) in args.chars().enumerate() {
        if in_str {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                let a = args[..i].trim();
                return if a.is_empty() {
                    None
                } else {
                    Some(a.to_string())
                };
            }
            _ => {}
        }
    }
    let a = args.trim();
    if a.is_empty() {
        None
    } else {
        Some(a.to_string())
    }
}

/// Strip `Objects.requireNonNull` / Kotlin `Intrinsics.checkNotNull*` wrappers.
pub(crate) fn collect_top_level_stmts(lines: &[&str], start: usize, end: usize) -> Vec<String> {
    let mut stmts = Vec::new();
    let mut i = start;
    while i < end {
        let t = lines[i].trim();
        if t.is_empty() {
            i += 1;
            continue;
        }
        if t.ends_with('{') {
            let block_start = i;
            let mut depth = 0i32;
            while i < end {
                let tt = lines[i].trim();
                depth += tt.chars().filter(|c| *c == '{').count() as i32;
                depth -= tt.chars().filter(|c| *c == '}').count() as i32;
                i += 1;
                if depth <= 0 {
                    break;
                }
            }
            // Opaque nested block — blocks suffix matching across it.
            stmts.push(lines[block_start..i].join("\n"));
            continue;
        }
        stmts.push(lines[i].to_string());
        i += 1;
    }
    stmts
}

/// Merge consecutive identical catch bodies into multi-catch `A | B`.
pub(crate) fn extract_paren_arg_list(expr: &str) -> Option<Vec<String>> {
    let e = expr.trim();
    let rest = e.strip_prefix('(')?;
    if !rest.ends_with(')') {
        return None;
    }
    let inner = &rest[..rest.len() - 1];
    Some(split_top_level_args(inner))
}

pub(crate) fn split_top_level_args(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    let mut start = 0usize;
    let chars: Vec<char> = args.chars().collect();
    for i in 0..chars.len() {
        let c = chars[i];
        if in_str {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                let piece = chars[start..i]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string();
                if !piece.is_empty() {
                    out.push(piece);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let piece = chars[start..].iter().collect::<String>().trim().to_string();
    if !piece.is_empty() {
        out.push(piece);
    }
    out
}

/// Rewrite legacy `vN = { 1, 2 };` to `vN = new int[]{ 1, 2 };`.
pub(crate) fn count_var_uses(lines: &[&str], start: usize, end: usize, var: &str) -> usize {
    lines[start..end]
        .iter()
        .filter(|l| line_uses_ident_as_var(l, var))
        .count()
}

/// `int[] a = new int[]{1}; int[][] m = new int[][]{ a };` → nested literal rows when each
/// row array is a single-use literal initializer.
pub(crate) fn parse_hashcode_assign(line: &str, disc: &str) -> Option<String> {
    // `int h = s.hashCode();` or `h = s.hashCode();`
    let line = line.trim().trim_end_matches(';').trim();
    let assign = if let Some(rest) = line.strip_prefix("int ") {
        rest.trim()
    } else {
        line
    };
    let (lhs, rhs) = assign.split_once('=')?;
    if lhs.trim() != disc {
        return None;
    }
    let rhs = rhs.trim();
    rhs.strip_suffix(".hashCode()")
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
}

pub(crate) fn parse_java_int_literal(s: &str) -> Option<i32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return i32::from_str_radix(hex, 16).ok();
    }
    s.parse::<i32>().ok()
}

/// Java `String.hashCode()` (31-bit polynomial over UTF-16 code units; BMP chars = Rust chars).
pub(crate) fn parse_java_string_literal(after: &str) -> Option<String> {
    if !after.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = after[1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.push(match n {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        other => other,
                    });
                }
            }
            '"' => return Some(out),
            other => out.push(other),
        }
    }
    None
}

/// `Foo.class` / `com.example.Foo.class` (DEX const-class).
/// `(Type) expr` — Type starts with uppercase letter / package.
pub(crate) fn is_cast_expr(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with('(') {
        return false;
    }
    let Some(close) = s.find(')') else {
        return false;
    };
    if close + 1 >= s.len() || s.as_bytes()[close + 1] != b' ' {
        return false;
    }
    let ty = s[1..close].trim();
    if ty.is_empty() {
        return false;
    }
    // Type name: Ident or pkg.Ident (allow [] for arrays)
    let ok_ty = ty
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '[' || c == ']')
        && ty
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false);
    if !ok_ty {
        return false;
    }
    let inner = s[close + 1..].trim();
    is_java_ident(inner)
        || is_simple_field_path(inner)
        || is_numeric_literal(inner)
        || matches!(inner, "null" | "true" | "false")
        || (inner.starts_with('"') && inner.ends_with('"'))
        || is_simple_call_expr(inner)
        || (inner.starts_with('(') && inner.contains(')') && is_cast_expr_shallow(inner))
}

pub(crate) fn is_cast_expr_shallow(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with('(') {
        return false;
    }
    let Some(close) = s.find(')') else {
        return false;
    };
    if close + 1 >= s.len() {
        return false;
    }
    let ty = s[1..close].trim();
    !ty.is_empty()
        && ty
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
}

/// `k_0` / `i0` is an SSA copy of `k` / `i`.
pub(crate) fn is_ssa_temp_of(temp: &str, named: &str) -> bool {
    if temp == named || named.is_empty() {
        return false;
    }
    let Some(rest) = temp.strip_prefix(named) else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '_')
}

pub(crate) fn is_index_increment(line: &str, i: &str) -> bool {
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    if t == format!("{}++;", i)
        || t == format!("++{};", i)
        || t == format!("{} = {} + 1;", i, i)
        || t == format!("{} += 1;", i)
    {
        return true;
    }
    if let Some((var, rhs)) = parse_simple_assign_line(line) {
        if var == i {
            let compact = rhs.replace(' ', "");
            if compact == format!("{i}+1") {
                return true;
            }
            // Dalvik SSA footer: `int j = i1 + 1;` after register reuse.
            if compact.ends_with("+1") {
                let base = &compact[..compact.len() - 2];
                if is_temp_like_name(base) {
                    return true;
                }
            }
        }
    }
    false
}

/// True when every use of `var` after its definition is only `var.length` (or embedded in a sum).
/// True when every use of `var` after its definition is only `var.length` (or embedded in a sum).
pub(crate) fn all_uses_are_length_member(
    lines: &[&str],
    var: &str,
    def_idx: usize,
    end_idx: usize,
) -> bool {
    let uses: Vec<&str> = lines[def_idx + 1..end_idx]
        .iter()
        .copied()
        .filter(|l| line_uses_ident_as_var(l, var))
        .collect();
    if uses.is_empty() {
        return false;
    }
    uses.iter().all(|l| {
        let t = strip_trailing_comment(l);
        let stmt = t.trim();
        if stmt.ends_with(&format!("{}.length;", var))
            || stmt.contains(&format!("{}.length +", var))
            || stmt.contains(&format!("+ {}.length", var))
            || stmt.contains(&format!("+ {}.length +", var))
        {
            return true;
        }
        // `length = arr.length` where rhs is exactly var.length
        if let Some((_, rhs)) = parse_simple_assign_line(l) {
            return rhs.trim() == format!("{}.length", var);
        }
        false
    })
}

pub(crate) fn line_declares_boolean_var(line: &str, var: &str) -> bool {
    let binding = strip_trailing_comment(line);
    let t = binding.trim();
    if !t.starts_with("boolean ") {
        return false;
    }
    assign_lhs_var(line).as_deref() == Some(var)
}

pub(crate) fn strip_null_check_expr(expr: &str) -> Option<String> {
    let e = expr.trim();
    const PREFIXES: &[&str] = &[
        "java.util.Objects.requireNonNull(",
        "Objects.requireNonNull(",
        "kotlin.jvm.internal.Intrinsics.checkNotNull(",
        "Intrinsics.checkNotNull(",
        "kotlin.jvm.internal.Intrinsics.checkNotNullParameter(",
        "Intrinsics.checkNotNullParameter(",
        "kotlin.jvm.internal.Intrinsics.checkNotNullExpressionValue(",
        "Intrinsics.checkNotNullExpressionValue(",
    ];
    for p in PREFIXES {
        if let Some(rest) = e.strip_prefix(p) {
            if !rest.ends_with(')') {
                continue;
            }
            let inner = &rest[..rest.len() - 1];
            if let Some(arg) = split_first_arg(inner) {
                return Some(arg);
            }
        }
    }
    None
}

/// `recv.method(args)` including nested receivers like `((TextView) this.foo(1)).bar()`.
/// `recv.method(args)` including nested receivers like `((TextView) this.foo(1)).bar()`.
pub(crate) fn is_simple_call_expr(s: &str) -> bool {
    let s = s.trim();
    if !s.ends_with(')') || s.contains('=') {
        return false;
    }
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut open_idx = None;
    for i in (0..bytes.len()).rev() {
        match bytes[i] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth == 0 {
                    open_idx = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(open) = open_idx else {
        return false;
    };
    if open == 0 {
        return false;
    }
    let recv_method = s[..open].trim();
    let Some(dot) = recv_method.rfind('.') else {
        return false;
    };
    let method = recv_method[dot + 1..].trim();
    if !is_java_ident(method) {
        return false;
    }
    let recv = recv_method[..dot].trim();
    !recv.is_empty()
}

/// Pure arithmetic / indexing without calls or `new` (safe to duplicate at one use site).
/// Pure arithmetic / indexing without calls or `new` (safe to duplicate at one use site).
pub(crate) fn is_simple_arith_expr(rhs: &str) -> bool {
    let rhs = rhs.trim();
    if rhs.is_empty() || rhs.contains('"') || rhs.contains("()") || rhs.contains(".(") {
        return false;
    }
    if rhs.contains("new ") || rhs.contains("->") {
        return false;
    }
    rhs.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || " +-*/%()[].".contains(c))
}

/// True if `rhs` is safe to inline into a single use.
/// True if `rhs` is safe to inline into a single use.
pub(crate) fn is_inlineable_rhs(rhs: &str) -> bool {
    let rhs = rhs.trim();
    if rhs.is_empty() {
        return false;
    }
    // String / char literals
    if (rhs.starts_with('"') && rhs.ends_with('"'))
        || (rhs.starts_with('\'') && rhs.ends_with('\'') && rhs.len() >= 3)
    {
        return true;
    }
    // null / booleans
    if matches!(rhs, "null" | "true" | "false") {
        return true;
    }
    // Numeric (decimal / hex / float / long / float suffixes)
    if is_numeric_literal(rhs) {
        return true;
    }
    // const-class: Context.class / android.content.Context.class
    if is_class_literal(rhs) {
        return true;
    }
    // Simple identifier (copy): i0, s1, this, pickerIntent
    if is_java_ident(rhs) {
        return true;
    }
    // Simple field / static path: this.this$0, Foo.BAR, obj.field — no spaces, parens, or operators
    if is_simple_field_path(rhs) {
        return true;
    }
    // check-cast: `(TextView) expr`
    if is_cast_expr(rhs) {
        return true;
    }
    // Method / static call used once: `this.findViewById(1)`, `view.getText()`, `TextUtils.isEmpty(email)`
    if is_simple_call_expr(rhs) {
        return true;
    }
    // `i * i`, `i + 1`, `arr[i]` — single-use arith/array index temps (fillArrayData, binarySearch mid).
    if is_simple_arith_expr(rhs) {
        return true;
    }
    false
}

/// `new int[]{1, 2, 3}` / `new Integer[]{ a, b }` — not sized `new Integer[2]`.
pub(crate) fn is_filled_array_literal(rhs: &str) -> bool {
    let rhs = rhs.trim();
    if !rhs.starts_with("new ") || !rhs.ends_with('}') {
        return false;
    }
    rhs.contains("[]{") || rhs.contains("[] {")
}
