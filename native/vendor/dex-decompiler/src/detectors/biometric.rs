//! BiometricPrompt without crypto-bound key / KeyGenParameterSpec auth.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, VulnFinding};

const BIO: &[&str] = &[
    "BiometricPrompt",
    "BiometricPrompt.Builder",
    "FingerprintManager",
    "authenticate",
];

const CRYPTO_BOUND: &[&str] = &[
    "CryptoObject",
    "setUserAuthenticationRequired",
    "setInvalidatedByBiometricEnrollment",
    "setUserAuthenticationValidityDurationSeconds",
    "KeyGenParameterSpec",
];

pub fn scan_biometric_misuse(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_bio = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, BIO))
        || owned
            .insn_at
            .values()
            .any(|s| s.contains("BiometricPrompt"));
    if !has_bio {
        return Vec::new();
    }
    let has_crypto = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, CRYPTO_BOUND))
        || owned
            .insn_at
            .values()
            .any(|s| method_matches_any(s, CRYPTO_BOUND));
    if has_crypto {
        return Vec::new();
    }
    invoke_scan(
        owned,
        class_name,
        method_name,
        "biometric_without_crypto",
        &["BiometricPrompt", "authenticate", "FingerprintManager"],
    )
}
