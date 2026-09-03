//! Tracker / advertising / fingerprinting SDK invoke inventory.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, VulnFinding};

const TRACKERS: &[&str] = &[
    "AdvertisingIdClient",
    "getAdvertisingIdInfo",
    "AppsFlyerLib",
    "Adjust.getDefaultInstance",
    "FirebaseAnalytics",
    "logEvent",
    "AppEventsLogger",
    "Branch.getInstance",
    "MixpanelAPI",
    "AmplitudeClient",
    "Crashlytics",
    "Fabric.with",
];

pub fn scan_tracker_inventory(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    invoke_scan(
        owned,
        class_name,
        method_name,
        "tracker_fingerprint_api",
        TRACKERS,
    )
}
