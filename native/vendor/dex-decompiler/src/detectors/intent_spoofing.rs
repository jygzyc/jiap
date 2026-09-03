//! Intent spoofing / interception: taint from getIntent/getData → startActivity/setResult.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{source_sink_scan, VulnFinding};

const INTENT_SOURCES: &[&str] = &[
    "getIntent",
    "getData",
    "getStringExtra",
    "getDataString",
    "getExtras",
    "getCharSequenceExtra",
    "getParcelableExtra",
    "getSerializableExtra",
    "getQueryParameter",
    "getLastPathSegment",
];
const INTENT_SINKS: &[&str] = &[
    "startActivity",
    "startActivityForResult",
    "setResult",
    "sendBroadcast",
    "sendOrderedBroadcast",
    "startService",
    "bindService",
];

pub fn scan_intent_spoofing(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    source_sink_scan(
        owned,
        class_name,
        method_name,
        "intent_spoofing",
        INTENT_SOURCES,
        INTENT_SINKS,
    )
}
