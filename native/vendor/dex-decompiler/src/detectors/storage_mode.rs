//! World-readable / world-writable file modes (`MODE_WORLD_*`).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{method_matches_any, VulnFinding};

const SINKS: &[&str] = &[
    "openFileOutput",
    "openOrCreateDatabase",
    "getSharedPreferences",
];

fn text_mentions_world_mode(text: &str) -> bool {
    let u = text.to_uppercase();
    u.contains("MODE_WORLD_READABLE")
        || u.contains("MODE_WORLD_WRITEABLE")
        || u.contains("MODE_WORLD_WRITABLE")
        || u.contains("WORLD_READABLE")
        || u.contains("WORLD_WRITEABLE")
}

/// Flag persist APIs when the method references MODE_WORLD_* (field/string/operand text).
pub fn scan_storage_mode(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_world = owned.insn_at.values().any(|s| text_mentions_world_mode(s))
        || owned
            .invoke_method_map
            .values()
            .any(|s| text_mentions_world_mode(s));
    if !has_world {
        return Vec::new();
    }
    let mut findings = Vec::new();
    for (offset, method_ref) in &owned.invoke_method_map {
        if method_matches_any(method_ref, SINKS) {
            findings.push(VulnFinding::new(
                "world_readable_storage",
                class_name,
                method_name,
                None,
                "MODE_WORLD_*",
                *offset,
                method_ref.clone(),
            ));
        }
    }
    // Even without a sink match, surface the MODE_WORLD reference.
    if findings.is_empty() {
        if let Some((offset, desc)) = owned
            .insn_at
            .iter()
            .find(|(_, s)| text_mentions_world_mode(s))
            .map(|(o, s)| (*o, s.clone()))
        {
            findings.push(VulnFinding::new(
                "world_readable_storage",
                class_name,
                method_name,
                None,
                "MODE_WORLD_*",
                offset,
                desc,
            ));
        }
    }
    findings
}
