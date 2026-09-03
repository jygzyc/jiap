//! Next-wave detectors (roadmap N2–N5, D1–D5, W1–W6 VF pieces).
//!
//! Hunt-layer enrichments (N6–N8, S*, Q*) live in androhunt `hunt/mod.rs`.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const USER_SOURCES: &[&str] = &[
    "getStringExtra",
    "getCharSequenceExtra",
    "getData",
    "getDataString",
    "getQueryParameter",
    "getIntent",
    "getParcelableExtra",
];

/// N2: Context.registerReceiver without an obvious permission argument nearby.
pub fn scan_dynamic_receiver(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = invoke_scan(
        owned,
        class_name,
        method_name,
        "dynamic_register_receiver",
        &[
            "registerReceiver",
            "Context.registerReceiver",
            "ContextCompat.registerReceiver",
            "LocalBroadcastManager.registerReceiver",
        ],
    );
    // LocalBroadcastManager is same-UID — demote by skipping.
    out.retain(|f| !f.sink_desc.contains("LocalBroadcastManager"));
    // If permission string constant co-occurs, skip (likely gated).
    let has_perm_hint = owned.insn_at.values().any(|s| {
        let u = s.to_uppercase();
        u.contains("PERMISSION") && (u.contains("SIGNATURE") || u.contains("CUSTOM"))
    });
    if has_perm_hint {
        return Vec::new();
    }
    out
}

/// N3: bindService / AIDL Stub.asInterface with Intent-controlled component.
pub fn scan_exported_binder(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "binder_intent_control",
        USER_SOURCES,
        &["bindService", "bindServiceAsUser", "Context.bindService"],
    );
    out.extend(invoke_scan(
        owned,
        class_name,
        method_name,
        "aidl_stub_as_interface",
        &["Stub.asInterface", ".Stub.asInterface", "asInterface"],
    ));
    // Only keep asInterface when bindService also present in method (reduce noise).
    let has_bind = owned
        .invoke_method_map
        .values()
        .any(|m| m.contains("bindService"));
    if !has_bind {
        out.retain(|f| f.category != "aidl_stub_as_interface");
    }
    out
}

/// N5: Room / SupportSQLiteDatabase rawQuery with user input.
pub fn scan_room_sql(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    source_sink_scan(
        owned,
        class_name,
        method_name,
        "room_sql_injection",
        USER_SOURCES,
        &[
            "SupportSQLiteDatabase.query",
            "SupportSQLiteDatabase.execSQL",
            "RoomDatabase.query",
            "SimpleSQLiteQuery",
            "rawQueryWithFactory",
        ],
    )
}

/// D1/D2: Activity Result API / contracts forwarding grants.
pub fn scan_activity_result_grant(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = Vec::new();
    let is_result_cb = method_name == "onActivityResult"
        || method_name.contains("ActivityResult")
        || method_name == "onReceive"
        || method_name.starts_with("on");
    if method_name == "onActivityResult"
        || method_name.contains("parseResult")
        || method_name.contains("ActivityResultCallback")
    {
        out.extend(source_sink_scan(
            owned,
            class_name,
            method_name,
            "activity_result_grant_smuggle",
            &[
                "getData",
                "getDataString",
                "getClipData",
                "getParcelableExtra",
                "Intent",
            ],
            &[
                "setResult",
                "addFlags",
                "setFlags",
                "setClipData",
                "grantUriPermission",
                "takePersistableUriPermission",
            ],
        ));
    }
    if is_result_cb {
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "activity_result_contracts",
            &[
                "ActivityResultContracts",
                "registerForActivityResult",
                "StartActivityForResult",
                "GetContent",
                "OpenDocument",
                "CreateDocument",
            ],
        ));
    }
    out
}

/// D3: Intent redirect without IntentSanitizer / removeFlags nearby.
pub fn scan_intent_sanitizer_gap(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_nested = owned.api_return_sources.iter().any(|(_, m)| {
        m.contains("getParcelableExtra")
            || m.contains("getParcelableArrayExtra")
            || m.contains("getSerializableExtra")
    });
    let has_launch = owned.invoke_method_map.values().any(|m| {
        m.contains("startActivity")
            || m.contains("startService")
            || m.contains("bindService")
            || m.contains("sendBroadcast")
    });
    let has_sanitize = owned.invoke_method_map.values().any(|m| {
        m.contains("IntentSanitizer")
            || m.contains("removeFlags")
            || m.contains("IntentFilter.check")
    });
    if !(has_nested && has_launch) || has_sanitize {
        return Vec::new();
    }
    invoke_scan(
        owned,
        class_name,
        method_name,
        "intent_redirect_no_sanitizer",
        &[
            "startActivity",
            "startActivityForResult",
            "startService",
            "bindService",
        ],
    )
}

/// D4: SliceProvider / SliceManager surface.
pub fn scan_slice_provider(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = invoke_scan(
        owned,
        class_name,
        method_name,
        "slice_provider_api",
        &[
            "SliceManager",
            "onBindSlice",
            "SliceProvider",
            "bindSlice",
            "mapIntentToUri",
            "Slice.Builder",
        ],
    );
    if class_name.contains("SliceProvider")
        && (method_name == "onBindSlice" || method_name == "onCreatePermissionRequest")
    {
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "slice_provider_api",
            &["onBindSlice", "createPermissionRequest"],
        ));
    }
    out
}

/// W1: shouldOverrideUrlLoading / shouldInterceptRequest with Intent URL.
pub fn scan_webview_url_override(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "webview_url_override",
        USER_SOURCES,
        &[
            "shouldOverrideUrlLoading",
            "shouldInterceptRequest",
            "loadUrl",
            "loadDataWithBaseURL",
        ],
    );
    if method_name == "shouldOverrideUrlLoading" || method_name == "shouldInterceptRequest" {
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "webview_url_override",
            &["loadUrl", "loadData", "evaluateJavascript", "startActivity"],
        ));
    }
    out
}

/// W2: postMessage / evaluateJavascript from extras; JS bridge on file/cache URL.
pub fn scan_webview_postmessage(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "webview_postmessage",
        USER_SOURCES,
        &[
            "evaluateJavascript",
            "postMessage",
            "loadData",
            "loadDataWithBaseURL",
        ],
    );
    let has_bridge = owned
        .invoke_method_map
        .values()
        .any(|m| m.contains("addJavascriptInterface"));
    let has_fileish = owned.insn_at.values().any(|s| {
        let l = s.to_lowercase();
        l.contains("file://") || l.contains("content://") || l.contains("/cache/")
    });
    if has_bridge && has_fileish {
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "webview_js_bridge_file_url",
            &["addJavascriptInterface"],
        ));
    }
    out
}

/// W4: Intent URL → OkHttp / HttpURLConnection (SSRF-ish).
pub fn scan_intent_url_ssrf(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    source_sink_scan(
        owned,
        class_name,
        method_name,
        "intent_url_network_fetch",
        USER_SOURCES,
        &[
            "Request.Builder.url",
            "OkHttpClient",
            "HttpURLConnection",
            "openConnection",
            "URL.<init>",
            "java.net.URL",
            "Call.execute",
            "Call.enqueue",
        ],
    )
}

/// W5 helper pieces: trust-all co-located with loadUrl is enriched in hunt.
pub fn scan_ssl_webview_colocate(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let has_ssl = owned.invoke_method_map.values().any(|m| {
        m.contains("checkServerTrusted")
            || m.contains("setHostnameVerifier")
            || m.contains("TrustAll")
            || m.contains("ALLOW_ALL")
    });
    let has_wv = owned
        .invoke_method_map
        .values()
        .any(|m| m.contains("loadUrl") || m.contains("evaluateJavascript"));
    if !(has_ssl && has_wv) {
        return Vec::new();
    }
    invoke_scan(
        owned,
        class_name,
        method_name,
        "ssl_bypass_webview_colocate",
        &["loadUrl", "setHostnameVerifier", "checkServerTrusted"],
    )
}

/// Bundle for `run_all_detectors`.
pub fn scan_next_wave(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut all = Vec::new();
    all.extend(scan_dynamic_receiver(owned, class_name, method_name));
    all.extend(scan_exported_binder(owned, class_name, method_name));
    all.extend(scan_room_sql(owned, class_name, method_name));
    all.extend(scan_activity_result_grant(owned, class_name, method_name));
    all.extend(scan_intent_sanitizer_gap(owned, class_name, method_name));
    all.extend(scan_slice_provider(owned, class_name, method_name));
    all.extend(scan_webview_url_override(owned, class_name, method_name));
    all.extend(scan_webview_postmessage(owned, class_name, method_name));
    all.extend(scan_intent_url_ssrf(owned, class_name, method_name));
    all.extend(scan_ssl_webview_colocate(owned, class_name, method_name));
    all
}
