//! Credential / sensitive data broadcast (exported receiver confusion).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const SOURCES: &[&str] = &[
    "getPassword",
    "getLoginData",
    "getToken",
    "getStringExtra",
    "getString",
    "SharedPreferences.getString",
    "getLoginUrl",
    "LoginData",
];

const SINKS: &[&str] = &[
    "sendBroadcast",
    "sendOrderedBroadcast",
    "sendStickyBroadcast",
    "LocalBroadcastManager.sendBroadcast",
];

pub fn scan_credential_broadcast(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "credential_broadcast",
        SOURCES,
        SINKS,
    );
    // Surface unprotected credential action strings + sendBroadcast in same method.
    let has_send = owned
        .invoke_method_map
        .values()
        .any(|m| SINKS.iter().any(|s| m.contains(s)));
    let has_cred_hint = owned.insn_at.values().any(|s| {
        let u = s.to_uppercase();
        u.contains("UNPROTECTED")
            || u.contains("CREDENTIAL")
            || u.contains("PASSWORD")
            || u.contains("LOGINDATA")
    });
    if out.is_empty() && has_send && has_cred_hint {
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "credential_broadcast",
            SINKS,
        ));
    }
    out
}
