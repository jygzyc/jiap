//! Interprocedural taint solver (Mariana Trench–style abstract interpretation).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use dex_parser::DexFile;
use rayon::prelude::*;

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::decompile::Decompiler;
use crate::error::Result;

use super::call_graph::{CallEdge, CallGraph};
use super::config::TaintConfig;
use super::index::{MethodId, MethodIndex};
use super::issue::{Issue, TraceFrame};
use super::models::{Port, SanitizerModel};
use super::report::{IssueReport, ReportStats};

#[derive(Clone, Debug, Default)]
pub struct SolveOptions {
    /// Max fixpoint iterations (default 8).
    pub max_iterations: usize,
    /// Skip methods whose class name starts with any of these prefixes.
    pub exclude_prefixes: Vec<String>,
    /// When non-empty, only analyze classes matching these prefixes
    /// (`class == prefix` or `class.starts_with(prefix + ".")`).
    /// Library classes are still always skipped via [`crate::detectors::is_library_class`].
    pub include_prefixes: Vec<String>,
    /// Skip classes whose Java name matches any of these regular expressions.
    pub exclude_regexps: Vec<String>,
}

impl SolveOptions {
    pub fn default_android() -> Self {
        Self {
            max_iterations: 8,
            // Kept for callers that customize; primary skip is `is_library_class`.
            exclude_prefixes: vec![
                "android.".into(),
                "androidx.".into(),
                "kotlin.".into(),
                "kotlinx.".into(),
                "java.".into(),
                "javax.".into(),
                "dalvik.".into(),
                "com.google.".into(),
                "com.android.".into(),
                "okhttp3.".into(),
                "okio.".into(),
                "retrofit2.".into(),
                "com.squareup.".into(),
                "com.facebook.".into(),
                "io.sentry.".into(),
                "org.apache.".into(),
                "org.bouncycastle.".into(),
                "org.checkerframework.".into(),
                "io.grpc.".into(),
                "io.reactivex.".into(),
            ],
            include_prefixes: Vec::new(),
            exclude_regexps: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SolveResult {
    pub issues: Vec<Issue>,
    pub report: IssueReport,
}

#[derive(Clone, Debug, Default)]
struct MethodSummary {
    /// Parameter index → kinds that may flow into this method via callers.
    param_kinds: HashMap<u32, HashSet<String>>,
    /// Kinds that may leave via return.
    return_kinds: HashSet<String>,
    /// Inferred: which params flow to return (TITO within method body).
    param_to_return: HashSet<u32>,
    /// Param index → sink kinds reached in this method (from that param).
    param_to_sinks: HashMap<u32, HashSet<String>>,
    /// Taint loaded from class-level static fields at `(sget offset, destination register)`.
    heap_injections: HashMap<(u32, u32), HashSet<String>>,
}

#[derive(Clone, Debug, Default)]
struct LocalTaint {
    /// (offset, reg) → kinds (object-level / smeared / unknown key).
    at: HashMap<(u32, u32), HashSet<String>>,
    /// (offset, reg, path) → kinds. Paths: `extra:<key>` or `field:<Class.field>`.
    paths: HashMap<(u32, u32, String), HashSet<String>>,
}

impl LocalTaint {
    fn insert(&mut self, offset: u32, reg: u32, kind: &str) -> bool {
        self.at
            .entry((offset, reg))
            .or_default()
            .insert(kind.to_string())
    }

    fn kinds_at(&self, offset: u32, reg: u32) -> HashSet<String> {
        self.at.get(&(offset, reg)).cloned().unwrap_or_default()
    }

    fn insert_path(&mut self, offset: u32, reg: u32, path: &str, kind: &str) -> bool {
        self.paths
            .entry((offset, reg, path.to_string()))
            .or_default()
            .insert(kind.to_string())
    }

    #[cfg(test)]
    fn kinds_at_path(&self, offset: u32, reg: u32, path: &str) -> HashSet<String> {
        self.paths
            .get(&(offset, reg, path.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// Union of path kinds on `reg` for `path` at any recorded offset (plus this one).
    fn path_kinds_on_reg(&self, reg: u32, path: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        for ((_, r, p), kinds) in &self.paths {
            if *r == reg && p == path {
                out.extend(kinds.iter().cloned());
            }
        }
        out
    }

    fn path_kinds_on_reg_before(&self, reg: u32, path: &str, at: u32) -> HashSet<String> {
        let mut out = HashSet::new();
        for ((off, r, p), kinds) in &self.paths {
            if *off <= at && *r == reg && p == path {
                out.extend(kinds.iter().cloned());
            }
        }
        out
    }

    fn array_kinds_on_reg_before(&self, reg: u32, at: u32) -> HashSet<String> {
        let mut out = HashSet::new();
        for ((off, r, path), kinds) in &self.paths {
            if *off <= at && *r == reg && path.starts_with("array:") {
                out.extend(kinds.iter().cloned());
            }
        }
        out
    }
}

fn port_to_arg_index(port: &Port) -> Option<u32> {
    match port {
        Port::Return => None,
        Port::This => Some(0),
        Port::Argument { index } => Some(*index),
    }
}

fn sanitize_kinds(kinds: &HashSet<String>, san: &SanitizerModel) -> HashSet<String> {
    if san.kinds.iter().any(|k| k == "*") {
        return HashSet::new();
    }
    if san.kinds.is_empty() {
        // Empty kinds list = do not sanitize (breadcrumb-only model).
        return kinds.clone();
    }
    kinds
        .iter()
        .filter(|k| !san.kinds.iter().any(|s| s == *k))
        .cloned()
        .collect()
}

pub(crate) fn param_reg_from_sizes(registers_size: u32, ins_size: u32, idx: u32) -> Option<u32> {
    if idx < ins_size {
        Some(registers_size.saturating_sub(ins_size) + idx)
    } else {
        None
    }
}

pub(crate) fn param_reg(owned: &ValueFlowAnalysisOwned, idx: u32) -> Option<u32> {
    param_reg_from_sizes(owned.registers_size, owned.ins_size, idx)
}

fn proto_param_widths(proto: &str) -> Vec<u32> {
    let Some(params) = proto
        .strip_prefix('(')
        .and_then(|s| s.split_once(')'))
        .map(|p| p.0)
    else {
        return Vec::new();
    };
    let bytes = params.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] as char {
            'J' | 'D' => {
                out.push(2);
                i += 1;
            }
            'L' => {
                out.push(1);
                i += 1;
                while i < bytes.len() && bytes[i] as char != ';' {
                    i += 1;
                }
                i += usize::from(i < bytes.len());
            }
            '[' => {
                out.push(1);
                i += 1;
                while i < bytes.len() && bytes[i] as char == '[' {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] as char == 'L' {
                    while i < bytes.len() && bytes[i] as char != ';' {
                        i += 1;
                    }
                }
                i += usize::from(i < bytes.len());
            }
            _ => {
                out.push(1);
                i += 1;
            }
        }
    }
    out
}

/// Valid incoming Dalvik word slots, excluding the high half of wide parameters.
fn formal_param_slots(mref: &super::index::MethodRef, owned: &ValueFlowAnalysisOwned) -> Vec<u32> {
    const ACC_STATIC: u32 = 0x8;
    let is_static = mref.encoded.access_flags & ACC_STATIC != 0;
    let mut slots = Vec::new();
    let mut slot = 0u32;
    if !is_static {
        slots.push(0);
        slot = 1;
    }
    for width in proto_param_widths(&mref.proto) {
        if slot < owned.ins_size {
            slots.push(slot);
        }
        slot += width;
    }
    if slots.is_empty() && owned.ins_size > 0 {
        // Malformed/missing proto: preserve the old conservative word-slot behavior.
        slots.extend(0..owned.ins_size.min(16));
    }
    slots
}

/// True for `return` / `return-wide` / `return-object` (not `return-void`).
pub(crate) fn is_return_insn(label: &str) -> bool {
    matches!(
        label.split_whitespace().next().unwrap_or(""),
        "return" | "return-wide" | "return-object"
    )
}

fn is_extra_put(method_ref: &str) -> bool {
    method_ref.contains("putExtra")
        || method_ref.contains("putString")
        || method_ref.contains("putCharSequence")
        || method_ref.contains("putParcelable")
        || method_ref.contains("putSerializable")
        || method_ref.contains("putBundle")
        || method_ref.contains("putPersistableBundle")
}

fn is_extra_get(method_ref: &str) -> bool {
    method_ref.contains("Extra")
        || method_ref.contains("Bundle.getString")
        || method_ref.contains("Bundle.getCharSequence")
        || method_ref.contains("Bundle.getParcelable")
        || method_ref.contains("Bundle.getSerializable")
        || method_ref.contains("Bundle.getBundle")
        || method_ref.contains("PersistableBundle.get")
}

/// `const-string vN, "foo"` / `const-string/jumbo vN, "foo"`.
fn parse_const_string_label(label: &str) -> Option<(u32, String)> {
    let rest = label
        .strip_prefix("const-string/jumbo")
        .or_else(|| label.strip_prefix("const-string"))?;
    let rest = rest.trim();
    let (reg_part, str_part) = rest.split_once(',')?;
    let reg_s = reg_part.trim().strip_prefix('v')?;
    let reg: u32 = reg_s.parse().ok()?;
    let s = str_part.trim();
    if !s.starts_with('"') {
        return None;
    }
    let inner = s.trim_matches('"');
    if inner.is_empty() {
        return None;
    }
    Some((reg, inner.to_string()))
}

fn const_string_for_reg(
    owned: &ValueFlowAnalysisOwned,
    invoke_off: u32,
    reg: u32,
) -> Option<String> {
    let mut best: Option<(u32, String)> = None;
    for (&off, label) in &owned.insn_at {
        if off > invoke_off {
            continue;
        }
        if let Some((r, s)) = parse_const_string_label(label) {
            if r == reg && best.as_ref().map(|(o, _)| off >= *o).unwrap_or(true) {
                best = Some((off, s));
            }
        }
    }
    best.map(|(_, s)| s)
}

fn parse_const_int_label(label: &str) -> Option<(u32, i64)> {
    let opcode = label.split_whitespace().next()?;
    if !opcode.starts_with("const") || opcode.starts_with("const-string") {
        return None;
    }
    let rest = label[opcode.len()..].trim();
    let (reg_part, value_part) = rest.split_once(',')?;
    let reg = parse_reg_token(reg_part)?;
    let value = value_part.trim().split_whitespace().next()?;
    let parsed = if let Some(hex) = value.strip_prefix("-0x") {
        -(i64::from_str_radix(hex, 16).ok()?)
    } else if let Some(hex) = value.strip_prefix("0x") {
        i64::from_str_radix(hex, 16).ok()?
    } else {
        value.parse().ok()?
    };
    Some((reg, parsed))
}

fn const_int_for_reg(owned: &ValueFlowAnalysisOwned, at_off: u32, reg: u32) -> Option<i64> {
    owned
        .insn_at
        .iter()
        .filter_map(|(&off, label)| {
            let (r, value) = parse_const_int_label(label)?;
            (off <= at_off && r == reg).then_some((off, value))
        })
        .max_by_key(|(off, _)| *off)
        .map(|(_, value)| value)
}

fn parse_reg_token(s: &str) -> Option<u32> {
    s.trim().strip_prefix('v')?.parse().ok()
}

fn normalize_field_path(field: &str) -> String {
    let f = field.trim();
    if let Some(rest) = f.strip_prefix('L') {
        if let Some((cls, after)) = rest.split_once(';') {
            let cls = cls.replace('/', ".");
            let name = after
                .trim_start_matches('.')
                .split(':')
                .next()
                .unwrap_or(after);
            return format!("field:{cls}.{name}");
        }
    }
    format!("field:{f}")
}

#[derive(Debug)]
enum InstanceFieldOp {
    Put { src: u32, obj: u32, path: String },
    Get { dest: u32, obj: u32, path: String },
}

/// `iput-object vSrc, vObj, Lpkg/Cls;.name:Type` or `pkg.Cls.name`.
fn parse_instance_field_label(label: &str) -> Option<InstanceFieldOp> {
    let opcode = label.split_whitespace().next()?;
    let is_put = opcode.starts_with("iput");
    let is_get = opcode.starts_with("iget");
    if !is_put && !is_get {
        return None;
    }
    let rest = label[opcode.len()..].trim();
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return None;
    }
    let r0 = parse_reg_token(parts[0])?;
    let r1 = parse_reg_token(parts[1])?;
    let path = normalize_field_path(parts[2]);
    if is_put {
        Some(InstanceFieldOp::Put {
            src: r0,
            obj: r1,
            path,
        })
    } else {
        Some(InstanceFieldOp::Get {
            dest: r0,
            obj: r1,
            path,
        })
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum StaticFieldOp {
    Put { src: u32, path: String },
    Get { dest: u32, path: String },
}

#[derive(Debug)]
enum ArrayOp {
    Put { src: u32, array: u32, index: u32 },
    Get { dest: u32, array: u32, index: u32 },
}

fn parse_array_label(label: &str) -> Option<ArrayOp> {
    let opcode = label.split_whitespace().next()?;
    let is_put = opcode.starts_with("aput");
    let is_get = opcode.starts_with("aget");
    if !is_put && !is_get {
        return None;
    }
    let rest = label[opcode.len()..].trim();
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return None;
    }
    let first = parse_reg_token(parts[0])?;
    let array = parse_reg_token(parts[1])?;
    let index = parse_reg_token(parts[2])?;
    if is_put {
        Some(ArrayOp::Put {
            src: first,
            array,
            index,
        })
    } else {
        Some(ArrayOp::Get {
            dest: first,
            array,
            index,
        })
    }
}

fn array_path(owned: &ValueFlowAnalysisOwned, off: u32, index: u32) -> String {
    const_int_for_reg(owned, off, index)
        .map(|i| format!("array:{i}"))
        .unwrap_or_else(|| "array:*".to_string())
}

/// `sget-object vN, Lpkg/Cls;.BASE:Type` or `pkg.Cls.BASE`.
fn parse_static_field_label(label: &str) -> Option<StaticFieldOp> {
    let opcode = label.split_whitespace().next()?;
    let is_put = opcode.starts_with("sput");
    let is_get = opcode.starts_with("sget");
    if !is_put && !is_get {
        return None;
    }
    let rest = label[opcode.len()..].trim();
    let parts: Vec<&str> = rest.split(',').map(str::trim).collect();
    if parts.len() < 2 {
        return None;
    }
    let reg = parse_reg_token(parts[0])?;
    let path = normalize_field_path(parts[1]);
    if is_put {
        Some(StaticFieldOp::Put { src: reg, path })
    } else {
        Some(StaticFieldOp::Get { dest: reg, path })
    }
}

fn field_leaf_name(path: &str) -> &str {
    path.rsplit(['.', ':']).next().unwrap_or(path)
}

fn is_host_field_name(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "API" | "BASE" | "HOST" | "URL" | "ENDPOINT"
    )
}

/// Class-level `field:Class.name` → const-string written by sput/iput.
fn collect_class_field_strings(
    vf_cache: &HashMap<MethodId, ValueFlowAnalysisOwned>,
) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    for owned in vf_cache.values() {
        let mut last_str: HashMap<u32, String> = HashMap::new();
        let mut offs: Vec<u32> = owned.insn_at.keys().copied().collect();
        offs.sort_unstable();
        for off in offs {
            let Some(label) = owned.insn_at.get(&off) else {
                continue;
            };
            if let Some((reg, s)) = parse_const_string_label(label) {
                last_str.insert(reg, s);
                continue;
            }
            if let Some(InstanceFieldOp::Put { src, path, .. }) = parse_instance_field_label(label)
            {
                if let Some(s) = last_str.get(&src) {
                    out.insert(path, s.clone());
                }
            }
            if let Some(StaticFieldOp::Put { src, path }) = parse_static_field_label(label) {
                if let Some(s) = last_str.get(&src) {
                    out.insert(path, s.clone());
                }
            }
        }
    }
    out
}

fn looks_like_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// Reconstruct dest URL from const-string / BASE+path / class field writes.
fn recover_dest_url(
    owned: &ValueFlowAnalysisOwned,
    field_consts: &HashMap<String, String>,
) -> Option<String> {
    let mut https: Vec<String> = Vec::new();
    let mut paths: Vec<String> = Vec::new();
    for label in owned.insn_at.values() {
        if let Some((_, s)) = parse_const_string_label(label) {
            if looks_like_url(&s) {
                https.push(s);
            } else if s.starts_with('/') {
                paths.push(s);
            }
        }
        if let Some(StaticFieldOp::Get { path, .. }) = parse_static_field_label(label) {
            if let Some(val) = field_consts.get(&path) {
                if looks_like_url(val) {
                    https.push(val.clone());
                }
            } else if is_host_field_name(field_leaf_name(&path)) {
                if let Some(val) = field_consts
                    .iter()
                    .find(|(k, _)| field_leaf_name(k).eq_ignore_ascii_case(field_leaf_name(&path)))
                    .map(|(_, v)| v.clone())
                {
                    if looks_like_url(&val) {
                        https.push(val);
                    }
                }
            }
        }
        if let Some(InstanceFieldOp::Get { path, .. }) = parse_instance_field_label(label) {
            if let Some(val) = field_consts.get(&path) {
                if looks_like_url(val) {
                    https.push(val.clone());
                }
            }
        }
    }
    // Prefer a full URL that already has a path.
    if let Some(full) = https.iter().find(|u| u.matches('/').count() >= 3).cloned() {
        return Some(full);
    }
    if let Some(base) = https.into_iter().next() {
        if let Some(path) = paths.into_iter().next() {
            let base = base.trim_end_matches('/');
            return Some(format!("{base}{path}"));
        }
        return Some(base);
    }
    None
}

/// Incoming params have no bytecode def, so VF from offset 0 misses uses.
/// Taint every unread-def use of `reg` and propagate through copies.
fn seed_param_register(
    local: &mut LocalTaint,
    analysis: &crate::decompile::value_flow::ValueFlowAnalysis<'_>,
    owned: &ValueFlowAnalysisOwned,
    config: &TaintConfig,
    reg: u32,
    kind: &str,
) {
    local.insert(0, reg, kind);
    let mut offsets: Vec<u32> = owned.rw_map.keys().copied().collect();
    offsets.sort_unstable();
    let mut copied = HashSet::new();
    for off in offsets {
        let Some((reads, writes)) = owned.rw_map.get(&off) else {
            continue;
        };
        if !reads.contains(&reg) {
            continue;
        }
        let defs = analysis.use_def(off, reg);
        if !defs.is_empty() {
            continue;
        }
        local.insert(off, reg, kind);
        for &wreg in writes {
            if copied.insert((off, wreg)) {
                seed_and_propagate(local, analysis, owned, config, off, wreg, kind);
            }
        }
    }
    let flow = analysis.value_flow_from_seed(0, reg);
    for &(woff, wreg) in &flow.writes {
        local.insert(woff, wreg, kind);
    }
    for &(roff, rreg) in &flow.reads {
        local.insert(roff, rreg, kind);
    }
    apply_propagations(local, owned, config);
}

fn infer_param_to_sinks(
    mref: &super::index::MethodRef,
    owned: &ValueFlowAnalysisOwned,
    config: &TaintConfig,
) -> HashMap<u32, HashSet<String>> {
    let analysis = owned.analysis();
    let mut out: HashMap<u32, HashSet<String>> = HashMap::new();
    for param_idx in formal_param_slots(mref, owned) {
        let Some(reg) = param_reg(owned, param_idx) else {
            continue;
        };
        let mut probe = LocalTaint::default();
        seed_param_register(&mut probe, &analysis, owned, config, reg, "__param");
        for (&invoke_off, method_ref) in &owned.invoke_method_map {
            let Some(sink) = config.find_sink(method_ref) else {
                continue;
            };
            let args = owned
                .rw_map
                .get(&invoke_off)
                .map(|(r, _)| r.clone())
                .unwrap_or_default();
            let Some(port_i) = port_to_arg_index(&sink.port) else {
                continue;
            };
            let Some(&arg_reg) = args.get(port_i as usize) else {
                continue;
            };
            if probe.kinds_at(invoke_off, arg_reg).contains("__param") || arg_reg == reg {
                out.entry(param_idx).or_default().insert(sink.kind.clone());
            }
        }
    }
    out
}

fn propagate_path(
    local: &mut LocalTaint,
    analysis: &crate::decompile::value_flow::ValueFlowAnalysis<'_>,
    offset: u32,
    reg: u32,
    path: &str,
    kind: &str,
) {
    local.insert_path(offset, reg, path, kind);
    let flow = analysis.value_flow_from_seed(offset, reg);
    for &(woff, wreg) in &flow.writes {
        local.insert_path(woff, wreg, path, kind);
    }
    for &(roff, rreg) in &flow.reads {
        local.insert_path(roff, rreg, path, kind);
    }
}

/// Pair an invoke with the following `move-result*`.
///
/// Prefer `api_return_sources` after `invoke_off` with no other invoke between;
/// fall back to the next `rw_map` instruction whose label starts with `move-result`.
pub(crate) fn move_result_for_invoke(
    owned: &ValueFlowAnalysisOwned,
    invoke_off: u32,
) -> Option<(u32, u32)> {
    let mut best: Option<(u32, u32)> = None;
    for &((off, reg), _) in &owned.api_return_sources {
        if off <= invoke_off {
            continue;
        }
        let blocked = owned
            .invoke_method_map
            .keys()
            .any(|&k| k > invoke_off && k < off);
        if blocked {
            continue;
        }
        if best.map(|(o, _)| off < o).unwrap_or(true) {
            best = Some((off, reg));
        }
    }
    if best.is_some() {
        return best;
    }

    let mut offsets: Vec<u32> = owned.rw_map.keys().copied().collect();
    offsets.sort_unstable();
    let pos = offsets.iter().position(|&o| o == invoke_off)?;
    for &off in &offsets[pos + 1..] {
        if owned.invoke_method_map.contains_key(&off) {
            break;
        }
        let Some(label) = owned.insn_at.get(&off) else {
            continue;
        };
        if !label.starts_with("move-result") {
            continue;
        }
        if let Some((_, writes)) = owned.rw_map.get(&off) {
            if let Some(&reg) = writes.first() {
                return Some((off, reg));
            }
        }
    }
    None
}

fn param_kinds_fingerprint(sum: &MethodSummary) -> u64 {
    let mut len = 0u64;
    let mut kind_sum = 0u64;
    for kinds in sum.param_kinds.values() {
        len = len.wrapping_add(1);
        kind_sum = kind_sum.wrapping_add(kinds.len() as u64);
    }
    for kinds in sum.heap_injections.values() {
        len = len.wrapping_add(1);
        kind_sum = kind_sum.wrapping_add(kinds.len() as u64);
    }
    (len << 32) | (kind_sum & 0xffff_ffff)
}

type SummaryMap = HashMap<MethodId, Arc<MethodSummary>>;
type LocalTaintCache = Mutex<HashMap<MethodId, (u64, LocalTaint)>>;

fn should_skip(class_name: &str, opts: &SolveOptions) -> bool {
    if crate::detectors::is_library_class(class_name) {
        return true;
    }
    if opts
        .exclude_prefixes
        .iter()
        .any(|p| class_name.starts_with(p.as_str()))
    {
        return true;
    }
    if crate::detectors::class_matches_exclude_regexps(class_name, &opts.exclude_regexps) {
        return true;
    }
    if !opts.include_prefixes.is_empty()
        && !crate::detectors::class_matches_prefixes(class_name, &opts.include_prefixes)
    {
        return true;
    }
    false
}

/// Compose app-method TITO and sink summaries through bounded helper chains.
fn compose_method_summaries(
    index: &MethodIndex,
    call_graph: &CallGraph,
    vf_cache: &HashMap<MethodId, ValueFlowAnalysisOwned>,
    config: &TaintConfig,
    summaries: &mut SummaryMap,
) {
    const MAX_ROUNDS: usize = 8;
    const MAX_SINK_KINDS_PER_PARAM: usize = 64;
    for _ in 0..MAX_ROUNDS {
        let snapshot = summaries.clone();
        let mut changed = false;
        for (caller, edges) in &call_graph.outs {
            let (Some(caller_ref), Some(owned)) = (index.get(*caller), vf_cache.get(caller)) else {
                continue;
            };
            let analysis = owned.analysis();
            for caller_param in formal_param_slots(caller_ref, owned) {
                let Some(param_reg) = param_reg(owned, caller_param) else {
                    continue;
                };
                let mut probe = LocalTaint::default();
                seed_param_register(&mut probe, &analysis, owned, config, param_reg, "__summary");
                for edge in edges {
                    let Some(callee_sum) = snapshot.get(&edge.callee) else {
                        continue;
                    };
                    for (callee_param, &arg_reg) in edge.arg_regs.iter().enumerate() {
                        if !probe
                            .kinds_at(edge.invoke_offset, arg_reg)
                            .contains("__summary")
                            && arg_reg != param_reg
                        {
                            continue;
                        }
                        if let Some(sinks) = callee_sum.param_to_sinks.get(&(callee_param as u32)) {
                            let dst = Arc::make_mut(
                                summaries
                                    .entry(*caller)
                                    .or_insert_with(|| Arc::new(MethodSummary::default())),
                            )
                            .param_to_sinks
                            .entry(caller_param)
                            .or_default();
                            for sink in sinks.iter().take(MAX_SINK_KINDS_PER_PARAM) {
                                if dst.len() >= MAX_SINK_KINDS_PER_PARAM {
                                    break;
                                }
                                changed |= dst.insert(sink.clone());
                            }
                        }
                        if callee_sum.param_to_return.contains(&(callee_param as u32)) {
                            if let Some((mr_off, mr_reg)) =
                                move_result_for_invoke(owned, edge.invoke_offset)
                            {
                                let flow = analysis.value_flow_from_seed(mr_off, mr_reg);
                                if flow.reads.iter().any(|(off, _)| {
                                    is_return_insn(
                                        owned.insn_at.get(off).map(String::as_str).unwrap_or(""),
                                    )
                                }) {
                                    changed |= Arc::make_mut(
                                        summaries
                                            .entry(*caller)
                                            .or_insert_with(|| Arc::new(MethodSummary::default())),
                                    )
                                    .param_to_return
                                    .insert(caller_param);
                                }
                            }
                        }
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

fn compose_static_heap(
    index: &MethodIndex,
    vf_cache: &HashMap<MethodId, ValueFlowAnalysisOwned>,
    config: &TaintConfig,
    summaries: &mut SummaryMap,
) -> HashSet<MethodId> {
    const MAX_ROUNDS: usize = 4;
    let mut affected = HashSet::new();
    for _ in 0..MAX_ROUNDS {
        let snapshot = summaries.clone();
        let mut heap: HashMap<String, HashSet<String>> = HashMap::new();
        for (&mid, owned) in vf_cache {
            let local = compute_local_taint_uncached(mid, index, owned, config, &snapshot);
            for (&off, label) in &owned.insn_at {
                if let Some(StaticFieldOp::Put { src, path }) = parse_static_field_label(label) {
                    heap.entry(path)
                        .or_default()
                        .extend(local.kinds_at(off, src));
                }
            }
        }
        let mut changed = false;
        for (&mid, owned) in vf_cache {
            for (&off, label) in &owned.insn_at {
                let Some(StaticFieldOp::Get { dest, path }) = parse_static_field_label(label)
                else {
                    continue;
                };
                let Some(kinds) = heap.get(&path) else {
                    continue;
                };
                let dst = Arc::make_mut(
                    summaries
                        .entry(mid)
                        .or_insert_with(|| Arc::new(MethodSummary::default())),
                )
                .heap_injections
                .entry((off, dest))
                .or_default();
                for kind in kinds {
                    changed |= dst.insert(kind.clone());
                }
                if !kinds.is_empty() {
                    affected.insert(mid);
                }
            }
        }
        if !changed {
            break;
        }
    }
    affected
}

/// Analyze a single DEX with the default Android options.
pub fn solve_dex(dex: &DexFile, config: &TaintConfig) -> Result<SolveResult> {
    solve_dexes(&[dex], config, &SolveOptions::default_android())
}

/// Analyze one or more DEX files (multi-DEX app).
///
/// Hot paths run in parallel via rayon:
/// - per-method value-flow cache construction
/// - Pass-0 local source/sink analysis
/// - fixpoint worklist batches and call-edge arg propagation
pub fn solve_dexes(
    dexes: &[&DexFile],
    config: &TaintConfig,
    opts: &SolveOptions,
) -> Result<SolveResult> {
    let index = MethodIndex::from_dexes(dexes);

    // Parallel value-flow cache: each worker builds its own Decompiler (RefCell !Sync).
    let vf_cache: HashMap<MethodId, ValueFlowAnalysisOwned> = index
        .methods
        .par_iter()
        .filter_map(|m| {
            if should_skip(&m.class_name, opts) {
                return None;
            }
            let dex = dexes.get(m.dex_index)?;
            let decompiler = Decompiler::new(dex);
            decompiler
                .value_flow_analysis(&m.encoded)
                .ok()
                .map(|owned| (m.id, owned))
        })
        .collect();

    let analyzed: HashSet<MethodId> = vf_cache.keys().copied().collect();
    let call_graph =
        CallGraph::build_from_vf_cache(&index, &vf_cache, |id| analyzed.contains(&id))?;
    let field_consts = collect_class_field_strings(&vf_cache);

    let max_iter = if opts.max_iterations == 0 {
        8
    } else {
        opts.max_iterations
    };

    let mut summaries: SummaryMap = analyzed
        .iter()
        .map(|&id| (id, Arc::new(MethodSummary::default())))
        .collect();
    let local_taint_cache: LocalTaintCache = Mutex::new(HashMap::new());

    let mut issues: Vec<Issue> = Vec::new();
    let mut seen_issue_keys: HashSet<String> = HashSet::new();
    let mut iterations = 0usize;
    let mut worklist: VecDeque<(MethodId, u32, String)> = VecDeque::new();

    // Pass 0: independent per-method analysis (empty interproc params) — parallel.
    {
        let pass0: Vec<_> = index
            .methods
            .par_iter()
            .filter(|m| vf_cache.contains_key(&m.id))
            .map(|m| {
                let mut local_summaries: SummaryMap = HashMap::new();
                local_summaries.insert(m.id, Arc::new(MethodSummary::default()));
                let mut local_issues = Vec::new();
                let mut local_seen = HashSet::new();
                let mut local_wl = VecDeque::new();
                analyze_method(
                    m.id,
                    &index,
                    &vf_cache,
                    config,
                    &mut local_summaries,
                    &mut local_issues,
                    &mut local_seen,
                    &mut local_wl,
                    /*seed_only_params=*/ false,
                    &field_consts,
                );
                (
                    m.id,
                    Arc::try_unwrap(local_summaries.remove(&m.id).unwrap_or_default())
                        .unwrap_or_else(|a| (*a).clone()),
                    local_issues,
                    local_wl,
                )
            })
            .collect();

        for (mid, sum, local_issues, local_wl) in pass0 {
            if let Some(dst) = summaries.get_mut(&mid) {
                let dst = Arc::make_mut(dst);
                dst.return_kinds.extend(sum.return_kinds);
                dst.param_to_return.extend(sum.param_to_return);
                for (k, kinds) in sum.param_kinds {
                    dst.param_kinds.entry(k).or_default().extend(kinds);
                }
                for (k, sinks) in sum.param_to_sinks {
                    dst.param_to_sinks.entry(k).or_default().extend(sinks);
                }
            } else {
                summaries.insert(mid, Arc::new(sum));
            }
            for issue in local_issues {
                let key = issue_dedup_key(&issue);
                if seen_issue_keys.insert(key) {
                    issues.push(issue);
                }
            }
            worklist.extend(local_wl);
        }
    }

    compose_method_summaries(&index, &call_graph, &vf_cache, config, &mut summaries);
    for mid in compose_static_heap(&index, &vf_cache, config, &mut summaries) {
        worklist.push_back((mid, 0x4000_0000, "__heap".to_string()));
    }

    // Interprocedural fixpoint.
    // First iter after pass-0 scans every call edge once so arg→param can start.
    // Later iters only scan outgoing edges of dirty methods + methods whose
    // return_kinds grew (plus callers that just received a return injection).
    let mut first_interproc_edge_scan = true;
    while iterations < max_iter {
        iterations += 1;
        let mut progressed = false;

        let batch: Vec<_> = worklist.drain(..).collect();

        // Apply param-kind inserts, then re-analyze unique methods in parallel.
        // (Empty batch is OK — we still run return + call-edge transfers below.)
        let mut dirty: HashSet<MethodId> = HashSet::new();
        let mut return_grew: HashSet<MethodId> = HashSet::new();
        for (mid, param_idx, kind) in &batch {
            if *param_idx == 0x4000_0000 {
                dirty.insert(*mid);
                progressed = true;
                continue;
            }
            let entry = summaries
                .entry(*mid)
                .or_insert_with(|| Arc::new(MethodSummary::default()));
            let sum = Arc::make_mut(entry);
            if sum
                .param_kinds
                .entry(*param_idx)
                .or_default()
                .insert(kind.clone())
            {
                dirty.insert(*mid);
                progressed = true;
            }
        }

        if !dirty.is_empty() {
            let dirty_ids: Vec<MethodId> = dirty.iter().copied().collect();
            let summaries_snap = summaries.clone();
            let parallel_out: Vec<_> = dirty_ids
                .par_iter()
                .map(|&mid| {
                    let mut local_summaries = summaries_snap.clone();
                    let mut local_issues = Vec::new();
                    let mut local_seen = HashSet::new();
                    let mut local_wl = VecDeque::new();
                    analyze_method(
                        mid,
                        &index,
                        &vf_cache,
                        config,
                        &mut local_summaries,
                        &mut local_issues,
                        &mut local_seen,
                        &mut local_wl,
                        /*seed_only_params=*/ true,
                        &field_consts,
                    );
                    let sum = local_summaries
                        .remove(&mid)
                        .map(|a| Arc::try_unwrap(a).unwrap_or_else(|a| (*a).clone()))
                        .unwrap_or_default();
                    (mid, sum, local_issues, local_wl)
                })
                .collect();

            for (mid, sum, local_issues, local_wl) in parallel_out {
                if let Some(dst) = summaries.get_mut(&mid) {
                    let dst = Arc::make_mut(dst);
                    let before_ret = dst.return_kinds.len();
                    let before_p2r = dst.param_to_return.len();
                    dst.return_kinds.extend(sum.return_kinds);
                    dst.param_to_return.extend(sum.param_to_return);
                    for (k, sinks) in sum.param_to_sinks {
                        dst.param_to_sinks.entry(k).or_default().extend(sinks);
                    }
                    if dst.return_kinds.len() > before_ret {
                        return_grew.insert(mid);
                        progressed = true;
                    } else if dst.param_to_return.len() > before_p2r {
                        progressed = true;
                    }
                }
                for issue in local_issues {
                    let key = issue_dedup_key(&issue);
                    if seen_issue_keys.insert(key) {
                        issues.push(issue);
                    }
                }
                worklist.extend(local_wl);
            }
        }

        let mut inject_callers: HashSet<MethodId> = HashSet::new();
        progressed |= propagate_returns(
            &index,
            &call_graph,
            &vf_cache,
            config,
            &mut summaries,
            &mut issues,
            &mut seen_issue_keys,
            &mut worklist,
            &mut inject_callers,
        );

        // Parallel scan of call edges for arg→param taint transfer.
        // Must run even when the worklist was empty after pass-0 (otherwise
        // interprocedural arg→param never starts).
        let edge_list: Vec<_> = if first_interproc_edge_scan {
            first_interproc_edge_scan = false;
            call_graph.outs.values().flat_map(|v| v.iter()).collect()
        } else {
            let mut scan_ids = dirty;
            scan_ids.extend(return_grew);
            scan_ids.extend(inject_callers);
            scan_ids
                .iter()
                .filter_map(|id| call_graph.outs.get(id))
                .flatten()
                .collect()
        };
        let summaries_snap = summaries.clone();
        let edge_out: Vec<(Vec<(MethodId, u32, String)>, Vec<Issue>)> = edge_list
            .par_iter()
            .map(|edge| {
                let Some(owned) = vf_cache.get(&edge.caller) else {
                    return (Vec::new(), Vec::new());
                };
                let local = compute_local_taint(
                    edge.caller,
                    &index,
                    owned,
                    config,
                    &summaries_snap,
                    &local_taint_cache,
                );
                let dest = recover_dest_url(owned, &field_consts);
                let dest_s = dest.as_deref();
                let mut transfers = Vec::new();
                let mut local_issues = Vec::new();
                let mut local_seen = HashSet::new();
                for (i, &reg) in edge.arg_regs.iter().enumerate() {
                    let kinds = local.kinds_at(edge.invoke_offset, reg);
                    let callee_sinks = summaries_snap
                        .get(&edge.callee)
                        .and_then(|s| s.param_to_sinks.get(&(i as u32)))
                        .cloned()
                        .unwrap_or_default();
                    let callee_returns_param = summaries_snap
                        .get(&edge.callee)
                        .map(|s| s.param_to_return.contains(&(i as u32)))
                        .unwrap_or(false);
                    for kind in kinds {
                        if let Some(san) = config.find_sanitizer(&edge.method_ref) {
                            let set = sanitize_kinds(&HashSet::from([kind.clone()]), san);
                            if set.is_empty() {
                                continue;
                            }
                        }
                        for sink_kind in &callee_sinks {
                            let frames =
                                interproc_frames(&index, owned, edge, &kind, sink_kind, dest_s);
                            emit_issue_frames(
                                &index,
                                edge.caller,
                                config,
                                &kind,
                                sink_kind,
                                edge.invoke_offset,
                                frames,
                                &mut local_issues,
                                &mut local_seen,
                            );
                        }
                        let before = summaries_snap
                            .get(&edge.callee)
                            .and_then(|s| s.param_kinds.get(&(i as u32)))
                            .map(|s| s.contains(&kind))
                            .unwrap_or(false);
                        if !before {
                            transfers.push((edge.callee, i as u32, kind.clone()));
                        }
                        if callee_returns_param
                            && move_result_for_invoke(owned, edge.invoke_offset).is_some()
                        {
                            let key = 0x8000_0000u32 | edge.invoke_offset;
                            let already = summaries_snap
                                .get(&edge.caller)
                                .and_then(|s| s.param_kinds.get(&key))
                                .map(|ks| ks.contains(&kind))
                                .unwrap_or(false);
                            if !already {
                                transfers.push((edge.caller, key, kind));
                            }
                        }
                    }
                }
                (transfers, local_issues)
            })
            .collect();

        for (transfers, local_issues) in edge_out {
            for issue in local_issues {
                let key = issue_dedup_key(&issue);
                if seen_issue_keys.insert(key) {
                    issues.push(issue);
                }
            }
            for (callee, idx, kind) in transfers {
                let before = summaries
                    .get(&callee)
                    .and_then(|s| s.param_kinds.get(&idx))
                    .map(|s| s.contains(&kind))
                    .unwrap_or(false);
                if !before {
                    worklist.push_back((callee, idx, kind));
                    progressed = true;
                }
            }
        }

        if !progressed && worklist.is_empty() {
            break;
        }
    }

    let report = IssueReport {
        tool: "dex-decompiler-taint".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        stats: ReportStats {
            methods_analyzed: vf_cache.len(),
            call_edges: call_graph.edge_count(),
            issues: issues.len(),
            iterations,
        },
        issues: issues.clone(),
    };

    Ok(SolveResult { issues, report })
}

fn issue_dedup_key(issue: &Issue) -> String {
    let sink_off = issue.trace.last().and_then(|f| f.offset).unwrap_or(0);
    let (cls, method) = issue
        .callable
        .split_once('#')
        .unwrap_or((issue.callable.as_str(), ""));
    format!(
        "{}:{}:{}:{}:{}:{}",
        issue.rule_code, cls, method, issue.source_kind, issue.sink_kind, sink_off
    )
}

fn propagate_returns(
    index: &MethodIndex,
    call_graph: &CallGraph,
    vf_cache: &HashMap<MethodId, ValueFlowAnalysisOwned>,
    config: &TaintConfig,
    summaries: &mut SummaryMap,
    issues: &mut Vec<Issue>,
    seen: &mut HashSet<String>,
    worklist: &mut VecDeque<(MethodId, u32, String)>,
    inject_callers: &mut HashSet<MethodId>,
) -> bool {
    let mut progressed = false;
    for edges in call_graph.ins.values() {
        for edge in edges {
            let ret_kinds = summaries
                .get(&edge.callee)
                .map(|s| s.return_kinds.clone())
                .unwrap_or_default();
            if ret_kinds.is_empty() {
                continue;
            }
            let Some(owned) = vf_cache.get(&edge.caller) else {
                continue;
            };
            let Some((mr_off, mr_reg)) = move_result_for_invoke(owned, edge.invoke_offset) else {
                continue;
            };

            // Inject return kinds into caller local analysis by treating them as sources at move-result,
            // then check sinks in caller (via analyze with synthetic param — easier: expand return into
            // a temporary local re-scan).
            for kind in &ret_kinds {
                // Record as if a source appeared at move-result; run sink check from that seed.
                let analysis = owned.analysis();
                let flow = analysis.value_flow_from_seed(mr_off, mr_reg);
                for (read_off, reg) in &flow.reads {
                    if let Some(mref) = owned.invoke_method_map.get(read_off) {
                        if let Some(sink) = config.find_sink(mref) {
                            if let Some(arg_i) = port_to_arg_index(&sink.port) {
                                let args = owned
                                    .rw_map
                                    .get(read_off)
                                    .map(|(r, _)| r.clone())
                                    .unwrap_or_default();
                                if args.get(arg_i as usize) == Some(reg) {
                                    emit_issue(
                                        index,
                                        edge.caller,
                                        config,
                                        kind,
                                        &sink.kind,
                                        mr_off,
                                        *read_off,
                                        mref,
                                        None,
                                        issues,
                                        seen,
                                    );
                                }
                            } else if sink.port == Port::Return {
                                // uncommon
                            }
                        }
                        // Callee param transfer already handled elsewhere.
                        let _ = worklist;
                    }
                }
                // Also: if return kinds flow into params of further calls — covered by next iteration
                // when we re-analyze caller with synthetic… For summaries, mark nothing else.
                let _ = progressed;
            }
            // Update: if callee returns tainted, caller "receives" taint — re-queue caller analysis
            // by adding a fake high param isn't right. Instead bump a generation by pushing
            // return kinds into a side channel: store on summary of caller as "local source inject".
            if !ret_kinds.is_empty() {
                for kind in ret_kinds {
                    let key = 0x8000_0000u32 | edge.invoke_offset;
                    let entry = summaries
                        .entry(edge.caller)
                        .or_insert_with(|| Arc::new(MethodSummary::default()));
                    let sum = Arc::make_mut(entry);
                    if sum.param_kinds.entry(key).or_default().insert(kind.clone()) {
                        worklist.push_back((edge.caller, key, kind));
                        inject_callers.insert(edge.caller);
                        progressed = true;
                    }
                }
            }
        }
    }
    progressed
}

fn analyze_method(
    mid: MethodId,
    index: &MethodIndex,
    vf_cache: &HashMap<MethodId, ValueFlowAnalysisOwned>,
    config: &TaintConfig,
    summaries: &mut SummaryMap,
    issues: &mut Vec<Issue>,
    seen: &mut HashSet<String>,
    worklist: &mut VecDeque<(MethodId, u32, String)>,
    seed_only_params: bool,
    field_consts: &HashMap<String, String>,
) {
    let Some(mref) = index.get(mid) else { return };
    let Some(owned) = vf_cache.get(&mid) else {
        return;
    };
    let dest_url = recover_dest_url(owned, field_consts);
    let dest_url_s = dest_url.as_deref();

    let mut local = LocalTaint::default();
    let mut modeled_return_kinds = HashSet::new();
    let analysis = owned.analysis();
    if let Some(sum) = summaries.get(&mid) {
        for (&(off, reg), kinds) in &sum.heap_injections {
            for kind in kinds {
                propagate_register(&mut local, &analysis, off, reg, kind);
            }
        }
    }

    // 1) Seed from modeled sources (API returns and callback/lifecycle arguments).
    if !seed_only_params {
        for &((offset, reg), ref method_ref) in &owned.api_return_sources {
            for src in config.find_sources(method_ref) {
                if matches!(src.port, Port::Return) {
                    seed_and_propagate(
                        &mut local, &analysis, owned, config, offset, reg, &src.kind,
                    );
                }
            }
        }
        let callable = format!("{}.{}{}", mref.class_name, mref.method_name, mref.proto);
        for src in config.find_sources(&callable) {
            if src.port == Port::Return {
                modeled_return_kinds.insert(src.kind.clone());
                continue;
            }
            let Some(param_idx) = port_to_arg_index(&src.port) else {
                continue;
            };
            if !formal_param_slots(mref, owned).contains(&param_idx) {
                continue;
            }
            if let Some(reg) = param_reg(owned, param_idx) {
                seed_param_register(&mut local, &analysis, owned, config, reg, &src.kind);
            }
        }
    }

    // 2) Seed from interprocedural parameter kinds (and return-injection sentinels).
    let param_kinds = summaries
        .get(&mid)
        .map(|s| s.param_kinds.clone())
        .unwrap_or_default();
    for (param_key, kinds) in &param_kinds {
        if *param_key >= 0x8000_0000 {
            // Return injection at invoke offset encoded in low bits.
            let invoke_off = param_key & 0x7fff_ffff;
            if let Some((off, reg)) = move_result_for_invoke(owned, invoke_off) {
                for kind in kinds {
                    seed_and_propagate(&mut local, &analysis, owned, config, off, reg, kind);
                }
            }
            continue;
        }
        // Dalvik: param i lives in v(registers_size - ins_size + i).
        if let Some(reg) = param_reg(owned, *param_key) {
            for kind in kinds {
                seed_param_register(&mut local, &analysis, owned, config, reg, kind);
                // Re-scan sink reads after param seed (VF from 0 misses incoming params).
                for (&roff, (reads, _)) in &owned.rw_map {
                    if !reads
                        .iter()
                        .any(|r| local.kinds_at(roff, *r).contains(kind.as_str()))
                    {
                        continue;
                    }
                    for &rreg in reads {
                        if local.kinds_at(roff, rreg).contains(kind.as_str()) {
                            check_sink_at(
                                mid, index, owned, config, &local, roff, rreg, kind, dest_url_s,
                                issues, seen,
                            );
                        }
                    }
                }
            }
        }
    }

    // 3) Apply modeled propagations at invokes (TITO).
    apply_propagations(&mut local, owned, config);

    // 4) Sink checks for all tainted regs at invoke sites.
    for (&invoke_off, method_ref) in &owned.invoke_method_map {
        if let Some(sink) = config.find_sink(method_ref) {
            let args = owned
                .rw_map
                .get(&invoke_off)
                .map(|(r, _)| r.clone())
                .unwrap_or_default();
            if let Some(idx) = port_to_arg_index(&sink.port) {
                if let Some(&reg) = args.get(idx as usize) {
                    let kinds = local.kinds_at(invoke_off, reg);
                    for kind in kinds.into_iter() {
                        emit_issue(
                            index,
                            mid,
                            config,
                            kind.as_str(),
                            &sink.kind,
                            invoke_off,
                            invoke_off,
                            method_ref,
                            dest_url_s,
                            issues,
                            seen,
                        );
                    }
                }
            }
        }
        // Sanitizer: clear kinds on outputs if modeled.
        if let Some(san) = config.find_sanitizer(method_ref) {
            if let Some((off, reg)) = move_result_for_invoke(owned, invoke_off) {
                if let Some(set) = local.at.get_mut(&(off, reg)) {
                    *set = sanitize_kinds(set, san);
                }
            }
        }
    }

    // 5) Update summary: return kinds + param→return. Only real return* insns.
    let sum = Arc::make_mut(
        summaries
            .entry(mid)
            .or_insert_with(|| Arc::new(MethodSummary::default())),
    );
    sum.return_kinds.extend(modeled_return_kinds);
    for (&off, (reads, _writes)) in &owned.rw_map {
        if !is_return_insn(owned.insn_at.get(&off).map(String::as_str).unwrap_or("")) {
            continue;
        }
        for &reg in reads {
            let kinds = local.kinds_at(off, reg);
            if !kinds.is_empty() {
                sum.return_kinds.extend(kinds);
            }
        }
    }
    // Infer param→return for interproc TITO of app methods.
    for param_idx in formal_param_slots(mref, owned) {
        if let Some(reg) = param_reg(owned, param_idx) {
            let flow = analysis.value_flow_from_seed(0, reg);
            for &(roff, _rreg) in &flow.reads {
                if is_return_insn(owned.insn_at.get(&roff).map(String::as_str).unwrap_or("")) {
                    sum.param_to_return.insert(param_idx);
                }
            }
        }
    }
    // Structural + live-taint param→sink summaries (1-hop).
    for (idx, sinks) in infer_param_to_sinks(mref, owned, config) {
        sum.param_to_sinks.entry(idx).or_default().extend(sinks);
    }
    for param_idx in formal_param_slots(mref, owned) {
        let Some(reg) = param_reg(owned, param_idx) else {
            continue;
        };
        for (&invoke_off, method_ref) in &owned.invoke_method_map {
            let Some(sink) = config.find_sink(method_ref) else {
                continue;
            };
            let args = owned
                .rw_map
                .get(&invoke_off)
                .map(|(r, _)| r.clone())
                .unwrap_or_default();
            let Some(port_i) = port_to_arg_index(&sink.port) else {
                continue;
            };
            let Some(&arg_reg) = args.get(port_i as usize) else {
                continue;
            };
            let kinds = local.kinds_at(invoke_off, arg_reg);
            if kinds.is_empty() {
                continue;
            }
            // Incoming param (or a copy) reached this sink with live taint.
            if arg_reg == reg || local.kinds_at(0, reg).iter().any(|k| kinds.contains(k)) {
                sum.param_to_sinks
                    .entry(param_idx)
                    .or_default()
                    .insert(sink.kind.clone());
            }
        }
    }

    // 6) Queue callee params from this method's call sites (local taint).
    // (Main loop also does this; here helps first pass.)
    let _ = (mref, worklist);
}

fn seed_and_propagate(
    local: &mut LocalTaint,
    analysis: &crate::decompile::value_flow::ValueFlowAnalysis<'_>,
    owned: &ValueFlowAnalysisOwned,
    config: &TaintConfig,
    offset: u32,
    reg: u32,
    kind: &str,
) {
    local.insert(offset, reg, kind);
    let flow = analysis.value_flow_from_seed(offset, reg);
    for &(woff, wreg) in &flow.writes {
        local.insert(woff, wreg, kind);
    }
    for &(roff, rreg) in &flow.reads {
        local.insert(roff, rreg, kind);
    }
    apply_propagations(local, owned, config);
}

fn propagate_register(
    local: &mut LocalTaint,
    analysis: &crate::decompile::value_flow::ValueFlowAnalysis<'_>,
    offset: u32,
    reg: u32,
    kind: &str,
) {
    local.insert(offset, reg, kind);
    let flow = analysis.value_flow_from_seed(offset, reg);
    for &(woff, wreg) in &flow.writes {
        local.insert(woff, wreg, kind);
    }
    for &(roff, rreg) in &flow.reads {
        local.insert(roff, rreg, kind);
    }
}

fn apply_propagations(
    local: &mut LocalTaint,
    owned: &ValueFlowAnalysisOwned,
    config: &TaintConfig,
) {
    let analysis = owned.analysis();
    // Fixed few rounds of TITO at invoke sites; forward-propagate onto object regs
    // so later startActivity/commit see putExtra/setData taint.
    for _ in 0..4 {
        let mut updates: Vec<(u32, u32, String)> = Vec::new();
        for (&invoke_off, method_ref) in &owned.invoke_method_map {
            let props = config.find_propagations(method_ref);
            if props.is_empty() {
                continue;
            }
            let args = owned
                .rw_map
                .get(&invoke_off)
                .map(|(r, _)| r.clone())
                .unwrap_or_default();
            for prop in props {
                let Some(from_i) = port_to_arg_index(&prop.from) else {
                    continue;
                };
                let Some(&from_reg) = args.get(from_i as usize) else {
                    continue;
                };
                let kinds = local.kinds_at(invoke_off, from_reg);
                if kinds.is_empty() {
                    continue;
                }
                match &prop.to {
                    Port::Return => {
                        if let Some((off, reg)) = move_result_for_invoke(owned, invoke_off) {
                            for k in &kinds {
                                updates.push((off, reg, k.clone()));
                            }
                        }
                    }
                    Port::This | Port::Argument { .. } => {
                        if let Some(to_i) = port_to_arg_index(&prop.to) {
                            if let Some(&to_reg) = args.get(to_i as usize) {
                                // Const-key extras: taint extra:<key> only (do not smear the Intent).
                                if is_extra_put(method_ref) {
                                    if let Some(&key_reg) = args.get(1) {
                                        if let Some(key) =
                                            const_string_for_reg(owned, invoke_off, key_reg)
                                        {
                                            let path = format!("extra:{key}");
                                            for k in &kinds {
                                                propagate_path(
                                                    local, &analysis, invoke_off, to_reg, &path, k,
                                                );
                                            }
                                            continue;
                                        }
                                    }
                                }
                                for k in &kinds {
                                    updates.push((invoke_off, to_reg, k.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
        if updates.is_empty() {
            break;
        }
        let mut any = false;
        for (o, r, k) in updates {
            if local.insert(o, r, &k) {
                any = true;
                let flow = analysis.value_flow_from_seed(o, r);
                for &(woff, wreg) in &flow.writes {
                    local.insert(woff, wreg, &k);
                }
                for &(roff, rreg) in &flow.reads {
                    local.insert(roff, rreg, &k);
                }
            }
        }
        if !any {
            break;
        }
    }
    apply_field_heap(local, owned);
}

fn apply_field_heap(local: &mut LocalTaint, owned: &ValueFlowAnalysisOwned) {
    let analysis = owned.analysis();
    #[derive(Debug)]
    enum HeapOp {
        InstancePut(u32, u32, String),
        InstanceGet(u32, u32, String),
        StaticPut(u32, String),
        StaticGet(u32, String),
        ArrayPut(u32, u32, String),
        ArrayGet(u32, u32, String),
    }
    let mut ops = Vec::new();
    for (&off, label) in &owned.insn_at {
        match parse_instance_field_label(label) {
            Some(InstanceFieldOp::Put { src, obj, path }) => {
                ops.push((off, HeapOp::InstancePut(src, obj, path)))
            }
            Some(InstanceFieldOp::Get { dest, obj, path }) => {
                ops.push((off, HeapOp::InstanceGet(dest, obj, path)))
            }
            None => {}
        }
        match parse_static_field_label(label) {
            Some(StaticFieldOp::Put { src, path }) => ops.push((off, HeapOp::StaticPut(src, path))),
            Some(StaticFieldOp::Get { dest, path }) => {
                ops.push((off, HeapOp::StaticGet(dest, path)))
            }
            None => {}
        }
        match parse_array_label(label) {
            Some(ArrayOp::Put { src, array, index }) => ops.push((
                off,
                HeapOp::ArrayPut(src, array, array_path(owned, off, index)),
            )),
            Some(ArrayOp::Get { dest, array, index }) => ops.push((
                off,
                HeapOp::ArrayGet(dest, array, array_path(owned, off, index)),
            )),
            None => {}
        }
    }
    ops.sort_by_key(|(off, _)| *off);
    let mut static_heap: HashMap<String, HashSet<String>> = HashMap::new();
    for (off, op) in ops {
        let (dest, kinds) = match op {
            HeapOp::InstancePut(src, obj, path) => {
                for k in local.kinds_at(off, src) {
                    propagate_path(local, &analysis, off, obj, &path, &k);
                }
                continue;
            }
            HeapOp::InstanceGet(dest, obj, path) => {
                let mut kinds = local.path_kinds_on_reg_before(obj, &path, off);
                if kinds.is_empty() {
                    kinds = local.kinds_at(off, obj);
                }
                (dest, kinds)
            }
            HeapOp::StaticPut(src, path) => {
                static_heap
                    .entry(path)
                    .or_default()
                    .extend(local.kinds_at(off, src));
                continue;
            }
            HeapOp::StaticGet(dest, path) => {
                (dest, static_heap.get(&path).cloned().unwrap_or_default())
            }
            HeapOp::ArrayPut(src, array, path) => {
                for k in local.kinds_at(off, src) {
                    propagate_path(local, &analysis, off, array, &path, &k);
                }
                continue;
            }
            HeapOp::ArrayGet(dest, array, path) => {
                let kinds = if path == "array:*" {
                    local.array_kinds_on_reg_before(array, off)
                } else {
                    let mut exact = local.path_kinds_on_reg_before(array, &path, off);
                    exact.extend(local.path_kinds_on_reg_before(array, "array:*", off));
                    exact
                };
                (dest, kinds)
            }
        };
        for k in kinds {
            propagate_register(local, &analysis, off, dest, &k);
        }
    }
    for (&invoke_off, method_ref) in &owned.invoke_method_map {
        if !is_extra_get(method_ref) {
            continue;
        }
        let args = owned
            .rw_map
            .get(&invoke_off)
            .map(|(r, _)| r.clone())
            .unwrap_or_default();
        let Some(&obj) = args.first() else {
            continue;
        };
        let Some((mr_off, mr_reg)) = move_result_for_invoke(owned, invoke_off) else {
            continue;
        };
        let kinds = match args
            .get(1)
            .copied()
            .and_then(|key_reg| const_string_for_reg(owned, invoke_off, key_reg))
        {
            Some(key) => local.path_kinds_on_reg(obj, &format!("extra:{key}")),
            None => local.kinds_at(invoke_off, obj),
        };
        for k in kinds {
            if local.insert(mr_off, mr_reg, &k) {
                let flow = analysis.value_flow_from_seed(mr_off, mr_reg);
                for &(woff, wreg) in &flow.writes {
                    local.insert(woff, wreg, &k);
                }
                for &(roff, rreg) in &flow.reads {
                    local.insert(roff, rreg, &k);
                }
            }
        }
    }
}

fn check_sink_at(
    mid: MethodId,
    index: &MethodIndex,
    owned: &ValueFlowAnalysisOwned,
    config: &TaintConfig,
    local: &LocalTaint,
    offset: u32,
    reg: u32,
    kind: &str,
    dest_url: Option<&str>,
    issues: &mut Vec<Issue>,
    seen: &mut HashSet<String>,
) {
    let Some(method_ref) = owned.invoke_method_map.get(&offset) else {
        return;
    };
    let Some(sink) = config.find_sink(method_ref) else {
        return;
    };
    let args = owned
        .rw_map
        .get(&offset)
        .map(|(r, _)| r.clone())
        .unwrap_or_default();
    if let Some(idx) = port_to_arg_index(&sink.port) {
        if args.get(idx as usize) == Some(&reg) {
            emit_issue(
                index, mid, config, kind, &sink.kind, offset, offset, method_ref, dest_url, issues,
                seen,
            );
        }
    }
    let _ = local;
}

fn interproc_frames(
    index: &MethodIndex,
    caller_owned: &ValueFlowAnalysisOwned,
    edge: &CallEdge,
    source_kind: &str,
    sink_kind: &str,
    dest_url: Option<&str>,
) -> Vec<TraceFrame> {
    let caller = index.get(edge.caller);
    let callee = index.get(edge.callee);
    let (c_cls, c_meth) = caller
        .map(|m| (m.class_name.clone(), m.method_name.clone()))
        .unwrap_or_else(|| ("?".into(), "?".into()));
    let (k_cls, k_meth) = callee
        .map(|m| (m.class_name.clone(), m.method_name.clone()))
        .unwrap_or_else(|| ("?".into(), "?".into()));
    let dest_s = dest_url.unwrap_or("");
    let dest_extra = dest_url.map(|s| s.to_string());

    let mut frames = Vec::new();
    // Cheap stitch: a modeled source or collect* return in the caller.
    if let Some(((off, _), mref)) = caller_owned.api_return_sources.iter().find(|(_, r)| {
        r.contains("getDeviceId")
            || r.contains("getImei")
            || r.contains("collectDeviceId")
            || r.contains("getLastLocation")
    }) {
        let (s_cls, s_meth) = mref
            .rsplit_once('.')
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .unwrap_or_else(|| (c_cls.clone(), mref.clone()));
        frames.push(TraceFrame {
            class_name: s_cls,
            method_name: s_meth,
            offset: Some(*off),
            kind: source_kind.to_string(),
            description: format!("source `{mref}` ({source_kind})"),
            extra: None,
            field: None,
        });
    }
    frames.push(TraceFrame {
        class_name: c_cls.clone(),
        method_name: c_meth.clone(),
        offset: Some(edge.invoke_offset),
        kind: source_kind.to_string(),
        description: format!("arg tainted ({source_kind}) before call"),
        extra: None,
        field: None,
    });
    let call_desc = if dest_s.is_empty() {
        format!("invoke {}#{} ({})", k_cls, k_meth, edge.method_ref)
    } else {
        format!("invoke {}#{} dest={}", k_cls, k_meth, dest_s)
    };
    frames.push(TraceFrame {
        class_name: c_cls,
        method_name: c_meth,
        offset: Some(edge.invoke_offset),
        kind: "call".into(),
        description: call_desc,
        extra: dest_extra.clone(),
        field: None,
    });
    frames.push(TraceFrame {
        class_name: k_cls,
        method_name: k_meth,
        offset: None,
        kind: sink_kind.to_string(),
        description: format!(
            "callee sink ({sink_kind}){}",
            dest_url.map(|u| format!(" dest={u}")).unwrap_or_default()
        ),
        extra: dest_extra,
        field: None,
    });
    frames
}

fn emit_issue(
    index: &MethodIndex,
    mid: MethodId,
    config: &TaintConfig,
    source_kind: &str,
    sink_kind: &str,
    source_offset: u32,
    sink_offset: u32,
    sink_ref: &str,
    dest_url: Option<&str>,
    issues: &mut Vec<Issue>,
    seen: &mut HashSet<String>,
) {
    let mref = match index.get(mid) {
        Some(m) => m,
        None => return,
    };
    let sink_desc = match dest_url {
        Some(u) => format!("sink `{sink_ref}` ({sink_kind}) dest={u}"),
        None => format!("sink `{sink_ref}` ({sink_kind})"),
    };
    let frames = vec![
        TraceFrame {
            class_name: mref.class_name.clone(),
            method_name: mref.method_name.clone(),
            offset: Some(source_offset),
            kind: source_kind.to_string(),
            description: format!("source kind `{source_kind}` introduced / flowing"),
            extra: None,
            field: None,
        },
        TraceFrame {
            class_name: mref.class_name.clone(),
            method_name: mref.method_name.clone(),
            offset: Some(sink_offset),
            kind: sink_kind.to_string(),
            description: sink_desc,
            extra: dest_url.map(|s| s.to_string()),
            field: None,
        },
    ];
    emit_issue_frames(
        index,
        mid,
        config,
        source_kind,
        sink_kind,
        sink_offset,
        frames,
        issues,
        seen,
    );
}

fn emit_issue_frames(
    index: &MethodIndex,
    mid: MethodId,
    config: &TaintConfig,
    source_kind: &str,
    sink_kind: &str,
    sink_offset: u32,
    frames: Vec<TraceFrame>,
    issues: &mut Vec<Issue>,
    seen: &mut HashSet<String>,
) {
    let rules = config.matching_rules(source_kind, sink_kind);
    if rules.is_empty() {
        return;
    }
    let mref = match index.get(mid) {
        Some(m) => m,
        None => return,
    };
    for rule in rules {
        let key = format!(
            "{}:{}:{}:{}:{}:{}",
            rule.code, mref.class_name, mref.method_name, source_kind, sink_kind, sink_offset
        );
        if !seen.insert(key) {
            continue;
        }
        let dest = frames.iter().find_map(|f| f.extra.clone());
        let mut description = rule.description.clone();
        if let Some(u) = dest {
            if !description.contains(&u) {
                description = format!("{description} dest={u}");
            }
        }
        issues.push(Issue {
            rule_code: rule.code,
            rule_name: rule.name.clone(),
            description,
            source_kind: source_kind.to_string(),
            sink_kind: sink_kind.to_string(),
            callable: format!("{}#{}", mref.class_name, mref.method_name),
            trace: frames.clone(),
        });
    }
}

fn compute_local_taint_uncached(
    mid: MethodId,
    index: &MethodIndex,
    owned: &ValueFlowAnalysisOwned,
    config: &TaintConfig,
    summaries: &SummaryMap,
) -> LocalTaint {
    let mut local = LocalTaint::default();
    let analysis = owned.analysis();
    if let Some(sum) = summaries.get(&mid) {
        for (&(off, reg), kinds) in &sum.heap_injections {
            for kind in kinds {
                propagate_register(&mut local, &analysis, off, reg, kind);
            }
        }
    }
    for &((offset, reg), ref method_ref) in &owned.api_return_sources {
        for src in config.find_sources(method_ref) {
            if matches!(src.port, Port::Return) {
                seed_and_propagate(&mut local, &analysis, owned, config, offset, reg, &src.kind);
            }
        }
    }
    if let Some(mref) = index.get(mid) {
        let callable = format!("{}.{}{}", mref.class_name, mref.method_name, mref.proto);
        for src in config.find_sources(&callable) {
            let Some(param_idx) = port_to_arg_index(&src.port) else {
                continue;
            };
            if !formal_param_slots(mref, owned).contains(&param_idx) {
                continue;
            }
            if let Some(reg) = param_reg(owned, param_idx) {
                seed_param_register(&mut local, &analysis, owned, config, reg, &src.kind);
            }
        }
    }
    if let Some(sum) = summaries.get(&mid) {
        for (param_key, kinds) in &sum.param_kinds {
            if *param_key >= 0x8000_0000 {
                let invoke_off = param_key & 0x7fff_ffff;
                if let Some((off, reg)) = move_result_for_invoke(owned, invoke_off) {
                    for kind in kinds {
                        seed_and_propagate(&mut local, &analysis, owned, config, off, reg, kind);
                    }
                }
                continue;
            }
            if let Some(reg) = param_reg(owned, *param_key) {
                for kind in kinds {
                    seed_param_register(&mut local, &analysis, owned, config, reg, kind);
                }
            }
        }
    }
    apply_propagations(&mut local, owned, config);
    local
}

fn compute_local_taint(
    mid: MethodId,
    index: &MethodIndex,
    owned: &ValueFlowAnalysisOwned,
    config: &TaintConfig,
    summaries: &SummaryMap,
    cache: &LocalTaintCache,
) -> LocalTaint {
    let fp = summaries
        .get(&mid)
        .map(|s| param_kinds_fingerprint(s))
        .unwrap_or(0);
    if let Ok(guard) = cache.lock() {
        if let Some((old_fp, cached)) = guard.get(&mid) {
            if *old_fp == fp {
                return cached.clone();
            }
        }
    }
    let local = compute_local_taint_uncached(mid, index, owned, config, summaries);
    if let Ok(mut guard) = cache.lock() {
        guard.insert(mid, (fp, local.clone()));
    }
    local
}

#[cfg(test)]
mod tests {
    use super::super::models::SanitizerModel;
    use super::*;
    use crate::decompile::cfg::MethodCfg;
    use crate::decompile::value_flow::ValueFlowAnalysisOwned;
    use dex_parser::EncodedMethod;
    use std::collections::HashMap;

    fn empty_cfg() -> MethodCfg {
        MethodCfg {
            blocks: Vec::new(),
            block_by_start: HashMap::new(),
            loop_headers: HashSet::new(),
            entry: 0,
            folded_const_offsets: HashSet::new(),
        }
    }

    fn stub_owned(
        rw_map: HashMap<u32, (Vec<u32>, Vec<u32>)>,
        api_return_sources: Vec<((u32, u32), String)>,
        invoke_method_map: HashMap<u32, String>,
        insn_at: HashMap<u32, String>,
        registers_size: u32,
        ins_size: u32,
    ) -> ValueFlowAnalysisOwned {
        ValueFlowAnalysisOwned {
            cfg: empty_cfg(),
            rw_map,
            exceptional_edges: vec![],
            api_return_sources,
            invoke_method_map,
            insn_at,
            registers_size,
            ins_size,
        }
    }

    fn stub_method(proto: &str, access_flags: u32) -> super::super::index::MethodRef {
        super::super::index::MethodRef {
            id: MethodId(0),
            class_name: "com.foo.Callback".into(),
            method_name: "callback".into(),
            proto: proto.into(),
            dex_index: 0,
            encoded: EncodedMethod {
                method_idx: 0,
                access_flags,
                code_off: 1,
            },
        }
    }

    #[test]
    fn param_reg_sizes_match_dalvik_layout() {
        assert_eq!(param_reg_from_sizes(5, 2, 0), Some(3));
        assert_eq!(param_reg_from_sizes(5, 2, 1), Some(4));
        assert_eq!(param_reg_from_sizes(5, 2, 2), None);
        assert_eq!(param_reg_from_sizes(0, 0, 0), None);
        let owned = stub_owned(
            HashMap::new(),
            Vec::new(),
            HashMap::new(),
            HashMap::new(),
            5,
            2,
        );
        assert_eq!(param_reg(&owned, 0), Some(3));
        assert_eq!(param_reg(&owned, 1), Some(4));
        assert_eq!(param_reg(&owned, 2), None);
    }

    #[test]
    fn move_result_pairs_each_invoke_not_the_later_one() {
        let mut rw_map = HashMap::new();
        rw_map.insert(0, (vec![0], vec![]));
        rw_map.insert(6, (vec![], vec![0]));
        rw_map.insert(12, (vec![1], vec![]));
        rw_map.insert(18, (vec![], vec![1]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(0, "com.foo.A.src".into());
        invoke_method_map.insert(12, "com.foo.B.src".into());
        let mut insn_at = HashMap::new();
        insn_at.insert(6, "move-result-object v0".into());
        insn_at.insert(18, "move-result-object v1".into());
        // Deliberately list the later return first so a first-match `off >= invoke` is wrong.
        let api_return_sources = vec![
            ((18, 1), "com.foo.B.src".into()),
            ((6, 0), "com.foo.A.src".into()),
        ];
        let owned = stub_owned(rw_map, api_return_sources, invoke_method_map, insn_at, 4, 0);
        assert_eq!(move_result_for_invoke(&owned, 0), Some((6, 0)));
        assert_eq!(move_result_for_invoke(&owned, 12), Some((18, 1)));
    }

    #[test]
    fn move_result_fallback_uses_insn_label() {
        let mut rw_map = HashMap::new();
        rw_map.insert(0, (vec![0], vec![]));
        rw_map.insert(4, (vec![], vec![2]));
        rw_map.insert(8, (vec![1], vec![]));
        rw_map.insert(12, (vec![], vec![3]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(0, "com.foo.A.src".into());
        invoke_method_map.insert(8, "com.foo.B.src".into());
        let mut insn_at = HashMap::new();
        insn_at.insert(4, "move-result v2".into());
        insn_at.insert(12, "move-result-wide v3".into());
        let owned = stub_owned(rw_map, Vec::new(), invoke_method_map, insn_at, 4, 0);
        assert_eq!(move_result_for_invoke(&owned, 0), Some((4, 2)));
        assert_eq!(move_result_for_invoke(&owned, 8), Some((12, 3)));
    }

    #[test]
    fn is_return_insn_accepts_value_returns_only() {
        assert!(is_return_insn("return-object v0"));
        assert!(is_return_insn("return v1"));
        assert!(is_return_insn("return-wide v2"));
        assert!(!is_return_insn("return-void"));
        assert!(!is_return_insn("invoke-virtual {v0}, foo"));
        assert!(!is_return_insn("move-result-object v0"));
    }

    #[test]
    fn empty_sanitizer_kinds_are_noop_star_clears() {
        let kinds: HashSet<String> = ["DeviceId", "UserInput"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let noop = SanitizerModel {
            patterns: vec!["MessageDigest.digest".into()],
            kinds: vec![],
        };
        assert_eq!(sanitize_kinds(&kinds, &noop), kinds);
        let star = SanitizerModel {
            patterns: vec!["Origin.sanitize".into()],
            kinds: vec!["*".into()],
        };
        assert!(sanitize_kinds(&kinds, &star).is_empty());
        let specific = SanitizerModel {
            patterns: vec!["hash".into()],
            kinds: vec!["DeviceId".into()],
        };
        let left = sanitize_kinds(&kinds, &specific);
        assert!(left.contains("UserInput"));
        assert!(!left.contains("DeviceId"));
    }

    #[test]
    fn param_kinds_fingerprint_changes_when_kinds_grow() {
        let mut sum = MethodSummary::default();
        let fp0 = param_kinds_fingerprint(&sum);
        sum.param_kinds.entry(0).or_default().insert("K".into());
        let fp1 = param_kinds_fingerprint(&sum);
        assert_ne!(fp0, fp1);
        sum.param_kinds.entry(0).or_default().insert("K2".into());
        let fp2 = param_kinds_fingerprint(&sum);
        assert_ne!(fp1, fp2);
        // Same contents → same fingerprint (cache stays valid).
        assert_eq!(fp2, param_kinds_fingerprint(&sum));
    }

    #[test]
    fn extra_path_put_get_is_key_sensitive() {
        let mut local = LocalTaint::default();
        assert!(local.insert_path(0, 0, "extra:url", "UserInput"));
        assert!(local.kinds_at_path(0, 0, "extra:url").contains("UserInput"));
        assert!(local.kinds_at_path(0, 0, "extra:theme").is_empty());
        assert!(
            local.kinds_at(0, 0).is_empty(),
            "const-key extra must not smear object-level taint"
        );
        assert!(local
            .path_kinds_on_reg(0, "extra:url")
            .contains("UserInput"));
        assert!(local.path_kinds_on_reg(0, "extra:theme").is_empty());
    }

    #[test]
    fn field_path_iput_iget_is_field_sensitive() {
        let mut local = LocalTaint::default();
        local.insert(8, 1, "DeviceId"); // vSrc tainted at the iput
        let mut rw_map = HashMap::new();
        rw_map.insert(8, (vec![1, 0], vec![])); // iput v1, v0
        rw_map.insert(12, (vec![0], vec![2])); // iget v2, v0
        let mut insn_at = HashMap::new();
        insn_at.insert(
            8,
            "iput-object v1, v0, Lcom/foo/Foo;.bar:Ljava/lang/String;".into(),
        );
        insn_at.insert(
            12,
            "iget-object v2, v0, Lcom/foo/Foo;.bar:Ljava/lang/String;".into(),
        );
        insn_at.insert(
            16,
            "iget-object v3, v0, Lcom/foo/Foo;.other:Ljava/lang/String;".into(),
        );
        let owned = stub_owned(rw_map, Vec::new(), HashMap::new(), insn_at, 4, 0);
        apply_field_heap(&mut local, &owned);
        assert!(
            local
                .kinds_at_path(8, 0, "field:com.foo.Foo.bar")
                .contains("DeviceId"),
            "iput should taint field:Foo.bar"
        );
        assert!(
            local.kinds_at(12, 2).contains("DeviceId"),
            "iget Foo.bar should receive kind: {:?}",
            local.kinds_at(12, 2)
        );
        assert!(
            local.kinds_at(16, 3).is_empty(),
            "iget Foo.other must stay clean: {:?}",
            local.kinds_at(16, 3)
        );
    }

    #[test]
    fn put_extra_const_key_does_not_smear_object() {
        let cfg = crate::taint::defaults::default_config();
        let mut local = LocalTaint::default();
        local.insert(10, 2, "UserInput"); // value arg
        let mut rw_map = HashMap::new();
        rw_map.insert(10, (vec![0, 1, 2], vec![]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(10, "android.content.Intent.putExtra".into());
        let mut insn_at = HashMap::new();
        insn_at.insert(4, "const-string v1, \"url\"".into());
        let owned = stub_owned(rw_map, Vec::new(), invoke_method_map, insn_at, 4, 0);
        apply_propagations(&mut local, &owned, &cfg);
        assert!(
            local
                .kinds_at_path(10, 0, "extra:url")
                .contains("UserInput"),
            "putExtra(\"url\") should taint extra:url: {:?}",
            local.kinds_at_path(10, 0, "extra:url")
        );
        assert!(
            local.kinds_at(10, 0).is_empty(),
            "const-key putExtra must not taint the whole Intent: {:?}",
            local.kinds_at(10, 0)
        );
        // getStringExtra("theme") reads a different key → empty
        let mut rw2 = HashMap::new();
        rw2.insert(10, (vec![0, 1, 2], vec![]));
        rw2.insert(20, (vec![0, 3], vec![]));
        rw2.insert(24, (vec![], vec![4]));
        let mut inv2 = HashMap::new();
        inv2.insert(10, "android.content.Intent.putExtra".into());
        inv2.insert(20, "android.content.Intent.getStringExtra".into());
        let mut insn2 = HashMap::new();
        insn2.insert(4, "const-string v1, \"url\"".into());
        insn2.insert(16, "const-string v3, \"theme\"".into());
        insn2.insert(24, "move-result-object v4".into());
        let owned2 = stub_owned(
            rw2,
            vec![((24, 4), "android.content.Intent.getStringExtra".into())],
            inv2,
            insn2,
            5,
            0,
        );
        let mut local2 = LocalTaint::default();
        local2.insert(10, 2, "UserInput");
        apply_propagations(&mut local2, &owned2, &cfg);
        assert!(local2
            .kinds_at_path(10, 0, "extra:url")
            .contains("UserInput"));
        assert!(
            local2.kinds_at(24, 4).is_empty(),
            "getStringExtra(\"theme\") must not see extra:url: {:?}",
            local2.kinds_at(24, 4)
        );
    }

    #[test]
    fn parse_const_string_and_field_labels() {
        assert_eq!(
            parse_const_string_label("const-string v1, \"url\""),
            Some((1, "url".into()))
        );
        assert_eq!(
            parse_const_string_label("const-string/jumbo v0, \"theme\""),
            Some((0, "theme".into()))
        );
        match parse_instance_field_label("iput-object v1, v0, Lcom/foo/Foo;.bar:Ljava/lang/String;")
        {
            Some(InstanceFieldOp::Put { src, obj, path }) => {
                assert_eq!((src, obj, path.as_str()), (1, 0, "field:com.foo.Foo.bar"));
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse_instance_field_label("iget-object v2, v0, com.foo.Foo.bar") {
            Some(InstanceFieldOp::Get { dest, obj, path }) => {
                assert_eq!((dest, obj, path.as_str()), (2, 0, "field:com.foo.Foo.bar"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn seed_param_register_taints_sink_arg_without_bytecode_def() {
        // Param 0 = v3 (registers=5, ins=2). Writer.write {v0, v3} at offset 8.
        let mut rw_map = HashMap::new();
        rw_map.insert(8, (vec![0, 3], vec![]));
        let mut invoke_method_map = HashMap::new();
        invoke_method_map.insert(8, "java.io.Writer.write".into());
        let mut insn_at = HashMap::new();
        insn_at.insert(8, "invoke-virtual v0, v3, java.io.Writer.write".into());
        let owned = stub_owned(rw_map, Vec::new(), invoke_method_map, insn_at, 5, 2);
        let cfg = crate::taint::defaults::default_config();
        let analysis = owned.analysis();
        let mut local = LocalTaint::default();
        seed_param_register(&mut local, &analysis, &owned, &cfg, 3, "DeviceId");
        assert!(
            local.kinds_at(8, 3).contains("DeviceId"),
            "incoming param must taint Writer.write arg: {:?}",
            local.kinds_at(8, 3)
        );
        let sinks = infer_param_to_sinks(&stub_method("(Ljava/lang/String;)V", 0), &owned, &cfg);
        assert!(
            sinks
                .get(&0)
                .map(|s| s.contains("Network"))
                .unwrap_or(false),
            "param 0 must reach Network: {sinks:?}"
        );
    }

    #[test]
    fn recover_dest_url_concatenates_base_and_path() {
        let mut insn_at = HashMap::new();
        insn_at.insert(
            0,
            "sget-object v0, Lcom/foo/Api;.BASE:Ljava/lang/String;".into(),
        );
        insn_at.insert(4, "const-string v1, \"/v1/concat\"".into());
        let owned = stub_owned(HashMap::new(), Vec::new(), HashMap::new(), insn_at, 4, 0);
        let mut fields = HashMap::new();
        fields.insert(
            "field:com.foo.Api.BASE".into(),
            "https://api.demohunt.androguard.com".into(),
        );
        let url = recover_dest_url(&owned, &fields).expect("reconstructed");
        assert!(
            url.contains("api.demohunt.androguard.com") && url.contains("/v1/concat"),
            "{url}"
        );
    }

    #[test]
    fn parse_static_field_sget_sput() {
        match parse_static_field_label("sget-object v0, Lcom/foo/Api;.BASE:Ljava/lang/String;") {
            Some(StaticFieldOp::Get { dest, path }) => {
                assert_eq!((dest, path.as_str()), (0, "field:com.foo.Api.BASE"));
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse_static_field_label("sput-object v1, com.foo.Api.HOST") {
            Some(StaticFieldOp::Put { src, path }) => {
                assert_eq!((src, path.as_str()), (1, "field:com.foo.Api.HOST"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn array_paths_are_index_sensitive_with_unknown_fallback() {
        let mut rw_map = HashMap::new();
        rw_map.insert(4, (vec![1, 0, 2], vec![]));
        rw_map.insert(8, (vec![0, 3], vec![4]));
        rw_map.insert(12, (vec![0, 2], vec![5]));
        let mut insn_at = HashMap::new();
        insn_at.insert(0, "const/4 v2, 0x1".into());
        insn_at.insert(4, "aput-object v1, v0, v2".into());
        insn_at.insert(8, "aget-object v4, v0, v3".into());
        insn_at.insert(12, "aget-object v5, v0, v2".into());
        let owned = stub_owned(rw_map, Vec::new(), HashMap::new(), insn_at, 6, 0);
        let mut local = LocalTaint::default();
        local.insert(4, 1, "UserInput");
        apply_field_heap(&mut local, &owned);
        assert!(local.kinds_at(12, 5).contains("UserInput"));
        assert!(
            local.kinds_at(8, 4).contains("UserInput"),
            "unknown aget index must conservatively read known elements"
        );
    }

    #[test]
    fn static_field_store_load_is_path_sensitive() {
        let mut rw_map = HashMap::new();
        rw_map.insert(4, (vec![1], vec![]));
        rw_map.insert(8, (vec![], vec![2]));
        rw_map.insert(12, (vec![], vec![3]));
        let mut insn_at = HashMap::new();
        insn_at.insert(
            4,
            "sput-object v1, Lcom/foo/State;.token:Ljava/lang/String;".into(),
        );
        insn_at.insert(
            8,
            "sget-object v2, Lcom/foo/State;.token:Ljava/lang/String;".into(),
        );
        insn_at.insert(
            12,
            "sget-object v3, Lcom/foo/State;.other:Ljava/lang/String;".into(),
        );
        let owned = stub_owned(rw_map, Vec::new(), HashMap::new(), insn_at, 4, 0);
        let mut local = LocalTaint::default();
        local.insert(4, 1, "DeviceId");
        apply_field_heap(&mut local, &owned);
        assert!(local.kinds_at(8, 2).contains("DeviceId"));
        assert!(local.kinds_at(12, 3).is_empty());
    }

    #[test]
    fn formal_slots_skip_wide_high_words() {
        let instance = stub_method("(JLjava/lang/String;D)V", 0);
        let owned = stub_owned(
            HashMap::new(),
            Vec::new(),
            HashMap::new(),
            HashMap::new(),
            10,
            6,
        );
        assert_eq!(formal_param_slots(&instance, &owned), vec![0, 1, 3, 4]);

        let static_method = stub_method("(JLjava/lang/String;D)V", 0x8);
        let static_owned = stub_owned(
            HashMap::new(),
            Vec::new(),
            HashMap::new(),
            HashMap::new(),
            10,
            5,
        );
        assert_eq!(
            formal_param_slots(&static_method, &static_owned),
            vec![0, 2, 3]
        );
    }
}
