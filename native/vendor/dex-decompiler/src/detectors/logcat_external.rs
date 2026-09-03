//! Logcat / dumpsys written to external storage via shell exec.
//!
//! Quokka Uhale CVE-2025-58389: `ShellUtils.execCommand("logcat -d >> /sdcard/...")`.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, VulnFinding};

const EXEC_APIS: &[&str] = &[
    "Runtime.exec",
    "ProcessBuilder.<init>",
    "ProcessBuilder.start",
    "ProcessBuilder.command",
    "execCommand",
    "ShellUtils.execCommand",
    "Shell.su",
    "Shell.exec",
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

/// Detect shelling out to logcat/dmesg/dumpsys into /sdcard or external paths.
pub fn scan_logcat_external(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let blob = method_blob(owned);
    let has_log_dump =
        blob.contains("logcat") || blob.contains("dmesg") || blob.contains("dumpsys");
    let has_external = blob.contains("/sdcard")
        || blob.contains("/storage/emulated")
        || blob.contains("getexternalstorage")
        || blob.contains("environment.getexternalstoragedirectory");
    let has_exec = owned.invoke_method_map.values().any(|m| {
        EXEC_APIS.iter().any(|p| m.contains(p))
            || m.contains("execCommand")
            || m.contains("Runtime.exec")
    });

    if !(has_log_dump && has_external && has_exec) {
        // Also catch class/method names that are explicit (ShellLogcat.logcatToWrite).
        let name_hit = class_name.to_ascii_lowercase().contains("logcat")
            || method_name.to_ascii_lowercase().contains("logcat");
        if !(name_hit && has_exec && (has_external || has_log_dump)) {
            return Vec::new();
        }
    }

    let mut findings = invoke_scan(
        owned,
        class_name,
        method_name,
        "logcat_external_storage",
        EXEC_APIS,
    );
    if findings.is_empty() {
        findings.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "logcat_external_storage",
            &["execCommand", "Runtime.exec"],
        ));
    }
    findings.truncate(1);
    findings
}
