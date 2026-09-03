//! Android Keystore misuse: keys without user authentication binding.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, VulnFinding};

const KEYGEN: &[&str] = &[
    "KeyGenParameterSpec.Builder",
    "KeyPairGeneratorSpec.Builder",
    "KeyGenParameterSpec",
];

const AUTH: &[&str] = &[
    "setUserAuthenticationRequired",
    "setUserAuthenticationParameters",
    "setIsStrongBoxBacked",
];

pub fn scan_keystore_misuse(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_keygen = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, KEYGEN));
    if !has_keygen {
        return Vec::new();
    }
    let has_auth = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, AUTH));
    if has_auth {
        return Vec::new();
    }
    invoke_scan(
        owned,
        class_name,
        method_name,
        "keystore_no_user_auth",
        KEYGEN,
    )
}
