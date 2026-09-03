//! Regression tests mirroring [jadx](https://github.com/skylot/jadx) integration
//! assertions (`jadx-core/.../tests/integration/{conditions,loops,switches,trycatch,generics,inner}`).
//!
//! Fixtures: `testdata/androguard_test_classes.dex` (androguard Test* classes) plus pure unit
//! checks for Signature / string-switch where a full Java→DEX toolchain is unavailable.
//!
//! Catalog: `tests/data/jadx_parity/CATALOG.md`.

#![allow(non_snake_case)] // keep jadx IntegrationTest names recognizable

use dex_decompiler::decompile::annotations::{
    parse_class_signature, signature_method_to_java, signature_to_java_generics,
    signature_type_to_java,
};
use dex_decompiler::decompile::simplify_restore_string_switch_for_tests;
use dex_decompiler::{parse_dex, Decompiler, DecompilerOptions};
use std::path::PathBuf;

fn androguard_dex_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/androguard_test_classes.dex")
}

fn require_androguard_dex() -> dex_decompiler::DexFile {
    let path = androguard_dex_path();
    assert!(
        path.exists(),
        "missing fixture {} (required for jadx-parity regression)",
        path.display()
    );
    let data = std::fs::read(&path).expect("read androguard_test_classes.dex");
    parse_dex(&data).expect("parse androguard_test_classes.dex")
}

fn decompile_only(package: &str) -> String {
    let dex = require_androguard_dex();
    let options = DecompilerOptions {
        only_package: Some(package.to_string()),
        exclude: vec![],
        ..Default::default()
    };
    Decompiler::with_options(&dex, options)
        .decompile()
        .expect("decompile")
}

fn decompile_excluding(excludes: &[&str]) -> String {
    let dex = require_androguard_dex();
    let options = DecompilerOptions {
        only_package: None,
        exclude: excludes.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    Decompiler::with_options(&dex, options)
        .decompile()
        .expect("decompile")
}

fn count_substr(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

fn method_region(java: &str, method_name: &str) -> String {
    let start = java
        .find(method_name)
        .unwrap_or_else(|| panic!("method {method_name} not found"));
    let after = &java[start..];
    let end = after
        .find("\n    public ")
        .or_else(|| after.find("\n    @"))
        .or_else(|| after.find("\n}"))
        .unwrap_or(after.len());
    after[..end].to_string()
}

// ---------------------------------------------------------------------------
// conditions — jadx TestElseIf / TestConditions
// ---------------------------------------------------------------------------

/// jadx: `conditions/TestElseIf` — if retained, no ternary collapse.
#[test]
fn jadx_conditions_TestElseIf_style() {
    let java = decompile_only("tests.androguard.TestIfs");
    assert!(java.contains("class TestIfs"), "got:\n{java}");
    let body = method_region(&java, "testIfBool");
    assert!(
        body.contains("if ("),
        "testIfBool should retain if; got:\n{body}"
    );
    assert!(
        !(body.contains('?') && body.contains(" : ")),
        "testIfBool must not be ternary-collapsed; got:\n{body}"
    );
}

/// jadx: `conditions/TestConditions` — short-circuit stays structured.
#[test]
fn jadx_conditions_TestConditions_style() {
    let java = decompile_only("tests.androguard.TestIfs");
    let body = method_region(&java, "testShortCircuit");
    assert!(
        body.contains("if ("),
        "short-circuit should stay structured; got:\n{body}"
    );
    assert!(
        body.contains("return"),
        "testShortCircuit must return; got:\n{body}"
    );
}

// ---------------------------------------------------------------------------
// loops — jadx TestBreakInLoop
// ---------------------------------------------------------------------------

/// jadx: `loops/TestBreakInLoop` — loop + conditional present.
#[test]
fn jadx_loops_TestBreakInLoop_style() {
    let java = decompile_only("tests.androguard.TestLoops");
    assert!(java.contains("class TestLoops"));
    let body = method_region(&java, "testBreak2");
    assert!(
        body.contains("while (") || body.contains("for ("),
        "loop expected; got:\n{body}"
    );
    assert!(
        body.contains("if ("),
        "break condition if expected; got:\n{body}"
    );
}

// ---------------------------------------------------------------------------
// switches — jadx TestSwitchSimple / TestSwitchBreak
// ---------------------------------------------------------------------------

/// jadx: `switches/TestSwitchSimple` — packed switch cases 1..5 with breaks.
#[test]
fn jadx_switches_TestSwitchSimple() {
    let java = decompile_excluding(&["android", "androidx", "android.support", "tests.androguard"]);
    assert!(
        java.contains("class TestDefaultPackage"),
        "expected TestDefaultPackage; got prefix:\n{}",
        &java[..java.len().min(400)]
    );
    let body = method_region(&java, "main");
    assert!(body.contains("switch ("), "got:\n{body}");
    for n in 1..=5 {
        assert!(
            body.contains(&format!("case {n}:")),
            "missing case {n}; got:\n{body}"
        );
    }
    assert!(
        count_substr(&body, "break;") >= 5,
        "each case should break (jadx TestSwitchBreak); got:\n{body}"
    );
}

/// jadx: `switches/TestSwitchOverStrings` — hashCode cases → string labels.
#[test]
fn jadx_switches_TestSwitchOverStrings_restore() {
    let body = r#"        switch (str.hashCode()) {
        case -603257287:
            if (!str.equals("frewhyh")) break;
            return 1;
        case 3556498:
            if (!str.equals("test")) break;
            return 3;
        default:
            return 0;
        }
"#;
    let restored = simplify_restore_string_switch_for_tests(body);
    assert!(restored.contains("switch (str)"), "got:\n{restored}");
    assert!(
        restored.contains("case \"frewhyh\":"),
        "string case required; got:\n{restored}"
    );
    assert!(
        restored.contains("case \"test\":"),
        "string case required; got:\n{restored}"
    );
    assert!(
        !restored.contains("case -603257287:")
            || restored.contains("// was -603257287")
            || restored.contains("case \"frewhyh\":"),
        "numeric hash case should be rewritten; got:\n{restored}"
    );
    assert!(
        !restored.contains("c = "),
        "must not leave synthetic char discriminant; got:\n{restored}"
    );
}

// ---------------------------------------------------------------------------
// try/catch — jadx TestTryCatch
// ---------------------------------------------------------------------------

/// jadx: `trycatch/TestTryCatch` — try + typed catch; handler not in try body.
#[test]
fn jadx_trycatch_TestTryCatch() {
    let java = decompile_only("tests.androguard.TestExceptions");
    let body = method_region(&java, "testTry1");
    assert!(body.contains("try {"), "got:\n{body}");
    assert!(
        body.contains("} catch (ArithmeticException e) {")
            || body.contains("} catch (ArithmeticException "),
        "typed catch required; got:\n{body}"
    );
    let try_pos = body.find("try {").unwrap();
    let catch_pos = body.find("} catch (").expect("catch");
    let try_body = &body[try_pos..catch_pos];
    assert!(
        !try_body.contains("move-exception"),
        "handler must not leak into try; try_body={try_body}"
    );
}

/// jadx: multi-try presence (TestTryCatch2 style).
#[test]
fn jadx_trycatch_multiple_methods_have_try() {
    let java = decompile_only("tests.androguard.TestExceptions");
    assert!(count_substr(&java, "try {") >= 2);
    assert!(count_substr(&java, "} catch (") >= 2);
}

// ---------------------------------------------------------------------------
// generics — jadx TestGenerics / method + class Signature
// ---------------------------------------------------------------------------

/// jadx: `generics/TestGenerics` — wildcards.
#[test]
fn jadx_generics_TestGenerics_wildcards() {
    assert_eq!(
        signature_type_to_java("Ljava/util/List<*>;").as_deref(),
        Some("java.util.List<?>")
    );
    assert_eq!(
        signature_type_to_java("Ljava/util/List<+Lgenerics/A;>;").as_deref(),
        Some("java.util.List<? extends generics.A>")
    );
    assert_eq!(
        signature_type_to_java("Ljava/util/List<-Lgenerics/A;>;").as_deref(),
        Some("java.util.List<? super generics.A>")
    );
}

/// jadx: method Signature with type param.
#[test]
fn jadx_generics_method_signature() {
    let m = signature_method_to_java("<T:Ljava/lang/Object;>(Ljava/util/List<TT;>;)TT;")
        .expect("parse method signature");
    assert_eq!(m.type_params.as_deref(), Some("<T>"));
    assert_eq!(m.params, vec!["java.util.List<T>"]);
    assert_eq!(m.return_type, "T");
}

/// jadx: class Signature type params + bounds + interfaces.
#[test]
fn jadx_generics_class_signature_bounds() {
    let g = signature_to_java_generics(
        "<T:Ljava/lang/Number;:Ljava/lang/Comparable<TT;>;>Ljava/lang/Object;",
    )
    .expect("type params");
    assert!(g.contains("<T extends "), "got {g}");
    assert!(g.contains("Number"), "got {g}");
    let cls = parse_class_signature(
        "<T:Ljava/lang/Object;>Ljava/util/AbstractList<TT;>;Ljava/io/Serializable;",
    )
    .expect("class sig");
    assert_eq!(cls.type_params.as_deref(), Some("<T>"));
    assert!(cls
        .superclass
        .as_deref()
        .unwrap_or("")
        .contains("AbstractList"));
    assert!(cls.interfaces.iter().any(|i| i.contains("Serializable")));
}

/// Nested generic class from androguard TestSynthetic.Bridge.
#[test]
fn jadx_generics_nested_Bridge_type_param() {
    let java = decompile_only("tests.androguard.TestSynthetic");
    assert!(
        java.contains("class Bridge<T>"),
        "Bridge should keep type param; got:\n{java}"
    );
    assert!(
        java.contains("BridgeExt") && java.contains("extends") && java.contains("Bridge"),
        "BridgeExt should extend Bridge; got:\n{java}"
    );
}

// ---------------------------------------------------------------------------
// inner / anonymous — jadx TestInnerClass / TestAnonymousClass
// ---------------------------------------------------------------------------

/// jadx: `inner/TestInnerClass` — named inners nested under outer.
#[test]
fn jadx_inner_TestInnerClass_nested() {
    let java = decompile_excluding(&["android", "androidx", "android.support", "tests.androguard"]);
    assert!(java.contains("class TestDefaultPackage"));
    assert!(
        java.contains("class TestInnerClass"),
        "named inner must nest; got:\n{}",
        &java[..java.len().min(2000)]
    );
    assert!(
        java.contains("class TestInnerInnerClass"),
        "nested-nested inner missing"
    );
    assert!(
        !java.contains("\npublic class TestDefaultPackage$TestInnerClass"),
        "named inner must not be a separate top-level class"
    );
}

/// jadx: Bridge nested under TestSynthetic (same as TestInnerClass).
#[test]
fn jadx_inner_TestSynthetic_Bridge_nested() {
    let java = decompile_only("tests.androguard.TestSynthetic");
    let bridge_at = java.find("class Bridge").expect("Bridge nested");
    let outer_at = java.find("class TestSynthetic").expect("outer");
    assert!(
        bridge_at > outer_at,
        "Bridge should appear inside TestSynthetic body"
    );
    assert!(
        !java.contains("\npublic class TestSynthetic$Bridge"),
        "Bridge must not be top-level"
    );
}

/// jadx: `inner/TestAnonymousClass` — no invented AnonymousClass_ names.
#[test]
fn jadx_inner_TestAnonymousClass_thread_site() {
    let java = decompile_only("tests.androguard.TestSynthetic");
    let body = method_region(&java, "TestSynthetic1");
    assert!(
        body.contains("new Object()") || body.contains("new java.lang.Object()"),
        "capture object retained; got:\n{body}"
    );
    assert!(
        body.contains(".start()") || body.contains("new Thread"),
        "Thread.start / inline Thread expected; got:\n{body}"
    );
    assert!(
        !body.to_lowercase().contains("anonymousclass_"),
        "must not invent AnonymousClass_ names; got:\n{body}"
    );
}

// ---------------------------------------------------------------------------
// Text-level jadx-parity simplify (conditions / loops / trycatch polish)
// ---------------------------------------------------------------------------

#[test]
fn jadx_conditions_short_circuit_and() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body = "        if (a) {\n            if (b) {\n                foo();\n            }\n        }\n";
    let out = simplify(body, false);
    assert!(out.contains("if (a && b)"), "got:\n{out}");
    assert!(out.contains("foo();"), "got:\n{out}");
}

#[test]
fn jadx_conditions_short_circuit_or() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body = "        if (a) {\n            foo();\n        } else {\n            if (b) {\n                foo();\n            }\n        }\n";
    let out = simplify(body, false);
    assert!(out.contains("if (a || b)"), "got:\n{out}");
    assert!(out.contains("foo();"), "got:\n{out}");
    assert!(!out.contains("else if"), "got:\n{out}");
}

#[test]
fn jadx_ternary_assign() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body =
        "        if (c) {\n            x = a;\n        } else {\n            x = b;\n        }\n";
    let out = simplify(body, false);
    assert!(out.contains("x = c ? a : b;"), "got:\n{out}");
    assert!(!out.contains("} else {"), "got:\n{out}");
}

#[test]
fn jadx_loops_foreach_iterator() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body = "        it = list.iterator();\n        while (it.hasNext()) {\n            String s = (String) it.next();\n            use(s);\n        }\n";
    let out = simplify(body, false);
    assert!(out.contains("for (String s : list)"), "got:\n{out}");
    assert!(!out.contains("hasNext()"), "got:\n{out}");
}

#[test]
fn jadx_strip_requireNonNull() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body = "        x = Objects.requireNonNull(y);\n        Objects.requireNonNull(z);\n";
    let out = simplify(body, false);
    assert!(out.contains("x = y;"), "got:\n{out}");
    assert!(!out.contains("requireNonNull"), "got:\n{out}");
}

#[test]
fn jadx_trycatch_multi_catch() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body = "        try {\n            foo();\n        } catch (IOException e) {\n            log(e);\n        } catch (RuntimeException e) {\n            log(e);\n        }\n";
    let out = simplify(body, false);
    assert!(
        out.contains("} catch (IOException | RuntimeException e)"),
        "got:\n{out}"
    );
}

#[test]
fn jadx_switches_enum_switchmap() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body = "        Outer.$SwitchMap$com$example$Color[Color.RED.ordinal()] = 1;\n        switch (Outer.$SwitchMap$com$example$Color[color.ordinal()]) {\n            case 1:\n                return \"red\";\n        }\n";
    let out = simplify(body, false);
    assert!(out.contains("switch (color)"), "got:\n{out}");
    assert!(
        out.contains("case Color.RED:") || out.contains("case com.example.Color.RED:"),
        "got:\n{out}"
    );
    assert!(!out.contains("$SwitchMap$"), "got:\n{out}");
}

#[test]
fn jadx_try_with_resources() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body = "        FileInputStream r = new FileInputStream(path);\n        try {\n            use(r);\n        } catch (IOException e) {\n            log(e);\n        } finally {\n            if (r != null) {\n                r.close();\n            }\n        }\n";
    let out = simplify(body, false);
    assert!(
        out.contains("try (FileInputStream r = new FileInputStream(path))"),
        "got:\n{out}"
    );
    assert!(!out.contains("finally"), "got:\n{out}");
}

#[test]
fn jadx_try_with_resources_multi() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body = "        FileInputStream in = new FileInputStream(path);\n        FileOutputStream out = new FileOutputStream(dest);\n        try {\n            copy(in, out);\n        } finally {\n            if (in != null) {\n                in.close();\n            }\n            if (out != null) {\n                out.close();\n            }\n        }\n";
    let simplified = simplify(body, false);
    assert!(
        simplified.contains(
            "try (FileInputStream in = new FileInputStream(path); FileOutputStream out = new FileOutputStream(dest))"
        ),
        "got:\n{simplified}"
    );
    assert!(!simplified.contains("finally"), "got:\n{simplified}");
}

#[test]
fn jadx_strip_redundant_cast() {
    use dex_decompiler::decompile::simplify_method_body_for_tests as simplify;
    let body =
        "        String s = (String) new String(\"x\");\n        Object o = (String) (String) y;\n";
    let out = simplify(body, false);
    assert!(out.contains("String s = new String(\"x\")"), "got:\n{out}");
    assert!(out.contains("Object o = (String) y"), "got:\n{out}");
}
