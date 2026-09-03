//! Custom Tabs / Trusted Web Activity: Intent-controlled URL → browser session.
//!
//! Oversecured / bounty pattern: deeplink or getStringExtra URL passed to
//! CustomTabsIntent.launchUrl can leak auth tokens / open phishing pages in
//! an app-branded chrome session.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, source_sink_scan, VulnFinding};

const URL_SOURCES: &[&str] = &[
    "getStringExtra",
    "getData",
    "getDataString",
    "getQueryParameter",
    "getIntent",
    "Uri.parse",
];

const CUSTOM_TAB_SINKS: &[&str] = &[
    "CustomTabsIntent.launchUrl",
    "launchUrl",
    "CustomTabsIntent$Builder.build",
    "TrustedWebActivityIntent",
    "TwaLauncher.launch",
    "launchTwa",
];

pub fn scan_custom_tabs(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut findings = source_sink_scan(
        owned,
        class_name,
        method_name,
        "custom_tabs_intent_url",
        URL_SOURCES,
        CUSTOM_TAB_SINKS,
    );

    if findings.is_empty() {
        let has_ct = owned.invoke_method_map.values().any(|m| {
            method_matches_any(
                m,
                &[
                    "CustomTabsIntent",
                    "launchUrl",
                    "TrustedWebActivity",
                    "TwaLauncher",
                ],
            )
        });
        let has_url_src = owned
            .invoke_method_map
            .values()
            .any(|m| method_matches_any(m, URL_SOURCES))
            || owned
                .api_return_sources
                .iter()
                .any(|(_, s)| URL_SOURCES.iter().any(|p| s.contains(p)));
        if has_ct && has_url_src {
            findings.extend(invoke_scan(
                owned,
                class_name,
                method_name,
                "custom_tabs_intent_url",
                &[
                    "CustomTabsIntent.launchUrl",
                    "launchUrl",
                    "CustomTabsIntent",
                ],
            ));
            findings.truncate(1);
        }
    }

    findings
}
