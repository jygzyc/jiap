//! Integration tests for user-defined renames: package, class, method, field, and variables.

use super::helpers::{minimal_dex_with_list_return_type, minimal_dex_with_method_code};
use dex_decompiler::{parse_dex, Decompiler, DecompilerOptions, RenameMap};
use std::collections::HashMap;

#[test]
fn test_rename_package() {
    // pkg.Test has package "pkg"; rename to "mypkg"
    let dex_bytes = minimal_dex_with_list_return_type();
    let dex = parse_dex(&dex_bytes).unwrap();
    let mut rename = RenameMap::default();
    rename
        .package
        .insert("pkg".to_string(), "mypkg".to_string());
    let options = DecompilerOptions {
        only_package: None,
        exclude: vec![],
        show_bytecode: false,
        rename_map: Some(rename),
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("// package mypkg"),
        "package rename should appear in output; got: {}",
        java
    );
    assert!(
        !java.contains("// package pkg"),
        "old package name should be replaced; got: {}",
        java
    );
}

#[test]
fn test_rename_class() {
    // pkg.Test -> mypkg.MyClass
    let dex_bytes = minimal_dex_with_list_return_type();
    let dex = parse_dex(&dex_bytes).unwrap();
    let mut rename = RenameMap::default();
    rename
        .class
        .insert("pkg.Test".to_string(), "mypkg.MyClass".to_string());
    let options = DecompilerOptions {
        only_package: None,
        exclude: vec![],
        show_bytecode: false,
        rename_map: Some(rename),
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("class MyClass"),
        "class rename should emit new class name; got: {}",
        java
    );
    assert!(
        !java.contains("class Test"),
        "old class name should be replaced; got: {}",
        java
    );
}

#[test]
fn test_rename_method() {
    // pkg.Test#getList -> fetchList
    let dex_bytes = minimal_dex_with_list_return_type();
    let dex = parse_dex(&dex_bytes).unwrap();
    let mut rename = RenameMap::default();
    rename
        .method
        .insert("pkg.Test#getList".to_string(), "fetchList".to_string());
    let options = DecompilerOptions {
        only_package: None,
        exclude: vec![],
        show_bytecode: false,
        rename_map: Some(rename),
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("fetchList"),
        "method rename should emit new method name; got: {}",
        java
    );
    assert!(
        !java.contains("getList"),
        "old method name should be replaced; got: {}",
        java
    );
}

#[test]
fn test_rename_variable() {
    // const/4+return is folded to `return 0`; a single new-instance is inlined
    // to `return new Object()`. Two uses of the same local keep a name.
    let bytecode: &[u8] = &[
        0x22, 0x00, 0x00, 0x00, // new-instance v0, type@0 Ljava/lang/Object;
        0x38, 0x00, 0x02, 0x00, // if-eqz v0, +2
        0x11, 0x00, // return-object v0
        0x11, 0x00, // return-object v0
    ];
    let dex_bytes = minimal_dex_with_method_code(bytecode);
    let dex = parse_dex(&dex_bytes).unwrap();
    let mut var_map = HashMap::new();
    // Whatever display name the decompiler picks, map it to myResult.
    for old in ["result", "local0", "p0", "o0", "obj", "object"] {
        var_map.insert(old.to_string(), "myResult".to_string());
    }
    let mut rename = RenameMap::default();
    rename.variable.insert("Simple#foo".to_string(), var_map);
    let options = DecompilerOptions {
        only_package: None,
        exclude: vec![],
        show_bytecode: false,
        rename_map: Some(rename),
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("myResult"),
        "variable rename should emit new var name; got: {}",
        java
    );
    assert!(
        !java.contains("return result"),
        "old variable name should be replaced in return; got: {}",
        java
    );
}

#[test]
fn test_rename_field() {
    // Use a DEX that has a field: we need a helper. For now test that field renames
    // are applied when the replacement list is built (unit-test replace_identifier).
    // Integration: if we had minimal_dex_with_field we would rename Class#fieldName.
    // Without a field in minimal DEX, we only verify the rename map accepts field entries.
    let mut rename = RenameMap::default();
    rename
        .field
        .insert("pkg.Test#someField".to_string(), "myField".to_string());
    let replacements = rename.replacements_for_class(
        "pkg.Test",
        &["getList".to_string()],
        &["someField".to_string()],
    );
    let has_field_rename = replacements
        .iter()
        .any(|(old, new)| old == "someField" && new == "myField");
    assert!(
        has_field_rename,
        "replacements_for_class should include field rename"
    );
}
