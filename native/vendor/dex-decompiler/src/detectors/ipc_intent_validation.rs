//! IPC / intent validation: getIntent → startActivity/setResult (validate target).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::intent_spoofing::scan_intent_spoofing;
use crate::detectors::types::VulnFinding;

pub fn scan_ipc_intent_validation(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    scan_intent_spoofing(owned, class_name, method_name)
        .into_iter()
        .map(|mut f| {
            f.category = "ipc_intent_validation".to_string();
            f.refresh_category_meta();
            f
        })
        .collect()
}
