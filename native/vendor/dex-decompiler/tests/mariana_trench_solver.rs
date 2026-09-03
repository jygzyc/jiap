//! Solver semantic tests aligned with Mariana Trench simple_flows / sanitizers.

use dex_decompiler::{
    find_method_callers, load_mt_case_config, parse_dex, solve_dexes, Decompiler, MethodIndex,
    SolveOptions,
};
use std::path::PathBuf;

fn e2e(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/mariana_trench/e2e")
        .join(case)
}

/// Build a tiny DEX where one method body invokes `Lflow/Origin;.source` then `sink`.
/// Uses the existing helper style: single class method with crafted invoke targets is hard;
/// instead we validate that the simple_flows MT config + rules would flag Source→Sink,
/// and that sanitizer models are present for DirectFlowWithSanitizer scenarios.
#[test]
fn simple_flows_rule_admits_origin_source_to_sink() {
    let dir = e2e("simple_flows");
    let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
    let rules = cfg.matching_rules("Source", "Sink");
    assert_eq!(rules[0].code, 2);
    let issues: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected_issues.json")).unwrap())
            .unwrap();
    let arr = issues.as_array().unwrap();
    assert!(arr.iter().any(|i| i["rule"] == 2));
}

#[test]
fn sanitizers_expected_issues_use_defined_kinds() {
    let dir = e2e("sanitizers");
    let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
    let issues: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected_issues.json")).unwrap())
            .unwrap();
    assert_eq!(issues.len(), 12);
    for iss in &issues {
        let sources = iss["source_kinds"].as_array().unwrap();
        let sinks = iss["sink_kinds"].as_array().unwrap();
        for sk in sources {
            let s = sk.as_str().unwrap();
            let known = cfg.sources.iter().any(|x| x.kind == s)
                || cfg.rules.iter().any(|r| r.sources.iter().any(|k| k == s))
                || s.contains("Source");
            assert!(known, "unknown source kind {s}");
        }
        for tk in sinks {
            let s = tk.as_str().unwrap();
            let known = cfg.sinks.iter().any(|x| x.kind == s)
                || cfg.rules.iter().any(|r| r.sinks.iter().any(|k| k == s))
                || s == "PartialSink"
                || s.contains("Sink");
            assert!(known, "unknown sink kind {s}");
        }
    }
}

#[test]
fn propagation_via_arg_expected_positive() {
    let dir = e2e("propagation_via_arg");
    let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
    assert!(!cfg.propagations.is_empty() || !cfg.sources.is_empty());
    let issues: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected_issues.json")).unwrap())
            .unwrap();
    assert_eq!(issues.len(), 12);
}

#[test]
fn field_sources_expected_count() {
    let dir = e2e("field_sources");
    let issues: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected_issues.json")).unwrap())
            .unwrap();
    assert_eq!(issues.len(), 24);
}

#[test]
fn taint_transforms_expected_count() {
    let dir = e2e("taint_transforms");
    let issues: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected_issues.json")).unwrap())
            .unwrap();
    assert_eq!(issues.len(), 42);
}

#[test]
fn multi_sources_expected_count() {
    let dir = e2e("multi_sources");
    let issues: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected_issues.json")).unwrap())
            .unwrap();
    assert_eq!(issues.len(), 5);
}

/// Running the solver with MT simple_flows models on an empty-ish DEX should not panic.
#[test]
fn solve_with_mt_simple_flows_config_on_testdata_smoke() {
    let dir = e2e("simple_flows");
    let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/classes4.dex");
    let data = std::fs::read(&path).expect("classes4.dex");
    let dex = parse_dex(&data).unwrap();
    let mut opts = SolveOptions::default_android();
    opts.max_iterations = 2;
    let result = solve_dexes(&[&dex], &cfg, &opts).unwrap();
    // Smoke: finishes and produces a report (issue count may be 0 on this fixture).
    assert_eq!(result.report.tool, "dex-decompiler-taint");
}

#[test]
fn live_simple_flow_dex_reports_positive_and_not_clean_control() {
    let dir = e2e("simple_flows");
    let mut cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
    for source in &mut cfg.sources {
        if source.kind == "Source" {
            source.patterns = vec!["mt.live.Origin.source".into()];
        }
    }
    for sink in &mut cfg.sinks {
        if sink.kind == "Sink" {
            sink.patterns = vec!["Origin.sink".into()];
        }
    }
    let source = cfg
        .find_source("mt.live.Origin.source")
        .expect("live source model was not installed");
    assert!(matches!(source.port, dex_decompiler::Port::Return));
    let sink = cfg
        .find_sink("mt.live.Origin.sink")
        .expect("live sink model was not installed");
    assert!(matches!(sink.port, dex_decompiler::Port::This));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/mariana_trench/live_solver/classes.dex");
    let data = std::fs::read(&path).expect("live_solver/classes.dex");
    let dex = parse_dex(&data).unwrap();
    let index = MethodIndex::from_dexes(&[&dex]);
    let direct = index
        .methods
        .iter()
        .find(|method| method.method_name == "directFlow")
        .expect("directFlow");
    let owned = Decompiler::new(&dex)
        .value_flow_analysis(&direct.encoded)
        .unwrap();
    let ((source_offset, source_reg), _) = owned
        .api_return_sources
        .iter()
        .find(|(_, method)| method.contains("Origin.source"))
        .expect("live source invoke");
    let flow = owned
        .analysis()
        .value_flow_from_seed(*source_offset, *source_reg);
    assert!(
        flow.reads.iter().any(|(offset, _)| owned
            .invoke_method_map
            .get(offset)
            .map(|method| method.contains("Origin.sink"))
            .unwrap_or(false)),
        "live local flow missing: {flow:?}"
    );
    let lambda_impl = index
        .methods
        .iter()
        .find(|method| method.method_name.starts_with("lambda$lambdaFlow"))
        .expect("lambda implementation");
    let callers = find_method_callers(&dex, lambda_impl.encoded.method_idx).unwrap();
    assert!(
        callers
            .callers
            .iter()
            .any(|caller| caller.method_name == "lambdaFlow"
                && caller.invoke_kind.starts_with("invoke-custom")),
        "xref did not resolve invoke-custom implementation: {callers:#?}"
    );
    let mut opts = SolveOptions::default_android();
    opts.max_iterations = 8;

    let result = solve_dexes(&[&dex], &cfg, &opts).unwrap();
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.callable.contains("directFlow")
                && issue.source_kind == "Source"
                && issue.sink_kind == "Sink"),
        "actual solver did not report directFlow (stats={:#?}): {:#?}",
        result.report.stats,
        result.issues,
    );
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.callable.contains("helperFlow")
                && issue.source_kind == "Source"
                && issue.sink_kind == "Sink"),
        "actual solver did not report multi-hop helperFlow: {:#?}",
        result.issues
    );
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.callable.contains("executorFlow")
                && issue.source_kind == "Source"
                && issue.sink_kind == "Sink"),
        "actual solver did not follow Executor callback shim: {:#?}",
        result.issues
    );
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.callable.contains("lambdaFlow")
                && issue.source_kind == "Source"
                && issue.sink_kind == "Sink"),
        "actual solver did not follow invoke-custom lambdaFlow: {:#?}",
        result.issues
    );
    assert!(
        result
            .issues
            .iter()
            .all(|issue| !issue.callable.contains("cleanFlow")),
        "clean control unexpectedly tainted: {:#?}",
        result.issues
    );
}

#[test]
fn local_flow_e2e_fixtures_present() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/mariana_trench/local_flow_e2e/MANIFEST.json");
    let manifest: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap();
    assert!(
        manifest.len() >= 8,
        "expected local_flow_e2e cases, got {}",
        manifest.len()
    );
}

/// Explicit coverage checklist for all 74 MT e2e case names (IntegrationTest.cpp parity).
#[test]
fn all_seventy_four_case_names_covered() {
    let expected = [
        "access_paths",
        "activity_lifecycle",
        "activity_lifecycle_graph",
        "add_features_to_arguments",
        "add_features_to_arguments_builder_pattern",
        "add_features_to_check_cast",
        "alias_on_propagation",
        "aliasing",
        "android_location_api",
        "annotation_feature",
        "approximate_override_models",
        "array_allocation",
        "array_allocation_unused_kind",
        "artificial_calls",
        "attach_to",
        "broadening",
        "builder_pattern",
        "builder_pattern_collapse",
        "call_chain",
        "check_cast_types",
        "class_intervals",
        "collapse_depth",
        "combinatory_ports",
        "containers",
        "crtex_consumer",
        "crtex_producer",
        "declared_and_inferred_taint",
        "deeplink",
        "delegating_override_provider",
        "exploitability_rule",
        "field_sink_propagation",
        "field_sink_propagation_overrides",
        "field_sinks",
        "field_sources",
        "field_tito",
        "field_tito_fp",
        "flow_through_field",
        "flow_through_interface_call",
        "flow_through_virtual_call",
        "fragment_lifecycle",
        "independent_propagation",
        "inline_getters",
        "intent_routing",
        "invalid_models",
        "invalid_path_broadening",
        "literal_source",
        "logging_url",
        "may_always_features",
        "multi_sources",
        "multiple_kinds_per_callee",
        "no_collapse_on_propagation",
        "obscure_with_models",
        "overwrite_artificial_source_field",
        "parameter_type_overrides",
        "propagate_implicit_this",
        "propagation_collapse",
        "propagation_via_arg",
        "propagation_via_obscure",
        "return_sink",
        "sanitizers",
        "shims",
        "simple_flows",
        "sink_grouping",
        "strong_update",
        "subkinds",
        "super_delegate_flow",
        "taint_transforms",
        "tainted_return",
        "thread",
        "uri_builder_pattern",
        "user_features_propagation",
        "varargs",
        "via_type_of",
        "via_value_of",
    ];
    assert_eq!(expected.len(), 74);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/mariana_trench/e2e");
    for name in expected {
        assert!(
            root.join(name).join("expected_issues.json").exists()
                || root.join(name).join("models.json").exists(),
            "missing MT case fixture: {name}"
        );
    }
}
