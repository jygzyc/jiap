//! Tests for the Mariana-Trench–style taint solver.
//!
//! CHA dispatch (`resolve_callees`) is covered by unit tests in `src/taint/index.rs`
//! with a mocked `MethodIndex` (Base/Child + library-skip). Building a two-class DEX
//! just to flow taint through `Child.foo` via a `Base` invoke is heavier than the
//! extra coverage it would add.

use dex_decompiler::taint::Port;
use dex_decompiler::{default_config, TaintConfig};

#[test]
fn default_config_parses_and_has_rules() {
    let cfg = default_config();
    assert!(!cfg.sources.is_empty());
    assert!(!cfg.sinks.is_empty());
    assert!(!cfg.propagations.is_empty());
    assert!(!cfg.rules.is_empty());
    assert!(cfg.find_source("android.app.Activity.getIntent").is_some());
    assert!(cfg.find_sink("java.lang.Runtime.exec").is_some());
    let rules = cfg.matching_rules("ActivityUserInput", "CodeExecution");
    assert!(!rules.is_empty());
}

#[test]
fn merge_configs() {
    let mut base = default_config();
    let extra = TaintConfig::from_json_str(
        r#"{
          "sources": [{"patterns": ["MyApi.secret"], "kind": "CustomSource"}],
          "sinks": [{"patterns": ["MyApi.leak"], "port": {"argument": {"index": 0}}, "kind": "CustomSink"}],
          "rules": [{"name": "custom", "code": 99, "sources": ["CustomSource"], "sinks": ["CustomSink"]}]
        }"#,
    )
    .unwrap();
    base.merge(extra);
    assert!(base.find_source("com.foo.MyApi.secret").is_some());
    assert!(base.find_sink("com.foo.MyApi.leak").is_some());
    assert!(base.rules.iter().any(|r| r.code == 99));
}

#[test]
fn sanitizer_and_propagation_lookup() {
    let cfg = default_config();
    assert!(cfg.find_sanitizer("MessageDigest.digest").is_some());
    assert!(!cfg.find_propagations("StringBuilder.append").is_empty());
    assert!(!cfg
        .find_propagations("java.lang.String.getBytes()[B")
        .is_empty());
}

#[test]
fn tightened_default_source_sink_patterns() {
    let cfg = default_config();
    assert!(cfg.find_source("android.app.Activity.getIntent").is_some());
    assert!(cfg.find_sink("java.lang.Runtime.exec").is_some());

    assert!(cfg.find_source("android.widget.EditText.getText").is_some());
    assert!(
        cfg.find_source("android.widget.TextView.getText").is_none(),
        "bare/TextView getText must not be a UserInput source"
    );
    assert!(cfg
        .find_source("android.content.ClipboardManager.getText")
        .is_some());

    assert!(cfg.find_sink("android.webkit.WebView.loadUrl").is_some());
    assert!(
        cfg.find_sink("com.foo.MyWebViewClient.loadUrl").is_none(),
        "unqualified loadUrl must not be ExecuteJavascript"
    );

    assert!(cfg
        .find_source("android.content.ContentResolver.query")
        .is_some());
    assert!(
        cfg.find_source("com.foo.DbHelper.query").is_none(),
        "bare query must not be a ProviderUserInput source"
    );
    assert!(cfg
        .find_sources(
            "com.foo.Receiver.onReceive(Landroid/content/Context;Landroid/content/Intent;)V"
        )
        .iter()
        .any(|s| {
            s.kind == "ReceiverUserInput" && matches!(s.port, Port::Argument { index: 2 })
        }));
    assert!(
        cfg.find_sources("com.foo.Helper.onReceiveEvent(Ljava/lang/String;)V")
            .is_empty(),
        "callback patterns must require an exact method boundary"
    );
    assert!(cfg
        .find_source("android.os.PersistableBundle.getString")
        .is_some());
}

#[test]
fn default_sanitizers_are_breadcrumb_noops() {
    let cfg = default_config();
    let san = cfg
        .find_sanitizer("MessageDigest.digest")
        .expect("default sanitizer");
    assert!(
        san.kinds.is_empty(),
        "default sanitizers must stay kinds=[] (do not hash DeviceId away)"
    );
}

#[test]
fn solve_twice_is_deterministic_on_sample_dex() {
    use dex_decompiler::{parse_dex, solve_dexes, SolveOptions};

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/classes4.dex");
    let data = std::fs::read(path).expect("classes4.dex");
    let dex = parse_dex(&data).unwrap();
    let cfg = default_config();
    let mut opts = SolveOptions::default_android();
    opts.max_iterations = 3;

    let a = solve_dexes(&[&dex], &cfg, &opts).unwrap();
    let b = solve_dexes(&[&dex], &cfg, &opts).unwrap();
    let keys = |r: &dex_decompiler::SolveResult| {
        let mut v: Vec<String> = r
            .issues
            .iter()
            .map(|i| {
                format!(
                    "{}:{}:{}:{}",
                    i.rule_code, i.callable, i.source_kind, i.sink_kind
                )
            })
            .collect();
        v.sort();
        v
    };
    assert_eq!(keys(&a), keys(&b));

    // 1-hop arg→param is found without needing the full default iteration budget.
    let mut opts8 = opts.clone();
    opts8.max_iterations = 8;
    let c = solve_dexes(&[&dex], &cfg, &opts8).unwrap();
    assert_eq!(keys(&a), keys(&c));
}
