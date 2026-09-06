//! Standard-Java archive support: jars of JVM `.class` files (library jars,
//! `android.jar`). JVM bytecode bodies are not decompiled — classes are
//! presented as javap-style skeletons (hierarchy, signatures, constants),
//! which covers API-surface analysis; `android.jar` stub bodies carry no
//! implementation anyway. Decoding reuses the vendored dexdec class-file
//! parser; member descriptors share the JVM syntax used by DEX, so the same
//! `names` helpers render them.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use dexdec::platform_symbols::{
    ClassFileDecoder, PlatformClass, PlatformConstant, PlatformField, PlatformMethod,
};

use crate::error::{DecxError, Result};
use crate::names::{descriptor_to_java, split_method_descriptor};

/// A parsed standard-Java archive, keyed by java class name.
pub struct JavaArchive {
    classes: BTreeMap<String, Arc<PlatformClass>>,
    order: Vec<String>,
}

impl JavaArchive {
    /// Read every `*.class` entry of a jar/zip file.
    pub fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)
            .map_err(|e| DecxError::invalid_parameter(format!("open {}: {e}", path.display())))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|_| DecxError::invalid_parameter("not a zip/jar archive"))?;
        let mut classes = BTreeMap::new();
        let mut order = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| DecxError::internal(e.to_string()))?;
            if !entry.name().ends_with(".class") || entry.name().ends_with("module-info.class") {
                continue;
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut bytes)
                .map_err(|e| DecxError::internal(format!("read {}: {e}", entry.name())))?;
            let class = ClassFileDecoder::decode(&bytes)
                .map_err(|e| DecxError::invalid_parameter(format!("decode {}: {e}", entry.name())))?;
            let java_name = descriptor_to_java(&class.descriptor);
            if !classes.contains_key(&java_name) {
                order.push(java_name.clone());
            }
            classes.insert(java_name, Arc::new(class));
        }
        if classes.is_empty() {
            return Err(DecxError::invalid_parameter("jar contains no .class entries"));
        }
        Ok(Self { classes, order })
    }

    pub fn len(&self) -> usize {
        self.classes.len()
    }

    pub fn class_names(&self) -> &[String] {
        &self.order
    }

    pub fn get(&self, java_name: &str) -> Option<Arc<PlatformClass>> {
        self.classes.get(java_name).map(Arc::clone)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<PlatformClass>)> {
        self.classes.iter()
    }

    /// javap-style skeleton: hierarchy, field constants, method signatures.
    /// JVM method bodies are intentionally not rendered.
    pub fn skeleton_source(class: &PlatformClass) -> String {
        let mut out = format!("package {};\n\n", package_of(&class.descriptor));
        out.push_str(&render_class_decl(class));
        for field in &class.fields {
            out.push_str(&render_field(field));
        }
        for method in &class.methods {
            out.push_str(&render_method(method));
        }
        out.push_str("}\n");
        out
    }

    /// Skeleton block for one method (all overloads with that name).
    pub fn method_skeleton(class: &PlatformClass, method_name: &str) -> Option<String> {
        let blocks: Vec<String> = class
            .methods
            .iter()
            .filter(|m| m.name == method_name)
            .map(|m| {
                let full = render_method(m);
                full.trim_end().trim_start_matches("    ").to_string()
            })
            .collect();
        if blocks.is_empty() {
            None
        } else {
            Some(blocks.join("\n"))
        }
    }
}

fn package_of(descriptor: &str) -> String {
    let internal = descriptor.trim_start_matches('L').trim_end_matches(';');
    match internal.rsplit_once('/') {
        Some((pkg, _)) => pkg.replace('/', "."),
        None => String::new(),
    }
}

fn render_class_decl(class: &PlatformClass) -> String {
    let keyword = if class.access_flags & 0x0200 != 0 && class.access_flags & 0x2000 != 0 {
        "@interface "
    } else if class.access_flags & 0x0200 != 0 {
        "interface "
    } else if class.access_flags & 0x4000 != 0 {
        "enum "
    } else {
        "class "
    };
    let mut out = format!("{}{}", render_flags(class.access_flags, &CLASS_MEMBER_FLAGS), keyword);
    out.push_str(&descriptor_to_java(&class.descriptor));
    if let Some(sup) = &class.super_class {
        if sup != "Ljava/lang/Object;" {
            out.push_str(" extends ");
            out.push_str(&descriptor_to_java(sup));
        }
    }
    if !class.interfaces.is_empty() {
        out.push_str(" implements ");
        out.push_str(
            &class
                .interfaces
                .iter()
                .map(|i| descriptor_to_java(i))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    out.push_str(" {\n");
    out
}

const CLASS_MEMBER_FLAGS: [&str; 4] = ["public", "final", "abstract", "strictfp"];
const FIELD_MEMBER_FLAGS: [&str; 7] = [
    "public",
    "private",
    "protected",
    "static",
    "final",
    "volatile",
    "transient",
];
const METHOD_MEMBER_FLAGS: [&str; 8] = [
    "public",
    "private",
    "protected",
    "static",
    "final",
    "abstract",
    "native",
    "synchronized",
];

fn render_flags(flags: u32, known: &[&str]) -> String {
    const MASKS: [(&str, u32); 10] = [
        ("public", 0x1),
        ("private", 0x2),
        ("protected", 0x4),
        ("static", 0x8),
        ("final", 0x10),
        ("volatile", 0x40),
        ("transient", 0x80),
        ("native", 0x100),
        ("abstract", 0x400),
        ("strictfp", 0x800),
    ];
    let mut parts: Vec<&str> = MASKS
        .iter()
        .filter(|(name, mask)| known.contains(name) && flags & mask != 0)
        .map(|(name, _)| *name)
        .collect();
    if flags & 0x1000 != 0 {
        parts.push("synthetic");
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{} ", parts.join(" "))
    }
}

fn render_field(field: &PlatformField) -> String {
    let mut out = String::from("    ");
    out.push_str(&render_flags(field.access_flags, &FIELD_MEMBER_FLAGS));
    out.push_str(&descriptor_to_java(&field.descriptor));
    out.push(' ');
    out.push_str(&field.name);
    if let Some(constant) = &field.constant {
        out.push_str(&format!(" = {}", constant_summary(constant)));
    }
    out.push_str(";\n");
    out
}

fn constant_summary(constant: &PlatformConstant) -> String {
    match constant {
        PlatformConstant::Integer(v) => v.to_string(),
        PlatformConstant::Float(v) => f32::from_bits(*v).to_string(),
        PlatformConstant::Double(v) => f64::from_bits(*v).to_string(),
        PlatformConstant::String(v) => format!("{v:?}"),
        _ => String::new(),
    }
}

fn render_method(method: &PlatformMethod) -> String {
    let (params, ret) = split_method_descriptor(&method.descriptor);
    let is_ctor = method.name == "<init>" || method.name == "<clinit>";
    let mut out = String::from("    ");
    if !is_ctor {
        out.push_str(&render_flags(method.access_flags, &METHOD_MEMBER_FLAGS));
        out.push_str(&descriptor_to_java(&ret));
        out.push(' ');
    }
    out.push_str(if method.name == "<clinit>" {
        "static"
    } else {
        &method.name
    });
    out.push('(');
    out.push_str(
        &params
            .iter()
            .map(|p| descriptor_to_java(p))
            .collect::<Vec<_>>()
            .join(", "),
    );
    out.push_str(");\n");
    out
}
