//! DEX descriptor ↔ Java name conversion helpers.

/// `Lcom/foo/Bar;` → `com.foo.Bar`, `[Lcom/foo/Bar;` → `com.foo.Bar[]`,
/// `I` → `int`, `[I` → `int[]`. Returns the input unchanged for anything else.
pub fn descriptor_to_java(desc: &str) -> String {
    let mut dims = 0usize;
    let mut s = desc;
    while let Some(rest) = s.strip_prefix('[') {
        dims += 1;
        s = rest;
    }
    let base = match s.strip_prefix('L').and_then(|x| x.strip_suffix(';')) {
        Some(cls) => cls.replace('/', "."),
        None => match s {
            "V" => "void".to_string(),
            "Z" => "boolean".to_string(),
            "B" => "byte".to_string(),
            "S" => "short".to_string(),
            "C" => "char".to_string(),
            "I" => "int".to_string(),
            "J" => "long".to_string(),
            "F" => "float".to_string(),
            "D" => "double".to_string(),
            other => other.replace('/', "."),
        },
    };
    let mut out = base;
    for _ in 0..dims {
        out.push_str("[]");
    }
    out
}

/// `com.foo.Bar` → `Lcom/foo/Bar;` (best effort, leaves primitives alone).
pub fn java_to_descriptor(java: &str) -> String {
    let mut dims = 0usize;
    let mut s = java.trim();
    while let Some(rest) = s.strip_suffix("[]") {
        dims += 1;
        s = rest.trim();
    }
    let base = match s {
        "void" => "V",
        "boolean" => "Z",
        "byte" => "B",
        "short" => "S",
        "char" => "C",
        "int" => "I",
        "long" => "J",
        "float" => "F",
        "double" => "D",
        other => return format!("{}{};", "L".to_string() + &other.replace('.', "/"), ""),
    }
    .to_string();
    format!("{}{}", "[".repeat(dims), base)
}

/// Simple name of a java class name: `com.foo.Bar$Baz` → `Baz`.
pub fn simple_name(java_name: &str) -> &str {
    java_name.rsplit(['.', '$']).next().unwrap_or(java_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_descriptors() {
        assert_eq!(descriptor_to_java("Ljava/lang/String;"), "java.lang.String");
        assert_eq!(descriptor_to_java("I"), "int");
        assert_eq!(descriptor_to_java("[I"), "int[]");
        assert_eq!(descriptor_to_java("[[Ljava/lang/Object;"), "java.lang.Object[][]");
        assert_eq!(descriptor_to_java("V"), "void");
    }

    #[test]
    fn simple_names() {
        assert_eq!(simple_name("com.foo.Bar"), "Bar");
        assert_eq!(simple_name("com.foo.Bar$Baz"), "Baz");
    }
}
