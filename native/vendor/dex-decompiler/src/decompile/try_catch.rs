//! Parse DEX try_item and encoded_catch_handler to emit try/catch blocks.
//! CodeItem in dex-parser does not expose tries/handlers; we parse from raw bytes.

use dex_parser::CodeItem;
use std::convert::TryInto;

/// One try range: [start_addr, start_addr + insn_count) in 16-bit code units.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TryItem {
    pub start_addr: u32,
    pub insn_count: u16,
    pub handler_off: u16,
}

/// One catch: exception type (type_idx) and handler bytecode address (16-bit units).
#[derive(Debug, Clone)]
pub struct EncodedTypeAddr {
    pub type_idx: u32,
    pub addr: u32,
}

/// One encoded_catch_handler: list of typed catches and optional catch-all.
#[derive(Debug, Clone)]
pub struct EncodedCatchHandler {
    pub handlers: Vec<EncodedTypeAddr>,
    pub catch_all_addr: Option<u32>,
}

fn read_uleb128(data: &[u8], pos: &mut usize) -> Option<u32> {
    let mut result: u32 = 0;
    let mut shift = 0;
    loop {
        if *pos >= data.len() {
            return None;
        }
        let b = data[*pos];
        *pos += 1;
        result |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            return None;
        }
    }
    Some(result)
}

fn read_sleb128(data: &[u8], pos: &mut usize) -> Option<i32> {
    let mut result: i32 = 0;
    let mut shift = 0;
    let mut size = 0;
    loop {
        if *pos >= data.len() {
            return None;
        }
        let b = data[*pos];
        *pos += 1;
        size += 1;
        result |= ((b & 0x7f) as i32) << shift;
        if b & 0x80 == 0 {
            if shift < 32 && (b & 0x40) != 0 {
                result |= !0 << (shift + 7);
            }
            break;
        }
        shift += 7;
        if size >= 5 {
            return None;
        }
    }
    Some(result)
}

/// Parse try_item array and encoded_catch_handler_list from code_item.
/// code_off is the file offset where the code_item starts.
pub fn parse_tries_and_handlers(
    data: &[u8],
    code_off: u32,
    code: &CodeItem,
) -> Option<(Vec<TryItem>, Vec<EncodedCatchHandler>)> {
    if code.tries_size == 0 {
        return Some((vec![], vec![]));
    }
    let code_off = code_off as usize;
    let insns_len = (code.insns_size as usize).saturating_mul(2);
    let mut tries_start = code_off + 16 + insns_len;
    if (code.insns_size as usize) % 2 == 1 {
        tries_start += 2;
    }
    if tries_start + (code.tries_size as usize) * 8 > data.len() {
        return None;
    }
    let mut try_items = Vec::with_capacity(code.tries_size as usize);
    for i in 0..(code.tries_size as usize) {
        let off = tries_start + i * 8;
        let start_addr = u32::from_le_bytes(data[off..off + 4].try_into().ok()?);
        let insn_count = u16::from_le_bytes(data[off + 4..off + 6].try_into().ok()?);
        let handler_off = u16::from_le_bytes(data[off + 6..off + 8].try_into().ok()?);
        try_items.push(TryItem {
            start_addr,
            insn_count,
            handler_off,
        });
    }
    let handlers_start = tries_start + (code.tries_size as usize) * 8;
    let mut pos = handlers_start;
    if pos >= data.len() {
        return Some((try_items, vec![]));
    }
    let list_size = read_uleb128(data, &mut pos)?;
    let mut handlers = Vec::with_capacity(list_size as usize);
    for _ in 0..list_size {
        let size_signed = read_sleb128(data, &mut pos)?;
        let size_abs = size_signed.unsigned_abs() as usize;
        let has_catch_all = size_signed <= 0;
        let mut type_addrs = Vec::with_capacity(size_abs);
        for _ in 0..size_abs {
            let type_idx = read_uleb128(data, &mut pos)?;
            let addr = read_uleb128(data, &mut pos)?;
            type_addrs.push(EncodedTypeAddr { type_idx, addr });
        }
        let catch_all_addr = if has_catch_all {
            Some(read_uleb128(data, &mut pos)?)
        } else {
            None
        };
        handlers.push(EncodedCatchHandler {
            handlers: type_addrs,
            catch_all_addr,
        });
    }
    Some((try_items, handlers))
}

/// Try and handler ranges in byte offsets (relative to start of insns, i.e. base_offset=0).
/// try_start_byte, try_end_byte: [try_start_byte, try_end_byte) is the try range.
/// handler_ranges: for each (type_idx, start_byte, end_byte), [start_byte, end_byte) is that handler's code.
///
/// Handler ends at the next handler / catch-all start. The last handler ends at
/// `post_handler_end` (continuation after all handlers), not necessarily `code_end`.
pub fn try_and_handler_byte_ranges(
    try_item: &TryItem,
    handler: &EncodedCatchHandler,
    insns_size_16bit: u32,
) -> (u32, u32, Vec<(u32, u32, u32)>) {
    try_and_handler_byte_ranges_with_end(try_item, handler, insns_size_16bit, None)
}

/// Like [`try_and_handler_byte_ranges`], but `post_handler_end` caps the last handler
/// (e.g. next try start or method continuation).
pub fn try_and_handler_byte_ranges_with_end(
    try_item: &TryItem,
    handler: &EncodedCatchHandler,
    insns_size_16bit: u32,
    post_handler_end: Option<u32>,
) -> (u32, u32, Vec<(u32, u32, u32)>) {
    let try_start_byte = try_item.start_addr * 2;
    let try_end_byte = (try_item.start_addr + try_item.insn_count as u32) * 2;
    let code_end_byte = insns_size_16bit * 2;
    let mut boundaries: Vec<u32> = handler.handlers.iter().map(|h| h.addr * 2).collect();
    if let Some(a) = handler.catch_all_addr {
        boundaries.push(a * 2);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let last_cap = post_handler_end.unwrap_or(code_end_byte).min(code_end_byte);
    // Prefer ending last handler before continuation: if handlers sit after the try,
    // use max(try_end, last handler start) stretch only to last_cap.
    boundaries.push(last_cap);
    let mut handler_ranges = Vec::new();
    for type_addr in &handler.handlers {
        let start_byte = type_addr.addr * 2;
        let end_byte = boundaries
            .iter()
            .find(|&&b| b > start_byte)
            .copied()
            .unwrap_or(last_cap);
        handler_ranges.push((type_addr.type_idx, start_byte, end_byte));
    }
    (try_start_byte, try_end_byte, handler_ranges)
}

/// Catch-all handler byte range `[start, end)`.
pub fn catch_all_byte_range(
    handler: &EncodedCatchHandler,
    typed_ranges: &[(u32, u32, u32)],
    insns_size_16bit: u32,
    post_handler_end: Option<u32>,
) -> Option<(u32, u32)> {
    let addr = handler.catch_all_addr?;
    let start = addr * 2;
    let code_end = insns_size_16bit * 2;
    let last_cap = post_handler_end.unwrap_or(code_end).min(code_end);
    let mut ends: Vec<u32> = typed_ranges.iter().map(|(_, s, _)| *s).collect();
    ends.push(last_cap);
    ends.sort_unstable();
    let end = ends.into_iter().find(|b| *b > start).unwrap_or(last_cap);
    Some((start, end))
}

/// True if catch-all body looks like a finally (cleanup + rethrow), not a real Throwable catch.
pub fn looks_like_finally(handler_java: &str) -> bool {
    let t = handler_java.to_ascii_lowercase();
    let trimmed = handler_java.trim();
    if trimmed.is_empty() {
        return true;
    }

    // Explicit exception handling → not finally.
    let handles = t.contains("addsuppressed")
        || t.contains("printstacktrace")
        || t.contains("log(")
        || t.contains("logger.")
        || t.contains("system.err")
        || t.contains("system.out")
        || t.contains("instanceof ")
        || (t.contains("return ") && (t.contains(" e") || t.contains("(e") || t.contains("= e")));
    if handles {
        return false;
    }

    let has_throw = t.contains("throw ") || t.contains("throw;");
    let uses_exception =
        t.contains(" e.") || t.contains("(e)") || t.contains(" e)") || t.contains("= e;");

    // Cleanup-only patterns even without rethrow, when exception local unused.
    let cleanup = t.contains(".close(")
        || t.contains(".recycle(")
        || t.contains(".unlock(")
        || t.contains(".disconnect(")
        || t.contains(".release(")
        || t.contains(".endtransaction(")
        || t.contains("monitor-exit")
        || t.contains("finally");

    if has_throw && !cleanup {
        return false;
    }
    if has_throw && cleanup {
        return true;
    }
    if cleanup && !uses_exception {
        return true;
    }
    // Classic heuristic: no meaningful use of the exception object.
    !uses_exception
}

/// First handler entry address in bytes, if any.
pub fn first_handler_start_byte(
    handler: &EncodedCatchHandler,
    handler_ranges: &[(u32, u32, u32)],
) -> Option<u32> {
    handler_ranges
        .iter()
        .map(|(_, start, _)| *start)
        .min()
        .or_else(|| handler.catch_all_addr.map(|a| a * 2))
}

/// When a try protects code with both `RuntimeException` and `Exception` handlers
/// (typical javac nested-try lowering), return `(runtime_handler_idx, exception_handler_idx)`.
pub fn nested_runtime_exception_handler_pair(type_names: &[String]) -> Option<(usize, usize)> {
    let re_idx = type_names.iter().position(|t| t == "RuntimeException")?;
    let ex_idx = type_names.iter().position(|t| t == "Exception")?;
    if re_idx == ex_idx {
        return None;
    }
    Some((re_idx, ex_idx))
}

/// D8/javac duplicate finally cleanup on the normal path inside the try range.
/// Strip those trailing statements so they appear only in the `finally` block.
pub fn peel_trailing_finally_from_try(try_body: &str, finally_cleanup: &str) -> String {
    let finally_lines: Vec<String> = finally_cleanup
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if finally_lines.is_empty() {
        return try_body.to_string();
    }
    let mut try_lines: Vec<String> = try_body.lines().map(String::from).collect();
    while try_lines.last().is_some_and(|l| l.trim().is_empty()) {
        try_lines.pop();
    }
    // D8 inlines finally cleanup on the normal path immediately before the try's return.
    let mut return_line: Option<String> = None;
    if try_lines
        .last()
        .is_some_and(|l| l.trim().starts_with("return "))
    {
        return_line = try_lines.pop();
        while try_lines.last().is_some_and(|l| l.trim().is_empty()) {
            try_lines.pop();
        }
    }
    let mut fi = finally_lines.len();
    let mut ti = try_lines.len();
    while fi > 0 && ti > 0 {
        let tt = try_lines[ti - 1].trim();
        if tt.is_empty() {
            ti -= 1;
            continue;
        }
        if tt == finally_lines[fi - 1] {
            fi -= 1;
            ti -= 1;
        } else {
            break;
        }
    }
    if fi != 0 {
        return try_body.to_string();
    }
    try_lines.truncate(ti);
    while try_lines.last().is_some_and(|l| l.trim().is_empty()) {
        try_lines.pop();
    }
    if let Some(ret) = return_line {
        try_lines.push(ret);
    }
    if try_lines.is_empty() {
        return String::new();
    }
    let mut out = try_lines.join("\n");
    if try_body.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// When d8 narrows the DEX try range, bytecode before `try {` may still belong in the try
/// body. Keep leading local declarations outside; move the rest into the try block.
pub fn split_pre_try_for_finally(pre_try: &str) -> (String, String) {
    let lines: Vec<&str> = pre_try.lines().collect();
    let mut outside: Vec<&str> = Vec::new();
    let mut inside: Vec<&str> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if is_local_declaration_line(t) && init_stays_outside_try(t) {
            outside.push(line);
        } else {
            inside.push(line);
        }
    }
    (join_nonempty_lines(&outside), join_nonempty_lines(&inside))
}

/// Pure literal / alloc inits may stay before `try`; compound inits belong inside.
fn init_stays_outside_try(line: &str) -> bool {
    let t = line.trim().trim_end_matches(';');
    let Some(eq_pos) = t.rfind(" = ") else {
        return false;
    };
    let rhs = t[eq_pos + 3..].trim();
    if rhs.starts_with("new ") {
        return true;
    }
    if rhs.contains('+')
        || rhs.contains('*')
        || rhs.contains('/')
        || rhs.contains('%')
        || rhs.contains('(')
        || rhs.contains('.')
    {
        return false;
    }
    rhs.parse::<i64>().is_ok()
        || matches!(rhs, "null" | "true" | "false")
        || (rhs.starts_with('"') && rhs.ends_with('"'))
        || (rhs.starts_with('\'') && rhs.ends_with('\''))
}

fn is_local_declaration_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("int ")
        || t.starts_with("long ")
        || t.starts_with("float ")
        || t.starts_with("double ")
        || t.starts_with("boolean ")
        || t.starts_with("byte ")
        || t.starts_with("short ")
        || t.starts_with("char ")
        || (t.starts_with("final ") && t.contains(" = "))
}

fn join_nonempty_lines(lines: &[&str]) -> String {
    let mut out = String::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        out.push_str(line);
        if !line.ends_with('\n') {
            out.push('\n');
        }
    }
    if !lines.is_empty() && lines.last().is_some_and(|l| l.ends_with('\n')) && !out.ends_with('\n')
    {
        out.push('\n');
    }
    out
}

/// Pairs of (try_item, its encoded_catch_handler) for emission.
pub fn try_handler_pairs(
    data: &[u8],
    code_off: u32,
    code: &CodeItem,
) -> Option<Vec<(TryItem, EncodedCatchHandler)>> {
    let (try_items, handlers) = parse_tries_and_handlers(data, code_off, code)?;
    let code_off = code_off as usize;
    let insns_len = (code.insns_size as usize).saturating_mul(2);
    let mut tries_end = code_off + 16 + insns_len;
    if (code.insns_size as usize) % 2 == 1 {
        tries_end += 2;
    }
    let list_start = tries_end + (code.tries_size as usize) * 8;
    let mut pos = list_start;
    if pos >= data.len() {
        return Some(vec![]);
    }
    let list_size = read_uleb128(data, &mut pos)? as usize;
    let mut handler_starts: Vec<usize> = Vec::with_capacity(list_size);
    for _ in 0..list_size {
        handler_starts.push(pos);
        let size_signed = read_sleb128(data, &mut pos)?;
        let size_abs = size_signed.unsigned_abs() as usize;
        for _ in 0..size_abs {
            read_uleb128(data, &mut pos)?;
            read_uleb128(data, &mut pos)?;
        }
        if size_signed <= 0 {
            read_uleb128(data, &mut pos)?;
        }
    }
    let mut out = Vec::new();
    for try_item in try_items {
        let off = list_start + (try_item.handler_off as usize);
        if let Some(idx) = handler_starts.iter().position(|&s| s == off) {
            if idx < handlers.len() {
                out.push((try_item, handlers[idx].clone()));
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{
        looks_like_finally, try_and_handler_byte_ranges, EncodedCatchHandler, EncodedTypeAddr,
        TryItem,
    };

    #[test]
    fn looks_like_finally_rethrow_with_cleanup() {
        assert!(looks_like_finally("    r.close();\n    throw e;\n"));
        assert!(!looks_like_finally("throw th;\n"));
        assert!(!looks_like_finally("    throw e;\n"));
    }

    #[test]
    fn looks_like_finally_cleanup_without_throw() {
        assert!(looks_like_finally("    r.close();\n"));
        assert!(looks_like_finally(
            "    if (r != null) {\n        r.close();\n    }\n"
        ));
        assert!(looks_like_finally("    lock.unlock();\n"));
        assert!(looks_like_finally(
            "    cursor.close();\n    db.endTransaction();\n"
        ));
    }

    #[test]
    fn looks_like_finally_not_when_handling() {
        assert!(!looks_like_finally("    e.printStackTrace();\n"));
        assert!(!looks_like_finally("    log(e);\n"));
        assert!(!looks_like_finally(
            "    if (e instanceof IOException) {\n        return;\n    }\n"
        ));
        assert!(!looks_like_finally("    return e.getMessage();\n"));
    }

    #[test]
    fn try_and_handler_byte_ranges_try_from_zero() {
        let try_item = TryItem {
            start_addr: 0,
            insn_count: 16,
            handler_off: 0,
        };
        let handler = EncodedCatchHandler {
            handlers: vec![EncodedTypeAddr {
                type_idx: 1,
                addr: 31,
            }],
            catch_all_addr: None,
        };
        let (try_start, try_end, handler_ranges) =
            try_and_handler_byte_ranges(&try_item, &handler, 64);
        assert_eq!(try_start, 0, "try starts at byte 0");
        assert_eq!(try_end, 32, "try ends at byte 32 (16 * 2)");
        assert_eq!(handler_ranges.len(), 1);
        assert_eq!(handler_ranges[0].0, 1);
        assert_eq!(handler_ranges[0].1, 62, "handler start 31 * 2 = 62");
        assert_eq!(handler_ranges[0].2, 128, "handler end = code_end 64*2");
    }

    #[test]
    fn looks_like_finally_not_addsuppressed_or_bare_rethrow() {
        assert!(!looks_like_finally("local0.addSuppressed(local1);\n"));
        assert!(!looks_like_finally(
            "local0.addSuppressed(local1);\n    throw local0;\n"
        ));
        assert!(!looks_like_finally("    throw e;\n"));
        assert!(looks_like_finally("    r.close();\n    throw e;\n"));
    }

    #[test]
    fn nested_try_fixture_dex_metadata() {
        use crate::parse_dex;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/decompiler_fixtures/classes.dex");
        let data = std::fs::read(&path).unwrap();
        let dex = parse_dex(&data).unwrap();
        let data = dex.data.as_ref();
        let mut found = false;
        for class_def in dex.class_defs().flatten() {
            let class_type = dex.get_type(class_def.class_idx).unwrap();
            let class_name = crate::java::descriptor_to_java(&class_type);
            if class_name != "com.androguard.decompilefixtures.TryCatchFixtures" {
                continue;
            }
            let cd = dex.get_class_data(&class_def).unwrap().unwrap();
            for enc in cd.direct_methods.iter().chain(cd.virtual_methods.iter()) {
                let info = dex.get_method_info(enc.method_idx).unwrap();
                if info.name != "nestedTry" {
                    continue;
                }
                found = true;
                let code = dex.get_code_item(enc.code_off).unwrap();
                assert!(
                    code.tries_size > 0,
                    "nestedTry fixture must ship with try metadata (use 100/x body so d8 keeps handlers)"
                );
                let pairs = super::try_handler_pairs(data, enc.code_off, &code).unwrap();
                assert!(!pairs.is_empty());
                let mut types = Vec::new();
                for (_, h) in &pairs {
                    for th in &h.handlers {
                        types.push(dex.get_type(th.type_idx).unwrap());
                    }
                }
                let all = types.join(",");
                assert!(
                    all.contains("RuntimeException") && all.contains("Exception"),
                    "{all}"
                );
            }
        }
        assert!(found, "nestedTry missing from fixtures dex");
    }

    #[test]
    fn try_finally_fixture_has_catch_all_in_dex() {
        use crate::parse_dex;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/decompiler_fixtures/classes.dex");
        let data = std::fs::read(&path).unwrap();
        let dex = parse_dex(&data).unwrap();
        let data = dex.data.as_ref();
        for class_def in dex.class_defs().flatten() {
            let class_type = dex.get_type(class_def.class_idx).unwrap();
            if crate::java::descriptor_to_java(&class_type)
                != "com.androguard.decompilefixtures.TryCatchFixtures"
            {
                continue;
            }
            let cd = dex.get_class_data(&class_def).unwrap().unwrap();
            for enc in cd.direct_methods.iter().chain(cd.virtual_methods.iter()) {
                let info = dex.get_method_info(enc.method_idx).unwrap();
                if info.name != "tryFinally" {
                    continue;
                }
                let code = dex.get_code_item(enc.code_off).unwrap();
                assert!(
                    code.tries_size > 0,
                    "tryFinally needs try metadata in fixture dex"
                );
                let pairs = super::try_handler_pairs(data, enc.code_off, &code).unwrap();
                let has_catch_all = pairs.iter().any(|(_, h)| h.catch_all_addr.is_some());
                assert!(
                    has_catch_all,
                    "tryFinally fixture should use catch-all finally handler"
                );
            }
        }
    }

    #[test]
    fn nested_runtime_exception_handler_pair_detects_fixture_shape() {
        let names = vec!["Exception".into(), "RuntimeException".into()];
        let (re, ex) = super::nested_runtime_exception_handler_pair(&names).unwrap();
        assert_eq!(re, 1);
        assert_eq!(ex, 0);
    }

    #[test]
    fn split_pre_try_for_finally_keeps_declarations_outside() {
        let pre = "        int acc = 0;\n        acc = acc + x;\n";
        let (outside, inside) = super::split_pre_try_for_finally(pre);
        assert_eq!(outside, "        int acc = 0;\n");
        assert_eq!(inside, "        acc = acc + x;\n");
    }

    #[test]
    fn split_pre_try_for_finally_moves_compound_init_inside() {
        let pre = "        int acc = 0 + x;\n";
        let (outside, inside) = super::split_pre_try_for_finally(pre);
        assert_eq!(outside, "");
        assert_eq!(inside, "        int acc = 0 + x;\n");
    }

    #[test]
    fn peel_trailing_finally_from_try_strips_inlined_cleanup() {
        let try_body = "acc = acc + x;\ntouchFinally();\nacc = acc + 1;\nreturn acc;\n";
        let finally = "acc = acc + 1;\n";
        let peeled = super::peel_trailing_finally_from_try(try_body, finally);
        assert_eq!(peeled, "acc = acc + x;\ntouchFinally();\nreturn acc;\n");
    }

    #[test]
    fn try_and_handler_byte_ranges_multiple_handlers() {
        let try_item = TryItem {
            start_addr: 0,
            insn_count: 10,
            handler_off: 0,
        };
        let handler = EncodedCatchHandler {
            handlers: vec![
                EncodedTypeAddr {
                    type_idx: 1,
                    addr: 20,
                },
                EncodedTypeAddr {
                    type_idx: 2,
                    addr: 25,
                },
            ],
            catch_all_addr: Some(30),
        };
        let (try_start, try_end, handler_ranges) =
            try_and_handler_byte_ranges(&try_item, &handler, 20);
        assert_eq!(try_start, 0);
        assert_eq!(try_end, 20);
        assert_eq!(handler_ranges.len(), 2);
        assert_eq!(
            handler_ranges[0],
            (1, 40, 50),
            "first handler 20..25 in bytes"
        );
        assert_eq!(
            handler_ranges[1],
            (2, 50, 60),
            "second handler 25..30 in bytes"
        );
    }
}
