//! WebResourceResponse / shouldInterceptRequest local-file leakage (Oversecured / Amazon).
//!
//! Pattern: shouldInterceptRequest → WebResourceResponse backed by FileInputStream /
//! getLastPathSegment without path containment, often with Access-Control-Allow-Origin: *.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, VulnFinding};

const INTERCEPT: &[&str] = &["shouldInterceptRequest", "WebResourceResponse.<init>"];
const FILE_READ: &[&str] = &[
    "FileInputStream.<init>",
    "getLastPathSegment",
    "getPath",
    "openFileInput",
    "getAssets().open",
    "AssetManager.open",
];

pub fn scan_webresource_response(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_intercept = method_name.contains("shouldInterceptRequest")
        || method_name.contains("handlePackage")
        || owned
            .invoke_method_map
            .values()
            .any(|m| method_matches_any(m, INTERCEPT));
    let has_file = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, FILE_READ));
    let has_cors_star = owned.insn_at.values().any(|s| {
        let l = s.to_lowercase();
        l.contains("access-control-allow-origin") || l.contains("access_control_allow_origin")
    });
    let has_canon = owned.invoke_method_map.values().any(|m| {
        m.contains("getCanonicalPath")
            || m.contains("getCanonicalFile")
            || m.contains("WebViewAssetLoader")
    });

    if !(has_intercept && has_file && !has_canon) {
        return Vec::new();
    }

    let mut findings = invoke_scan(
        owned,
        class_name,
        method_name,
        "webview_resource_response_file",
        &[
            "WebResourceResponse.<init>",
            "FileInputStream.<init>",
            "shouldInterceptRequest",
            "getLastPathSegment",
        ],
    );
    findings.truncate(1);
    if has_cors_star {
        for f in &mut findings {
            f.message.push_str(
                " Headers include Access-Control-Allow-Origin: * which enables XHR exfil from attacker pages.",
            );
        }
    }
    findings
}
