//! Implicit Intent launches (no component/package) — hijackable by other apps.
//!
//! Bare implicit launches are common and noisy (Info). Escalate when sensitive
//! extras / credential-like strings co-occur (Oversecured / bounty ROI).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, VulnFinding};

const LAUNCH: &[&str] = &[
    "startActivity",
    "startActivityForResult",
    "startService",
    "bindService",
    "sendBroadcast",
    "sendOrderedBroadcast",
];

const EXPLICIT: &[&str] = &[
    "setComponent",
    "setClass",
    "setClassName",
    "setPackage",
    "Intent.setComponent",
    "Intent.setClass",
    "Intent.setClassName",
    "Intent.setPackage",
];

const IMPLICIT_HINTS: &[&str] = &[
    "setAction",
    "Intent.setAction",
    "Intent.<init>",
    "setData",
    "addCategory",
];

const SENSITIVE_KEYS: &[&str] = &[
    "token",
    "password",
    "passwd",
    "secret",
    "session",
    "cookie",
    "credential",
    "auth",
    "api_key",
    "apikey",
    "bearer",
    "refresh",
    "access_token",
];

fn method_blob(owned: &ValueFlowAnalysisOwned) -> String {
    let mut s = String::new();
    for v in owned.invoke_method_map.values() {
        s.push_str(v);
        s.push('\n');
    }
    for v in owned.insn_at.values() {
        s.push_str(v);
        s.push('\n');
    }
    for (_, src) in &owned.api_return_sources {
        s.push_str(src);
        s.push('\n');
    }
    s.to_ascii_lowercase()
}

pub fn scan_implicit_intent(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_launch = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, LAUNCH));
    if !has_launch {
        return Vec::new();
    }
    let has_explicit = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, EXPLICIT));
    if has_explicit {
        return Vec::new();
    }
    let has_implicit_hint = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, IMPLICIT_HINTS));
    if !has_implicit_hint {
        return Vec::new();
    }

    let blob = method_blob(owned);
    let sensitive = SENSITIVE_KEYS.iter().any(|k| blob.contains(k));
    let category = if sensitive {
        "implicit_intent_sensitive"
    } else {
        "implicit_intent_launch"
    };

    let mut findings = invoke_scan(owned, class_name, method_name, category, LAUNCH);
    findings.truncate(1);
    findings
}
