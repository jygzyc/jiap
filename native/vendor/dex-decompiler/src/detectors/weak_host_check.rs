//! Weak WebView / deeplink host checks (contains/endsWith vs allow-list equals).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{method_matches_any, VulnFinding};

const WEAK_CHECKS: &[&str] = &["contains", "endsWith", "startsWith", "indexOf"];
const HOST_APIS: &[&str] = &["getHost", "getAuthority", "Uri.getHost", "toString"];
const WEBVIEW_LOAD: &[&str] = &[
    "loadUrl",
    "loadData",
    "loadDataWithBaseURL",
    "startActivity",
];

pub fn scan_weak_host_validation(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_weak = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, WEAK_CHECKS));
    let has_host = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, HOST_APIS))
        || owned.insn_at.values().any(|s| {
            let l = s.to_lowercase();
            l.contains("gethost") || l.contains("host")
        });
    let has_nav = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, WEBVIEW_LOAD));
    if !(has_weak && has_host && has_nav) {
        return Vec::new();
    }
    // Emit one finding at first weak-check invoke.
    for (offset, method_ref) in &owned.invoke_method_map {
        if method_matches_any(method_ref, WEAK_CHECKS) {
            return vec![VulnFinding::new(
                "webview_weak_host_check",
                class_name,
                method_name,
                None,
                method_ref.clone(),
                *offset,
                method_ref.clone(),
            )];
        }
    }
    Vec::new()
}
