//! Indent normalization and synchronized-block cleanup.

use std::fmt::Write;

use super::util::*;

/// Re-indent Java-like source to 4 spaces per brace level.
/// Preserves the indent of the first non-empty line as the base level.
pub fn normalize_java_indent(body: &str) -> String {
    if body.is_empty() {
        return body.to_string();
    }
    const IND: &str = "    ";
    let lines: Vec<&str> = body.lines().collect();
    let mut base_levels = 0usize;
    for line in &lines {
        if !line.trim().is_empty() {
            let spaces = line.len() - line.trim_start().len();
            base_levels = spaces / 4;
            // If someone used 8-space steps, first content line may be at 16 spaces
            // while the method signature (not in body) is at 4. Bodies usually start
            // at indent 2 (8 spaces) after the fix; clamp wild bases.
            if base_levels > 2 && spaces % 8 == 0 && spaces >= 16 {
                base_levels = 2;
            }
            break;
        }
    }
    let mut depth = base_levels;
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
                out.push('\n');
            }
            continue;
        }
        let starts_with_close = trimmed.starts_with('}');
        if starts_with_close {
            depth = depth.saturating_sub(1);
        }
        out.push_str(&IND.repeat(depth));
        out.push_str(trimmed);
        if idx < lines.len().saturating_sub(1) || body.ends_with('\n') {
            out.push('\n');
        }
        // Adjust depth from braces outside strings/comments (best-effort).
        let mut in_str: Option<char> = None;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let bytes: Vec<char> = trimmed.chars().collect();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i];
            let next = bytes.get(i + 1).copied();
            if in_line_comment {
                break;
            }
            if in_block_comment {
                if c == '*' && next == Some('/') {
                    in_block_comment = false;
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            if let Some(q) = in_str {
                if c == '\\' {
                    i += 2;
                    continue;
                }
                if c == q {
                    in_str = None;
                }
                i += 1;
                continue;
            }
            if c == '/' && next == Some('/') {
                in_line_comment = true;
                i += 2;
                continue;
            }
            if c == '/' && next == Some('*') {
                in_block_comment = true;
                i += 2;
                continue;
            }
            if c == '"' || c == '\'' {
                in_str = Some(c);
                i += 1;
                continue;
            }
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                // Already decreased once for a leading `}` before emit.
                if !(starts_with_close && i == 0) {
                    depth = depth.saturating_sub(1);
                }
            }
            i += 1;
        }
    }
    // Avoid adding a trailing newline if the input had none.
    if !body.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Replace try { /* monitor-enter(lock) */ body /* monitor-exit */ } catch (Throwable ...) with synchronized (lock) { body }.
/// Must be run after wrap_body_with_try_catch so the body actually contains the "try {" wrapper.
/// Replace try { /* monitor-enter(lock) */ body /* monitor-exit */ } catch (Throwable ...) with synchronized (lock) { body }.
/// Must be run after wrap_body_with_try_catch so the body actually contains the "try {" wrapper.
pub fn simplify_synchronized_blocks(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() < 4 {
        return body.to_string();
    }
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let stmt = line.trim();
        let indent = leading_indent(line);
        if stmt == "try {" {
            let mut j = i + 1;
            let mut lock: Option<String> = None;
            let mut monitor_enter_line = None;
            while j < lines.len() {
                let l = lines[j];
                let t = l.trim();
                if let Some(open) = t.find("/* monitor-enter(") {
                    let start = open + "/* monitor-enter(".len();
                    let end = t[start..].find(')').map(|p| start + p).unwrap_or(t.len());
                    lock = Some(t[start..end].trim().to_string());
                    monitor_enter_line = Some(j);
                    break;
                }
                if t.starts_with("} catch (Throwable") {
                    break;
                }
                j += 1;
            }
            if let (Some(lock_var), Some(mon_ln)) = (lock, monitor_enter_line) {
                let mut body_lines: Vec<&str> = Vec::new();
                let mut k = mon_ln + 1;
                while k < lines.len() {
                    let l = lines[k];
                    let t = l.trim();
                    if t.starts_with("} catch (Throwable") {
                        break;
                    }
                    if t.contains("/* monitor-exit(") {
                        k += 1;
                        continue;
                    }
                    body_lines.push(l);
                    k += 1;
                }
                let catch_start = k;
                if catch_start < lines.len()
                    && lines[catch_start].trim().starts_with("} catch (Throwable")
                {
                    let mut brace_count = 0;
                    let mut catch_end = catch_start;
                    for (idx, l) in lines[catch_start..].iter().enumerate() {
                        for c in l.chars() {
                            if c == '{' {
                                brace_count -= 1;
                            } else if c == '}' {
                                brace_count += 1;
                            }
                        }
                        catch_end = catch_start + idx;
                        if brace_count > 0 {
                            break;
                        }
                    }
                    writeln!(out, "{}synchronized ({}) {{", indent, lock_var).ok();
                    for bl in &body_lines {
                        out.push_str(bl);
                        out.push('\n');
                    }
                    writeln!(out, "{}}}", indent).ok();
                    i = catch_end + 1;
                    continue;
                }
            }
        }
        out.push_str(line);
        if i < lines.len().saturating_sub(1) {
            out.push('\n');
        }
        i += 1;
    }
    out
}
