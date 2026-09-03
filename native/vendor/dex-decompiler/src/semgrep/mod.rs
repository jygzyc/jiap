//! Native Semgrep-style rules for Android (general rules + OWASP MASTG + custom YAML).
//!
//! Matches use decompiled Java where possible, and SSA/value-flow (`native:`
//! hints) so rules work reliably on DEX without a full Java AST / Semgrep binary.

mod match_java;
mod rule;
mod scan;

pub use match_java::java_matches_pattern;
pub use rule::{
    load_rules_from_dir, load_rules_from_str, load_rules_from_yaml_file, NativeKind, NativeMatch,
    SemgrepRule, Severity,
};
pub use scan::{
    builtin_android_rules, default_android_rule_paths, load_android_rules, rule_matches_source,
    scan_dex_semgrep, scan_dex_semgrep_sequential,
    scan_dex_semgrep_sequential_scoped_with_progress, scan_dex_semgrep_sequential_with_progress,
    scan_dex_semgrep_with_progress, scan_method_semgrep, scan_xml_semgrep,
    scan_xml_semgrep_sequential, SemgrepFinding, ANDROID_ALL_RULES_YAML,
    ANDROID_GENERAL_RULES_YAML,
};
