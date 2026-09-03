//! Arbitrary code execution via createPackageContext / third-party ClassLoader
//! (Oversecured + OVAA plugin ACE).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, VulnFinding};

pub fn scan_package_context_ace(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_ctx = owned
        .invoke_method_map
        .values()
        .any(|m| m.contains("createPackageContext") || m.contains("Context.createPackageContext"));
    let has_loader = owned.invoke_method_map.values().any(|m| {
        method_matches_any(
            m,
            &[
                "getClassLoader",
                "loadClass",
                "DexClassLoader",
                "PathClassLoader",
            ],
        )
    });
    // Signature check reduces risk; still flag if createPackageContext+loadClass present
    // without checkSignatures (common OVAA plugin bug).
    let has_sig_check = owned
        .invoke_method_map
        .values()
        .any(|m| m.contains("checkSignatures") || m.contains("hasSigningCertificate"));

    if !(has_ctx && has_loader) {
        return Vec::new();
    }
    if has_sig_check {
        return Vec::new();
    }

    let mut findings = invoke_scan(
        owned,
        class_name,
        method_name,
        "rce_package_context",
        &["createPackageContext", "getClassLoader", "loadClass"],
    );
    findings.truncate(1);
    findings
}
