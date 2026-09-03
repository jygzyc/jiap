//! Weak crypto: SecretKeySpec / weak Cipher / MessageDigest algorithms.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, VulnFinding};

const KEY_SPEC: &[&str] = &[
    "SecretKeySpec.<init>",
    "IvParameterSpec.<init>",
    "DESKeySpec.<init>",
    "PBEKeySpec.<init>",
];

const ALGO_APIS: &[&str] = &[
    "Cipher.getInstance",
    "MessageDigest.getInstance",
    "SecureRandom.setSeed",
];

fn method_mentions_weak_algo(owned: &ValueFlowAnalysisOwned) -> bool {
    owned.insn_at.values().any(|s| {
        let u = s.to_uppercase();
        u.contains("\"DES\"")
            || u.contains("\"DESEDE\"")
            || u.contains("\"AES/ECB")
            || u.contains("\"RC4\"")
            || u.contains("\"MD5\"")
            || u.contains("\"SHA-1\"")
            || u.contains("\"SHA1\"")
    })
}

pub fn scan_weak_crypto(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut findings = invoke_scan(owned, class_name, method_name, "weak_crypto", KEY_SPEC);
    if method_mentions_weak_algo(owned) {
        findings.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "weak_crypto",
            ALGO_APIS,
        ));
    }
    findings
}
