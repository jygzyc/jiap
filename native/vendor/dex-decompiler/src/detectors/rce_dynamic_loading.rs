//! RCE via dynamic code loading and process execution.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const RCE_PATTERNS: &[&str] = &[
    "DexClassLoader",
    "PathClassLoader",
    "InMemoryDexClassLoader",
    "loadClass",
    "Runtime.exec",
    "ProcessBuilder.<init>",
    "ProcessBuilder.start",
];

const EXEC_SOURCES: &[&str] = &[
    "getStringExtra",
    "getQueryParameter",
    "getIntent",
    "getDataString",
    "EditText.getText",
    "getText",
    "File.getAbsolutePath",
];

const EXEC_SINKS: &[&str] = &[
    "Runtime.exec",
    "ProcessBuilder.<init>",
    "ProcessBuilder.start",
    "ProcessBuilder.command",
];

pub fn scan_rce_dynamic_loading(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = invoke_scan(
        owned,
        class_name,
        method_name,
        "rce_dynamic_loading",
        RCE_PATTERNS,
    );
    out.extend(source_sink_scan(
        owned,
        class_name,
        method_name,
        "rce_process_exec",
        EXEC_SOURCES,
        EXEC_SINKS,
    ));
    out
}
