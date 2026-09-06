//! Standard-java archive (standard jar / android.jar) support tests against
//! the committed `testdata/standard.jar` fixture (javac-compiled).

use decx_core::{api, Project, ProjectKind};
use serde_json::{json, Value};

fn fixture_jar() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/standard.jar")
}

fn project() -> Project {
    Project::open(&fixture_jar()).expect("open standard.jar")
}

fn dispatch_ok(p: &Project, endpoint: &str, body: Value) -> Value {
    api::dispatch(p, endpoint, &body).unwrap_or_else(|e| panic!("{endpoint} failed: {e}"))
}

fn items_of(v: &Value) -> &[Value] {
    v["items"].as_array().expect("items array")
}

#[test]
fn opens_as_java_archive_with_all_classes() {
    let p = project();
    assert_eq!(p.kind, ProjectKind::JavaArchive);
    assert_eq!(p.entries().len(), 3);
    assert!(p.lookup("demo.StdSample").is_some());
    assert!(p.lookup("demo.Base").is_some());
    assert!(p.lookup("demo.Constants").is_some());
}

#[test]
fn class_source_is_skeleton_with_hierarchy_and_constants() {
    let p = project();
    let entry = p.lookup("demo.StdSample").unwrap();
    let src = p.class_source(entry).expect("skeleton");
    let text = src.as_str();
    assert!(text.contains("package demo;"));
    let text = src.as_str();
    assert!(
        text.contains("abstract class demo.StdSample extends demo.Base implements java.lang.Runnable"),
        "skeleton was:\n{text}"
    );
    assert!(text.contains("public static final int MAX = 42"));
    assert!(text.contains("private static final java.lang.String NAME = \"sample\""));
    assert!(text.contains("public abstract int compute(int);"));
    assert!(text.contains("public boolean isReady();"));
}

#[test]
fn method_source_returns_signature_block() {
    let p = project();
    let src = dispatch_ok(
        &p,
        "get_method_source",
        json!({ "mth": "demo.StdSample#compute" }),
    );
    let content = items_of(&src)[0]["content"].as_str().expect("content");
    assert!(content.contains("compute(int)"), "got: {content}");
}

#[test]
fn class_context_and_hierarchy_work() {
    let p = project();
    let ctx = dispatch_ok(&p, "get_class_context", json!({ "cls": "demo.StdSample" }));
    assert!(!items_of(&ctx).is_empty());

    // demo.Constants is the interface present in the fixture jar
    let impls = dispatch_ok(&p, "get_implementations", json!({ "iface": "demo.Constants" }));
    let ids: Vec<&str> = items_of(&impls).iter().filter_map(|i| i["id"].as_str()).collect();
    assert!(
        ids.iter().any(|v| *v == "demo.StdSample"),
        "implementations were: {ids:?}"
    );

    let subs = dispatch_ok(&p, "get_subclasses", json!({ "cls": "demo.Base" }));
    let ids: Vec<&str> = items_of(&subs).iter().filter_map(|i| i["id"].as_str()).collect();
    assert!(
        ids.iter().any(|v| *v == "demo.StdSample"),
        "subclasses were: {ids:?}"
    );
}

#[test]
fn search_finds_skeleton_content() {
    let p = project();
    let hits = dispatch_ok(
        &p,
        "search_global_key",
        json!({ "key": "compute", "search": { "limit": 5 } }),
    );
    assert!(!items_of(&hits).is_empty(), "expected skeleton hits");
}

#[test]
fn dex_only_endpoints_fail_cleanly() {
    let p = project();
    for endpoint in ["get_method_cfg", "get_method_xref"] {
        let err = api::dispatch(&p, endpoint, &json!({ "mth": "demo.StdSample#compute" }))
            .err()
            .unwrap_or_else(|| panic!("{endpoint} should fail on java targets"));
        assert_eq!(
            err.code, "UNSUPPORTED_FOR_TARGET",
            "{endpoint}: {}",
            err.message
        );
    }
}

#[test]
fn debug_android_jar_flags() {
    let aj = std::path::Path::new("C:/Users/zhangyuchao/AppData/Local/Android/Sdk/platforms/android-35/android.jar");
    if !aj.exists() { return; }
    let p = Project::open(aj).expect("open android.jar");
    let entry = p.lookup("android.app.Activity").expect("Activity entry");
    let class = p.java_class(entry).expect("decoded class");
    println!("Activity access_flags = 0x{:04x}", class.access_flags);
    let b = p.lookup("demo.Base");
    let _ = b;
}
