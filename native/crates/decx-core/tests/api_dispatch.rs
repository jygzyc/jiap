//! API-level integration tests: run the full `dispatch` surface against the
//! bundled testdata dex (5920 classes, androidx/support). Slow-but-global
//! sweeps (warm/search over the whole archive) are #[ignore]d.
//!
//! Response contract (see `envelope.rs`): every dispatch returns
//! `{ ok, kind, query, summary: { total, returned, truncated, .. },
//!    items: [ { id, kind, title, content?, meta? } ], page: { .. } }`.

use decx_core::{api, Project};
use serde_json::{json, Value};

fn project() -> Project {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/classes.dex");
    Project::open(&path).expect("open testdata dex")
}

fn dispatch_ok(p: &Project, endpoint: &str, body: Value) -> Value {
    api::dispatch(p, endpoint, &body)
        .unwrap_or_else(|e| panic!("{endpoint} failed: {e}"))
}

fn items_of(v: &Value) -> &[Value] {
    v["items"].as_array().expect("items array")
}

#[test]
fn get_classes_lists_and_filters() {
    let p = project();
    // no limit: summary.total is the full class count
    let all = dispatch_ok(&p, "get_classes", json!({}));
    assert_eq!(all["ok"].as_bool(), Some(true));
    assert_eq!(all["summary"]["total"].as_u64(), Some(5920));
    assert!(!items_of(&all).is_empty());

    // with limit: results are capped (summary.total reflects the cap)
    let limited = dispatch_ok(&p, "get_classes", json!({ "filter": { "limit": 10 } }));
    assert_eq!(limited["summary"]["total"].as_u64(), Some(10));
    assert_eq!(items_of(&limited).len(), 10);

    let filtered = dispatch_ok(
        &p,
        "get_classes",
        json!({ "filter": { "limit": 10, "includes": ["^androidx\\.activity\\."] } }),
    );
    let items = items_of(&filtered);
    assert!(!items.is_empty());
    for item in items {
        assert!(item["id"].as_str().unwrap().starts_with("androidx.activity."));
    }
}

#[test]
fn class_source_and_context() {
    let p = project();
    let cls = "androidx.activity.result.ActivityResultRegistry";
    let src = dispatch_ok(&p, "get_class_source", json!({ "cls": cls }));
    let item = &items_of(&src)[0];
    let text = item["content"].as_str().expect("source content");
    assert!(text.contains("package androidx.activity.result;"), "bad source head");
    assert!(text.contains("class ActivityResultRegistry"));
    assert_eq!(item["meta"]["language"].as_str(), Some("java"));

    // filter.limit truncates the returned lines
    let truncated = dispatch_ok(
        &p,
        "get_class_source",
        json!({ "cls": cls, "filter": { "limit": 3 } }),
    );
    let titem = &items_of(&truncated)[0];
    assert_eq!(titem["meta"]["returned_lines"].as_u64(), Some(3));
    assert!(titem["meta"]["total_lines"].as_u64().unwrap_or(0) > 3);

    let ctx = dispatch_ok(&p, "get_class_context", json!({ "cls": cls }));
    assert!(!items_of(&ctx).is_empty());
}

#[test]
fn method_source_and_cfg() {
    let p = project();
    let spec = "androidx.activity.OnBackPressedDispatcher#addCallback";
    let src = dispatch_ok(&p, "get_method_source", json!({ "mth": spec }));
    let content = items_of(&src)[0]["content"]
        .as_str()
        .expect("method source");
    assert!(content.contains("public"), "got: {}", &content[..100.min(content.len())]);

    let cfg = dispatch_ok(&p, "get_method_cfg", json!({ "mth": spec }));
    assert!(!items_of(&cfg).is_empty());
}

#[test]
fn xref_endpoints_return_results() {
    let p = project();
    let xref = dispatch_ok(
        &p,
        "get_method_xref",
        json!({ "mth": "androidx.activity.OnBackPressedCallback#handleOnBackPressed" }),
    );
    assert!(!items_of(&xref).is_empty(), "expected callers");

    let cxref = dispatch_ok(
        &p,
        "get_class_xref",
        json!({ "cls": "androidx.activity.OnBackPressedCallback" }),
    );
    assert!(!items_of(&cxref).is_empty(), "expected references");
}

#[test]
fn search_class_key_greps_source() {
    let p = project();
    let hits = dispatch_ok(
        &p,
        "search_class_key",
        json!({ "cls": "androidx.activity.OnBackPressedCallback", "key": "public", "grep": { "limit": 5 } }),
    );
    assert!(!items_of(&hits).is_empty());
}

#[test]
fn unknown_class_and_endpoint_error() {
    let p = project();
    let err = api::dispatch(&p, "get_class_source", &json!({ "cls": "does.not.Exist" }))
        .expect_err("expected class error");
    assert_eq!(err.code, "CLASS_NOT_FOUND");

    let err = api::dispatch(&p, "no_such_endpoint", &json!({})).expect_err("expected unknown");
    assert_eq!(err.code, "UNKNOWN_ENDPOINT");
}

/// Real APK path: multi-dex loading, manifest decode, strings table.
#[test]
fn apk_multidex_manifest_strings() {
    let apk = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor/rusty-dex/tests/multidex.apk");
    assert!(apk.exists(), "multidex.apk missing");
    let p = Project::open(&apk).expect("open multidex apk");
    assert!(p.entries().len() >= 2, "expected multiple dexes' classes");

    let first = &p.entries()[0];
    let src = p.class_source(first).expect("class source from apk");
    assert!(!src.is_empty());

    let manifest = p.app_manifest().expect("manifest");
    assert!(manifest.contains('<'), "got: {}", &manifest[..200.min(manifest.len())]);

    let strings = p.strings().expect("strings");
    assert!(!strings.is_empty());
}

/// Whole-archive sweeps: expensive (tens of seconds), run explicitly.
#[test]
#[ignore]
fn global_search_and_warm() {
    let p = project();
    let hits = dispatch_ok(
        &p,
        "search_global_key",
        json!({ "key": "OnBackPressedCallback", "search": { "limit": 2 } }),
    );
    assert!(!items_of(&hits).is_empty());

    let (ok, failed) = p.warm_all();
    println!("warm: {ok} ok, {} failed", failed.len());
    assert!(failed.len() < ok / 20, "too many failures: {failed:?}");
}
