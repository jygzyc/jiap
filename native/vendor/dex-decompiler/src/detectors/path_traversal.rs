//! Path traversal / ContentProvider openFile: Uri path → File without canonical checks.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, source_sink_scan, VulnFinding};

const SOURCES: &[&str] = &[
    "getLastPathSegment",
    "getPath",
    "getQueryParameter",
    "getStringExtra",
    "Uri.getPath",
    "getEncodedPath",
];
const SINKS: &[&str] = &[
    "File.<init>",
    "java.io.File.<init>",
    "openFile",
    "openAssetFile",
    "openTypedAssetFile",
    "ParcelFileDescriptor.open",
    "FileInputStream.<init>",
    "FileOutputStream.<init>",
];

const PROVIDER_OPEN: &[&str] = &["openFile", "openAssetFile", "openTypedAssetFile"];

pub fn scan_path_traversal(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut findings = source_sink_scan(
        owned,
        class_name,
        method_name,
        "path_traversal",
        SOURCES,
        SINKS,
    );

    // R4: ContentProvider openFile/openAssetFile without getCanonicalPath containment.
    let is_provider_open = PROVIDER_OPEN.iter().any(|p| method_name.contains(p))
        || owned
            .invoke_method_map
            .values()
            .any(|m| method_matches_any(m, PROVIDER_OPEN));
    let has_uri_path = owned.invoke_method_map.values().any(|m| {
        method_matches_any(
            m,
            &[
                "getLastPathSegment",
                "getPath",
                "getEncodedPath",
                "Uri.getPath",
            ],
        )
    }) || owned.api_return_sources.iter().any(|(_, s)| {
        s.contains("getLastPathSegment") || s.contains("getPath") || s.contains("getEncodedPath")
    });
    let has_file = owned.invoke_method_map.values().any(|m| {
        m.contains("File.<init>")
            || m.contains("FileInputStream")
            || m.contains("FileOutputStream")
            || m.contains("ParcelFileDescriptor.open")
    });
    let has_canon = owned.invoke_method_map.values().any(|m| {
        m.contains("getCanonicalPath") || m.contains("getCanonicalFile") || m.contains("toRealPath")
    });

    if is_provider_open && has_uri_path && has_file && !has_canon && findings.is_empty() {
        findings.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "provider_path_traversal",
            &["openFile", "File.<init>", "getLastPathSegment", "getPath"],
        ));
        findings.truncate(1);
    } else if is_provider_open && has_uri_path && !has_canon {
        // Escalate existing path_traversal on provider open methods.
        for f in &mut findings {
            if f.category == "path_traversal" {
                f.category = "provider_path_traversal".into();
                f.refresh_category_meta();
            }
        }
    }

    findings
}
