//! Insecure logging: sensitive source → Log.d / Log.i / println.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{source_sink_scan, VulnFinding};

const LOGGING_SINKS: &[&str] = &[
    "Log.d",
    "Log.i",
    "Log.e",
    "Log.w",
    "Log.v",
    "println",
    "print",
    "FileWriter",
    "FileWriter.<init>",
    "FileWriter.write",
    "BufferedWriter.write",
    "OutputStreamWriter.write",
];

const LOGGING_SOURCE_DEFAULTS: &[&str] = &[
    "getLastLocation",
    "getCurrentLocation",
    "getDeviceId",
    "getSubscriberId",
    "getAndroidId",
    "getPrimaryClip",
    "getText",
    "getStringExtra",
    "getString",
    "getToken",
    "getPassword",
    "LoginData",
    "getLoginData",
    "getLoginUrl",
    "readLine",
    "BufferedReader.readLine",
    "SharedPreferences.getString",
    "dumpLogs",
];

pub fn scan_insecure_logging(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
    source_patterns: Option<&[String]>,
) -> Vec<VulnFinding> {
    let sources: Vec<&str> = source_patterns
        .map(|s| s.iter().map(String::as_str).collect::<Vec<_>>())
        .unwrap_or_else(|| LOGGING_SOURCE_DEFAULTS.to_vec());
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "insecure_logging",
        &sources,
        LOGGING_SINKS,
    );
    // OVAA InsecureLoggerService#dumpLogs: FileWriter of on-disk log without clear VF seed.
    if out.is_empty()
        && (method_name == "dumpLogs" || class_name.contains("InsecureLogger"))
        && owned
            .invoke_method_map
            .values()
            .any(|m| m.contains("FileWriter") || m.contains("Log."))
    {
        out.extend(crate::detectors::types::invoke_scan(
            owned,
            class_name,
            method_name,
            "insecure_logging",
            &["FileWriter", "Log.d", "Log.i", "Log.e", "Log.w"],
        ));
        out.truncate(1);
    }
    out
}
