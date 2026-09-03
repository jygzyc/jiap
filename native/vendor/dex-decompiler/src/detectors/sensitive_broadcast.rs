//! S2: Implicit broadcast of sensitive extras (IMSI / token / phone / clipboard).
//!
//! Broader than `credential_broadcast` — covers telephony identifiers and auth
//! tokens sent on the public intent bus (Samsung implicit IPC leakage class).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const SENSITIVE_SOURCES: &[&str] = &[
    "getSubscriberId",
    "getImei",
    "getDeviceId",
    "getMeid",
    "getNai",
    "getLine1Number",
    "getSimSerialNumber",
    "getVoiceMailNumber",
    "getAuthToken",
    "getAccessToken",
    "getRefreshToken",
    "getIdToken",
    "getPassword",
    "getToken",
    "getStringExtra",
    "ClipboardManager.getText",
    "getPrimaryClip",
    "getText",
];

const SINKS: &[&str] = &[
    "sendBroadcast",
    "sendOrderedBroadcast",
    "sendStickyBroadcast",
    "sendStickyOrderedBroadcast",
];

const SENSITIVE_HINTS: &[&str] = &[
    "IMSI",
    "MSISDN",
    "IMEI",
    "ICCID",
    "SUBSCRIBER",
    "AUTH_TOKEN",
    "ACCESS_TOKEN",
    "REFRESH_TOKEN",
    "ID_TOKEN",
    "BEARER",
    "API_KEY",
    "APIKEY",
    "PHONE_NUMBER",
    "LINE1NUMBER",
    "SIM_SERIAL",
    "NAI",
];

fn has_sensitive_hint(owned: &ValueFlowAnalysisOwned) -> bool {
    owned.insn_at.values().any(|s| {
        let u = s.to_uppercase();
        SENSITIVE_HINTS.iter().any(|h| u.contains(h))
    })
}

pub fn scan_sensitive_broadcast(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "sensitive_broadcast",
        SENSITIVE_SOURCES,
        SINKS,
    );

    // Co-occurrence: sendBroadcast + sensitive string constants (extra keys).
    if out.is_empty() && has_sensitive_hint(owned) {
        let has_send = owned
            .invoke_method_map
            .values()
            .any(|m| SINKS.iter().any(|s| m.contains(s)));
        if has_send {
            out.extend(invoke_scan(
                owned,
                class_name,
                method_name,
                "sensitive_broadcast",
                SINKS,
            ));
        }
    }

    out
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
    fn imsi_hint_with_send_broadcast() {
        let mut rw_map = HashMap::new();
        rw_map.insert(2, (vec![], vec![]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(2, "android.content.Context.sendBroadcast".to_string());
        let mut insn_at = HashMap::new();
        insn_at.insert(0, "const-string v0, \"imsi\"".into());
        insn_at.insert(2, "invoke-virtual {v1, v2}, sendBroadcast".into());
        let owned = ValueFlowAnalysisOwned {
            cfg: make_cfg(vec![0, 2]),
            rw_map,
            exceptional_edges: vec![],
            api_return_sources: vec![],
            invoke_method_map,
            insn_at,
            registers_size: 0,
            ins_size: 0,
        };
        let findings = scan_sensitive_broadcast(&owned, "com.example.Leak", "share");
        assert!(
            findings.iter().any(|f| f.category == "sensitive_broadcast"),
            "{findings:?}"
        );
    }
}
