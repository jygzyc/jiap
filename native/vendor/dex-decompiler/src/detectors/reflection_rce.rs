//! Reflection RCE: user-influenced Class.forName / getMethod / Method.invoke.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const SOURCES: &[&str] = &[
    "getStringExtra",
    "getCharSequenceExtra",
    "getQueryParameter",
    "getIntent",
    "getDataString",
    "EditText.getText",
    "getText",
];

const SINKS: &[&str] = &[
    "Class.forName",
    "loadClass",
    "getMethod",
    "getDeclaredMethod",
    "Method.invoke",
    "Constructor.newInstance",
    "newInstance",
];

pub fn scan_reflection_rce(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "reflection_rce",
        SOURCES,
        SINKS,
    );
    // Presence of invoke + forName in same method is medium surface.
    if out.is_empty() {
        let has_for_name = owned.invoke_method_map.values().any(|m| {
            m.contains("forName") || m.contains("getMethod") || m.contains("getDeclaredMethod")
        });
        let has_invoke = owned
            .invoke_method_map
            .values()
            .any(|m| m.contains("Method.invoke") || m.ends_with(".invoke"));
        if has_for_name && has_invoke {
            out.extend(invoke_scan(
                owned,
                class_name,
                method_name,
                "reflection_rce",
                &["Method.invoke", "Class.forName"],
            ));
        }
    }
    out
}
