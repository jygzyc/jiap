//! Vulnerability detectors: one module per detector, shared types and run_all.
//!
//! Each detector lives in its own `.rs` file. Use `run_all_detectors` to run
//! every detector (except PendingIntent, which has its own finding type and CLI flag).

mod biometric;
mod broadcast_abuse;
mod broadcast_redirect;
mod command_receiver;
mod credential_broadcast;
mod custom_tabs;
mod hardcoded_secrets;
mod implicit_intent;
mod insecure_logging;
mod intent_parse_uri;
mod intent_redirect;
mod intent_spoofing;
mod ipc_intent_validation;
mod keystore;
mod logcat_external;
mod next_wave;
mod package_context;
mod path_traversal;
pub mod pending_intent;
mod pick_file_theft;
mod pinning_bypass;
mod rce_dynamic_loading;
mod reflection_rce;
mod sensitive_broadcast;
mod sql_injection;
mod sqlcipher_passphrase;
mod ssl_trust_all;
mod storage_mode;
mod trackers;
mod types;
mod unsafe_deserialization;
mod uri_grant;
mod weak_crypto;
mod weak_host_check;
mod webresource_response;
mod webview;
mod zip_slip;

pub use biometric::scan_biometric_misuse;
pub use broadcast_abuse::scan_broadcast_abuse;
pub use broadcast_redirect::scan_broadcast_intent_redirect;
pub use command_receiver::scan_command_receiver;
pub use credential_broadcast::scan_credential_broadcast;
pub use custom_tabs::scan_custom_tabs;
pub use hardcoded_secrets::scan_hardcoded_secrets;
pub use implicit_intent::scan_implicit_intent;
pub use insecure_logging::scan_insecure_logging;
pub use intent_parse_uri::scan_intent_parse_uri;
pub use intent_redirect::scan_intent_redirect;
pub use intent_spoofing::scan_intent_spoofing;
pub use ipc_intent_validation::scan_ipc_intent_validation;
pub use keystore::scan_keystore_misuse;
pub use logcat_external::scan_logcat_external;
pub use next_wave::scan_next_wave;
pub use package_context::scan_package_context_ace;
pub use path_traversal::scan_path_traversal;
pub use pending_intent::{scan_pending_intents, PendingIntentFinding};
pub use pick_file_theft::scan_pick_file_theft;
pub use pinning_bypass::scan_pinning_bypass;
pub use rce_dynamic_loading::scan_rce_dynamic_loading;
pub use reflection_rce::scan_reflection_rce;
pub use sensitive_broadcast::scan_sensitive_broadcast;
pub use sql_injection::scan_sql_injection;
pub use sqlcipher_passphrase::scan_sqlcipher_passphrase;
pub use ssl_trust_all::scan_ssl_trust_all;
pub use storage_mode::scan_storage_mode;
pub use trackers::scan_tracker_inventory;
pub use types::{
    category_meta, invoke_scan, method_matches_any, source_sink_scan, CategoryMeta, VulnFinding,
    VulnTraceStep,
};
pub use unsafe_deserialization::scan_unsafe_deserialization;
pub use uri_grant::scan_uri_grant;
pub use weak_crypto::scan_weak_crypto;
pub use weak_host_check::scan_weak_host_validation;
pub use webresource_response::scan_webresource_response;
pub use webview::scan_webview_unsafe;
pub use zip_slip::scan_zip_slip;

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::decompile::Decompiler;
use crate::java::descriptor_to_java;
use dex_parser::{DexFile, EncodedMethod};
use rayon::prelude::*;
use std::collections::HashSet;

/// Lab / CTF apps that reuse the `com.android.*` namespace but are **not** AOSP.
/// Matched as a package root only (`root` or `root.…`) — never a bare `com.android` prefix.
const COM_ANDROID_LAB_ROOTS: &[&str] = &["com.android.insecurebankv2"];

/// True when `class_name` is under an allowlisted lab package (not real platform code).
pub fn is_com_android_lab_app(class_name: &str) -> bool {
    let n = class_name.trim().trim_start_matches('.');
    COM_ANDROID_LAB_ROOTS
        .iter()
        .any(|root| n == *root || n.starts_with(&format!("{root}.")))
}

/// True for Android platform / AndroidX / common SDK library classes (not app code).
/// Used so vuln detectors do not flag androidx.* / android.* noise.
///
/// Real `com.android.*` / `com.google.*` stay library. Only explicit lab roots in
/// [`COM_ANDROID_LAB_ROOTS`] are treated as app code.
pub fn is_library_class(class_name: &str) -> bool {
    let n = class_name.trim();
    if is_com_android_lab_app(n) {
        return false;
    }
    n == "android"
        || n.starts_with("android.")
        || n == "androidx"
        || n.starts_with("androidx.")
        || n.starts_with("android.support.")
        || n == "java"
        || n.starts_with("java.")
        || n == "javax"
        || n.starts_with("javax.")
        || n == "kotlin"
        || n.starts_with("kotlin.")
        || n == "kotlinx"
        || n.starts_with("kotlinx.")
        || n.starts_with("dalvik.")
        || n.starts_with("libcore.")
        || n.starts_with("sun.")
        || n.starts_with("com.android.")
        // Broad Google / Play / GMS — commercial APKs ship huge DEX slices here.
        || n.starts_with("com.google.")
        || n.starts_with("com.fasterxml.")
        || n.starts_with("okhttp3.")
        || n.starts_with("okio.")
        || n.starts_with("retrofit2.")
        || n.starts_with("com.squareup.")
        || n.starts_with("io.reactivex.")
        || n.starts_with("io.grpc.")
        || n.starts_with("io.opencensus.")
        || n.starts_with("io.perfmark.")
        || n.starts_with("org.apache.")
        || n.starts_with("org.jetbrains.")
        || n.starts_with("org.json.")
        || n.starts_with("org.xmlpull.")
        || n.starts_with("org.bouncycastle.")
        || n.starts_with("org.chromium.")
        || n.starts_with("org.checkerframework.")
        || n.starts_with("org.codehaus.")
        || n.starts_with("org.intellij.")
        || n.starts_with("com.facebook.react.")
        || n.starts_with("com.facebook.fresco.")
        || n.starts_with("com.facebook.imagepipeline.")
        || n.starts_with("com.facebook.yoga.")
        || n.starts_with("com.facebook.soloader.")
        || n.starts_with("com.facebook.jni.")
        || n.starts_with("com.facebook.common.")
        || n.starts_with("com.facebook.infer.")
}

/// Keep app classes when `prefixes` is set (class == prefix or starts with `prefix.`).
pub fn class_matches_prefixes(class_name: &str, prefixes: &[String]) -> bool {
    if prefixes.is_empty() {
        return true;
    }
    prefixes
        .iter()
        .any(|p| class_name == p.as_str() || class_name.starts_with(&format!("{p}.")))
}

/// True when `class_name` matches any of the exclude regular expressions.
pub fn class_matches_exclude_regexps(class_name: &str, exclude_regexps: &[String]) -> bool {
    if exclude_regexps.is_empty() {
        return false;
    }
    exclude_regexps.iter().any(|pat| {
        let pat = pat.trim();
        if pat.is_empty() {
            return false;
        }
        regex::Regex::new(pat)
            .map(|re| re.is_match(class_name))
            .unwrap_or(false)
    })
}

/// Compile exclude patterns once; invalid patterns are skipped (logged by caller if needed).
pub fn compile_exclude_regexps(patterns: &[String]) -> Vec<regex::Regex> {
    patterns
        .iter()
        .filter_map(|pat| {
            let pat = pat.trim();
            if pat.is_empty() {
                return None;
            }
            regex::Regex::new(pat).ok()
        })
        .collect()
}

fn class_excluded(class_name: &str, excludes: &[regex::Regex]) -> bool {
    excludes.iter().any(|re| re.is_match(class_name))
}

/// One method with enough metadata to run detectors in parallel.
#[derive(Clone)]
struct MethodJob {
    class_name: String,
    method_name: String,
    encoded: EncodedMethod,
}

fn collect_method_jobs(dex: &DexFile) -> Vec<MethodJob> {
    collect_method_jobs_scoped(dex, None, &[])
}

/// Number of non-library (and optionally prefix/exclude-scoped) methods with code.
pub fn count_method_jobs_scoped(dex: &DexFile, prefixes: Option<&[String]>) -> usize {
    count_method_jobs_scoped_ex(dex, prefixes, &[])
}

/// Like [`count_method_jobs_scoped`] with package-exclude regexps.
pub fn count_method_jobs_scoped_ex(
    dex: &DexFile,
    prefixes: Option<&[String]>,
    exclude_regexps: &[String],
) -> usize {
    let excludes = compile_exclude_regexps(exclude_regexps);
    collect_method_jobs_scoped(dex, prefixes, &excludes).len()
}

fn collect_method_jobs_scoped(
    dex: &DexFile,
    prefixes: Option<&[String]>,
    excludes: &[regex::Regex],
) -> Vec<MethodJob> {
    let mut jobs = Vec::new();
    for class_result in dex.class_defs() {
        let Ok(class_def) = class_result else {
            continue;
        };
        let Ok(class_type) = dex.get_type(class_def.class_idx) else {
            continue;
        };
        let class_name = descriptor_to_java(&class_type);
        if is_library_class(&class_name) {
            continue;
        }
        if class_excluded(&class_name, excludes) {
            continue;
        }
        if let Some(prefs) = prefixes {
            if !class_matches_prefixes(&class_name, prefs) {
                continue;
            }
        }
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            continue;
        };
        for encoded in class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
        {
            if encoded.code_off == 0 {
                continue;
            }
            let Ok(method_info) = dex.get_method_info(encoded.method_idx) else {
                continue;
            };
            jobs.push(MethodJob {
                class_name: class_name.clone(),
                method_name: method_info.name.to_string(),
                encoded: encoded.clone(),
            });
        }
    }
    jobs
}

/// Run all detectors (except PendingIntent) and return findings, deduplicated by category+class+method+sink_offset.
pub fn run_all_detectors(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
    logging_sources: Option<&[String]>,
) -> Vec<VulnFinding> {
    let mut all = Vec::new();
    all.extend(scan_intent_spoofing(owned, class_name, method_name));
    all.extend(scan_intent_redirect(owned, class_name, method_name));
    all.extend(scan_broadcast_intent_redirect(
        owned,
        class_name,
        method_name,
    ));
    all.extend(scan_command_receiver(owned, class_name, method_name));
    all.extend(scan_rce_dynamic_loading(owned, class_name, method_name));
    all.extend(scan_insecure_logging(
        owned,
        class_name,
        method_name,
        logging_sources,
    ));
    all.extend(scan_sql_injection(owned, class_name, method_name));
    all.extend(scan_webview_unsafe(owned, class_name, method_name));
    all.extend(scan_custom_tabs(owned, class_name, method_name));
    all.extend(scan_hardcoded_secrets(owned, class_name, method_name));
    all.extend(scan_ipc_intent_validation(owned, class_name, method_name));
    all.extend(scan_path_traversal(owned, class_name, method_name));
    all.extend(scan_zip_slip(owned, class_name, method_name));
    all.extend(scan_logcat_external(owned, class_name, method_name));
    all.extend(scan_webresource_response(owned, class_name, method_name));
    all.extend(scan_package_context_ace(owned, class_name, method_name));
    all.extend(scan_intent_parse_uri(owned, class_name, method_name));
    all.extend(scan_uri_grant(owned, class_name, method_name));
    all.extend(scan_weak_crypto(owned, class_name, method_name));
    all.extend(scan_unsafe_deserialization(owned, class_name, method_name));
    all.extend(scan_storage_mode(owned, class_name, method_name));
    all.extend(scan_broadcast_abuse(owned, class_name, method_name));
    all.extend(scan_ssl_trust_all(owned, class_name, method_name));
    all.extend(scan_pinning_bypass(owned, class_name, method_name));
    all.extend(scan_reflection_rce(owned, class_name, method_name));
    all.extend(scan_sqlcipher_passphrase(owned, class_name, method_name));
    all.extend(scan_implicit_intent(owned, class_name, method_name));
    all.extend(scan_biometric_misuse(owned, class_name, method_name));
    all.extend(scan_keystore_misuse(owned, class_name, method_name));
    all.extend(scan_tracker_inventory(owned, class_name, method_name));
    all.extend(scan_credential_broadcast(owned, class_name, method_name));
    all.extend(scan_sensitive_broadcast(owned, class_name, method_name));
    all.extend(scan_weak_host_validation(owned, class_name, method_name));
    all.extend(scan_pick_file_theft(owned, class_name, method_name));
    all.extend(scan_next_wave(owned, class_name, method_name));
    let mut seen: HashSet<(String, String, String, u32)> = HashSet::new();
    all.into_iter()
        .filter(|f| {
            let key = (
                f.category.clone(),
                f.class_name.clone(),
                f.method_name.clone(),
                f.sink_offset,
            );
            seen.insert(key)
        })
        .collect()
}

/// Parallel scan of every method with code in `dex` using the standard detectors.
///
/// Each rayon worker creates its own [`Decompiler`] (`!Sync` caches).
/// Parallel scan over every method with code in `dex`.
pub fn scan_dex_parallel(dex: &DexFile, logging_sources: Option<&[String]>) -> Vec<VulnFinding> {
    scan_dex_parallel_scoped(dex, logging_sources, None, &[])
}

/// Like [`scan_dex_parallel`], but only methods whose class matches `class_prefixes`
/// (when non-empty / `Some`) and does not match `exclude_regexps`.
pub fn scan_dex_parallel_scoped(
    dex: &DexFile,
    logging_sources: Option<&[String]>,
    class_prefixes: Option<&[String]>,
    exclude_regexps: &[String],
) -> Vec<VulnFinding> {
    let excludes = compile_exclude_regexps(exclude_regexps);
    let jobs = collect_method_jobs_scoped(dex, class_prefixes, &excludes);
    let logging = logging_sources.map(|s| s.to_vec());
    jobs.par_iter()
        .flat_map(|job| {
            let decompiler = Decompiler::new(dex);
            let Ok(owned) = decompiler.value_flow_analysis(&job.encoded) else {
                return Vec::new();
            };
            run_all_detectors(
                &owned,
                &job.class_name,
                &job.method_name,
                logging.as_deref(),
            )
        })
        .collect()
}

/// Parallel PendingIntent scan over every method with code in `dex`.
pub fn scan_pending_intents_dex_parallel(dex: &DexFile) -> Vec<PendingIntentFinding> {
    scan_pending_intents_dex_parallel_scoped(dex, None, &[])
}

/// Scoped variant of [`scan_pending_intents_dex_parallel`].
pub fn scan_pending_intents_dex_parallel_scoped(
    dex: &DexFile,
    class_prefixes: Option<&[String]>,
    exclude_regexps: &[String],
) -> Vec<PendingIntentFinding> {
    let excludes = compile_exclude_regexps(exclude_regexps);
    let jobs = collect_method_jobs_scoped(dex, class_prefixes, &excludes);
    jobs.par_iter()
        .flat_map(|job| {
            let decompiler = Decompiler::new(dex);
            let Ok(owned) = decompiler.value_flow_analysis(&job.encoded) else {
                return Vec::new();
            };
            scan_pending_intents(&owned, &job.class_name, &job.method_name)
        })
        .collect()
}

#[cfg(test)]
mod library_class_tests {
    use super::{is_com_android_lab_app, is_library_class};

    #[test]
    fn real_aosp_stays_library() {
        for n in [
            "com.android.settings.Settings",
            "com.android.systemui.SystemUI",
            "com.android.phone.PhoneApp",
            "com.android.internal.app.IntentForwarderActivity",
            "com.android.providers.contacts.ContactsProvider2",
            "com.google.android.gms.common.api.GoogleApi",
        ] {
            assert!(is_library_class(n), "{n}");
            assert!(!is_com_android_lab_app(n), "{n}");
        }
    }

    #[test]
    fn insecurebankv2_is_app_not_library() {
        assert!(!is_library_class("com.android.insecurebankv2"));
        assert!(!is_library_class(
            "com.android.insecurebankv2.LoginActivity"
        ));
        assert!(is_com_android_lab_app("com.android.insecurebankv2.DoLogin"));
    }

    #[test]
    fn lab_allowlist_is_package_boundary() {
        // Must not match a longer sibling under a different package name.
        assert!(is_library_class("com.android.insecurebankv2evil.Foo"));
        assert!(!is_com_android_lab_app(
            "com.android.insecurebankv2evil.Foo"
        ));
    }
}
