//! Intent.parseUri → startActivity (limited intent redirection via intent:// WebView/deeplink).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{source_sink_scan, VulnFinding};

const SOURCES: &[&str] = &["Intent.parseUri", "parseUri", "Uri.parse"];
const SINKS: &[&str] = &[
    "startActivity",
    "startActivityForResult",
    "startService",
    "startForegroundService",
];

pub fn scan_intent_parse_uri(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut findings = source_sink_scan(
        owned,
        class_name,
        method_name,
        "intent_parse_uri_redirect",
        SOURCES,
        SINKS,
    );
    // Prefer parseUri specifically: Uri.parse→startActivity is too noisy alone.
    findings.retain(|f| {
        f.source_desc.contains("parseUri")
            || f.sink_desc.contains("parseUri")
            || owned
                .invoke_method_map
                .values()
                .any(|m| m.contains("parseUri"))
    });
    if findings.is_empty() {
        let has_parse = owned
            .invoke_method_map
            .values()
            .any(|m| m.contains("parseUri"));
        let has_launch = owned
            .invoke_method_map
            .values()
            .any(|m| SINKS.iter().any(|s| m.contains(s)));
        if has_parse && has_launch {
            findings.extend(crate::detectors::types::invoke_scan(
                owned,
                class_name,
                method_name,
                "intent_parse_uri_redirect",
                &["parseUri", "startActivity"],
            ));
            findings.truncate(1);
        }
    }
    findings
}
