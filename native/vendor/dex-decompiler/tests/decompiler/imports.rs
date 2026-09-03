//! Tests for import emission: classes that use non-java.lang types should get "import pkg.Class;" and short names in body.

use super::helpers::minimal_dex_with_list_return_type;
use dex_decompiler::{parse_dex, Decompiler};

#[test]
fn test_decompiled_class_contains_import_java_util_list() {
    let data = minimal_dex_with_list_return_type();
    let dex = parse_dex(&data).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("import java.util.List;"),
        "expected 'import java.util.List;' in decompiled output, got:\n{}",
        java
    );
}

#[test]
fn test_decompiled_class_uses_short_name_list_in_signature() {
    let data = minimal_dex_with_list_return_type();
    let dex = parse_dex(&data).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("List getList()"),
        "expected short name 'List getList()' in method signature, got:\n{}",
        java
    );
}

#[test]
fn test_decompiled_class_has_import_before_class_declaration() {
    let data = minimal_dex_with_list_return_type();
    let dex = parse_dex(&data).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    let import_pos = java.find("import java.util.List;").expect("import line");
    let class_pos = java
        .find("class Test ")
        .or_else(|| java.find("class Test{"))
        .expect("class decl");
    assert!(
        import_pos < class_pos,
        "import should appear before class declaration"
    );
}
