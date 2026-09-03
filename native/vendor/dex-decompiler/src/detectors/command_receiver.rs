//! S3: Unprotected command BroadcastReceiver (FactoryCamera-style).
//!
//! Exported receivers that perform dangerous side effects from `onReceive`
//! without needing a nested Intent (camera, exec, component enable, file write).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const INTENT_SOURCES: &[&str] = &[
    "getStringExtra",
    "getIntExtra",
    "getBooleanExtra",
    "getData",
    "getDataString",
    "getAction",
    "getExtras",
];

/// Sinks that are dangerous when driven by a broadcast Intent.
const COMMAND_SINKS: &[&str] = &[
    "Runtime.exec",
    "ProcessBuilder",
    "ProcessBuilder.start",
    "exec",
    "MediaRecorder.start",
    "MediaRecorder",
    "Camera.open",
    "Camera.startPreview",
    "setComponentEnabledSetting",
    "PackageManager.setComponentEnabledSetting",
    "openFileOutput",
    "FileOutputStream",
    "deleteFile",
    "loadUrl",
    "evaluateJavascript",
    "grantUriPermission",
    "revokeUriPermission",
    "installPackage",
    "deletePackage",
    "PackageInstaller",
    "DevicePolicyManager",
    "lockNow",
    "wipeData",
    "setCameraDisabled",
];

fn is_receiver_entry(class_name: &str, method_name: &str) -> bool {
    method_name == "onReceive"
        || (method_name.starts_with("on")
            && (class_name.contains("Receiver") || class_name.contains("Broadcast")))
}

pub fn scan_command_receiver(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    if !is_receiver_entry(class_name, method_name) {
        return Vec::new();
    }

    let mut findings = source_sink_scan(
        owned,
        class_name,
        method_name,
        "command_receiver",
        INTENT_SOURCES,
        COMMAND_SINKS,
    );

    // Invoke-only: dangerous API in onReceive even if VF misses the hop.
    if findings.is_empty() {
        let hits = invoke_scan(
            owned,
            class_name,
            method_name,
            "command_receiver",
            COMMAND_SINKS,
        );
        // Skip bare `exec` substring noise — require a recognizable dangerous API.
        for f in hits {
            let sink = f.sink_desc.to_lowercase();
            let strong = [
                "runtime.exec",
                "processbuilder",
                "mediarecorder",
                "camera.open",
                "setcomponentenabled",
                "openfileoutput",
                "fileoutputstream",
                "loadurl",
                "granturipermission",
                "installpackage",
                "deletepackage",
                "packageinstaller",
                "devicepolicymanager",
                "locknow",
                "wipedata",
            ]
            .iter()
            .any(|s| sink.contains(s));
            if strong {
                findings.push(f);
            }
        }
    }

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
    fn on_receive_runtime_exec() {
        let mut rw_map = HashMap::new();
        rw_map.insert(2, (vec![], vec![]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(2, "java.lang.Runtime.exec".to_string());
        let mut insn_at = HashMap::new();
        insn_at.insert(2, "invoke-virtual {v0, v1}, exec".into());
        let owned = ValueFlowAnalysisOwned {
            cfg: make_cfg(vec![2]),
            rw_map,
            exceptional_edges: vec![],
            api_return_sources: vec![],
            invoke_method_map,
            insn_at,
            registers_size: 0,
            ins_size: 0,
        };
        let findings =
            scan_command_receiver(&owned, "com.sec.factory.camera.TestReceiver", "onReceive");
        assert!(
            findings.iter().any(|f| f.category == "command_receiver"),
            "{findings:?}"
        );
    }
}
