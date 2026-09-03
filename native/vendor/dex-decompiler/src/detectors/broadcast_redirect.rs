//! S1: Broadcast-mediated intent redirection (Samsung Galaxy Store `install_complete`).
//!
//! Nested Parcelable Intent launched from `BroadcastReceiver.onReceive` (or a
//! Receiver-named class). Often triggered by package-install / result broadcasts.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const NESTED_SOURCES: &[&str] = &[
    "getParcelableExtra",
    "getParcelableArrayExtra",
    "getParcelableArrayListExtra",
    "getParcelable",
    "getSerializableExtra",
];

const LAUNCH_SINKS: &[&str] = &[
    "startActivity",
    "startActivityForResult",
    "startService",
    "startForegroundService",
    "bindService",
];

const INSTALL_HINTS: &[&str] = &[
    "INSTALL_COMPLETE",
    "install_complete",
    "PACKAGE_ADDED",
    "PACKAGE_REPLACED",
    "ACTION_PACKAGE_ADDED",
    "MY_PACKAGE_REPLACED",
    "SESSION_COMMITTED",
    "packageadded",
];

fn is_receiver_context(class_name: &str, method_name: &str) -> bool {
    method_name == "onReceive"
        || class_name.contains("Receiver")
        || class_name.contains("Broadcast")
}

fn has_install_hint(owned: &ValueFlowAnalysisOwned) -> bool {
    owned.insn_at.values().any(|s| {
        let u = s.to_uppercase();
        INSTALL_HINTS.iter().any(|h| u.contains(&h.to_uppercase()))
    })
}

/// Nested Intent launch from a broadcast/receiver entry (S1).
pub fn scan_broadcast_intent_redirect(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    if !is_receiver_context(class_name, method_name) {
        return Vec::new();
    }

    let mut findings = source_sink_scan(
        owned,
        class_name,
        method_name,
        "broadcast_intent_redirect",
        NESTED_SOURCES,
        LAUNCH_SINKS,
    );

    // Co-occurrence: install/package broadcast string + nested launch APIs
    // (VF may miss a hop; still a strong Samsung #5-style signal).
    if findings.is_empty() && has_install_hint(owned) {
        let has_nested_src = owned
            .api_return_sources
            .iter()
            .any(|(_, m)| NESTED_SOURCES.iter().any(|s| m.contains(s)));
        let has_launch = owned
            .invoke_method_map
            .values()
            .any(|m| LAUNCH_SINKS.iter().any(|s| m.contains(s)));
        if has_nested_src && has_launch {
            findings.extend(invoke_scan(
                owned,
                class_name,
                method_name,
                "broadcast_intent_redirect",
                LAUNCH_SINKS,
            ));
        }
    }

    for f in &mut findings {
        if has_install_hint(owned) {
            f.message = format!(
                "{} Install/package-complete broadcast string co-occurs — Galaxy Store AppLinker-style install_complete redirect.",
                f.message
            );
        }
    }

    // Same VF pattern on an exported receiver is the confused-deputy
    // exported_receiver_intent_redirect category (manifest export is AndroHunt-side).
    let extras: Vec<VulnFinding> = findings
        .iter()
        .filter(|f| f.category == "broadcast_intent_redirect")
        .cloned()
        .map(|mut f| {
            f.category = "exported_receiver_intent_redirect".into();
            f.refresh_category_meta();
            f
        })
        .collect();
    findings.extend(extras);

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile::cfg::{BlockEnd, CfgBlock, MethodCfg};
    use std::collections::{HashMap, HashSet};

    fn make_cfg(instruction_offsets: Vec<u32>) -> MethodCfg {
        let block = CfgBlock {
            start_offset: *instruction_offsets.first().unwrap_or(&0),
            end_offset: instruction_offsets.last().copied().unwrap_or(0) + 2,
            end: BlockEnd::Exit,
            instruction_offsets: instruction_offsets.clone(),
        };
        let mut block_by_start = HashMap::new();
        block_by_start.insert(block.start_offset, 0);
        MethodCfg {
            blocks: vec![block],
            block_by_start,
            loop_headers: HashSet::new(),
            entry: 0,
            folded_const_offsets: HashSet::new(),
        }
    }

    #[test]
    fn on_receive_nested_intent_redirect() {
        let mut rw_map = HashMap::new();
        rw_map.insert(0, (vec![], vec![0]));
        rw_map.insert(2, (vec![0], vec![]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(2, "android.content.Context.startActivity".to_string());
        let mut insn_at = HashMap::new();
        insn_at.insert(0, "move-result-object v0".into());
        insn_at.insert(2, "invoke-virtual {v1, v0}, startActivity".into());
        let owned = ValueFlowAnalysisOwned {
            cfg: make_cfg(vec![0, 2]),
            rw_map,
            exceptional_edges: vec![],
            api_return_sources: vec![((0, 0), "android.content.Intent.getParcelableExtra".into())],
            invoke_method_map,
            insn_at,
            registers_size: 0,
            ins_size: 0,
        };
        let findings =
            scan_broadcast_intent_redirect(&owned, "com.example.InstallReceiver", "onReceive");
        assert!(
            findings
                .iter()
                .any(|f| f.category == "broadcast_intent_redirect"),
            "{findings:?}"
        );
    }
}
