//! Intent ACTION_PICK / GET_CONTENT → local copy (file theft via result URI).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, VulnFinding};

const PICK_HINTS: &[&str] = &[
    "ACTION_PICK",
    "ACTION_GET_CONTENT",
    "android.intent.action.PICK",
    "android.intent.action.GET_CONTENT",
    "MediaStore",
    "content://",
];

const COPY_SINKS: &[&str] = &[
    "copyToCache",
    "openInputStream",
    "ContentResolver.openInputStream",
    "FileOutputStream",
    "startActivityForResult",
];

pub fn scan_pick_file_theft(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let hint = owned.insn_at.values().any(|s| {
        let u = s.to_uppercase();
        PICK_HINTS.iter().any(|p| u.contains(&p.to_uppercase()))
    }) || owned
        .invoke_method_map
        .values()
        .any(|m| m.contains("setAction") || m.contains("MediaStore"));
    if !hint {
        return Vec::new();
    }
    let has_pick_string = owned.insn_at.values().any(|s| {
        let u = s.to_uppercase();
        u.contains("ACTION_PICK")
            || u.contains("ACTION_GET_CONTENT")
            || u.contains("ANDROID.INTENT.ACTION.PICK")
            || u.contains("GET_CONTENT")
            || u.contains("MEDIASTORE")
    });
    if !has_pick_string
        && !owned.invoke_method_map.values().any(|m| {
            method_matches_any(
                m,
                &["startActivityForResult", "copyToCache", "openInputStream"],
            )
        })
    {
        return Vec::new();
    }
    let mut out = invoke_scan(
        owned,
        class_name,
        method_name,
        "pick_file_theft",
        COPY_SINKS,
    );
    if out.is_empty() && has_pick_string {
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "pick_file_theft",
            &["startActivityForResult", "startActivity"],
        ));
    }
    out.truncate(1);
    out
}
