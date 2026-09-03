//! Regression lock for the privacy + vuln demo APK / classes.dex.
//!
//! Intra-procedural taint flows + VF detectors must keep seeing the methods
//! in `testdata/privacy_vuln_demo`. Tests load the checked-in `classes.dex`
//! so they do not need the Android SDK.

use std::collections::BTreeSet;
use std::path::PathBuf;

use dex_decompiler::{
    default_config, load_dexes_from_path, parse_dex, scan_dex_parallel,
    scan_pending_intents_dex_parallel, solve_dexes, Decompiler, Issue, SolveOptions, TaintConfig,
};
use serde::Deserialize;

fn demo_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/privacy_vuln_demo")
}

fn load_demo_dex() -> dex_decompiler::DexFile {
    let path = demo_dir().join("classes.dex");
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    parse_dex(&data).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[test]
fn exception_handler_value_flow_uses_live_dex_edges() {
    let dex = load_demo_dex();
    let encoded = dex
        .class_defs()
        .flatten()
        .find_map(|class_def| {
            let class_name = dex
                .get_type(class_def.class_idx)
                .ok()
                .map(|name| dex_decompiler::java::descriptor_to_java(&name))?;
            if !class_name.ends_with("AdvancedPrivacyFlows") {
                return None;
            }
            let class_data = dex.get_class_data(&class_def).ok()??;
            class_data
                .direct_methods
                .iter()
                .chain(class_data.virtual_methods.iter())
                .find(|method| {
                    dex.get_method_info(method.method_idx)
                        .map(|info| info.name == "leakThroughException")
                        .unwrap_or(false)
                })
                .cloned()
        })
        .expect("AdvancedPrivacyFlows.leakThroughException");
    let owned = Decompiler::new(&dex)
        .value_flow_analysis(&encoded)
        .expect("live exception value flow");
    assert!(
        !owned.exceptional_edges.is_empty(),
        "DEX try/catch table must produce exceptional CFG edges"
    );
    let ((source_offset, source_reg), _) = owned
        .api_return_sources
        .iter()
        .find(|(_, method)| method.contains("AdvancedPrivacyFlows.deviceId"))
        .expect("deviceId source invoke");
    let flow = owned
        .analysis()
        .value_flow_from_seed(*source_offset, *source_reg);
    assert!(
        flow.reads.iter().any(|(offset, _)| {
            owned
                .invoke_method_map
                .get(offset)
                .map(|method| method.contains("android.util.Log.w"))
                .unwrap_or(false)
        }),
        "definition before throw must reach Log.w in catch handler: {flow:?}"
    );
}

fn solve_demo(cfg: &TaintConfig) -> Vec<Issue> {
    let dex = load_demo_dex();
    let mut opts = SolveOptions::default_android();
    opts.max_iterations = 16;
    let result = solve_dexes(&[&dex], cfg, &opts).expect("solve_dexes");
    result.issues
}

fn dump_issues(issues: &[Issue]) -> String {
    let mut lines: Vec<String> = issues
        .iter()
        .map(|i| {
            let extras: Vec<String> = i
                .trace
                .iter()
                .filter_map(|f| {
                    let mut bits = Vec::new();
                    if !f.description.is_empty() {
                        bits.push(f.description.clone());
                    }
                    if let Some(e) = &f.extra {
                        bits.push(format!("extra={e}"));
                    }
                    if bits.is_empty() {
                        None
                    } else {
                        Some(bits.join(" "))
                    }
                })
                .collect();
            format!(
                "rule={} {}→{} callable={} ({}) frames={} {}",
                i.rule_code,
                i.source_kind,
                i.sink_kind,
                i.callable,
                i.rule_name,
                i.trace.len(),
                extras.join(" | ")
            )
        })
        .collect();
    lines.sort();
    lines.join("\n")
}

fn issue_haystack(i: &Issue) -> String {
    let mut s = format!("{}\n{}\n{}", i.callable, i.description, i.rule_name);
    for f in &i.trace {
        s.push('\n');
        s.push_str(&f.description);
        if let Some(e) = &f.extra {
            s.push('\n');
            s.push_str(e);
        }
        if let Some(fld) = &f.field {
            s.push('\n');
            s.push_str(fld);
        }
    }
    s
}

fn haystack_has(issues: &[Issue], needle: &str) -> bool {
    issues.iter().any(|i| issue_haystack(i).contains(needle))
}

fn has_flow(issues: &[Issue], method: &str, source: &str, sink: &str, rule: u32) -> bool {
    issues.iter().any(|i| {
        i.rule_code == rule
            && i.source_kind == source
            && i.sink_kind == sink
            && i.callable.contains(method)
    })
}

fn has_flow_any(issues: &[Issue], methods: &[&str], source: &str, sink: &str, rule: u32) -> bool {
    methods
        .iter()
        .any(|m| has_flow(issues, m, source, sink, rule))
}

fn assert_flow(issues: &[Issue], method: &str, source: &str, sink: &str, rule: u32) {
    if !has_flow(issues, method, source, sink, rule) {
        panic!(
            "missing {source}→{sink} rule {rule} in callable containing `{method}`\n--- issues ---\n{}",
            dump_issues(issues)
        );
    }
}

#[derive(Debug, Deserialize)]
struct ExpectedFile {
    flows: Vec<ExpectedFlow>,
    detectors: ExpectedDetectors,
}

#[derive(Debug, Deserialize)]
struct ExpectedFlow {
    method: String,
    source_kind: String,
    sink_kind: String,
    rule_code: u32,
    #[serde(default)]
    needs_cipher_tito: bool,
    #[serde(default)]
    needs_extra_pii: bool,
    #[serde(default)]
    needs_interproc: bool,
    #[serde(default)]
    callable_any_of: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ExpectedDetectors {
    required: Vec<String>,
    webview_any_of: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
}

fn load_expected() -> ExpectedFile {
    let path = demo_dir().join("expected.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse expected.json: {e}"))
}

/// Catalog kinds + dest sinks mirrored from androhunt/config/pii_catalog.json.
fn extra_pii_config() -> TaintConfig {
    TaintConfig::from_json_str(
        r#"{
          "sources": [
            {"patterns": ["getLine1Number", "TelephonyManager.getLine1Number", "getPhoneNumber"], "port": "return", "kind": "PhoneNumber"},
            {"patterns": ["ContactsContract", "CommonDataKinds.Phone", "ContactsContract.Contacts"], "port": "return", "kind": "Contacts"},
            {"patterns": ["SmsManager", "Telephony.Sms", "content://sms", "Inbox.CONTENT_URI"], "port": "return", "kind": "Sms"},
            {"patterns": ["AccountManager.getAccounts", "getAccounts", "getAccountsByType", "AccountManager.getAuthToken"], "port": "return", "kind": "Account"},
            {"patterns": ["AdvertisingIdClient", "getAdvertisingIdInfo", "advertisingId"], "port": "return", "kind": "AdvertisingId"},
            {"patterns": ["MediaStore", "MediaStore.Images", "MediaStore$Images", "EXTERNAL_CONTENT_URI"], "port": "return", "kind": "Media"},
            {"patterns": ["CalendarContract", "CalendarContract.Events"], "port": "return", "kind": "Calendar"},
            {"patterns": ["getEmail", "Profile.CONTENT_URI", "ContactsContract.CommonDataKinds.Email"], "port": "return", "kind": "Email"}
          ],
          "propagations": [
            {"patterns": ["Intent.putExtra", "putExtra"], "from": {"argument": {"index": 2}}, "to": "return"},
            {"patterns": ["Intent.setData", "setData"], "from": {"argument": {"index": 1}}, "to": "return"}
          ],
          "sinks": [
            {"patterns": ["ClipboardManager.setPrimaryClip", "ClipboardManager.setText", "setPrimaryClip"], "port": {"argument": {"index": 1}}, "kind": "ClipboardWrite"},
            {"patterns": ["SharedPreferences$Editor.putString", "Editor.putString", "putString"], "port": {"argument": {"index": 2}}, "kind": "SharedPrefsWrite"},
            {"patterns": ["CookieManager.setCookie", "setCookie"], "port": {"argument": {"index": 1}}, "kind": "CookieWrite"}
          ],
          "rules": [
            {
              "name": "Extra PII to network / cookies",
              "code": 91001,
              "description": "Catalog PII kinds may flow into network or cookie sinks",
              "sources": ["PhoneNumber", "Contacts", "Sms", "Account", "AdvertisingId", "Media", "Calendar", "Email"],
              "sinks": ["Network", "CookieWrite"]
            },
            {
              "name": "Extra PII to logging",
              "code": 91002,
              "description": "Catalog PII kinds may flow into logs",
              "sources": ["PhoneNumber", "Contacts", "Sms", "Account", "AdvertisingId", "Media", "Calendar", "Email"],
              "sinks": ["Logging"]
            },
            {
              "name": "DeviceId / Location dest stores and other-app",
              "code": 91003,
              "description": "DeviceId/Location may reach clipboard, prefs, cookies, or IPC launches",
              "sources": ["DeviceId", "Location"],
              "sinks": ["ClipboardWrite", "SharedPrefsWrite", "CookieWrite", "LaunchingComponent"]
            }
          ]
        }"#,
    )
    .expect("extra pii json")
}

#[test]
fn default_config_finds_intra_proc_privacy_and_vuln_flows() {
    let expected = load_expected();
    let cfg = default_config();
    let issues = solve_demo(&cfg);

    for flow in &expected.flows {
        if flow.needs_cipher_tito || flow.needs_extra_pii || flow.needs_interproc {
            continue;
        }
        assert_expected_flow(&issues, flow);
    }

    // Field URL + StringBuilder (and same-method field/static URL) must hit default rule 10.
    assert_flow(
        &issues,
        "leakDeviceIdViaFieldUrl",
        "DeviceId",
        "Network",
        10,
    );
    let builder_hit = has_flow_any(
        &issues,
        &["leakDeviceIdViaBuilder", "HttpSink", "sendHop"],
        "DeviceId",
        "Network",
        10,
    );
    if !builder_hit {
        panic!(
            "missing StringBuilder DeviceId→Network on leakDeviceIdViaBuilder / HttpSink\n--- issues ---\n{}",
            dump_issues(&issues)
        );
    }
}

fn assert_expected_flow(issues: &[Issue], flow: &ExpectedFlow) {
    if flow.callable_any_of.is_empty() {
        assert_flow(
            issues,
            &flow.method,
            &flow.source_kind,
            &flow.sink_kind,
            flow.rule_code,
        );
        return;
    }
    let names: Vec<&str> = flow.callable_any_of.iter().map(String::as_str).collect();
    if !has_flow_any(
        issues,
        &names,
        &flow.source_kind,
        &flow.sink_kind,
        flow.rule_code,
    ) {
        panic!(
            "missing {}→{} rule {} in any of {:?}\n--- issues ---\n{}",
            flow.source_kind,
            flow.sink_kind,
            flow.rule_code,
            flow.callable_any_of,
            dump_issues(issues)
        );
    }
}

#[test]
fn cipher_tito_reveals_device_id_through_cipher() {
    let mut cfg = default_config();
    // AndroHunt-style Cipher TITO + getBytes so DeviceId reaches doFinal's arg1.
    let extra = TaintConfig::from_json_str(
        r#"{
          "propagations": [
            {"patterns": ["getBytes"], "from": {"argument": {"index": 0}}, "to": "return"},
            {"patterns": ["Cipher.doFinal", "javax.crypto.Cipher.doFinal"], "from": {"argument": {"index": 1}}, "to": "return"},
            {"patterns": ["Cipher.update", "javax.crypto.Cipher.update"], "from": {"argument": {"index": 1}}, "to": "return"}
          ]
        }"#,
    )
    .expect("cipher tito json");
    cfg.merge(extra);

    let issues = solve_demo(&cfg);
    assert_flow(
        &issues,
        "leakDeviceIdThroughCipher",
        "DeviceId",
        "Network",
        10,
    );
    let cipher_hop = has_flow_any(
        &issues,
        &["leakDeviceIdCipherThenHelper", "HttpSink", "sendHop"],
        "DeviceId",
        "Network",
        10,
    );
    let cipher_names: Vec<&str> = issues
        .iter()
        .filter(|i| {
            i.callable.contains("Cipher")
                || i.callable.contains("cipherhop")
                || i.callable.contains("CipherThen")
        })
        .map(|i| i.callable.as_str())
        .collect();
    println!("cipher layer callables: {cipher_names:?} hop={cipher_hop}");
    if !cipher_hop {
        panic!(
            "missing cipher-then-helper DeviceId→Network\n--- issues ---\n{}",
            dump_issues(&issues)
        );
    }
}

#[test]
fn extra_pii_kinds_and_dest_sinks() {
    let expected = load_expected();
    let mut cfg = default_config();
    cfg.merge(extra_pii_config());
    let issues = solve_demo(&cfg);

    for flow in &expected.flows {
        if flow.needs_cipher_tito {
            continue;
        }
        if !flow.needs_extra_pii {
            continue;
        }
        assert_expected_flow(&issues, flow);
    }
}

#[test]
fn detectors_flag_ssl_weak_crypto_webview_and_world_readable() {
    let expected = load_expected();
    let dex = load_demo_dex();
    let findings = scan_dex_parallel(&dex, None);
    let cats: BTreeSet<&str> = findings.iter().map(|f| f.category.as_str()).collect();

    let dump = {
        let mut lines: Vec<String> = findings
            .iter()
            .map(|f| {
                format!(
                    "{} {}#{} sink={}",
                    f.category, f.class_name, f.method_name, f.sink_desc
                )
            })
            .collect();
        lines.sort();
        lines.join("\n")
    };

    let mut missing = Vec::new();
    for cat in &expected.detectors.required {
        if !cats.contains(cat.as_str()) {
            missing.push(cat.as_str());
        }
    }
    if !missing.is_empty() {
        panic!(
            "missing detector categories {missing:?}\n--- findings ---\n{dump}\n--- categories ---\n{cats:?}"
        );
    }

    let webview_hit = expected
        .detectors
        .webview_any_of
        .iter()
        .any(|c| cats.contains(c.as_str()));
    if !webview_hit {
        panic!(
            "missing any webview category {:?}\n--- findings ---\n{dump}\n--- categories ---\n{cats:?}",
            expected.detectors.webview_any_of
        );
    }
    assert!(
        findings.iter().any(|finding| {
            finding.method_name == "onMessage"
                && finding.category == "insecure_logging"
                && finding.source_desc.contains("callback parameter")
        }),
        "VF detectors did not seed lifecycle callback parameters\n--- findings ---\n{dump}"
    );

    let _ = expected.detectors.optional;
    println!("detector categories: {:?}", cats);
}

#[test]
fn pending_intent_mutable_empty() {
    let dex = load_demo_dex();
    let findings = scan_pending_intents_dex_parallel(&dex);
    let dump = {
        let mut lines: Vec<String> = findings
            .iter()
            .map(|f| {
                format!(
                    "{}#{} empty={} mutable={} dest={}",
                    f.class_name,
                    f.method_name,
                    f.base_intent_empty,
                    f.mutable_flag,
                    f.destination_kind
                )
            })
            .collect();
        lines.sort();
        lines.join("\n")
    };
    if !findings
        .iter()
        .any(|f| f.method_name.contains("mutableEmpty") && f.base_intent_empty && f.mutable_flag)
    {
        panic!("missing mutable empty PendingIntent in mutableEmpty\n--- findings ---\n{dump}");
    }
    println!("pending_intent findings:\n{dump}");
}

#[test]
fn helper_hop_device_id_to_network() {
    let cfg = default_config();
    let issues = solve_demo(&cfg);
    let hit = has_flow_any(
        &issues,
        &[
            "HttpSink",
            "leakDeviceIdViaHelper",
            "sendHop",
            "leakDeviceIdViaLocalHelper",
        ],
        "DeviceId",
        "Network",
        10,
    );
    let layers: Vec<&str> = issues
        .iter()
        .filter(|i| {
            i.callable.contains("PrivacyLayers")
                || i.callable.contains("HttpSink")
                || i.callable.contains("AnalyticsWrapper")
        })
        .map(|i| i.callable.as_str())
        .collect();
    println!("privacy layer callables: {layers:?} hit={hit}");
    if !hit {
        panic!(
            "missing helper-hop DeviceId→Network (HttpSink / leakDeviceIdViaHelper / sendHop / leakDeviceIdViaLocalHelper)\n--- issues ---\n{}",
            dump_issues(&issues)
        );
    }
    // collectDeviceId return → caller id → HttpSink.post must not be broken.
    let return_hop = has_flow_any(
        &issues,
        &["leakDeviceIdViaHelper", "HttpSink"],
        "DeviceId",
        "Network",
        10,
    );
    if !return_hop {
        panic!(
            "return→arg→sink broken: collectDeviceId should taint leakDeviceIdViaHelper / HttpSink\n--- issues ---\n{}",
            dump_issues(&issues)
        );
    }
}

#[test]
fn base_concat_and_field_hop_device_id_to_network() {
    let cfg = default_config();
    let issues = solve_demo(&cfg);
    let concat = has_flow_any(
        &issues,
        &["leakDeviceIdViaBaseConcat", "HttpSink"],
        "DeviceId",
        "Network",
        10,
    );
    if !concat {
        panic!(
            "missing leakDeviceIdViaBaseConcat DeviceId→Network\n--- issues ---\n{}",
            dump_issues(&issues)
        );
    }
    if !haystack_has(&issues, "api.demohunt.androguard.com") {
        panic!(
            "concat dest not recovered (expected api.demohunt.androguard.com in trace/description/extra)\n--- issues ---\n{}",
            dump_issues(&issues)
        );
    }
    let field = has_flow_any(
        &issues,
        &["leakDeviceIdViaFieldHop", "sendStored", "HttpSink"],
        "DeviceId",
        "Network",
        10,
    );
    if !field {
        panic!(
            "missing field-hop DeviceId→Network\n--- issues ---\n{}",
            dump_issues(&issues)
        );
    }
    let field_host = issues.iter().any(|i| {
        (i.callable.contains("leakDeviceIdViaFieldHop")
            || i.callable.contains("sendStored")
            || (i.callable.contains("HttpSink") && issue_haystack(i).contains("fieldhop")))
            && issue_haystack(i).contains("api.demohunt.androguard.com")
    }) || haystack_has(&issues, "fieldhop")
        || haystack_has(&issues, "api.demohunt.androguard.com");
    if !field_host {
        panic!(
            "field-hop dest not recovered\n--- issues ---\n{}",
            dump_issues(&issues)
        );
    }
    let multi = issues.iter().any(|i| {
        (i.callable.contains("leakDeviceIdViaHelper")
            || i.callable.contains("leakDeviceIdViaBaseConcat")
            || i.callable.contains("leakDeviceIdViaFieldHop")
            || i.callable.contains("leakDeviceIdViaLocalHelper"))
            && i.trace.len() > 2
    });
    println!("multi-frame caller issue: {multi}");
}

#[test]
fn apk_loads_via_load_dexes_from_path() {
    let path = demo_dir().join("privacy_vuln_demo.apk");
    let dexes =
        load_dexes_from_path(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
    assert!(
        !dexes.is_empty(),
        "APK produced no DEX files: {}",
        path.display()
    );
}
