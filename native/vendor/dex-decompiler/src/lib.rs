//! DEX to Java decompiler in pure Rust.
//! Uses dex-bytecode for Dalvik instruction disassembly and dex-parser for DEX parsing.

pub mod decompile;
pub mod detectors;
pub mod emulator;
pub mod error;
pub mod input;
pub mod java;
pub mod semgrep;
pub mod taint;
pub mod xref;

pub use decompile::deobf::{build_deobf_rename_map, merge_rename_maps, DeobfuscateOptions};
pub use decompile::json_export::{ClassJson, DexJsonExport, FieldJson, MethodJson};
pub use decompile::mapping::{
    load_mapping_file, load_mapping_str, mapping_format_from_path, save_mapping_file,
    save_mapping_str, MappingFormat,
};
pub use decompile::value_flow::{ValueFlowAnalysis, ValueFlowAnalysisOwned, ValueFlowResult};
pub use decompile::{
    class_name_to_path, rename::RenameMap, CfgEdgeInfo, CfgNodeInfo, DecompilationMode, Decompiler,
    DecompilerOptions, MethodBytecodeRow,
};
pub use detectors::pending_intent::{scan_pending_intents, PendingIntentFinding};
pub use detectors::{
    category_meta, class_matches_exclude_regexps, class_matches_prefixes, count_method_jobs_scoped,
    count_method_jobs_scoped_ex, is_com_android_lab_app, is_library_class, run_all_detectors,
    scan_dex_parallel, scan_dex_parallel_scoped, scan_pending_intents_dex_parallel,
    scan_pending_intents_dex_parallel_scoped, CategoryMeta, VulnFinding, VulnTraceStep,
};
pub use dex_parser::{ClassDef, CodeItem, DexFile, EncodedMethod};
pub use error::{DexDecompilerError, Result};
pub use input::{
    extract_android_manifest_from_apk, extract_dex_entries_from_apk, is_classes_dex_name,
    load_dexes_from_bytes, load_dexes_from_path, load_dexes_from_paths, looks_like_text_xml,
};
pub use semgrep::{
    builtin_android_rules, default_android_rule_paths, load_android_rules, load_rules_from_dir,
    load_rules_from_str, load_rules_from_yaml_file, scan_dex_semgrep, scan_dex_semgrep_sequential,
    scan_dex_semgrep_sequential_scoped_with_progress, scan_dex_semgrep_sequential_with_progress,
    scan_dex_semgrep_with_progress, scan_method_semgrep, scan_xml_semgrep,
    scan_xml_semgrep_sequential, SemgrepFinding, SemgrepRule, ANDROID_ALL_RULES_YAML,
    ANDROID_GENERAL_RULES_YAML,
};
pub use taint::{
    convert_models_json, convert_rules_json, default_config, load_mt_case_config, method_patterns,
    parse_mt_port, solve_dex, solve_dexes, write_issues_json, Issue, IssueReport, MethodIndex,
    Port, SolveOptions, SolveResult, TaintConfig,
};
pub use xref::{
    find_field_xrefs, find_method_call_traces, find_method_call_traces_by_class_method,
    find_method_call_traces_with_index, find_method_callees, find_method_callees_by_class_method,
    find_method_callers, find_method_callers_by_class_method, CallTraceFrame, CallTracePath,
    FieldXref, FieldXrefsInfo, MethodCallTracesInfo, MethodCallee, MethodCalleesInfo, MethodCaller,
    MethodCallersInfo, ReverseCallIndex,
};

/// Parse a DEX file from raw bytes. Returns decompiler Result (maps parser errors to Parse).
pub fn parse_dex(data: &[u8]) -> Result<DexFile> {
    DexFile::parse(data).map_err(|e| DexDecompilerError::Parse(e.to_string()))
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn parse_empty_fails() {
        let r = parse_dex(&[]);
        assert!(r.is_err());
    }

    #[test]
    fn parse_garbage_fails() {
        let r = parse_dex(b"not a DEX file!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
        assert!(r.is_err());
    }
}
