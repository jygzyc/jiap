//! Unit tests ported from Mariana Trench `source/tests/*Test.cpp` concepts
//! (Kind, Rule, Sanitizer, CallGraph, Model) against our Rust taint solver.

use dex_decompiler::taint::{
    default_config, method_patterns, parse_mt_port, CallGraph, MethodIndex, Port, SolveOptions,
    TaintConfig,
};
use dex_decompiler::{parse_dex, Decompiler};

#[test]
fn kind_rule_matching_like_mt_rule_test() {
    let cfg = TaintConfig::from_json_str(
        r#"{
          "rules": [
            {"name": "Flow", "code": 2, "sources": ["Source"], "sinks": ["Sink"]},
            {"name": "RCE", "code": 1, "sources": ["ActivityUserInput"], "sinks": ["CodeExecution"]}
          ]
        }"#,
    )
    .unwrap();
    assert_eq!(cfg.matching_rules("Source", "Sink").len(), 1);
    assert!(cfg.matching_rules("Source", "CodeExecution").is_empty());
    assert_eq!(
        cfg.matching_rules("ActivityUserInput", "CodeExecution")
            .len(),
        1
    );
}

#[test]
fn sanitizer_clears_star_kinds_like_mt_sanitizer_test() {
    let cfg = TaintConfig::from_json_str(
        r#"{
          "sanitizers": [
            {"patterns": ["Origin.sanitize"], "kinds": ["*"]},
            {"patterns": ["hash"], "kinds": ["Source"]}
          ]
        }"#,
    )
    .unwrap();
    let san = cfg.find_sanitizer("com.foo.Origin.sanitize").unwrap();
    assert!(san.kinds.iter().any(|k| k == "*"));
    let kind_specific = cfg.find_sanitizer("MessageDigest.hash").unwrap();
    assert_eq!(kind_specific.kinds, vec!["Source".to_string()]);
}

#[test]
fn port_parsing_covers_mt_ports() {
    assert!(matches!(parse_mt_port("Return"), Port::Return));
    assert!(matches!(parse_mt_port("This"), Port::This));
    assert!(matches!(
        parse_mt_port("Argument(2)"),
        Port::Argument { index: 2 }
    ));
}

#[test]
fn method_patterns_from_mt_descriptors() {
    let p = method_patterns("Landroid/app/Activity;.getIntent:()Landroid/content/Intent;");
    assert!(p.iter().any(|s| s.contains("Activity.getIntent")));
}

#[test]
fn default_android_config_has_mt_aligned_rules() {
    let cfg = default_config();
    assert!(!cfg
        .matching_rules("ActivityUserInput", "CodeExecution")
        .is_empty());
    assert!(!cfg
        .matching_rules("ActivityUserInput", "SQLQuery")
        .is_empty());
    assert!(!cfg.matching_rules("DeviceId", "Logging").is_empty());
}

#[test]
fn call_graph_builds_on_sample_dex() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/classes.dex");
    let data = std::fs::read(path).expect("testdata/classes.dex");
    let dex = parse_dex(&data).unwrap();
    let index = MethodIndex::from_dexes(&[&dex]);
    let decompiler = Decompiler::new(&dex);
    let mut vf_cache = std::collections::HashMap::new();
    for m in &index.methods {
        if let Ok(owned) = decompiler.value_flow_analysis(&m.encoded) {
            vf_cache.insert(m.id, owned);
        }
    }
    let cg = CallGraph::build_from_vf_cache(&index, &vf_cache, |_| true).unwrap();
    assert!(cg.edge_count() > 0 || index.methods.len() < 5);
}

#[test]
fn solve_options_exclude_framework() {
    let opts = SolveOptions::default_android();
    assert!(opts
        .exclude_prefixes
        .iter()
        .any(|p| p.starts_with("android.")));
}
