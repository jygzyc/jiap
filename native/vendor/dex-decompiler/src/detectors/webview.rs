//! WebView: user input → loadUrl/loadDataWithBaseURL; JS bridge; file access; cookies.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, method_matches_any, source_sink_scan, VulnFinding};

const WEBVIEW_SOURCES: &[&str] = &[
    "getStringExtra",
    "getText",
    "getData",
    "getDataString",
    "getQueryParameter",
    "getIntent",
    "getParcelableExtra",
    "getEncodedPath",
    "getLastPathSegment",
    "getPath",
];
const WEBVIEW_SINKS: &[&str] = &[
    "loadUrl",
    "loadDataWithBaseURL",
    "loadData",
    "evaluateJavascript",
];
const JAVASCRIPT_INTERFACE_PATTERNS: &[&str] =
    &["addJavascriptInterface", "addJavascriptInterfaceOut"];
const URI_PATH_SOURCES: &[&str] = &["getEncodedPath", "getLastPathSegment", "getPath"];
const FILE_ACCESS_PATTERNS: &[&str] = &[
    "setAllowFileAccessFromFileURLs",
    "setAllowUniversalAccessFromFileURLs",
    "setAllowFileAccess",
];
const COOKIE_APIS: &[&str] = &[
    "CookieManager.getCookie",
    "CookieManager.setCookie",
    "android.webkit.CookieManager.getCookie",
    "android.webkit.CookieManager.setCookie",
];

pub fn scan_webview_unsafe(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "webview_unsafe_url",
        WEBVIEW_SOURCES,
        WEBVIEW_SINKS,
    );
    out.extend(invoke_scan(
        owned,
        class_name,
        method_name,
        "webview_javascript_interface",
        JAVASCRIPT_INTERFACE_PATTERNS,
    ));
    out.extend(invoke_scan(
        owned,
        class_name,
        method_name,
        "webview_file_access",
        FILE_ACCESS_PATTERNS,
    ));

    let has_bridge = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, JAVASCRIPT_INTERFACE_PATTERNS));
    let has_user_load = !source_sink_scan(
        owned,
        class_name,
        method_name,
        "webview_unsafe_url",
        WEBVIEW_SOURCES,
        WEBVIEW_SINKS,
    )
    .is_empty();
    if has_bridge && has_user_load {
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "webview_js_bridge_user_url",
            &["addJavascriptInterface", "loadUrl", "evaluateJavascript"],
        ));
        // Deeplink triad at DEX level: getDataString / Deeplink* + JS bridge + user URL.
        let deeplink = class_name.contains("Deeplink")
            || method_name.to_ascii_lowercase().contains("deeplink")
            || owned
                .invoke_method_map
                .values()
                .any(|m| m.contains("getDataString") || m.contains("getData"));
        if deeplink {
            out.extend(invoke_scan(
                owned,
                class_name,
                method_name,
                "deeplink_webview_js_bridge",
                &["addJavascriptInterface", "loadUrl", "getDataString"],
            ));
        }
        let has_uri_path = owned
            .invoke_method_map
            .values()
            .any(|m| method_matches_any(m, URI_PATH_SOURCES));
        if has_uri_path {
            out.extend(invoke_scan(
                owned,
                class_name,
                method_name,
                "deeplink_webview_path_traversal",
                &[
                    "addJavascriptInterface",
                    "loadUrl",
                    "getEncodedPath",
                    "getLastPathSegment",
                    "getPath",
                ],
            ));
        }
    }

    // TikTok CVE-2022-28799 impact path: attacker URL in WebView → cookie/token exfil.
    let has_cookie = owned
        .invoke_method_map
        .values()
        .any(|m| method_matches_any(m, COOKIE_APIS));
    if has_user_load && has_cookie {
        let mut cookie = invoke_scan(
            owned,
            class_name,
            method_name,
            "webview_cookie_exfil",
            COOKIE_APIS,
        );
        cookie.truncate(1);
        out.extend(cookie);
    }

    out
}
