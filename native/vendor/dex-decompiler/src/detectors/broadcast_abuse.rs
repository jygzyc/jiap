//! Sticky / ordered broadcast send sites (confused-deputy / hijack surface).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, VulnFinding};

const PATTERNS: &[&str] = &[
    "sendOrderedBroadcast",
    "sendStickyBroadcast",
    "sendStickyOrderedBroadcast",
];

pub fn scan_broadcast_abuse(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    invoke_scan(
        owned,
        class_name,
        method_name,
        "sticky_ordered_broadcast",
        PATTERNS,
    )
}
