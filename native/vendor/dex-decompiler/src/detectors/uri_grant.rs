//! URI permission grant / result-forwarding.
//!
//! Classic pattern (OVAA `grant_uri_permissions`): `onActivityResult` forwards the
//! result Intent via `setResult(code, data)` so an attacker who handled an implicit
//! `startActivityForResult` can return `FLAG_GRANT_*` + a content URI.
//!
//! Also flags Intent-derived values flowing into grant-related APIs.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const GRANT_SOURCES: &[&str] = &[
    "getIntent",
    "getData",
    "getDataString",
    "getParcelableExtra",
    "getSerializableExtra",
    "getClipData",
    "getStringExtra",
];

const GRANT_SINKS: &[&str] = &[
    "setResult",
    "Intent.addFlags",
    "addFlags",
    "Intent.setFlags",
    "setFlags",
    "Intent.setClipData",
    "setClipData",
    "grantUriPermission",
    "takePersistableUriPermission",
];

/// Detect URI grant / result-forward surfaces in one method.
pub fn scan_uri_grant(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut findings = Vec::new();

    // OVAA-style: onActivityResult → setResult(…, Intent) without stripping grant flags.
    if method_name == "onActivityResult" {
        for f in invoke_scan(
            owned,
            class_name,
            method_name,
            "uri_permission_result_forward",
            &["setResult"],
        ) {
            findings.push(f);
        }
    }

    // Oversecured VulnerableActivity: setResult(-1, getIntent()) / setResult(getIntent()).
    if method_name == "onCreate" || method_name == "onNewIntent" {
        let has_set_result = owned
            .invoke_method_map
            .values()
            .any(|m| m.contains("setResult"));
        let has_get_intent = owned
            .invoke_method_map
            .values()
            .any(|m| m.contains("getIntent"))
            || owned
                .api_return_sources
                .iter()
                .any(|(_, s)| s.contains("getIntent"));
        if has_set_result && has_get_intent {
            let mut passthrough = invoke_scan(
                owned,
                class_name,
                method_name,
                "uri_permission_setresult_passthrough",
                &["setResult"],
            );
            passthrough.truncate(1);
            findings.extend(passthrough);
        }
    }

    findings.extend(source_sink_scan(
        owned,
        class_name,
        method_name,
        "uri_permission_grant_flow",
        GRANT_SOURCES,
        GRANT_SINKS,
    ));

    findings
}
