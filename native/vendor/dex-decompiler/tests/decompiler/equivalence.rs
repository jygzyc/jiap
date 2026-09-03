//! Decompilation output tests: minimal DEX, parse failures, optional fixtures.

use super::helpers::{minimal_dex_bytes, minimal_dex_with_method_code};
use dex_decompiler::{parse_dex, Decompiler, DecompilerOptions};
use std::path::Path;

#[test]
fn test_decompiler_minimal_dex_empty_output() {
    let minimal = minimal_dex_bytes();
    let dex = parse_dex(&minimal).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.is_empty(),
        "empty DEX (no classes) should decompile to empty string"
    );
}

/// Parse of invalid bytes must fail.
#[test]
fn test_decompiler_parse_fails_invalid() {
    assert!(parse_dex(&[]).is_err());
    assert!(parse_dex(b"not a DEX file!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!").is_err());
}

/// Non-enum class (extends Object) must emit "class" not "enum".
#[test]
fn test_decompiler_non_enum_emits_class() {
    let dex_bytes = minimal_dex_with_method_code(&[0x0e, 0x00]);
    let dex = parse_dex(&dex_bytes).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("class "),
        "non-enum class should emit 'class'; got: {}",
        java
    );
    assert!(
        !java.contains("enum "),
        "non-enum class should not emit 'enum'; got: {}",
        java
    );
}

/// If testdata/Enum.dex exists (DEX with a class extending java.lang.Enum and static final self-type fields),
/// decompile and assert output contains "enum" and constant list.
#[test]
fn test_decompiler_enum_emits_enum() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/Enum.dex");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(&path).unwrap();
    let dex = parse_dex(&data).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("enum "),
        "enum class should emit 'enum'; got: {}",
        java
    );
}

/// Optional: if tests/data/APK/Test.dex exists, assert decompiled source contains simplification pattern.
#[test]
#[ignore = "requires androguard test data: tests/data/APK/Test.dex"]
fn test_decompiler_simplification() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/APK/Test.dex");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(&path).unwrap();
    let dex = parse_dex(&data).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("return ((23 - p3) | ((p3 + 66) & 26));"),
        "expected simplification pattern in decompiled source"
    );
}

/// Decompile androguard.test.TestExceptions and assert try/catch is correct:
/// handler code (e.g. move-exception) must appear in the catch block, not in the try block.
#[test]
fn test_decompiler_try_catch_handler_in_catch_block() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/androguard_test_classes.dex");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(&path).unwrap();
    let dex = parse_dex(&data).unwrap();
    let options = DecompilerOptions {
        only_package: Some("tests.androguard".to_string()),
        exclude: vec![],
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("testCatch1"),
        "decompiled output must contain testCatch1 method; got (first 500 chars): {:?}",
        &java[..java.len().min(500)]
    );
    assert!(
        java.contains("} catch (ArithmeticException e) {"),
        "decompiled output must contain catch block"
    );
    let method_start = java.find("testCatch1").expect("testCatch1 present");
    let after_method = &java[method_start..];
    let try_marker = "try {";
    let catch_marker = "} catch (ArithmeticException e) {";
    let try_pos = after_method.find(try_marker).expect("try block present");
    let try_body_start = method_start + try_pos + try_marker.len();
    let rest_after_try = &java[try_body_start..];
    let catch_pos = rest_after_try
        .find(catch_marker)
        .expect("catch block present");
    let try_body = &rest_after_try[..catch_pos];
    assert!(
        !try_body.contains("move-exception"),
        "try block must not contain handler code (move-exception); handler code belongs in the catch block. \
         Try body contained move-exception (decompiler put catch body inside try)."
    );
    let catch_body_start = try_body_start + catch_pos + catch_marker.len();
    let catch_rest = &java[catch_body_start..];
    let catch_end = catch_rest
        .find("\n        }")
        .or_else(|| catch_rest.find("        }"))
        .unwrap_or(catch_rest.len());
    let catch_body = catch_rest[..catch_end].trim();
    assert!(
        catch_body.len() > 20
            || catch_body.contains("println")
            || catch_body.contains("test2")
            || catch_body.contains("n0 = 12"),
        "catch block should contain handler code, not just a comment; got: {:?}",
        catch_body
    );
}

/// Condition register renaming must not produce use-before-declare:
/// A variable that is first declared (with a type like `int n0 = ...`) inside
/// the if-body must not appear in the if condition itself.
#[test]
fn test_decompiler_no_use_before_declare_in_conditions() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/androguard_test_classes.dex");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(&path).unwrap();
    let dex = parse_dex(&data).unwrap();
    let options = DecompilerOptions {
        only_package: Some("tests.androguard".to_string()),
        exclude: vec![],
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    if !java.contains("testException2") {
        return;
    }
    let method_start = java.find("testException2").unwrap();
    let after = &java[method_start..];
    let body_end = after.find("\n    public ").unwrap_or(after.len());
    let method_body = &after[..body_end];

    let lines: Vec<&str> = method_body.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("if (") {
            continue;
        }
        let cond_end = trimmed
            .find(") {")
            .unwrap_or(trimmed.find(')').unwrap_or(trimmed.len()));
        let condition = &trimmed[4..cond_end];
        // Scan the if-body for typed declarations like "int n0 = ..."
        for body_line in &lines[i + 1..] {
            let bt = body_line.trim();
            if bt == "}" || bt.starts_with("} else") {
                break;
            }
            for type_kw in &[
                "int ", "String ", "long ", "double ", "float ", "boolean ", "char ", "byte ",
            ] {
                if let Some(rest) = bt.strip_prefix(type_kw) {
                    let var_end = rest
                        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .unwrap_or(rest.len());
                    let var_name = &rest[..var_end];
                    assert!(
                        !condition.contains(var_name),
                        "Condition '{}' references '{}' which is first declared inside the if-body: '{}'\n\
                         This is a use-before-declare bug.",
                        condition, var_name, bt
                    );
                }
            }
        }
    }
}

/// Packed-switch in TestDefault.main should emit case labels (1..5).
#[test]
fn test_decompiler_switch_packed_cases() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/androguard_test_classes.dex");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(&path).unwrap();
    let dex = parse_dex(&data).unwrap();
    let options = DecompilerOptions {
        only_package: Some("tests.androguard".to_string()),
        exclude: vec![],
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    if java.is_empty() || !java.contains("TestDefault") {
        return;
    }
    assert!(
        java.contains("case 1:") && java.contains("case 5:"),
        "packed-switch should emit case labels; got (unknown payload) or (payload not found)?"
    );
}

/// Parameter registers must be correctly identified (at the HIGH end of the register space).
/// Local registers (holding arrays, loop vars, etc.) must NOT be named pN.
#[test]
fn test_decompiler_param_registers_not_misidentified() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/androguard_test_classes.dex");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(&path).unwrap();
    let dex = parse_dex(&data).unwrap();
    let options = DecompilerOptions {
        only_package: Some("tests.androguard".to_string()),
        exclude: vec![],
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    if !java.contains("testException2") {
        return;
    }
    let method_start = java.find("testException2").unwrap();
    let after = &java[method_start..];
    let body_end = after.find("\n    public ").unwrap_or(after.len());
    let method_body = &after[..body_end];

    // Array operations like arr0[i] must NOT use parameter names as arrays.
    // p0 and p1 are int parameters — indexing into them (p0[p1]) is invalid Java.
    for line in method_body.lines() {
        let t = line.trim();
        if t.contains('[') && t.contains(']') {
            assert!(
                !t.contains("p0[") && !t.contains("p1["),
                "Array operation must not index into parameter (int) variables. \
                 p0/p1 are int, not arrays. Got: '{}'",
                t
            );
        }
    }

    // If the method declares an array (e.g. int[] arr0 = ...), subsequent uses must
    // use the same name, not a different alias like 'array'.
    let mut declared_array: Option<String> = None;
    for line in method_body.lines() {
        let t = line.trim();
        if t.contains("[] ") && t.contains(" = new ") {
            if let Some(name_start) = t.find("[] ") {
                let after_bracket = &t[name_start + 3..];
                let name_end = after_bracket
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .unwrap_or(after_bracket.len());
                declared_array = Some(after_bracket[..name_end].to_string());
            }
        }
        if let Some(ref arr_name) = declared_array {
            if t.contains('[') && !t.contains("new ") {
                if let Some(bracket_pos) = t.find('[') {
                    let before = t[..bracket_pos].trim_start();
                    let var = before
                        .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .next()
                        .unwrap_or("");
                    if !var.is_empty()
                        && var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && var != arr_name
                    {
                        if var.starts_with("arr") || var == "array" {
                            panic!(
                                "Array usage '{}' uses name '{}' but declared name is '{}'. \
                                 Names must be consistent.",
                                t, var, arr_name
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Optional: if tests/data/APK/FillArrays.dex exists, assert array literals in output.
#[test]
#[ignore = "requires androguard test data: tests/data/APK/FillArrays.dex"]
fn test_decompiler_arrays() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/APK/FillArrays.dex");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(&path).unwrap();
    let dex = parse_dex(&data).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(java.contains("{20, 30, 40, 50};"));
    assert!(java.contains("{1, 2, 3, 4, 5, 999, 10324234};"));
    assert!(java.contains("{97, 98, 120, 122, 99};"));
    assert!(java.contains("{5, 10, 15, 20};"));
    assert!(!java.contains("{97, 0, 98, 0, 120};"));
    assert!(!java.contains("{5, 0, 10, 0};"));
}
