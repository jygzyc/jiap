//! Shared assertions for try/catch/finally/TWR decompilation (DEX or APK pipeline).

use std::path::PathBuf;

pub const TRY_CATCH_CLASS: &str = "com.androguard.decompilefixtures.TryCatchFixtures";

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/decompiler_fixtures")
}

pub fn fixtures_dex_path() -> PathBuf {
    fixtures_dir().join("classes.dex")
}

pub fn fixtures_apk_path() -> PathBuf {
    fixtures_dir().join("decompiler_fixtures.apk")
}

/// Extract one method declaration + balanced `{ … }` body.
pub fn method_region(java: &str, method_name: &str) -> String {
    let needle = format!("{method_name}(");
    let mut idx = 0;
    let sig_start = loop {
        let Some(rel) = java[idx..].find(&needle) else {
            panic!("method {method_name} not found in:\n{java}");
        };
        let start = idx + rel;
        let line_start = java[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = &java[line_start..start];
        let before_name = line.trim_end();
        let is_call = before_name.ends_with('.');
        let is_decl = line.contains("public")
            || line.contains("private")
            || line.contains("protected")
            || line.contains("static");
        if is_decl && !is_call {
            break start;
        }
        idx = start + needle.len();
    };
    let open_rel = java[sig_start..]
        .find('{')
        .unwrap_or_else(|| panic!("method {method_name} has no opening brace"));
    let open = sig_start + open_rel;
    let mut depth = 0i32;
    for (i, ch) in java[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return java[sig_start..=open + i].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("method {method_name} has unbalanced braces");
}

pub fn assert_nested_try(body: &str) {
    assert_eq!(
        body.matches("try {").count(),
        2,
        "nestedTry should have two try blocks; got:\n{body}"
    );
    assert!(body.contains("catch (RuntimeException"), "{body}");
    assert!(body.contains("catch (Exception"), "{body}");
    let re = body.find("catch (RuntimeException").unwrap();
    let ex = body.find("catch (Exception").unwrap();
    assert!(
        re < ex,
        "RuntimeException catch should be inner; got:\n{body}"
    );
    assert!(
        body.contains("/ x") || body.contains("/x"),
        "nestedTry should divide (can throw); got:\n{body}"
    );
    assert!(
        !body.contains("x * 2") && !body.contains("* 2"),
        "nestedTry must not collapse to bare multiply; got:\n{body}"
    );
    assert!(
        body.contains("return -1") && body.contains("return -2"),
        "{body}"
    );
    assert!(!body.contains("catch (Throwable"), "{body}");
}

pub fn assert_simple_try_catch(body: &str) {
    assert!(body.contains("try {"), "{body}");
    assert!(body.contains("catch (ArithmeticException"), "{body}");
    assert!(body.contains("/ x") || body.contains("/x"), "{body}");
    assert!(body.contains("return -1"), "{body}");
    assert_eq!(body.matches("try {").count(), 1, "{body}");
}

pub fn assert_multi_catch(body: &str) {
    assert!(
        body.contains("try {") && body.contains("toString()"),
        "{body}"
    );
    assert!(
        body.contains("NullPointerException") && body.contains("ClassCastException"),
        "{body}"
    );
    assert!(body.contains("return \"err\""), "{body}");
}

pub fn assert_try_finally(body: &str) {
    assert!(
        body.contains("try {") && body.contains("finally {"),
        "tryFinally should decompile to try/finally; got:\n{body}"
    );
    assert!(
        body.contains("int acc = 0") || body.contains("acc = acc + x") || body.contains("acc += x"),
        "{body}"
    );
    assert!(
        body.contains("acc = acc + x")
            || body.contains("acc += x")
            || (body.contains("try {")
                && body.split("try {").nth(1).is_some_and(|t| {
                    t.contains("acc = acc + x") || t.contains("acc += x") || t.contains("0 + x")
                })),
        "acc update should be inside try; got:\n{body}"
    );
    assert!(
        body.contains("return acc") || body.contains("return (acc)"),
        "return should follow try/finally; got:\n{body}"
    );
    assert!(
        body.contains("acc = acc + 1")
            || body.contains("acc += 1")
            || body.contains("acc++")
            || body.contains("acc = 0 + 1"),
        "{body}"
    );
    assert!(!body.contains("catch (Throwable"), "{body}");
}

pub fn assert_try_with_resources(twr: &str) {
    assert!(
        twr.contains("try (") && twr.contains("StubResource"),
        "tryWithResources should use try-with-resources; got:\n{twr}"
    );
    assert!(
        twr.contains("touch()") && twr.contains("return 42"),
        "{twr}"
    );
    assert!(
        !twr.contains(".close()") && !twr.contains("addSuppressed"),
        "{twr}"
    );
    assert!(!twr.contains("finally"), "{twr}");
}

pub fn assert_try_with_resources_two(twr_two: &str) {
    assert!(
        twr_two.contains("try (")
            && twr_two.contains("StubResource a =")
            && twr_two.contains("StubResource b ="),
        "tryWithResourcesTwo should use multi-resource TWR; got:\n{twr_two}"
    );
    assert!(
        twr_two.contains("a.touch()") && twr_two.contains("b.touch()"),
        "{twr_two}"
    );
    assert!(twr_two.contains("return 7"), "{twr_two}");
    assert!(
        !twr_two.contains(".close()") && !twr_two.contains("addSuppressed"),
        "{twr_two}"
    );
    let brace_only = twr_two.lines().filter(|l| l.trim() == "}").count();
    assert!(
        brace_only <= 2,
        "tryWithResourcesTwo should not have stray closing braces; got:\n{twr_two}"
    );
}

pub fn assert_all_try_catch_fixtures(java: &str) {
    assert_nested_try(&method_region(java, "nestedTry"));
    assert_simple_try_catch(&method_region(java, "simpleTryCatch"));
    assert_multi_catch(&method_region(java, "multiCatch"));
    assert_try_finally(&method_region(java, "tryFinally"));
    assert_try_with_resources(&method_region(java, "tryWithResources"));
    assert_try_with_resources_two(&method_region(java, "tryWithResourcesTwo"));
}
