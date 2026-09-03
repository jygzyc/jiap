//! Nested Intent redirection and FLAG_GRANT smuggling on redirected Intents.
//!
//! Play / Android docs: untrusted nested Intent from extras launched via
//! startActivity/startService/bindService is a confused-deputy classic.
//! Smuggling FLAG_GRANT_* on that Intent (or via addFlags/setFlags on Intent
//! extras) unlocks private providers / FileProviders.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const NESTED_SOURCES: &[&str] = &[
    "getParcelableExtra",
    "getParcelableArrayExtra",
    "getParcelableArrayListExtra",
    "getParcelable",
    // Older / Bundle paths often used with nested Intents.
    "getSerializableExtra",
];

const LAUNCH_SINKS: &[&str] = &[
    "startActivity",
    "startActivityForResult",
    "startService",
    "startForegroundService",
    "bindService",
    "sendBroadcast",
    "sendOrderedBroadcast",
];

const GRANT_SINKS: &[&str] = &[
    "addFlags",
    "Intent.addFlags",
    "setFlags",
    "Intent.setFlags",
    "setClipData",
    "Intent.setClipData",
    "grantUriPermission",
    "takePersistableUriPermission",
    "setData",
    "Intent.setData",
    "setDataAndType",
    "Intent.setDataAndType",
];

/// Nested Intent redirect + URI grant smuggling surfaces in one method.
pub fn scan_intent_redirect(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut findings = Vec::new();

    // R1: parcelable/nested Intent → component launch.
    findings.extend(source_sink_scan(
        owned,
        class_name,
        method_name,
        "intent_redirect_nested",
        NESTED_SOURCES,
        LAUNCH_SINKS,
    ));

    // R2a: nested Intent → flag / grant APIs (FLAG_GRANT_* smuggling).
    findings.extend(source_sink_scan(
        owned,
        class_name,
        method_name,
        "intent_redirect_grant_smuggle",
        NESTED_SOURCES,
        GRANT_SINKS,
    ));

    // R2b: method both launches a nested Intent and touches grant flags
    // (co-occurrence strengthens smuggle signal even if VF misses a hop).
    let has_nested_launch = findings
        .iter()
        .any(|f| f.category == "intent_redirect_nested");
    if has_nested_launch {
        let flag_invokes = invoke_scan(
            owned,
            class_name,
            method_name,
            "intent_redirect_grant_smuggle",
            &["addFlags", "setFlags", "setClipData", "grantUriPermission"],
        );
        for mut f in flag_invokes {
            // Avoid duplicate category+offset noise if VF already reported grant flow.
            if findings.iter().any(|e| {
                e.category == "intent_redirect_grant_smuggle" && e.sink_offset == f.sink_offset
            }) {
                continue;
            }
            f.message = format!(
                "Nested Intent launch co-occurs with {} in the same method — review FLAG_GRANT_* smuggling on the redirected Intent.",
                f.sink_desc
            );
            f.refresh_category_meta();
            findings.push(f);
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile::cfg::{BlockEnd, CfgBlock, MethodCfg};
    use crate::decompile::value_flow::ValueFlowAnalysisOwned;
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
    fn nested_parcelable_to_start_activity() {
        let mut rw_map = HashMap::new();
        rw_map.insert(0, (vec![], vec![0]));
        rw_map.insert(2, (vec![0], vec![]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(2, "android.app.Activity.startActivity".to_string());
        let mut insn_at = HashMap::new();
        insn_at.insert(0, "move-result-object v0".into());
        insn_at.insert(2, "invoke-virtual {v0}, startActivity".into());
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
        let findings = scan_intent_redirect(&owned, "com.example.Login", "onCreate");
        assert!(
            findings
                .iter()
                .any(|f| f.category == "intent_redirect_nested"),
            "{findings:?}"
        );
    }
}
