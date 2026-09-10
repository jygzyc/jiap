//! Java-flavored source emitter (structural): declarations + annotated instruction flow.
//! Honest output: signatures are real Java; bodies are structured bytecode listings
//! (the DECX "IR text" mode), not dexdec-grade recovered source.

use crate::code::walk;
use crate::dex::{ClassDef, Dex};

pub fn desc_to_java(d: &str) -> String {
    let dims = d.matches('[').count();
    let base = d.trim_start_matches('[');
    let mut s = match base.as_bytes().first() {
        Some(b'V') => "void".to_string(),
        Some(b'Z') => "boolean".to_string(),
        Some(b'B') => "byte".to_string(),
        Some(b'S') => "short".to_string(),
        Some(b'C') => "char".to_string(),
        Some(b'I') => "int".to_string(),
        Some(b'J') => "long".to_string(),
        Some(b'F') => "float".to_string(),
        Some(b'D') => "double".to_string(),
        Some(b'L') => base
            .trim_start_matches('L')
            .trim_end_matches(';')
            .replace('/', "."),
        _ => base.to_string(),
    };
    for _ in 0..dims {
        s.push_str("[]");
    }
    s
}

fn class_flags(f: u32) -> String {
    let mut v = Vec::new();
    if f & 0x1 != 0 {
        v.push("public");
    }
    if f & 0x400 != 0 {
        v.push("abstract");
    }
    if f & 0x10 != 0 && f & 0x200 == 0 {
        v.push("final");
    }
    if f & 0x200 != 0 {
        v.push("interface");
    } else if f & 0x2000 != 0 {
        v.push("@interface");
    } else if f & 0x4000 != 0 {
        v.push("enum");
    }
    if f & 0x1000 != 0 {
        v.push("/*synthetic*/");
    }
    v.join(" ")
}

fn method_flags(f: u32) -> String {
    let mut v = Vec::new();
    if f & 0x1 != 0 {
        v.push("public");
    }
    if f & 0x2 != 0 {
        v.push("private");
    }
    if f & 0x4 != 0 {
        v.push("protected");
    }
    if f & 0x8 != 0 {
        v.push("static");
    }
    if f & 0x10 != 0 {
        v.push("final");
    }
    if f & 0x100 != 0 {
        v.push("native");
    }
    if f & 0x200 != 0 {
        v.push("abstract");
    }
    if f & 0x20 != 0 {
        v.push("synchronized");
    }
    v.join(" ")
}

fn field_flags(f: u32) -> String {
    let mut v = Vec::new();
    if f & 0x1 != 0 {
        v.push("public");
    }
    if f & 0x2 != 0 {
        v.push("private");
    }
    if f & 0x4 != 0 {
        v.push("protected");
    }
    if f & 0x8 != 0 {
        v.push("static");
    }
    if f & 0x10 != 0 {
        v.push("final");
    }
    if f & 0x40 != 0 {
        v.push("volatile");
    }
    if f & 0x80 != 0 {
        v.push("transient");
    }
    v.join(" ")
}

pub fn class_short(desc: &str) -> String {
    desc.trim_start_matches('L')
        .trim_end_matches(';')
        .rsplit('/')
        .next()
        .unwrap_or(desc)
        .to_string()
}

/// package part of a class descriptor
pub fn class_package(desc: &str) -> String {
    let full = desc.trim_start_matches('L').trim_end_matches(';');
    match full.rfind('/') {
        Some(i) => full[..i].replace('/', "."),
        None => String::new(),
    }
}

/// Emit one method body as annotated instruction flow.
pub fn emit_method_body(dex: &Dex, method_idx: u32, code_off: u32) -> Option<String> {
    let code = dex.code_item(code_off)?;
    let mut out = String::new();
    out.push_str(&format!(
        "    // registers: {} in: {} out: {}\n",
        code.registers_size, code.ins_size, code.outs_size
    ));
    for insn in walk(&code.insns) {
        let mut line = format!("    L{:04x}: {}", insn.pc * 2, insn.name);
        let mut ops: Vec<String> = insn.regs.iter().map(|r| format!("v{r}")).collect();
        if let Some(si) = insn.str_idx {
            let s = dex.str(si);
            let esc = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            ops.push(format!("string@{} \"{}\"", si, esc));
        }
        if let Some(ti) = insn.type_idx {
            ops.push(format!("type@{} {}", ti, dex.type_descriptor(ti)));
        }
        if let Some(fi) = insn.field_idx {
            ops.push(format!("field@{} {}", fi, dex.field_full(fi)));
        }
        if let Some(mi) = insn.method_idx {
            ops.push(format!("method@{} {}", mi, dex.method_full(mi)));
        }
        if insn.lit != 0 && insn.method_idx.is_none() && insn.field_idx.is_none() && insn.str_idx.is_none() {
            ops.push(format!("#{}", insn.lit));
        }
        if !ops.is_empty() {
            line.push(' ');
            line.push_str(&ops.join(", "));
        }
        out.push_str(&line);
        out.push('\n');
    }
    if code.tries_size > 0 {
        out.push_str(&format!(
            "    // {} try/catch region(s)\n",
            code.tries_size
        ));
    }
    Some(out)
}

/// Emit a full class file.
pub fn emit_class(dex: &Dex, def: &ClassDef, dex_name: &str) -> String {
    let desc = dex.type_descriptor(def.class_idx);
    let pkg = class_package(desc);
    let short = class_short(desc);
    let cd = dex.class_data(def);
    let mut out = String::new();
    out.push_str(&format!(
        "// decx-native structural source\n// dex: {dex_name}\n"
    ));
    if let Ok(sfi) = <u32 as TryInto<usize>>::try_into(def.source_file_idx) {
        if def.source_file_idx != 0xffff_ffff {
            let _ = sfi;
            out.push_str(&format!("// source: {}\n", dex.str(def.source_file_idx)));
        }
    }
    if !pkg.is_empty() {
        out.push_str(&format!("\npackage {pkg};\n"));
    }
    out.push('\n');
    let flags = class_flags(def.access_flags);
    let decl = if flags.is_empty() {
        format!("class {short}")
    } else {
        format!("{flags} class {short}")
    };
    let mut header = decl;
    if def.superclass_idx != 0xffff_ffff && def.superclass_idx != u32::MAX {
        let sup = dex.type_descriptor(def.superclass_idx);
        if sup != "Ljava/lang/Object;" {
            header.push_str(&format!(" extends {}", desc_to_java(sup)));
        }
    }
    let ifaces = dex.interfaces(def);
    if !ifaces.is_empty() {
        let list: Vec<String> = ifaces.iter().map(|i| desc_to_java(i)).collect();
        let kw = if def.access_flags & 0x200 != 0 { "extends" } else { "implements" };
        header.push_str(&format!(" {kw} {}", list.join(", ")));
    }
    out.push_str(&header);
    out.push_str(" {\n\n");
    // fields
    let mut emit_fields = |list: &[crate::dex::EncodedField], out: &mut String| {
        for f in list {
            if let Some(fid) = dex.field_ids.get(f.field_idx as usize) {
                let ftype = desc_to_java(dex.type_descriptor(fid.type_idx as u32));
                let fname = dex.str(fid.name_idx);
                let ff = field_flags(f.access_flags);
                if ff.is_empty() {
                    out.push_str(&format!("    {ftype} {fname};\n"));
                } else {
                    out.push_str(&format!("    {ff} {ftype} {fname};\n"));
                }
            }
        }
    };
    if !cd.static_fields.is_empty() {
        out.push_str("    // static fields\n");
        emit_fields(&cd.static_fields, &mut out);
        out.push('\n');
    }
    if !cd.instance_fields.is_empty() {
        out.push_str("    // instance fields\n");
        emit_fields(&cd.instance_fields, &mut out);
        out.push('\n');
    }
    // methods
    for section in [&cd.direct_methods, &cd.virtual_methods] {
        for m in section {
            let Some(mid_ref) = dex.method_ids.get(m.method_idx as usize) else {
                continue;
            };
            let ret = dex
                .proto_ids
                .get(mid_ref.proto_idx as usize)
                .map(|p| desc_to_java(dex.type_descriptor(p.return_type_idx)))
                .unwrap_or_else(|| "void".into());
            let name = dex.str(mid_ref.name_idx);
            let params = dex.proto_params(mid_ref.proto_idx);
            let plist: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{} p{}", desc_to_java(p), i))
                .collect();
            let mf = method_flags(m.access_flags);
            let sig = format!("{mf} {ret} {name}({})", plist.join(", "));
            let sig = sig.trim_start().to_string();
            out.push_str(&format!("    {sig} {{\n"));
            if m.code_off == 0 {
                out.push_str("        // abstract or native: no code\n");
            } else if let Some(body) = emit_method_body(dex, m.method_idx, m.code_off) {
                out.push_str(&body);
            } else {
                out.push_str("        // code item missing\n");
            }
            out.push_str("    }\n\n");
        }
    }
    out.push_str("}\n");
    out
}
