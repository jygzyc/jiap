//! ZIP path traversal (ZipSlip): ZipEntry.getName → File / FileOutputStream.
//!
//! Classic pattern (Quokka Uhale CVE-2025-58391 / zeroturnaround Zips.process):
//! `new File(destination, zipEntry.getName())` without canonical-path checks
//! lets `../` entries escape the extract directory.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const ZIP_NAME_SOURCES: &[&str] = &[
    "ZipEntry.getName",
    "java.util.zip.ZipEntry.getName",
    "org.apache.commons.compress.archivers.zip.ZipArchiveEntry.getName",
];

const FILE_SINKS: &[&str] = &[
    "File.<init>",
    "java.io.File.<init>",
    "FileOutputStream.<init>",
    "java.io.FileOutputStream.<init>",
    "FileInputStream.<init>",
    "java.io.FileInputStream.<init>",
    "Files.newOutputStream",
    "Files.write",
    "FileUtils.copy",
    "FileUtils.copyInputStreamToFile",
    "IOUtils.copy",
];

/// Detect ZipSlip-style extraction (entry name → File path).
pub fn scan_zip_slip(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut findings = source_sink_scan(
        owned,
        class_name,
        method_name,
        "zip_slip",
        ZIP_NAME_SOURCES,
        FILE_SINKS,
    );

    // Co-occurrence fallback: ZipEntry.getName + File.<init> in same method
    // without getCanonicalPath / startsWith destination check (common in libs).
    if findings.is_empty() {
        let has_zip_name = owned
            .invoke_method_map
            .values()
            .any(|m| m.contains("ZipEntry.getName") || m.contains("ZipArchiveEntry.getName"));
        let has_file = owned.invoke_method_map.values().any(|m| {
            m.contains("File.<init>")
                || m.contains("FileOutputStream")
                || m.contains("FileUtils.copy")
        });
        let has_canon = owned.invoke_method_map.values().any(|m| {
            m.contains("getCanonicalPath")
                || m.contains("getCanonicalFile")
                || m.contains("toRealPath")
        });
        if has_zip_name && has_file && !has_canon {
            findings.extend(invoke_scan(
                owned,
                class_name,
                method_name,
                "zip_slip",
                &["ZipEntry.getName", "File.<init>", "FileOutputStream.<init>"],
            ));
            findings.truncate(1);
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
    fn zip_entry_name_to_file() {
        let mut rw_map = HashMap::new();
        rw_map.insert(0, (vec![], vec![0]));
        rw_map.insert(2, (vec![0], vec![]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(2, "java.io.File.<init>".to_string());
        let mut insn_at = HashMap::new();
        insn_at.insert(0, "move-result-object v0".into());
        insn_at.insert(2, "invoke-direct {v1, v0}, File.<init>".into());
        let owned = ValueFlowAnalysisOwned {
            cfg: make_cfg(vec![0, 2]),
            rw_map,
            exceptional_edges: vec![],
            api_return_sources: vec![((0, 0), "java.util.zip.ZipEntry.getName".into())],
            invoke_method_map,
            insn_at,
            registers_size: 0,
            ins_size: 0,
        };
        let findings = scan_zip_slip(&owned, "org.zeroturnaround.zip.Zips", "process");
        assert!(
            findings.iter().any(|f| f.category == "zip_slip"),
            "{findings:?}"
        );
    }
}
