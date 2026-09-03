//! Hardcoded secrets: high-signal persist / network body writes (review for constants).
//!
//! SharedPreferences `put*` / `edit` alone are too noisy for hunt/chain — prefer
//! taint `SharedPrefsWrite` and string hunters (`hardcoded_url_secret`) for that.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, VulnFinding};

/// Prefer network/body and file writers over prefs put* (prefs → Info noise).
const SECRET_SINKS: &[&str] = &[
    "RequestBody.create",
    "FormBody.add",
    "MultipartBody",
    "okhttp3.RequestBody",
    "FileWriter",
    "FileOutputStream.<init>",
    "java.io.FileOutputStream.<init>",
];

pub fn scan_hardcoded_secrets(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    invoke_scan(
        owned,
        class_name,
        method_name,
        "hardcoded_secrets_review",
        SECRET_SINKS,
    )
}
