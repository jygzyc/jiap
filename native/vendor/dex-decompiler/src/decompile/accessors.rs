//! Inline / hide R8 synthetic `access$*` accessors and bridge methods.

use dex_bytecode::decode_all;
use dex_parser::{DexFile, EncodedMethod};
use std::collections::HashMap;

use crate::java;

use super::const_fields::apply_const_field_replacements;

/// True if this method should be omitted from Java class dumps (bridge / access$ / R8 lambda shims).
pub fn should_skip_method_emit(name: &str, access_flags: u32) -> bool {
    if java::is_bridge_method(access_flags) {
        return true;
    }
    if name.starts_with("access$") && java::is_synthetic(access_flags) {
        return true;
    }
    // R8 lowers method references to package-private `$r8$lambda$…` static shims.
    if name.starts_with("$r8$lambda$") {
        return true;
    }
    false
}

/// Build replacements: `Outer.access$123(` / `access$123(` → field/method expression.
pub fn build_accessor_replacements(
    dex: &DexFile,
    class_name: &str,
    methods: &[&EncodedMethod],
) -> Vec<(String, String)> {
    let simple = class_name.rsplit('.').next().unwrap_or(class_name);
    let mut reps = Vec::new();
    for enc in methods {
        let Ok(info) = dex.get_method_info(enc.method_idx) else {
            continue;
        };
        if !info.name.starts_with("access$") {
            continue;
        }
        if enc.code_off == 0 {
            continue;
        }
        let Ok(code) = dex.get_code_item(enc.code_off) else {
            continue;
        };
        let insns = code.insns_slice(&dex.data);
        let Ok(decoded) = decode_all(insns, 0) else {
            continue;
        };
        let Some(expr) = infer_accessor_expr(dex, &decoded) else {
            continue;
        };
        // Call forms commonly seen in decompiled source.
        let keys = [
            format!("{}.{}(", simple, info.name),
            format!("{}(", info.name),
            format!("{}.{}(", class_name, info.name),
        ];
        for k in keys {
            reps.push((k, format!("{}(", expr)));
        }
    }
    reps
}

pub fn apply_accessor_replacements(body: &str, reps: &[(String, String)]) -> String {
    if reps.is_empty() {
        return body.to_string();
    }
    apply_const_field_replacements(body, reps)
}

/// Collect accessor maps for a whole DEX (class FQN → replacements), for multi-pass use.
pub fn collect_dex_accessor_map(dex: &DexFile) -> HashMap<String, Vec<(String, String)>> {
    let mut out = HashMap::new();
    for class_result in dex.class_defs() {
        let Ok(class_def) = class_result else {
            continue;
        };
        let Ok(class_type) = dex.get_type(class_def.class_idx) else {
            continue;
        };
        let class_name = java::descriptor_to_java(&class_type);
        let Ok(Some(cd)) = dex.get_class_data(&class_def) else {
            continue;
        };
        let methods: Vec<&EncodedMethod> = cd
            .direct_methods
            .iter()
            .chain(cd.virtual_methods.iter())
            .collect();
        let reps = build_accessor_replacements(dex, &class_name, &methods);
        if !reps.is_empty() {
            out.insert(class_name, reps);
        }
    }
    out
}

fn infer_accessor_expr(dex: &DexFile, decoded: &[dex_bytecode::Instruction]) -> Option<String> {
    // Typical: (optional check-cast) + iget/sget/iput/sput/invoke + return
    for ins in decoded {
        let m = ins.mnemonic;
        if matches!(
            m,
            "iget"
                | "iget-object"
                | "iget-boolean"
                | "iget-byte"
                | "iget-char"
                | "iget-short"
                | "iget-wide"
        ) {
            return field_expr_from_ops(dex, &ins.operands, false);
        }
        if matches!(
            m,
            "sget"
                | "sget-object"
                | "sget-boolean"
                | "sget-byte"
                | "sget-char"
                | "sget-short"
                | "sget-wide"
        ) {
            return field_expr_from_ops(dex, &ins.operands, true);
        }
        if matches!(
            m,
            "iput"
                | "iput-object"
                | "iput-boolean"
                | "iput-byte"
                | "iput-char"
                | "iput-short"
                | "iput-wide"
        ) {
            // Setter: still expose field name; call sites are messier — use field for get-style rewrite.
            return field_expr_from_ops(dex, &ins.operands, false);
        }
        if matches!(
            m,
            "sput"
                | "sput-object"
                | "sput-boolean"
                | "sput-byte"
                | "sput-char"
                | "sput-short"
                | "sput-wide"
        ) {
            return field_expr_from_ops(dex, &ins.operands, true);
        }
        if m.starts_with("invoke-") {
            return method_expr_from_ops(dex, &ins.operands);
        }
    }
    None
}

fn field_expr_from_ops(dex: &DexFile, ops: &str, is_static: bool) -> Option<String> {
    // operands often: "v0, v1, Field@N" or resolved "v0, Lcom/Foo;->bar:I"
    let field_tok = ops.split(',').map(str::trim).last()?;
    if let Some(idx) = field_tok
        .strip_prefix("Field@")
        .or_else(|| field_tok.strip_prefix("field@"))
        .and_then(|s| s.parse::<u32>().ok())
    {
        let fi = dex.get_field_info(idx).ok()?;
        let class = java::descriptor_to_java(&fi.class);
        let simple = class.rsplit('.').next().unwrap_or(&class);
        if is_static {
            return Some(format!("{}.{}", simple, fi.name));
        }
        // Instance: receiver is first arg of access$ — keep as `.field` suffix on rewritten call.
        // access$0(obj) → obj.field  requires replacing `access$0(x)` with `x.field` (no paren).
        return Some(format!("/*field:{}.{}*/", simple, fi.name));
    }
    // Already resolved like Lcom/Foo;->name:I
    if let Some((cls, rest)) = field_tok.split_once("->") {
        let name = rest.split(':').next()?.trim();
        let class = java::descriptor_to_java(cls.trim());
        let simple = class.rsplit('.').next().unwrap_or(&class);
        if is_static {
            return Some(format!("{}.{}", simple, name));
        }
        return Some(format!("/*field:{}.{}*/", simple, name));
    }
    None
}

fn method_expr_from_ops(dex: &DexFile, ops: &str) -> Option<String> {
    let method_tok = ops.split(',').map(str::trim).last()?;
    if let Some(idx) = method_tok
        .strip_prefix("Method@")
        .or_else(|| method_tok.strip_prefix("method@"))
        .and_then(|s| s.parse::<u32>().ok())
    {
        let mi = dex.get_method_info(idx).ok()?;
        let class = java::descriptor_to_java(&mi.class);
        let simple = class.rsplit('.').next().unwrap_or(&class);
        return Some(format!("{}.{}", simple, mi.name));
    }
    if let Some((cls, rest)) = method_tok.split_once("->") {
        let name = rest.split('(').next()?.trim();
        let class = java::descriptor_to_java(cls.trim());
        let simple = class.rsplit('.').next().unwrap_or(&class);
        return Some(format!("{}.{}", simple, name));
    }
    None
}

/// Post-process: `access$0(obj)` where we stored `/*field:Cls.name*/(` → `obj.name`.
pub fn polish_field_accessor_calls(body: &str) -> String {
    // Replace pattern: /*field:Simple.name*/(expr) → expr.name
    let mut out = body.to_string();
    loop {
        let Some(start) = out.find("/*field:") else {
            break;
        };
        let after = start + "/*field:".len();
        let Some(end_mark) = out[after..].find("*/(") else {
            break;
        };
        let field_path = &out[after..after + end_mark];
        let field_name = field_path.rsplit('.').next().unwrap_or(field_path);
        let call_start = after + end_mark + "*/(".len();
        // Find matching close paren for the argument list.
        let mut depth = 1i32;
        let mut i = call_start;
        let bytes = out.as_bytes();
        while i < bytes.len() {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if depth != 0 || i >= bytes.len() {
            // abort this occurrence
            out.replace_range(start..start + 1, " ");
            continue;
        }
        let args = out[call_start..i].trim();
        let repl = if args.is_empty() {
            field_name.to_string()
        } else if !args.contains(',') {
            format!("{}.{}", args, field_name)
        } else {
            // multi-arg setter-ish: leave first arg.field
            let first = args.split(',').next().unwrap_or(args).trim();
            format!("{}.{}", first, field_name)
        };
        out.replace_range(start..=i, &repl);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_bridge_and_access() {
        assert!(should_skip_method_emit("foo", 0x40));
        assert!(should_skip_method_emit("access$000", 0x1000));
        assert!(!should_skip_method_emit("foo", 0x1));
    }

    #[test]
    fn skips_r8_lambda_shims() {
        assert!(should_skip_method_emit(
            "$r8$lambda$3Y7zL6aAKGHmmofj-9_cxS0RPF4",
            0x1009
        ));
        assert!(!should_skip_method_emit("lambda$identityLambda$0", 0x1008));
    }

    #[test]
    fn polish_field_call() {
        let s = "int x = /*field:Outer.bar*/(obj);";
        let out = polish_field_accessor_calls(s);
        assert_eq!(out, "int x = obj.bar;");
    }
}
