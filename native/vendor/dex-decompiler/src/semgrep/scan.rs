//! Run Semgrep-style Android rules over a DEX (SSA/value-flow + decompiled Java).

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::decompile::Decompiler;
use crate::detectors::{invoke_scan, method_matches_any, source_sink_scan, VulnFinding};
use crate::java::descriptor_to_java;
use crate::semgrep::match_java::{PreparedPattern, TokenizedSource};
use crate::semgrep::rule::{
    load_rules_from_dir, load_rules_from_str, load_rules_from_yaml_file, NativeKind, SemgrepRule,
};
use dex_parser::{DexFile, EncodedMethod};
use rayon::prelude::*;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// One Semgrep-style finding (compatible fields with triage tooling).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SemgrepFinding {
    pub rule_id: String,
    pub severity: String,
    pub message: String,
    pub class_name: String,
    pub method_name: String,
    pub sink_offset: Option<u32>,
    pub sink_desc: String,
    pub vuln_class: Option<String>,
    pub chain_tag: Option<String>,
    /// How the match was obtained: `native` (SSA/VF), `java_pattern`, or `java_regex`.
    pub match_kind: String,
}

impl SemgrepFinding {
    pub fn to_vuln_finding(&self) -> VulnFinding {
        let mut f = VulnFinding::new(
            &format!("semgrep:{}", self.rule_id),
            &self.class_name,
            &self.method_name,
            None,
            self.message.clone(),
            self.sink_offset.unwrap_or(0),
            self.sink_desc.clone(),
        );
        // Prefer Semgrep's own severity/message when present.
        if !self.severity.is_empty() {
            f.severity = self.severity.to_lowercase();
        }
        if !self.message.is_empty() {
            f.title = format!("Semgrep: {}", self.rule_id);
            f.message = self.message.clone();
            if !self.sink_desc.is_empty() {
                f.message.push_str(&format!(" Sink: `{}`.", self.sink_desc));
            }
        }
        f
    }
}

/// Embedded general Android rules YAML (WebView, intent redirect, SSL bypass, etc.).
pub const ANDROID_GENERAL_RULES_YAML: &str =
    include_str!("../../rules/semgrep/android/general.yml");

/// Embedded **all** Android rules: general (MobHunt-style) + OWASP MASTG.
/// Single-file include so WASM / droid2web get the full set without a filesystem.
pub const ANDROID_ALL_RULES_YAML: &str = include_str!("../../rules/semgrep/android/all.yml");

fn mastg_rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/semgrep/android/mastg")
}

fn general_rules_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/semgrep/android/general.yml")
}

fn all_rules_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/semgrep/android/all.yml")
}

/// Built-in Android rules: general native rules + OWASP MASTG Semgrep rules.
pub fn builtin_android_rules() -> Vec<SemgrepRule> {
    load_rules_from_str(ANDROID_ALL_RULES_YAML).unwrap_or_else(|e| {
        eprintln!("warning: embedded all.yml failed ({e}); falling back to general + mastg dir");
        let mut rules = load_rules_from_str(ANDROID_GENERAL_RULES_YAML)
            .expect("embedded general rules must parse");
        let mastg = mastg_rules_dir();
        if mastg.is_dir() {
            match load_rules_from_dir(&mastg) {
                Ok(mut rs) => rules.append(&mut rs),
                Err(e) => {
                    eprintln!(
                        "warning: could not load MASTG rules from {}: {}",
                        mastg.display(),
                        e
                    )
                }
            }
        }
        rules
    })
}

/// Load rules from a YAML file or directory, or the built-in Android set if `path` is None.
pub fn load_android_rules(path: Option<&Path>) -> Result<Vec<SemgrepRule>, String> {
    match path {
        Some(p) if p.is_dir() => load_rules_from_dir(p),
        Some(p) => load_rules_from_yaml_file(p),
        None => Ok(builtin_android_rules()),
    }
}

/// Default rule search paths (combined all.yml, or general + MASTG directory).
pub fn default_android_rule_paths() -> Vec<PathBuf> {
    let all = all_rules_path();
    if all.is_file() {
        vec![all]
    } else {
        vec![general_rules_path(), mastg_rules_dir()]
    }
}

/// True if `src` matches the rule's Java/Kotlin patterns (AND for `patterns:`, OR for `pattern-either`).
pub fn rule_matches_source(rule: &SemgrepRule, src: &str) -> Option<&'static str> {
    PreparedRule::from_rule(rule).matches_source(src)
}

/// Precompiled / pre-tokenized rule for hot scan loops.
struct PreparedRule<'a> {
    rule: &'a SemgrepRule,
    patterns: Vec<PreparedPattern>,
    regexes: Vec<Regex>,
    conjunction: bool,
    needs_vf: bool,
    needs_java: bool,
}

impl<'a> PreparedRule<'a> {
    fn from_rule(rule: &'a SemgrepRule) -> Self {
        let patterns: Vec<PreparedPattern> = rule
            .pattern_strings()
            .iter()
            .map(|p| PreparedPattern::new(p))
            .filter(|p| !p.is_empty())
            .collect();
        let regexes: Vec<Regex> = rule
            .pattern_regexes()
            .iter()
            .filter_map(|re| Regex::new(re).ok())
            .collect();
        let has_source_patterns = !patterns.is_empty() || !regexes.is_empty();
        let needs_vf = rule.native.is_some();
        let needs_java = match &rule.native {
            None => has_source_patterns,
            Some(n) => matches!(n.kind, NativeKind::MethodInvoke) && has_source_patterns,
        };
        Self {
            rule,
            patterns,
            regexes,
            conjunction: rule.patterns_are_conjunction(),
            needs_vf,
            needs_java,
        }
    }

    fn matches_source(&self, src: &str) -> Option<&'static str> {
        if self.patterns.is_empty() && self.regexes.is_empty() {
            return None;
        }
        let hay = TokenizedSource::new(src);
        self.matches_tokenized(src, &hay)
    }

    fn matches_tokenized(&self, src: &str, hay: &TokenizedSource) -> Option<&'static str> {
        if self.patterns.is_empty() && self.regexes.is_empty() {
            return None;
        }

        let regex_ok = if self.regexes.is_empty() {
            true
        } else if self.conjunction {
            self.regexes.iter().all(|re| re.is_match(src))
        } else {
            self.regexes.iter().any(|re| re.is_match(src))
        };

        let patterns_ok = if self.patterns.is_empty() {
            true
        } else if self.conjunction {
            self.patterns.iter().all(|p| p.matches(hay))
        } else {
            self.patterns.iter().any(|p| p.matches(hay))
        };

        if self.patterns.is_empty() && !self.regexes.is_empty() && regex_ok {
            return Some("java_regex");
        }
        if self.regexes.is_empty() && !self.patterns.is_empty() && patterns_ok {
            return Some("java_pattern");
        }
        if !self.regexes.is_empty() && !self.patterns.is_empty() && regex_ok && patterns_ok {
            return Some("java_pattern");
        }
        if !self.conjunction
            && ((regex_ok && !self.regexes.is_empty())
                || (patterns_ok && !self.patterns.is_empty()))
        {
            if !self.regexes.is_empty() && self.regexes.iter().any(|re| re.is_match(src)) {
                return Some("java_regex");
            }
            if patterns_ok {
                return Some("java_pattern");
            }
        }
        None
    }
}

/// Index of rules prepared once per scan (regex compile + pattern tokenize).
struct PreparedRules<'a> {
    java: Vec<PreparedRule<'a>>,
    xml: Vec<PreparedRule<'a>>,
}

impl<'a> PreparedRules<'a> {
    fn new(rules: &'a [SemgrepRule]) -> Self {
        let java = rules
            .iter()
            .filter(|r| r.applies_to_java())
            .map(PreparedRule::from_rule)
            .filter(|p| p.needs_vf || p.needs_java)
            .collect();
        let xml = rules
            .iter()
            .filter(|r| r.applies_to_xml())
            .map(PreparedRule::from_rule)
            .filter(|p| !p.patterns.is_empty() || !p.regexes.is_empty())
            .collect();
        Self { java, xml }
    }
}

/// Scan one method with the given rules (value-flow + optional Java pattern match).
pub fn scan_method_semgrep(
    decompiler: &Decompiler,
    encoded: &EncodedMethod,
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
    rules: &[SemgrepRule],
) -> Vec<SemgrepFinding> {
    let prepared = PreparedRules::new(rules);
    scan_method_prepared(
        decompiler,
        encoded,
        Some(owned),
        class_name,
        method_name,
        &prepared,
    )
}

fn scan_method_prepared(
    decompiler: &Decompiler,
    encoded: &EncodedMethod,
    owned_pre: Option<&ValueFlowAnalysisOwned>,
    class_name: &str,
    method_name: &str,
    prepared: &PreparedRules<'_>,
) -> Vec<SemgrepFinding> {
    let mut findings = Vec::new();
    let mut owned_local: Option<ValueFlowAnalysisOwned> = None;
    let mut java: Option<String> = None;
    let mut java_tok: Option<TokenizedSource> = None;

    for prep in &prepared.java {
        let rule = prep.rule;

        if let Some(native) = &rule.native {
            // Resolve value-flow lazily.
            let owned = if let Some(o) = owned_pre {
                Some(o)
            } else {
                if owned_local.is_none() {
                    owned_local = decompiler.value_flow_analysis(encoded).ok();
                }
                owned_local.as_ref()
            };

            if let Some(owned) = owned {
                let mut native_hit = false;
                match native.kind {
                    NativeKind::SourceSink => {
                        let sources: Vec<&str> =
                            native.sources.iter().map(|s| s.as_str()).collect();
                        let sinks: Vec<&str> = native.sinks.iter().map(|s| s.as_str()).collect();
                        for vf in source_sink_scan(
                            owned,
                            class_name,
                            method_name,
                            &rule.id,
                            &sources,
                            &sinks,
                        ) {
                            native_hit = true;
                            findings.push(finding_from_rule(
                                rule,
                                class_name,
                                method_name,
                                Some(vf.sink_offset),
                                vf.sink_desc,
                                "native",
                            ));
                        }
                    }
                    NativeKind::Invoke => {
                        let methods: Vec<&str> =
                            native.methods.iter().map(|s| s.as_str()).collect();
                        for vf in invoke_scan(owned, class_name, method_name, &rule.id, &methods) {
                            native_hit = true;
                            findings.push(finding_from_rule(
                                rule,
                                class_name,
                                method_name,
                                Some(vf.sink_offset),
                                vf.sink_desc,
                                "native",
                            ));
                        }
                    }
                    NativeKind::MethodInvoke => {
                        let want = native.method_name.as_deref().unwrap_or("");
                        if want.is_empty() || method_name == want {
                            let methods: Vec<&str> =
                                native.methods.iter().map(|s| s.as_str()).collect();
                            for (offset, method_ref) in &owned.invoke_method_map {
                                if method_matches_any(method_ref, &methods) {
                                    native_hit = true;
                                    findings.push(finding_from_rule(
                                        rule,
                                        class_name,
                                        method_name,
                                        Some(*offset),
                                        method_ref.clone(),
                                        "native",
                                    ));
                                }
                            }
                        }
                        if !native_hit && prep.needs_java {
                            ensure_java(decompiler, encoded, class_name, &mut java, &mut java_tok);
                            if let (Some(src), Some(tok)) = (java.as_ref(), java_tok.as_ref()) {
                                if let Some(kind) = prep.matches_tokenized(src, tok) {
                                    findings.push(finding_from_rule(
                                        rule,
                                        class_name,
                                        method_name,
                                        None,
                                        format!("pattern match: {}", rule.id),
                                        kind,
                                    ));
                                }
                            }
                        }
                    }
                }
                let _ = native_hit;
            } else if prep.needs_java {
                ensure_java(decompiler, encoded, class_name, &mut java, &mut java_tok);
                if let (Some(src), Some(tok)) = (java.as_ref(), java_tok.as_ref()) {
                    if let Some(kind) = prep.matches_tokenized(src, tok) {
                        findings.push(finding_from_rule(
                            rule,
                            class_name,
                            method_name,
                            None,
                            format!("pattern match: {}", rule.id),
                            kind,
                        ));
                    }
                }
            }
            continue;
        }

        if prep.needs_java {
            ensure_java(decompiler, encoded, class_name, &mut java, &mut java_tok);
            if let (Some(src), Some(tok)) = (java.as_ref(), java_tok.as_ref()) {
                if let Some(kind) = prep.matches_tokenized(src, tok) {
                    findings.push(finding_from_rule(
                        rule,
                        class_name,
                        method_name,
                        None,
                        format!("pattern match: {}", rule.id),
                        kind,
                    ));
                }
            }
        }
    }

    findings
}

fn ensure_java(
    decompiler: &Decompiler,
    encoded: &EncodedMethod,
    class_name: &str,
    java: &mut Option<String>,
    java_tok: &mut Option<TokenizedSource>,
) {
    if java.is_some() {
        return;
    }
    let src = decompiler
        .decompile_method(
            encoded,
            Some(class_name.rsplit('.').next().unwrap_or(class_name)),
            Some(class_name),
        )
        .unwrap_or_default();
    *java_tok = Some(TokenizedSource::new(&src));
    *java = Some(src);
}

fn finding_from_rule(
    rule: &SemgrepRule,
    class_name: &str,
    method_name: &str,
    sink_offset: Option<u32>,
    sink_desc: String,
    match_kind: &str,
) -> SemgrepFinding {
    let vuln = rule
        .metadata
        .vuln_class
        .clone()
        .or_else(|| rule.metadata.summary.clone());
    SemgrepFinding {
        rule_id: rule.id.clone(),
        severity: rule.severity.as_str().to_string(),
        message: if rule.message.is_empty() {
            rule.metadata.summary.clone().unwrap_or_default()
        } else {
            rule.message.trim().to_string()
        },
        class_name: class_name.to_string(),
        method_name: method_name.to_string(),
        sink_offset,
        sink_desc,
        vuln_class: vuln,
        chain_tag: rule.metadata.chain_tag.clone(),
        match_kind: match_kind.to_string(),
    }
}

struct MethodJob {
    class_name: String,
    method_name: String,
    encoded: EncodedMethod,
}

/// True for Android platform / AndroidX library classes (not app code).
fn is_platform_class(class_name: &str) -> bool {
    crate::detectors::is_library_class(class_name)
}

fn collect_method_jobs(dex: &DexFile) -> Vec<MethodJob> {
    collect_method_jobs_scoped(dex, None)
}

fn collect_method_jobs_scoped(dex: &DexFile, prefixes: Option<&[String]>) -> Vec<MethodJob> {
    let mut jobs = Vec::new();
    for class_result in dex.class_defs() {
        let Ok(class_def) = class_result else {
            continue;
        };
        let Ok(class_type) = dex.get_type(class_def.class_idx) else {
            continue;
        };
        let class_name = descriptor_to_java(&class_type);
        if is_platform_class(&class_name) {
            continue;
        }
        if let Some(prefs) = prefixes {
            if !crate::detectors::class_matches_prefixes(&class_name, prefs) {
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
            let Ok(info) = dex.get_method_info(encoded.method_idx) else {
                continue;
            };
            jobs.push(MethodJob {
                class_name: class_name.clone(),
                method_name: info.name.to_string(),
                encoded: encoded.clone(),
            });
        }
    }
    jobs
}

/// Parallel Semgrep-style scan of every method with code in `dex`.
pub fn scan_dex_semgrep(dex: &DexFile, rules: &[SemgrepRule]) -> Vec<SemgrepFinding> {
    scan_dex_semgrep_with_progress(dex, rules, None::<fn(usize, usize, &str)>)
}

/// Sequential Semgrep scan (WASM / single-threaded friendly — no rayon).
/// Caps work on huge DEXes so browser scans finish (Facebook-scale APKs).
pub fn scan_dex_semgrep_sequential(dex: &DexFile, rules: &[SemgrepRule]) -> Vec<SemgrepFinding> {
    scan_dex_semgrep_sequential_with_progress(
        dex,
        rules,
        None::<fn(usize, usize, &[SemgrepFinding])>,
    )
}

/// Soft cap for sequential WASM / UI scans. Full CLI parallel scan is uncapped.
pub fn sequential_scan_method_cap() -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        2_500
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        25_000
    }
}

/// Like [`scan_dex_semgrep_sequential`], with optional progress `(done, total, findings_so_far)`.
pub fn scan_dex_semgrep_sequential_with_progress<F>(
    dex: &DexFile,
    rules: &[SemgrepRule],
    on_progress: Option<F>,
) -> Vec<SemgrepFinding>
where
    F: FnMut(usize, usize, &[SemgrepFinding]),
{
    scan_dex_semgrep_sequential_scoped_with_progress(dex, rules, None, on_progress)
}

/// Sequential Semgrep limited to `class_prefixes` (when `Some` and non-empty).
pub fn scan_dex_semgrep_sequential_scoped_with_progress<F>(
    dex: &DexFile,
    rules: &[SemgrepRule],
    class_prefixes: Option<&[String]>,
    mut on_progress: Option<F>,
) -> Vec<SemgrepFinding>
where
    F: FnMut(usize, usize, &[SemgrepFinding]),
{
    let prepared = PreparedRules::new(rules);
    if prepared.java.is_empty() {
        if let Some(ref mut cb) = on_progress {
            cb(0, 0, &[]);
        }
        return Vec::new();
    }
    let mut jobs = collect_method_jobs_scoped(dex, class_prefixes);
    let cap = sequential_scan_method_cap();
    if jobs.len() > cap {
        jobs.truncate(cap);
    }
    let total = jobs.len();
    // Recreate periodically: Decompiler caches (inner/lambda bodies) grow without bound
    // across thousands of methods and Jetsam-kill the process on macOS.
    const REFRESH_EVERY: usize = 48;
    let mut decompiler = Decompiler::new(dex);
    let mut out = Vec::new();
    let progress_every = (total / 200).max(16);
    let mut last_reported_len = 0usize;
    if let Some(ref mut cb) = on_progress {
        cb(0, total, &out);
    }
    for (i, job) in jobs.iter().enumerate() {
        if i > 0 && i % REFRESH_EVERY == 0 {
            decompiler = Decompiler::new(dex);
        }
        out.extend(scan_method_prepared(
            &decompiler,
            &job.encoded,
            None,
            &job.class_name,
            &job.method_name,
            &prepared,
        ));
        let n = i + 1;
        let grew = out.len() > last_reported_len;
        if let Some(ref mut cb) = on_progress {
            if grew || n == total || n == 1 || n % progress_every == 0 {
                cb(n, total, &out);
                last_reported_len = out.len();
            }
        }
    }
    out
}

/// Like [`scan_dex_semgrep`], with optional progress callback `(done, total, class#method)`.
///
/// Optimizations:
/// - Rayon over methods with a **per-worker** [`Decompiler`] (`flat_map_init`) so indexes reuse
/// - Rules prepared once (compiled regexes + tokenized patterns)
/// - Value-flow / Java decompile only when a rule needs them
pub fn scan_dex_semgrep_with_progress<F>(
    dex: &DexFile,
    rules: &[SemgrepRule],
    on_progress: Option<F>,
) -> Vec<SemgrepFinding>
where
    F: Fn(usize, usize, &str) + Sync,
{
    let prepared = PreparedRules::new(rules);
    if prepared.java.is_empty() {
        if let Some(ref cb) = on_progress {
            cb(0, 0, "(no java rules)");
        }
        return Vec::new();
    }

    let jobs = collect_method_jobs(dex);
    let total = jobs.len();
    if let Some(ref cb) = on_progress {
        cb(
            0,
            total,
            if total == 0 {
                "(no methods)"
            } else {
                "starting…"
            },
        );
    }
    if total == 0 {
        return Vec::new();
    }

    let done = AtomicUsize::new(0);
    let progress_every = (total / 200).max(32);

    jobs.par_iter()
        .map_init(
            || Decompiler::new(dex),
            |decompiler, job| {
                let findings = scan_method_prepared(
                    decompiler,
                    &job.encoded,
                    None,
                    &job.class_name,
                    &job.method_name,
                    &prepared,
                );
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if let Some(ref cb) = on_progress {
                    if n == total || n == 1 || n % progress_every == 0 {
                        cb(n, total, &format!("{}#{}", job.class_name, job.method_name));
                    }
                }
                findings
            },
        )
        .flatten()
        .collect()
}

/// Scan AndroidManifest / XML text with XML-language Semgrep rules (parallel over rules).
pub fn scan_xml_semgrep(xml: &str, path_label: &str, rules: &[SemgrepRule]) -> Vec<SemgrepFinding> {
    let prepared = PreparedRules::new(rules);
    if prepared.xml.is_empty() {
        return Vec::new();
    }
    let hay = TokenizedSource::new(xml);
    prepared
        .xml
        .par_iter()
        .filter_map(|prep| {
            prep.matches_tokenized(xml, &hay).map(|kind| {
                finding_from_rule(
                    prep.rule,
                    path_label,
                    "(xml)",
                    None,
                    format!("pattern match: {}", prep.rule.id),
                    kind,
                )
            })
        })
        .collect()
}

/// Sequential XML Semgrep scan (WASM-friendly).
pub fn scan_xml_semgrep_sequential(
    xml: &str,
    path_label: &str,
    rules: &[SemgrepRule],
) -> Vec<SemgrepFinding> {
    let prepared = PreparedRules::new(rules);
    if prepared.xml.is_empty() {
        return Vec::new();
    }
    let hay = TokenizedSource::new(xml);
    prepared
        .xml
        .iter()
        .filter_map(|prep| {
            prep.matches_tokenized(xml, &hay).map(|kind| {
                finding_from_rule(
                    prep.rule,
                    path_label,
                    "(xml)",
                    None,
                    format!("pattern match: {}", prep.rule.id),
                    kind,
                )
            })
        })
        .collect()
}
