//! Native Semgrep tests: loaders, matcher, XML/Java rules, DEX scan e2e.

use dex_decompiler::semgrep::{
    builtin_android_rules, default_android_rule_paths, java_matches_pattern, load_android_rules,
    load_rules_from_dir, load_rules_from_str, load_rules_from_yaml_file, rule_matches_source,
    scan_dex_semgrep, scan_xml_semgrep, NativeKind, Severity, ANDROID_GENERAL_RULES_YAML,
};
use dex_decompiler::{parse_dex, DexFile};
use std::path::{Path, PathBuf};

fn mastg_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/semgrep/android/mastg")
}

fn general_rules_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/semgrep/android/general.yml")
}

fn androguard_dex() -> DexFile {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/androguard_test_classes.dex");
    assert!(path.exists(), "missing {}", path.display());
    let data = std::fs::read(&path).expect("read androguard_test_classes.dex");
    parse_dex(&data).expect("parse androguard_test_classes.dex")
}

// --- rule loading ----------------------------------------------------------------

#[test]
fn builtin_includes_general_and_mastg() {
    let rules = builtin_android_rules();
    assert!(
        rules.len() >= 50,
        "expected general (4) + MASTG Android rules, got {}",
        rules.len()
    );
    let ids: Vec<_> = rules.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&"android.webview.loadurl-from-intent"));
    assert!(ids.contains(&"android.webview.js-interface-added"));
    assert!(ids.contains(&"android.intent.redirect"));
    assert!(ids.contains(&"android.ssl.bypass-handler-proceed"));
    assert!(
        ids.iter()
            .any(|id| id.starts_with("mastg-android-") || id.starts_with("detect-")),
        "expected OWASP MASTG rule ids, sample={:?}",
        &ids[..ids.len().min(8)]
    );
}

#[test]
fn default_android_rule_paths_exist() {
    let paths = default_android_rule_paths();
    assert!(!paths.is_empty(), "expected default rule paths");
    assert!(
        paths[0].is_file(),
        "primary rules YAML missing: {}",
        paths[0].display()
    );
    if paths.len() >= 2 {
        assert!(
            paths[1].is_dir(),
            "MASTG dir missing: {}",
            paths[1].display()
        );
    }
}

#[test]
fn load_android_rules_none_file_and_dir() {
    let builtin = load_android_rules(None).unwrap();
    assert!(builtin.len() >= 50);

    let from_file = load_android_rules(Some(&general_rules_path())).unwrap();
    assert!(
        from_file.len() >= 4,
        "general.yml should include the original native rules, got {}",
        from_file.len()
    );

    let from_dir = load_android_rules(Some(&mastg_dir())).unwrap();
    assert!(from_dir.len() >= 40);
    assert!(from_dir
        .iter()
        .all(|r| !r.id.starts_with("android.webview.")));
}

#[test]
fn load_rules_from_yaml_file_general() {
    let rules = load_rules_from_yaml_file(&general_rules_path()).unwrap();
    assert!(
        rules.len() >= 4,
        "general.yml should include the original native rules, got {}",
        rules.len()
    );
}

#[test]
fn mastg_dir_loads_every_yaml_file() {
    let dir = mastg_dir();
    let yaml_files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "yml" || e == "yaml")
        })
        .collect();
    assert!(
        yaml_files.len() >= 40,
        "expected many MASTG YAML files, got {}",
        yaml_files.len()
    );

    let mut failed = Vec::new();
    let mut rule_count = 0;
    for path in &yaml_files {
        match load_rules_from_yaml_file(path) {
            Ok(rs) => {
                assert!(!rs.is_empty(), "{} produced zero rules", path.display());
                rule_count += rs.len();
            }
            Err(e) => failed.push(format!("{}: {}", path.display(), e)),
        }
    }
    assert!(
        failed.is_empty(),
        "MASTG YAML parse failures:\n{}",
        failed.join("\n")
    );
    assert!(
        rule_count >= 40,
        "expected >=40 MASTG rules, got {rule_count}"
    );

    let rules = load_rules_from_dir(&dir).expect("load MASTG dir");
    assert_eq!(rules.len(), rule_count);
    assert!(rules.iter().any(|r| r.applies_to_xml()));
    assert!(rules
        .iter()
        .any(|r| r.applies_to_java() && !r.applies_to_xml()));
}

#[test]
fn each_general_rule_has_native_hint() {
    let rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML).unwrap();
    assert!(
        rules.len() >= 4,
        "embedded general rules should include the original native set, got {}",
        rules.len()
    );
    for rule in &rules {
        assert!(
            rule.native.is_some(),
            "rule {} should have native: for DEX/SSA matching",
            rule.id
        );
    }
}

#[test]
fn yaml_roundtrip_from_embedded() {
    let rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML).unwrap();
    assert!(
        rules.len() >= 4,
        "embedded general rules should include the original native set, got {}",
        rules.len()
    );
    let ssl = rules
        .iter()
        .find(|r| r.id == "android.ssl.bypass-handler-proceed")
        .unwrap();
    let native = ssl.native.as_ref().unwrap();
    assert!(matches!(native.kind, NativeKind::MethodInvoke));
    assert_eq!(native.method_name.as_deref(), Some("onReceivedSslError"));
    assert_eq!(ssl.severity, Severity::Error);
    assert_eq!(ssl.metadata.vuln_class.as_deref(), Some("network"));
}

#[test]
fn parse_custom_rule_with_pattern_regex_and_either() {
    let yaml = r#"
rules:
  - id: test.regex.ptrace
    message: ptrace attach
    severity: WARNING
    languages: [java]
    pattern-regex: "PTRACE_(ATTACH|SEIZE)"
  - id: test.either.foo
    message: either
    severity: INFO
    languages: [java]
    pattern-either:
      - pattern: foo()
      - pattern: bar()
"#;
    let rules = load_rules_from_str(yaml).unwrap();
    assert_eq!(rules.len(), 2);
    assert_eq!(
        rules[0].pattern_regexes(),
        vec!["PTRACE_(ATTACH|SEIZE)".to_string()]
    );
    assert!(rule_matches_source(&rules[0], "x = PTRACE_ATTACH;").is_some());
    assert_eq!(
        rule_matches_source(&rules[0], "x = PTRACE_ATTACH;"),
        Some("java_regex")
    );
    assert!(rule_matches_source(&rules[0], "no match here").is_none());

    assert!(!rules[1].patterns_are_conjunction());
    assert!(rule_matches_source(&rules[1], "void m() { foo(); }").is_some());
    assert!(rule_matches_source(&rules[1], "void m() { bar(); }").is_some());
    assert!(rule_matches_source(&rules[1], "void m() { baz(); }").is_none());
}

#[test]
fn language_filters_java_vs_xml() {
    let yaml = r#"
rules:
  - id: only.xml
    languages: [xml]
    pattern: 'android:debuggable="$ARG"'
  - id: only.java
    languages: [java]
    pattern: System.out.println($X);
  - id: generic.perm
    languages: [generic]
    pattern: WRITE_EXTERNAL_STORAGE
"#;
    let rules = load_rules_from_str(yaml).unwrap();
    assert!(rules[0].applies_to_xml() && !rules[0].applies_to_java());
    assert!(rules[1].applies_to_java() && !rules[1].applies_to_xml());
    assert!(rules[2].applies_to_java() && rules[2].applies_to_xml());

    let xml = r#"<application android:debuggable="true"/>"#;
    let java = r#"void f() { System.out.println("hi"); WRITE_EXTERNAL_STORAGE; }"#;
    let xml_hits = scan_xml_semgrep(xml, "m.xml", &rules);
    let xml_ids: Vec<_> = xml_hits.iter().map(|f| f.rule_id.as_str()).collect();
    assert!(xml_ids.contains(&"only.xml"));
    assert!(!xml_ids.contains(&"only.java"));
    assert!(
        rule_matches_source(&rules[1], java).is_some(),
        "java-only rule should match java source"
    );
    assert!(rule_matches_source(&rules[2], "WRITE_EXTERNAL_STORAGE").is_some());
}

// --- Java pattern matcher --------------------------------------------------------

#[test]
fn java_pattern_js_interface_rule() {
    let rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML).unwrap();
    let rule = rules
        .iter()
        .find(|r| r.id == "android.webview.js-interface-added")
        .unwrap();
    let java = r#"
        public class Main {
            void setup(WebView wv, Object bridge) {
                wv.addJavascriptInterface(bridge, "Android");
            }
        }
    "#;
    let matched = rule
        .pattern_strings()
        .iter()
        .any(|p| java_matches_pattern(java, p));
    assert!(
        matched,
        "JS interface pattern should match decompiled-like Java"
    );
}

#[test]
fn java_pattern_ssl_bypass_rule() {
    let rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML).unwrap();
    let rule = rules
        .iter()
        .find(|r| r.id == "android.ssl.bypass-handler-proceed")
        .unwrap();
    let java = r#"
        public void onReceivedSslError(WebView v, SslErrorHandler handler, SslError error) {
            handler.proceed();
        }
    "#;
    let matched = rule
        .pattern_strings()
        .iter()
        .any(|p| java_matches_pattern(java, p));
    assert!(matched, "SSL bypass pattern should match");
}

#[test]
fn java_pattern_loadurl_intent() {
    let rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML).unwrap();
    let rule = rules
        .iter()
        .find(|r| r.id == "android.webview.loadurl-from-intent")
        .unwrap();
    let java = "wv.loadUrl((String) intent.getStringExtra(\"u\"));";
    let matched = rule
        .pattern_strings()
        .iter()
        .any(|p| java_matches_pattern(java, p));
    assert!(matched, "loadUrl from Intent extra should match");
}

#[test]
fn java_pattern_intent_redirect() {
    let rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML).unwrap();
    let rule = rules
        .iter()
        .find(|r| r.id == "android.intent.redirect")
        .unwrap();
    let java = r#"
        Intent i = (Intent) getIntent().getParcelableExtra("next");
        startActivity(i);
    "#;
    let matched = rule
        .pattern_strings()
        .iter()
        .any(|p| java_matches_pattern(java, p));
    assert!(matched, "intent redirect pattern should match");
}

#[test]
fn java_pattern_underscore_wildcard() {
    assert!(java_matches_pattern(
        r#"wv.addJavascriptInterface(bridge, "Android");"#,
        "$WV.addJavascriptInterface($_, $NAME);"
    ));
    assert!(!java_matches_pattern(
        r#"wv.addJavascriptInterface(bridge, "Android");"#,
        "$WV.addJavascriptInterface($_, $_); $MISSING();"
    ));
}

#[test]
fn mastg_webview_bridge_pattern() {
    let rules = load_rules_from_dir(&mastg_dir()).unwrap();
    let rule = rules
        .iter()
        .find(|r| r.id == "mastg-android-webview-bridges-javascriptinterface")
        .expect("MASTG webview bridge rule");
    let java = r#"
        @JavascriptInterface
        public String getToken() {
            return secret;
        }
    "#;
    assert!(
        rule_matches_source(rule, java).is_some(),
        "MASTG @JavascriptInterface pattern should match"
    );
}

// --- XML scanning ----------------------------------------------------------------

#[test]
fn mastg_xml_debuggable_scan() {
    let rules = load_rules_from_dir(&mastg_dir()).unwrap();
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
  <application android:debuggable="true" android:allowBackup="true">
  </application>
</manifest>
"#;
    let findings = scan_xml_semgrep(xml, "AndroidManifest.xml", &rules);
    let ids: Vec<_> = findings.iter().map(|f| f.rule_id.as_str()).collect();
    assert!(
        ids.contains(&"mastg-android-debuggable-flag"),
        "expected debuggable finding, got {:?}",
        ids
    );
    assert!(
        ids.contains(&"mastg-android-backup-manifest-allow-backup"),
        "expected allowBackup finding, got {:?}",
        ids
    );
    let dbg = findings
        .iter()
        .find(|f| f.rule_id == "mastg-android-debuggable-flag")
        .unwrap();
    assert_eq!(dbg.class_name, "AndroidManifest.xml");
    assert_eq!(dbg.method_name, "(xml)");
    assert_eq!(dbg.match_kind, "java_pattern");
    assert!(!dbg.message.is_empty());
}

#[test]
fn xml_scan_skips_java_only_rules() {
    let rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML).unwrap();
    let xml = r#"<application android:debuggable="true"/>"#;
    let findings = scan_xml_semgrep(xml, "m.xml", &rules);
    assert!(
        findings.is_empty(),
        "Java-only rules must not fire on XML: {:?}",
        findings
    );
}

// --- DEX e2e ---------------------------------------------------------------------

#[test]
fn semgrep_skips_android_platform_classes() {
    let rules = builtin_android_rules();
    let dex = androguard_dex();
    let findings = scan_dex_semgrep(&dex, &rules);
    assert!(
        findings.iter().all(|f| {
            !f.class_name.starts_with("android.")
                && !f.class_name.starts_with("androidx.")
                && f.class_name != "android"
                && f.class_name != "androidx"
        }),
        "findings must not come from android.* / androidx.* classes: {:?}",
        findings
            .iter()
            .filter(|f| {
                f.class_name.starts_with("android.") || f.class_name.starts_with("androidx.")
            })
            .take(5)
            .map(|f| (&f.rule_id, &f.class_name))
            .collect::<Vec<_>>()
    );
}

#[test]
fn scan_dex_native_invoke_finds_testinvoke() {
    let yaml = r#"
rules:
  - id: test.dex.invoke-testinvoke1
    message: TestInvoke1 call site
    severity: WARNING
    languages: [java]
    metadata:
      vuln_class: test
      chain_tag: e2e
    native:
      kind: invoke
      methods:
        - TestInvoke1
"#;
    let rules = load_rules_from_str(yaml).unwrap();
    let dex = androguard_dex();
    let findings = scan_dex_semgrep(&dex, &rules);
    assert!(
        !findings.is_empty(),
        "expected native invoke findings for TestInvoke1"
    );
    assert!(findings
        .iter()
        .all(|f| f.rule_id == "test.dex.invoke-testinvoke1"));
    assert!(findings.iter().all(|f| f.match_kind == "native"));
    assert!(findings.iter().any(|f| f.class_name.contains("TestInvoke")));
    assert!(findings.iter().any(|f| f.sink_offset.is_some()));
    assert!(findings.iter().any(|f| f.sink_desc.contains("TestInvoke1")));

    let vf = findings[0].to_vuln_finding();
    assert!(vf.category.starts_with("semgrep:"));
    assert_eq!(vf.class_name, findings[0].class_name);
}

#[test]
fn scan_dex_java_pattern_rule_on_androguard() {
    // Pattern-only (no native:): match decompiled call sites.
    let yaml = r#"
rules:
  - id: test.dex.java-testinvoke2
    message: pattern TestInvoke2
    severity: INFO
    languages: [java]
    pattern: $R.TestInvoke2($A, $B);
"#;
    let rules = load_rules_from_str(yaml).unwrap();
    let dex = androguard_dex();
    let findings = scan_dex_semgrep(&dex, &rules);
    assert!(
        !findings.is_empty(),
        "expected Java-pattern findings for TestInvoke2 on androguard DEX"
    );
    assert!(findings.iter().any(|f| f.match_kind == "java_pattern"));
    assert!(findings.iter().any(|f| f.class_name.contains("TestInvoke")));
}

#[test]
fn scan_dex_general_rules_on_androguard_runs_clean() {
    // Fixture has no WebView/SSL sinks; ensure engine completes and returns zero general-rule hits.
    let rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML).unwrap();
    let dex = androguard_dex();
    let findings = scan_dex_semgrep(&dex, &rules);
    assert!(
        findings.is_empty(),
        "androguard test DEX should not match general Android rules, got {:?}",
        findings
            .iter()
            .map(|f| f.rule_id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn scan_dex_method_invoke_native_kind() {
    let yaml = r#"
rules:
  - id: test.dex.method-invoke-ctor
    message: ctor calls TestInvoke1
    severity: WARNING
    languages: [java]
    native:
      kind: method_invoke
      method_name: <init>
      methods:
        - TestInvoke1
"#;
    let rules = load_rules_from_str(yaml).unwrap();
    let dex = androguard_dex();
    let findings = scan_dex_semgrep(&dex, &rules);
    assert!(
        findings.iter().any(|f| {
            f.rule_id == "test.dex.method-invoke-ctor"
                && f.method_name == "<init>"
                && f.match_kind == "native"
                && f.class_name.contains("TestInvoke")
        }),
        "expected method_invoke finding in TestInvoke.<init>, got {:?}",
        findings
    );
}

#[test]
fn load_android_rules_missing_path_errors() {
    let err = load_android_rules(Some(Path::new("/no/such/semgrep-rules.yml"))).unwrap_err();
    assert!(
        err.contains("read") || err.contains("No such") || err.contains("failed"),
        "unexpected error: {err}"
    );
}
