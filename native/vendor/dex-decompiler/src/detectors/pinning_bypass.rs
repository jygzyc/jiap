//! Certificate pinning bypass via reflection / TrustKit / OkHttp hooks.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, VulnFinding};

const PINNING_APIS: &[&str] = &[
    "CertificatePinner",
    "TrustKit",
    "PinningTrustManager",
    "NetworkSecurityTrustManager",
];

const REFLECT: &[&str] = &[
    "Class.forName",
    "getDeclaredMethod",
    "getMethod",
    "Method.invoke",
    "setAccessible",
];

fn mentions_pinning(owned: &ValueFlowAnalysisOwned) -> bool {
    owned.insn_at.values().any(|s| {
        let u = s.to_uppercase();
        u.contains("CERTIFICATEPINNER")
            || u.contains("TRUSTKIT")
            || u.contains("PINNING")
            || u.contains("SSLPEERUNVERIFIED")
    }) || owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, PINNING_APIS))
}

fn bypass_hints(owned: &ValueFlowAnalysisOwned) -> bool {
    owned.insn_at.values().any(|s| {
        let l = s.to_lowercase();
        l.contains("bypass")
            || l.contains("disable")
            || l.contains("unpin")
            || l.contains("noop")
            || l.contains("\"\"")
            || l.contains("empty")
    })
}

pub fn scan_pinning_bypass(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let pinning = mentions_pinning(owned);
    let reflect = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, REFLECT));
    let hints = bypass_hints(owned);

    // High-signal: reflection used together with pinning APIs / strings.
    if pinning && reflect {
        let mut out = invoke_scan(owned, class_name, method_name, "pinning_bypass", REFLECT);
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "pinning_bypass",
            PINNING_APIS,
        ));
        out.truncate(2);
        return out;
    }

    // Medium: CertificatePinner with empty-pin / bypass hints only (not every Builder use).
    if pinning && hints {
        return invoke_scan(
            owned,
            class_name,
            method_name,
            "pinning_bypass",
            &["CertificatePinner", "TrustKit"],
        )
        .into_iter()
        .take(1)
        .collect();
    }

    Vec::new()
}
