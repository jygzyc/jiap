//! Mariana Trench end-to-end test suite port.
//!
//! Fixtures: `tests/data/mariana_trench/e2e/` — 74 cases from
//! facebook/mariana-trench `source/tests/integration/end-to-end/code`.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use dex_decompiler::{convert_models_json, convert_rules_json, load_mt_case_config};
use serde::Deserialize;

fn e2e_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/mariana_trench/e2e")
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    case: String,
    issue_count: usize,
    has_models: bool,
    has_rules: bool,
}

#[derive(Debug, Deserialize)]
struct ExpectedIssue {
    rule: Option<u32>,
    #[serde(default)]
    source_kinds: Vec<String>,
    #[serde(default)]
    sink_kinds: Vec<String>,
}

fn load_manifest() -> Vec<ManifestEntry> {
    let text = fs::read_to_string(e2e_root().join("MANIFEST.json")).expect("MANIFEST.json");
    serde_json::from_str(&text).expect("parse MANIFEST")
}

fn case_dir(name: &str) -> PathBuf {
    e2e_root().join(name)
}

#[test]
fn all_mt_cases_convert_models_and_rules() {
    let manifest = load_manifest();
    assert_eq!(manifest.len(), 74, "expected 74 Mariana Trench e2e cases");
    let mut converted = 0usize;
    for entry in &manifest {
        if !entry.has_models {
            continue;
        }
        let dir = case_dir(&entry.case);
        let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json"))
            .unwrap_or_else(|e| panic!("case {}: {e}", entry.case));
        assert!(
            !cfg.sources.is_empty()
                || !cfg.sinks.is_empty()
                || !cfg.propagations.is_empty()
                || !cfg.sanitizers.is_empty()
                || !cfg.rules.is_empty(),
            "case {} produced empty config",
            entry.case
        );
        converted += 1;
    }
    assert!(converted >= 70, "converted only {converted} cases");
}

#[test]
fn all_mt_expected_issues_catalogs() {
    let manifest = load_manifest();
    let mut total = 0usize;
    for entry in &manifest {
        let path = case_dir(&entry.case).join("expected_issues.json");
        if !path.exists() {
            assert_eq!(
                entry.issue_count, 0,
                "missing expected_issues for {}",
                entry.case
            );
            continue;
        }
        let issues: Vec<ExpectedIssue> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            issues.len(),
            entry.issue_count,
            "issue count mismatch for {}",
            entry.case
        );
        total += issues.len();
    }
    assert_eq!(total, 406, "Mariana Trench e2e expected 406 issues total");
}

#[test]
fn all_mt_expected_issues_have_matching_rules() {
    let manifest = load_manifest();
    for entry in &manifest {
        if entry.issue_count == 0 || !entry.has_rules {
            continue;
        }
        let dir = case_dir(&entry.case);
        let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
        if cfg.rules.is_empty() {
            continue;
        }
        let issues: Vec<ExpectedIssue> =
            serde_json::from_str(&fs::read_to_string(dir.join("expected_issues.json")).unwrap())
                .unwrap();
        for iss in &issues {
            let mut matched = false;
            for sk in &iss.source_kinds {
                for tk in &iss.sink_kinds {
                    if !cfg.matching_rules(sk, tk).is_empty() {
                        matched = true;
                    }
                }
            }
            if let Some(code) = iss.rule {
                if cfg.rules.iter().any(|r| r.code == code) {
                    matched = true;
                }
            }
            assert!(
                matched,
                "case {} issue rule={:?} sources={:?} sinks={:?} has no matching rule",
                entry.case, iss.rule, iss.source_kinds, iss.sink_kinds
            );
        }
    }
}

#[test]
fn simple_flows_models_include_origin() {
    let dir = case_dir("simple_flows");
    let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
    assert!(cfg
        .find_source("com.facebook.marianatrench.integrationtests.Origin.source")
        .is_some());
    assert!(cfg
        .find_sink("com.facebook.marianatrench.integrationtests.Origin.sink")
        .is_some());
    assert!(!cfg.matching_rules("Source", "Sink").is_empty());
    assert!(!cfg
        .matching_rules("ExternalUserInput", "LaunchingComponent")
        .is_empty());
}

#[test]
fn sanitizers_models_convert() {
    let dir = case_dir("sanitizers");
    let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
    assert!(!cfg.sanitizers.is_empty());
}

#[test]
fn call_chain_models_convert() {
    let dir = case_dir("call_chain");
    let cfg = load_mt_case_config(&dir.join("models.json"), &dir.join("rules.json")).unwrap();
    assert!(!cfg.rules.is_empty() || !cfg.sources.is_empty() || !cfg.sinks.is_empty());
}

#[test]
fn manifest_case_names_unique() {
    let manifest = load_manifest();
    let mut seen = HashSet::new();
    for e in &manifest {
        assert!(seen.insert(e.case.clone()), "duplicate case {}", e.case);
        assert!(case_dir(&e.case).is_dir());
    }
}

#[test]
fn convert_models_json_roundtrip_kinds() {
    let json = r#"[
      {
        "method": "Lcom/facebook/marianatrench/integrationtests/Origin;.source:()Ljava/lang/Object;",
        "generations": [{"kind": "Source", "port": "Return"}]
      },
      {
        "method": "Lcom/facebook/marianatrench/integrationtests/Origin;.sink:(Ljava/lang/Object;)V",
        "sinks": [{"kind": "Sink", "port": "Argument(0)"}]
      },
      {
        "method": "Landroid/content/Intent;.getData:()Landroid/net/Uri;",
        "propagation": [{"input": "Argument(0)", "output": "Return"}]
      }
    ]"#;
    let cfg = convert_models_json(json).unwrap();
    assert_eq!(cfg.sources.len(), 1);
    assert_eq!(cfg.sinks.len(), 1);
    assert_eq!(cfg.propagations.len(), 1);
}

#[test]
fn convert_rules_json_basic() {
    let json = r#"[
      {"name": "Flow", "code": 2, "description": "", "sources": ["Source"], "sinks": ["Sink"]}
    ]"#;
    let cfg = convert_rules_json(json).unwrap();
    assert_eq!(cfg.rules.len(), 1);
    assert_eq!(cfg.rules[0].code, 2);
}

/// Per-case presence test generated from MANIFEST — ensures fixtures are complete.
#[test]
fn every_case_has_expected_issues_file_or_zero() {
    for entry in load_manifest() {
        let exp = case_dir(&entry.case).join("expected_issues.json");
        if entry.issue_count > 0 {
            assert!(exp.is_file(), "{} missing expected_issues.json", entry.case);
        }
        if entry.has_models {
            assert!(
                case_dir(&entry.case).join("models.json").is_file(),
                "{} missing models.json",
                entry.case
            );
        }
    }
}
