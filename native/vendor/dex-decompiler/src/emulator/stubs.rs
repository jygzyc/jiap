//! API stubs for Java and Android standard library methods.
//!
//! Each stub receives the resolved method signature, the register arguments,
//! and mutable access to the emulator state. It returns an optional result
//! value (consumed by `move-result`) and a human-readable description.

use super::state::*;

/// Result of executing an API stub.
pub struct StubResult {
    pub result: Option<Value>,
    pub description: String,
    /// If set, append this line to console_output.
    pub console_line: Option<String>,
}

/// Try to handle an invoke as a known API stub.
/// `method_sig` is the resolved method reference (e.g. `java.io.PrintStream.println(java.lang.String)`).
/// `arg_regs` is the list of register numbers passed to the invoke.
/// Returns `Some(StubResult)` if handled, `None` if not a known stub.
pub fn try_stub(emu: &mut Emulator, method_sig: &str, arg_regs: &[u32]) -> Option<StubResult> {
    // Normalize: strip parameter types to get "Class.method" for matching.
    let paren_pos = method_sig.find('(');
    let base = if let Some(p) = paren_pos {
        &method_sig[..p]
    } else {
        method_sig
    };

    match base {
        // ---- java.io.PrintStream ----
        "java.io.PrintStream.println" | "java.io.PrintStream.print" => {
            Some(stub_print(emu, base, arg_regs))
        }

        // ---- java.lang.StringBuilder ----
        "java.lang.StringBuilder.<init>" => Some(stub_sb_init(emu, arg_regs)),
        "java.lang.StringBuilder.append" => Some(stub_sb_append(emu, arg_regs)),
        "java.lang.StringBuilder.toString" => Some(stub_sb_to_string(emu, arg_regs)),
        "java.lang.StringBuilder.length" => Some(stub_sb_length(emu, arg_regs)),

        // ---- java.lang.String ----
        "java.lang.String.length" => Some(stub_string_length(emu, arg_regs)),
        "java.lang.String.equals" => Some(stub_string_equals(emu, arg_regs)),
        "java.lang.String.charAt" => Some(stub_string_charat(emu, arg_regs)),
        "java.lang.String.substring" => Some(stub_string_substring(emu, arg_regs)),
        "java.lang.String.indexOf" => Some(stub_string_indexof(emu, arg_regs)),
        "java.lang.String.contains" => Some(stub_string_contains(emu, arg_regs)),
        "java.lang.String.isEmpty" => Some(stub_string_isempty(emu, arg_regs)),
        "java.lang.String.valueOf" => Some(stub_string_valueof(emu, arg_regs)),
        "java.lang.String.concat" => Some(stub_string_concat(emu, arg_regs)),
        "java.lang.String.startsWith" => Some(stub_string_startswith(emu, arg_regs)),
        "java.lang.String.endsWith" => Some(stub_string_endswith(emu, arg_regs)),
        "java.lang.String.trim" => Some(stub_string_trim(emu, arg_regs)),
        "java.lang.String.toUpperCase" => Some(stub_string_touppercase(emu, arg_regs)),
        "java.lang.String.toLowerCase" => Some(stub_string_tolowercase(emu, arg_regs)),
        "java.lang.String.replace" => Some(stub_string_replace(emu, arg_regs)),

        // ---- java.lang.Integer ----
        "java.lang.Integer.parseInt" => Some(stub_integer_parseint(emu, arg_regs)),
        "java.lang.Integer.valueOf" => Some(stub_integer_valueof(emu, arg_regs)),
        "java.lang.Integer.toString" => Some(stub_integer_tostring(emu, arg_regs)),
        "java.lang.Integer.intValue" => Some(stub_integer_intvalue(emu, arg_regs)),

        // ---- java.lang.Math ----
        "java.lang.Math.max" => Some(stub_math_max(emu, arg_regs)),
        "java.lang.Math.min" => Some(stub_math_min(emu, arg_regs)),
        "java.lang.Math.abs" => Some(stub_math_abs(emu, arg_regs)),

        // ---- java.lang.Object ----
        "java.lang.Object.<init>" => Some(StubResult {
            result: None,
            description: "Object.<init>()".into(),
            console_line: None,
        }),

        // ---- java.lang.System ----
        "java.lang.System.arraycopy" => Some(stub_arraycopy(emu, arg_regs)),
        "java.lang.System.currentTimeMillis" => Some(StubResult {
            result: Some(Value::Long(0)),
            description: "System.currentTimeMillis() → 0L (stubbed)".into(),
            console_line: None,
        }),

        // ---- java.util.Arrays ----
        "java.util.Arrays.fill" => Some(stub_arrays_fill(emu, arg_regs)),

        // ---- android.util.Log ----
        "android.util.Log.d" | "android.util.Log.i" | "android.util.Log.w"
        | "android.util.Log.e" | "android.util.Log.v" => {
            Some(stub_android_log(emu, base, arg_regs))
        }

        // ---- android.os.Bundle ----
        "android.os.Bundle.<init>" => Some(stub_bundle_init(emu, arg_regs)),
        "android.os.Bundle.putString" => Some(stub_bundle_put(emu, arg_regs, "putString")),
        "android.os.Bundle.putInt" => Some(stub_bundle_put(emu, arg_regs, "putInt")),
        "android.os.Bundle.getString" => Some(stub_bundle_get_string(emu, arg_regs)),
        "android.os.Bundle.getInt" => Some(stub_bundle_get_int(emu, arg_regs)),

        // ---- android.content.Intent ----
        "android.content.Intent.<init>" => Some(stub_intent_init(emu, arg_regs)),
        "android.content.Intent.putExtra" => Some(stub_intent_put_extra(emu, arg_regs)),
        "android.content.Intent.getStringExtra" => {
            Some(stub_intent_get_string_extra(emu, arg_regs))
        }

        _ => None,
    }
}

// ========== Helpers ==========

fn read_string(emu: &Emulator, reg: u32) -> String {
    match emu.get_reg(reg) {
        Ok(Value::Str(s)) => s.clone(),
        Ok(v) => v.display_short(),
        Err(_) => "?".into(),
    }
}

fn read_int(emu: &Emulator, reg: u32) -> i32 {
    emu.get_reg(reg).ok().and_then(|v| v.as_int()).unwrap_or(0)
}

fn get_sb_buffer(emu: &Emulator, heap_idx: usize) -> String {
    if let Some(obj) = emu.heap.get(heap_idx) {
        if let HeapObjectKind::Instance { ref fields, .. } = obj.kind {
            if let Some(Value::Str(s)) = fields.get("__buffer__") {
                return s.clone();
            }
        }
    }
    String::new()
}

fn set_sb_buffer(emu: &mut Emulator, heap_idx: usize, val: String) {
    if let Some(obj) = emu.heap.get_mut(heap_idx) {
        if let HeapObjectKind::Instance { ref mut fields, .. } = obj.kind {
            fields.insert("__buffer__".into(), Value::Str(val));
        }
    }
}

// ========== PrintStream ==========

fn stub_print(emu: &mut Emulator, base: &str, args: &[u32]) -> StubResult {
    let newline = base.ends_with("println");
    let text = if args.len() >= 2 {
        let val = emu.get_reg(args[1]).cloned().unwrap_or(Value::Null);
        match val {
            Value::Str(s) => s,
            other => other.display_short(),
        }
    } else {
        String::new()
    };
    let line = if newline {
        format!("{}\n", text)
    } else {
        text.clone()
    };
    StubResult {
        result: None,
        description: format!(
            "{}(\"{}\")",
            base.rsplit('.').next().unwrap_or("print"),
            text
        ),
        console_line: Some(line),
    }
}

// ========== StringBuilder ==========

fn stub_sb_init(emu: &mut Emulator, args: &[u32]) -> StubResult {
    if let Some(&reg) = args.first() {
        if let Ok(Value::Ref(idx)) = emu.get_reg(reg).cloned() {
            set_sb_buffer(emu, idx, String::new());
            return StubResult {
                result: None,
                description: "StringBuilder.<init>()".into(),
                console_line: None,
            };
        }
    }
    StubResult {
        result: None,
        description: "StringBuilder.<init>()".into(),
        console_line: None,
    }
}

fn stub_sb_append(emu: &mut Emulator, args: &[u32]) -> StubResult {
    if args.len() >= 2 {
        let sb_val = emu.get_reg(args[0]).cloned().unwrap_or(Value::Null);
        let append_val = emu.get_reg(args[1]).cloned().unwrap_or(Value::Null);
        let append_str = match &append_val {
            Value::Str(s) => s.clone(),
            other => other.display_short(),
        };
        if let Value::Ref(idx) = sb_val {
            let mut buf = get_sb_buffer(emu, idx);
            buf.push_str(&append_str);
            set_sb_buffer(emu, idx, buf);
            return StubResult {
                result: Some(Value::Ref(idx)),
                description: format!("StringBuilder.append(\"{}\")", append_str),
                console_line: None,
            };
        }
    }
    StubResult {
        result: None,
        description: "StringBuilder.append(?)".into(),
        console_line: None,
    }
}

fn stub_sb_to_string(emu: &Emulator, args: &[u32]) -> StubResult {
    if let Some(&reg) = args.first() {
        if let Ok(Value::Ref(idx)) = emu.get_reg(reg).cloned() {
            let buf = get_sb_buffer(emu, idx);
            return StubResult {
                result: Some(Value::Str(buf.clone())),
                description: format!("StringBuilder.toString() → \"{}\"", buf),
                console_line: None,
            };
        }
    }
    StubResult {
        result: Some(Value::Str(String::new())),
        description: "StringBuilder.toString()".into(),
        console_line: None,
    }
}

fn stub_sb_length(emu: &Emulator, args: &[u32]) -> StubResult {
    if let Some(&reg) = args.first() {
        if let Ok(Value::Ref(idx)) = emu.get_reg(reg).cloned() {
            let len = get_sb_buffer(emu, idx).len() as i32;
            return StubResult {
                result: Some(Value::Int(len)),
                description: format!("StringBuilder.length() → {}", len),
                console_line: None,
            };
        }
    }
    StubResult {
        result: Some(Value::Int(0)),
        description: "StringBuilder.length() → 0".into(),
        console_line: None,
    }
}

// ========== String ==========

fn stub_string_length(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if let Some(&r) = args.first() {
        read_string(emu, r)
    } else {
        String::new()
    };
    let len = s.len() as i32;
    StubResult {
        result: Some(Value::Int(len)),
        description: format!("String.length() → {}", len),
        console_line: None,
    }
}

fn stub_string_equals(emu: &Emulator, args: &[u32]) -> StubResult {
    let a = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let b = if args.len() >= 2 {
        read_string(emu, args[1])
    } else {
        String::new()
    };
    let eq = a == b;
    StubResult {
        result: Some(Value::Int(if eq { 1 } else { 0 })),
        description: format!("String.equals({}, {}) → {}", a, b, eq),
        console_line: None,
    }
}

fn stub_string_charat(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let idx = if args.len() >= 2 {
        read_int(emu, args[1])
    } else {
        0
    };
    let ch = s.chars().nth(idx as usize).map(|c| c as i32).unwrap_or(0);
    StubResult {
        result: Some(Value::Int(ch)),
        description: format!("String.charAt({}) → '{}'", idx, ch as u8 as char),
        console_line: None,
    }
}

fn stub_string_substring(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let begin = if args.len() >= 2 {
        read_int(emu, args[1]) as usize
    } else {
        0
    };
    let end = if args.len() >= 3 {
        read_int(emu, args[2]) as usize
    } else {
        s.len()
    };
    let sub: String = s
        .chars()
        .skip(begin)
        .take(end.saturating_sub(begin))
        .collect();
    StubResult {
        result: Some(Value::Str(sub.clone())),
        description: format!("String.substring({},{}) → \"{}\"", begin, end, sub),
        console_line: None,
    }
}

fn stub_string_indexof(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let needle = if args.len() >= 2 {
        read_string(emu, args[1])
    } else {
        String::new()
    };
    let idx = s.find(&needle).map(|i| i as i32).unwrap_or(-1);
    StubResult {
        result: Some(Value::Int(idx)),
        description: format!("String.indexOf(\"{}\") → {}", needle, idx),
        console_line: None,
    }
}

fn stub_string_contains(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let needle = if args.len() >= 2 {
        read_string(emu, args[1])
    } else {
        String::new()
    };
    let c = s.contains(&needle);
    StubResult {
        result: Some(Value::Int(if c { 1 } else { 0 })),
        description: format!("String.contains(\"{}\") → {}", needle, c),
        console_line: None,
    }
}

fn stub_string_isempty(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let empty = s.is_empty();
    StubResult {
        result: Some(Value::Int(if empty { 1 } else { 0 })),
        description: format!("String.isEmpty() → {}", empty),
        console_line: None,
    }
}

fn stub_string_valueof(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        "null".into()
    };
    StubResult {
        result: Some(Value::Str(s.clone())),
        description: format!("String.valueOf() → \"{}\"", s),
        console_line: None,
    }
}

fn stub_string_concat(emu: &Emulator, args: &[u32]) -> StubResult {
    let a = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let b = if args.len() >= 2 {
        read_string(emu, args[1])
    } else {
        String::new()
    };
    let r = format!("{}{}", a, b);
    StubResult {
        result: Some(Value::Str(r.clone())),
        description: format!("String.concat() → \"{}\"", r),
        console_line: None,
    }
}

fn stub_string_startswith(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let pfx = if args.len() >= 2 {
        read_string(emu, args[1])
    } else {
        String::new()
    };
    let r = s.starts_with(&pfx);
    StubResult {
        result: Some(Value::Int(if r { 1 } else { 0 })),
        description: format!("String.startsWith(\"{}\") → {}", pfx, r),
        console_line: None,
    }
}

fn stub_string_endswith(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let sfx = if args.len() >= 2 {
        read_string(emu, args[1])
    } else {
        String::new()
    };
    let r = s.ends_with(&sfx);
    StubResult {
        result: Some(Value::Int(if r { 1 } else { 0 })),
        description: format!("String.endsWith(\"{}\") → {}", sfx, r),
        console_line: None,
    }
}

fn stub_string_trim(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let r = s.trim().to_string();
    StubResult {
        result: Some(Value::Str(r.clone())),
        description: format!("String.trim() → \"{}\"", r),
        console_line: None,
    }
}

fn stub_string_touppercase(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let r = s.to_uppercase();
    StubResult {
        result: Some(Value::Str(r.clone())),
        description: format!("String.toUpperCase() → \"{}\"", r),
        console_line: None,
    }
}

fn stub_string_tolowercase(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let r = s.to_lowercase();
    StubResult {
        result: Some(Value::Str(r.clone())),
        description: format!("String.toLowerCase() → \"{}\"", r),
        console_line: None,
    }
}

fn stub_string_replace(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        String::new()
    };
    let old = if args.len() >= 2 {
        read_string(emu, args[1])
    } else {
        String::new()
    };
    let new = if args.len() >= 3 {
        read_string(emu, args[2])
    } else {
        String::new()
    };
    let r = s.replace(&old, &new);
    StubResult {
        result: Some(Value::Str(r.clone())),
        description: format!("String.replace(\"{}\",\"{}\") → \"{}\"", old, new, r),
        console_line: None,
    }
}

// ========== Integer ==========

fn stub_integer_parseint(emu: &Emulator, args: &[u32]) -> StubResult {
    let s = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        "0".into()
    };
    let v = s.parse::<i32>().unwrap_or(0);
    StubResult {
        result: Some(Value::Int(v)),
        description: format!("Integer.parseInt(\"{}\") → {}", s, v),
        console_line: None,
    }
}

fn stub_integer_valueof(emu: &Emulator, args: &[u32]) -> StubResult {
    let v = if args.len() >= 1 {
        read_int(emu, args[0])
    } else {
        0
    };
    StubResult {
        result: Some(Value::Int(v)),
        description: format!("Integer.valueOf({}) → {}", v, v),
        console_line: None,
    }
}

fn stub_integer_tostring(emu: &Emulator, args: &[u32]) -> StubResult {
    let v = if args.len() >= 1 {
        read_int(emu, args[0])
    } else {
        0
    };
    let s = v.to_string();
    StubResult {
        result: Some(Value::Str(s.clone())),
        description: format!("Integer.toString({}) → \"{}\"", v, s),
        console_line: None,
    }
}

fn stub_integer_intvalue(emu: &Emulator, args: &[u32]) -> StubResult {
    let v = if args.len() >= 1 {
        read_int(emu, args[0])
    } else {
        0
    };
    StubResult {
        result: Some(Value::Int(v)),
        description: format!("Integer.intValue() → {}", v),
        console_line: None,
    }
}

// ========== Math ==========

fn stub_math_max(emu: &Emulator, args: &[u32]) -> StubResult {
    let a = if args.len() >= 1 {
        read_int(emu, args[0])
    } else {
        0
    };
    let b = if args.len() >= 2 {
        read_int(emu, args[1])
    } else {
        0
    };
    let r = a.max(b);
    StubResult {
        result: Some(Value::Int(r)),
        description: format!("Math.max({}, {}) → {}", a, b, r),
        console_line: None,
    }
}

fn stub_math_min(emu: &Emulator, args: &[u32]) -> StubResult {
    let a = if args.len() >= 1 {
        read_int(emu, args[0])
    } else {
        0
    };
    let b = if args.len() >= 2 {
        read_int(emu, args[1])
    } else {
        0
    };
    let r = a.min(b);
    StubResult {
        result: Some(Value::Int(r)),
        description: format!("Math.min({}, {}) → {}", a, b, r),
        console_line: None,
    }
}

fn stub_math_abs(emu: &Emulator, args: &[u32]) -> StubResult {
    let a = if args.len() >= 1 {
        read_int(emu, args[0])
    } else {
        0
    };
    let r = a.abs();
    StubResult {
        result: Some(Value::Int(r)),
        description: format!("Math.abs({}) → {}", a, r),
        console_line: None,
    }
}

// ========== System.arraycopy ==========

fn stub_arraycopy(emu: &mut Emulator, args: &[u32]) -> StubResult {
    // arraycopy(src, srcPos, dst, dstPos, length) — 5 args, all registers
    if args.len() < 5 {
        return StubResult {
            result: None,
            description: "System.arraycopy (insufficient args)".into(),
            console_line: None,
        };
    }
    let src_val = emu.get_reg(args[0]).cloned().unwrap_or(Value::Null);
    let src_pos = read_int(emu, args[1]) as usize;
    let dst_val = emu.get_reg(args[2]).cloned().unwrap_or(Value::Null);
    let dst_pos = read_int(emu, args[3]) as usize;
    let length = read_int(emu, args[4]) as usize;

    if let (Value::Ref(src_idx), Value::Ref(dst_idx)) = (&src_val, &dst_val) {
        let copied: Vec<Value> = if let Some(obj) = emu.heap.get(*src_idx) {
            if let HeapObjectKind::Array { ref values, .. } = obj.kind {
                values.iter().skip(src_pos).take(length).cloned().collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        if let Some(obj) = emu.heap.get_mut(*dst_idx) {
            if let HeapObjectKind::Array { ref mut values, .. } = obj.kind {
                for (i, v) in copied.into_iter().enumerate() {
                    if dst_pos + i < values.len() {
                        values[dst_pos + i] = v;
                    }
                }
            }
        }
    }
    StubResult {
        result: None,
        description: format!("System.arraycopy(len={})", length),
        console_line: None,
    }
}

// ========== Arrays.fill ==========

fn stub_arrays_fill(emu: &mut Emulator, args: &[u32]) -> StubResult {
    if args.len() >= 2 {
        let arr_val = emu.get_reg(args[0]).cloned().unwrap_or(Value::Null);
        let fill_val = emu.get_reg(args[1]).cloned().unwrap_or(Value::Int(0));
        if let Value::Ref(idx) = arr_val {
            if let Some(obj) = emu.heap.get_mut(idx) {
                if let HeapObjectKind::Array { ref mut values, .. } = obj.kind {
                    for v in values.iter_mut() {
                        *v = fill_val.clone();
                    }
                }
            }
        }
    }
    StubResult {
        result: None,
        description: "Arrays.fill()".into(),
        console_line: None,
    }
}

// ========== android.util.Log ==========

fn stub_android_log(emu: &mut Emulator, base: &str, args: &[u32]) -> StubResult {
    let level = base.rsplit('.').next().unwrap_or("d");
    let tag = if args.len() >= 1 {
        read_string(emu, args[0])
    } else {
        "?".into()
    };
    let msg = if args.len() >= 2 {
        read_string(emu, args[1])
    } else {
        "".into()
    };
    let line = format!("[Log.{}] {}: {}", level, tag, msg);
    StubResult {
        result: Some(Value::Int(0)),
        description: line.clone(),
        console_line: Some(line),
    }
}

// ========== android.os.Bundle ==========

fn stub_bundle_init(emu: &mut Emulator, args: &[u32]) -> StubResult {
    if let Some(&reg) = args.first() {
        if let Ok(Value::Ref(idx)) = emu.get_reg(reg).cloned() {
            if let Some(obj) = emu.heap.get_mut(idx) {
                if let HeapObjectKind::Instance { ref mut fields, .. } = obj.kind {
                    fields.clear();
                }
            }
        }
    }
    StubResult {
        result: None,
        description: "Bundle.<init>()".into(),
        console_line: None,
    }
}

fn stub_bundle_put(emu: &mut Emulator, args: &[u32], op: &str) -> StubResult {
    if args.len() >= 3 {
        let bundle_val = emu.get_reg(args[0]).cloned().unwrap_or(Value::Null);
        let key = read_string(emu, args[1]);
        let val = emu.get_reg(args[2]).cloned().unwrap_or(Value::Null);
        if let Value::Ref(idx) = bundle_val {
            if let Some(obj) = emu.heap.get_mut(idx) {
                if let HeapObjectKind::Instance { ref mut fields, .. } = obj.kind {
                    fields.insert(key.clone(), val.clone());
                }
            }
        }
        return StubResult {
            result: None,
            description: format!("Bundle.{}(\"{}\", {})", op, key, val.display_short()),
            console_line: None,
        };
    }
    StubResult {
        result: None,
        description: format!("Bundle.{}(?)", op),
        console_line: None,
    }
}

fn stub_bundle_get_string(emu: &Emulator, args: &[u32]) -> StubResult {
    if args.len() >= 2 {
        let bundle_val = emu.get_reg(args[0]).cloned().unwrap_or(Value::Null);
        let key = read_string(emu, args[1]);
        if let Value::Ref(idx) = bundle_val {
            if let Some(obj) = emu.heap.get(idx) {
                if let HeapObjectKind::Instance { ref fields, .. } = obj.kind {
                    if let Some(val) = fields.get(&key) {
                        return StubResult {
                            result: Some(val.clone()),
                            description: format!(
                                "Bundle.getString(\"{}\") → {}",
                                key,
                                val.display_short()
                            ),
                            console_line: None,
                        };
                    }
                }
            }
        }
        return StubResult {
            result: Some(Value::Null),
            description: format!("Bundle.getString(\"{}\") → null", key),
            console_line: None,
        };
    }
    StubResult {
        result: Some(Value::Null),
        description: "Bundle.getString(?)".into(),
        console_line: None,
    }
}

fn stub_bundle_get_int(emu: &Emulator, args: &[u32]) -> StubResult {
    if args.len() >= 2 {
        let bundle_val = emu.get_reg(args[0]).cloned().unwrap_or(Value::Null);
        let key = read_string(emu, args[1]);
        if let Value::Ref(idx) = bundle_val {
            if let Some(obj) = emu.heap.get(idx) {
                if let HeapObjectKind::Instance { ref fields, .. } = obj.kind {
                    if let Some(val) = fields.get(&key) {
                        let v = val.as_int().unwrap_or(0);
                        return StubResult {
                            result: Some(Value::Int(v)),
                            description: format!("Bundle.getInt(\"{}\") → {}", key, v),
                            console_line: None,
                        };
                    }
                }
            }
        }
        return StubResult {
            result: Some(Value::Int(0)),
            description: format!("Bundle.getInt(\"{}\") → 0", key),
            console_line: None,
        };
    }
    StubResult {
        result: Some(Value::Int(0)),
        description: "Bundle.getInt(?)".into(),
        console_line: None,
    }
}

// ========== android.content.Intent ==========

fn stub_intent_init(emu: &mut Emulator, args: &[u32]) -> StubResult {
    if let Some(&reg) = args.first() {
        if let Ok(Value::Ref(idx)) = emu.get_reg(reg).cloned() {
            if let Some(obj) = emu.heap.get_mut(idx) {
                if let HeapObjectKind::Instance { ref mut fields, .. } = obj.kind {
                    fields.clear();
                }
            }
        }
    }
    StubResult {
        result: None,
        description: "Intent.<init>()".into(),
        console_line: None,
    }
}

fn stub_intent_put_extra(emu: &mut Emulator, args: &[u32]) -> StubResult {
    if args.len() >= 3 {
        let intent_val = emu.get_reg(args[0]).cloned().unwrap_or(Value::Null);
        let key = read_string(emu, args[1]);
        let val = emu.get_reg(args[2]).cloned().unwrap_or(Value::Null);
        if let Value::Ref(idx) = &intent_val {
            if let Some(obj) = emu.heap.get_mut(*idx) {
                if let HeapObjectKind::Instance { ref mut fields, .. } = obj.kind {
                    fields.insert(key.clone(), val.clone());
                }
            }
        }
        return StubResult {
            result: Some(intent_val),
            description: format!("Intent.putExtra(\"{}\", {})", key, val.display_short()),
            console_line: None,
        };
    }
    StubResult {
        result: None,
        description: "Intent.putExtra(?)".into(),
        console_line: None,
    }
}

fn stub_intent_get_string_extra(emu: &Emulator, args: &[u32]) -> StubResult {
    if args.len() >= 2 {
        let intent_val = emu.get_reg(args[0]).cloned().unwrap_or(Value::Null);
        let key = read_string(emu, args[1]);
        if let Value::Ref(idx) = intent_val {
            if let Some(obj) = emu.heap.get(idx) {
                if let HeapObjectKind::Instance { ref fields, .. } = obj.kind {
                    if let Some(val) = fields.get(&key) {
                        return StubResult {
                            result: Some(val.clone()),
                            description: format!(
                                "Intent.getStringExtra(\"{}\") → {}",
                                key,
                                val.display_short()
                            ),
                            console_line: None,
                        };
                    }
                }
            }
        }
        return StubResult {
            result: Some(Value::Null),
            description: format!("Intent.getStringExtra(\"{}\") → null", key),
            console_line: None,
        };
    }
    StubResult {
        result: Some(Value::Null),
        description: "Intent.getStringExtra(?)".into(),
        console_line: None,
    }
}

/// List all stubbed method prefixes for documentation / UI.
pub fn list_stubs() -> Vec<&'static str> {
    vec![
        "java.io.PrintStream.println",
        "java.io.PrintStream.print",
        "java.lang.StringBuilder.<init>",
        "java.lang.StringBuilder.append",
        "java.lang.StringBuilder.toString",
        "java.lang.StringBuilder.length",
        "java.lang.String.length",
        "java.lang.String.equals",
        "java.lang.String.charAt",
        "java.lang.String.substring",
        "java.lang.String.indexOf",
        "java.lang.String.contains",
        "java.lang.String.isEmpty",
        "java.lang.String.valueOf",
        "java.lang.String.concat",
        "java.lang.String.startsWith",
        "java.lang.String.endsWith",
        "java.lang.String.trim",
        "java.lang.String.toUpperCase",
        "java.lang.String.toLowerCase",
        "java.lang.String.replace",
        "java.lang.Integer.parseInt",
        "java.lang.Integer.valueOf",
        "java.lang.Integer.toString",
        "java.lang.Integer.intValue",
        "java.lang.Math.max",
        "java.lang.Math.min",
        "java.lang.Math.abs",
        "java.lang.Object.<init>",
        "java.lang.System.arraycopy",
        "java.lang.System.currentTimeMillis",
        "java.util.Arrays.fill",
        "android.util.Log.d/i/w/e/v",
        "android.os.Bundle.<init>",
        "android.os.Bundle.putString",
        "android.os.Bundle.putInt",
        "android.os.Bundle.getString",
        "android.os.Bundle.getInt",
        "android.content.Intent.<init>",
        "android.content.Intent.putExtra",
        "android.content.Intent.getStringExtra",
    ]
}
