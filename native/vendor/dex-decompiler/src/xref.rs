//! Cross-references for DEX methods (callers / callees).
//!
//! Scans bytecode for `invoke-*` instructions that reference a given `method_ids` index.
//! Intended for reuse by CLIs, WASM UIs (droid2web), and analysis tools.

use dex_bytecode::decode_all;
use dex_parser::{DexFile, EncodedMethod};
use serde::Serialize;

use crate::error::{DexDecompilerError, Result};
use crate::java;

/// One invoke site that calls a given method_ids entry.
#[derive(Debug, Clone, Serialize)]
pub struct MethodCaller {
    /// Caller class in Java form (`com.foo.Bar`).
    pub class_name: String,
    /// Caller method name (DEX name; `<init>` kept as-is).
    pub method_name: String,
    /// Caller prototype string, e.g. `V(Landroid/os/Bundle;)`.
    pub method_descriptor: String,
    /// Index into `class_defs` (same order as light DEX parse UIs).
    pub class_idx: usize,
    /// Index into direct_methods then virtual_methods for that class.
    pub method_idx_in_class: usize,
    /// `method_ids` index of the caller method itself.
    pub caller_method_idx: u32,
    /// Instruction offset within the caller method bytecode.
    pub offset: u32,
    /// Absolute file offset of the invoke instruction in the DEX.
    pub file_offset: u32,
    /// Mnemonic (`invoke-virtual`, `invoke-static/range`, …).
    pub invoke_kind: String,
    /// Callee `method_ids` index that was matched.
    pub callee_method_idx: u32,
}

/// Result of looking up callers for one method.
#[derive(Debug, Clone, Serialize)]
pub struct MethodCallersInfo {
    pub callee_method_idx: u32,
    pub callee_class_name: String,
    pub callee_method_name: String,
    pub callee_descriptor: String,
    pub callers: Vec<MethodCaller>,
    /// True when more sites exist than were returned (capped).
    pub truncated: bool,
}

const METHOD_CALLERS_CAP: usize = 500;

/// Invoke opcodes that encode a `method_ids` index at bytes `[offset+2 .. offset+4)` (u16 LE).
///
/// - `0x6e..=0x72` — invoke-{virtual,super,direct,static,interface} (F35c)
/// - `0x74..=0x78` — same `/range` forms (F3rc)
/// - `0xfa`, `0xfb` — invoke-polymorphic{,/range} (method + proto)
///
fn invoke_method_index(insns: &[u8], offset: u32, opcode: u8) -> Option<u32> {
    let o = offset as usize;
    let is_method_invoke = matches!(
        opcode,
        0x6e..=0x72 | 0x74..=0x78 | 0xfa | 0xfb
    );
    if !is_method_invoke || o + 4 > insns.len() {
        return None;
    }
    Some(u16::from_le_bytes([insns[o + 2], insns[o + 3]]) as u32)
}

/// Resolve normal invokes and invoke-custom lambda implementation handles.
fn resolved_invoke_method_index(
    dex: &DexFile,
    insns: &[u8],
    offset: u32,
    opcode: u8,
) -> Option<u32> {
    if let Some(method_idx) = invoke_method_index(insns, offset, opcode) {
        return Some(method_idx);
    }
    if !matches!(opcode, 0xfc | 0xfd) {
        return None;
    }
    let o = offset as usize;
    if o + 4 > insns.len() {
        return None;
    }
    let callsite_idx = u16::from_le_bytes([insns[o + 2], insns[o + 3]]) as u32;
    let callsite = dex.get_call_site(callsite_idx).ok()?;
    dex_parser::DexCallSites::impl_method_id(&callsite).map(u32::from)
}

fn invoke_kind_name(opcode: u8) -> &'static str {
    match opcode {
        0x6e => "invoke-virtual",
        0x6f => "invoke-super",
        0x70 => "invoke-direct",
        0x71 => "invoke-static",
        0x72 => "invoke-interface",
        0x74 => "invoke-virtual/range",
        0x75 => "invoke-super/range",
        0x76 => "invoke-direct/range",
        0x77 => "invoke-static/range",
        0x78 => "invoke-interface/range",
        0xfa => "invoke-polymorphic",
        0xfb => "invoke-polymorphic/range",
        0xfc => "invoke-custom",
        0xfd => "invoke-custom/range",
        _ => "invoke",
    }
}

fn format_method_descriptor(return_type: &str, params: &[String]) -> String {
    let mut s = String::from("(");
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&java::descriptor_to_java(p));
    }
    s.push(')');
    s.push_str(&java::descriptor_to_java(return_type));
    s
}

fn method_descriptor_from_dex(dex: &DexFile, method_idx: u32) -> Result<(String, String, String)> {
    let info = dex
        .get_method_info(method_idx)
        .map_err(|e| DexDecompilerError::Parse(e.to_string()))?;
    let class_name = java::descriptor_to_java(&info.class);
    let descriptor = format_method_descriptor(&info.return_type, &info.params);
    Ok((class_name, info.name.to_string(), descriptor))
}

/// Find all invoke sites that reference `callee_method_idx` in `method_ids`.
pub fn find_method_callers(dex: &DexFile, callee_method_idx: u32) -> Result<MethodCallersInfo> {
    if callee_method_idx >= dex.header.method_ids_size {
        return Err(DexDecompilerError::Parse(format!(
            "method index {} out of range (size {})",
            callee_method_idx, dex.header.method_ids_size
        )));
    }

    let (callee_class_name, callee_method_name, callee_descriptor) =
        method_descriptor_from_dex(dex, callee_method_idx)?;

    let mut callers = Vec::new();
    let mut truncated = false;

    'classes: for (class_idx, class_result) in dex.class_defs().enumerate() {
        let Ok(class_def) = class_result else {
            continue;
        };
        let Ok(class_type) = dex.get_type(class_def.class_idx) else {
            continue;
        };
        let class_name = java::descriptor_to_java(&class_type);
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            continue;
        };

        let all_methods: Vec<&EncodedMethod> = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
            .collect();

        for (method_idx_in_class, encoded) in all_methods.into_iter().enumerate() {
            if encoded.code_off == 0 {
                continue;
            }
            let Ok(method_info) = dex.get_method_info(encoded.method_idx) else {
                continue;
            };
            let method_name = method_info.name.to_string();
            let method_descriptor =
                format_method_descriptor(&method_info.return_type, &method_info.params);
            let Ok(code) = dex.get_code_item(encoded.code_off) else {
                continue;
            };
            let insns = code.insns_slice(&dex.data);
            let Ok(decoded) = decode_all(insns, 0) else {
                continue;
            };
            let insns_base = code.insns_off as u32;
            for ins in decoded {
                let Some(midx) = resolved_invoke_method_index(dex, insns, ins.offset, ins.opcode)
                else {
                    continue;
                };
                if midx != callee_method_idx {
                    continue;
                }
                if callers.len() >= METHOD_CALLERS_CAP {
                    truncated = true;
                    break 'classes;
                }
                callers.push(MethodCaller {
                    class_name: class_name.clone(),
                    method_name: method_name.clone(),
                    method_descriptor: method_descriptor.clone(),
                    class_idx,
                    method_idx_in_class,
                    caller_method_idx: encoded.method_idx,
                    offset: ins.offset,
                    file_offset: insns_base.saturating_add(ins.offset),
                    invoke_kind: invoke_kind_name(ins.opcode).to_string(),
                    callee_method_idx,
                });
            }
        }
    }

    Ok(MethodCallersInfo {
        callee_method_idx,
        callee_class_name,
        callee_method_name,
        callee_descriptor,
        callers,
        truncated,
    })
}

/// Resolve a UI class/method pair (light-parse indices) to a `method_ids` index, then find callers.
pub fn find_method_callers_by_class_method(
    dex: &DexFile,
    class_idx: usize,
    method_idx_in_class: usize,
) -> Result<MethodCallersInfo> {
    let mut n = 0usize;
    for class_result in dex.class_defs() {
        let Ok(class_def) = class_result else {
            n += 1;
            continue;
        };
        if n != class_idx {
            n += 1;
            continue;
        }
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            return Err(DexDecompilerError::Parse(format!(
                "class_idx {} has no class_data",
                class_idx
            )));
        };
        let all: Vec<&EncodedMethod> = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
            .collect();
        let encoded = all.get(method_idx_in_class).ok_or_else(|| {
            DexDecompilerError::Parse(format!(
                "method_idx {} out of range (class has {} methods)",
                method_idx_in_class,
                all.len()
            ))
        })?;
        return find_method_callers(dex, encoded.method_idx);
    }
    Err(DexDecompilerError::Parse(format!(
        "class_idx {} out of range",
        class_idx
    )))
}

/// One callee referenced by invokes inside a method.
#[derive(Debug, Clone, Serialize)]
pub struct MethodCallee {
    pub method_idx: u32,
    pub class_name: String,
    pub method_name: String,
    pub method_descriptor: String,
    pub offset: u32,
    pub invoke_kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MethodCalleesInfo {
    pub caller_method_idx: u32,
    pub callees: Vec<MethodCallee>,
    pub truncated: bool,
}

const METHOD_CALLEES_CAP: usize = 400;

/// List methods invoked from the given `method_ids` entry (unique by method_idx, first site kept).
pub fn find_method_callees(dex: &DexFile, caller_method_idx: u32) -> Result<MethodCalleesInfo> {
    let mut callees = Vec::new();
    let mut truncated = false;
    let mut seen = std::collections::HashSet::new();

    'classes: for class_result in dex.class_defs() {
        let Ok(class_def) = class_result else {
            continue;
        };
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            continue;
        };
        for encoded in class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
        {
            if encoded.method_idx != caller_method_idx || encoded.code_off == 0 {
                continue;
            }
            let Ok(code) = dex.get_code_item(encoded.code_off) else {
                continue;
            };
            let insns = code.insns_slice(&dex.data);
            let Ok(decoded) = decode_all(insns, 0) else {
                continue;
            };
            for ins in decoded {
                let Some(midx) = resolved_invoke_method_index(dex, insns, ins.offset, ins.opcode)
                else {
                    continue;
                };
                if !seen.insert(midx) {
                    continue;
                }
                if callees.len() >= METHOD_CALLEES_CAP {
                    truncated = true;
                    break 'classes;
                }
                let (class_name, method_name, method_descriptor) =
                    method_descriptor_from_dex(dex, midx)
                        .unwrap_or_else(|_| ("?".into(), "?".into(), "()".into()));
                callees.push(MethodCallee {
                    method_idx: midx,
                    class_name,
                    method_name,
                    method_descriptor,
                    offset: ins.offset,
                    invoke_kind: invoke_kind_name(ins.opcode).to_string(),
                });
            }
        }
    }

    Ok(MethodCalleesInfo {
        caller_method_idx,
        callees,
        truncated,
    })
}

pub fn find_method_callees_by_class_method(
    dex: &DexFile,
    class_idx: usize,
    method_idx_in_class: usize,
) -> Result<MethodCalleesInfo> {
    let callers = find_method_callers_by_class_method(dex, class_idx, method_idx_in_class)?;
    // Reuse index resolution via callers API's callee idx… actually we need the method itself.
    let mut n = 0usize;
    for class_result in dex.class_defs() {
        let Ok(class_def) = class_result else {
            n += 1;
            continue;
        };
        if n != class_idx {
            n += 1;
            continue;
        }
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            return Err(DexDecompilerError::Parse("no class_data".into()));
        };
        let all: Vec<&EncodedMethod> = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
            .collect();
        let encoded = all
            .get(method_idx_in_class)
            .ok_or_else(|| DexDecompilerError::Parse("method_idx out of range".into()))?;
        let _ = callers;
        return find_method_callees(dex, encoded.method_idx);
    }
    Err(DexDecompilerError::Parse("class_idx out of range".into()))
}

/// One frame on a reverse call path (caller → … → target).
#[derive(Debug, Clone, Serialize)]
pub struct CallTraceFrame {
    pub class_name: String,
    pub method_name: String,
    pub method_descriptor: String,
    pub class_idx: usize,
    pub method_idx_in_class: usize,
    pub method_idx: u32,
    /// Invoke offset inside this method that reaches the next frame (None on the target).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoke_kind: Option<String>,
}

/// One full reverse call path: entry/root first, target last.
#[derive(Debug, Clone, Serialize)]
pub struct CallTracePath {
    pub frames: Vec<CallTraceFrame>,
}

/// Reverse call traces for a method (who calls it, and who calls those callers, …).
#[derive(Debug, Clone, Serialize)]
pub struct MethodCallTracesInfo {
    pub target_method_idx: u32,
    pub target_class_name: String,
    pub target_method_name: String,
    pub target_descriptor: String,
    pub paths: Vec<CallTracePath>,
    /// Depth / fan-in / path caps were hit.
    pub truncated: bool,
    pub max_depth: usize,
}

const CALL_TRACE_MAX_DEPTH: usize = 6;
const CALL_TRACE_MAX_PATHS: usize = 48;
const CALL_TRACE_MAX_FANIN: usize = 16;

#[derive(Clone, Copy, Debug)]
struct CompactInEdge {
    caller_method_idx: u32,
    class_idx: u32,
    method_idx_in_class: u32,
    offset: u32,
    opcode: u8,
}

fn resolve_encoded_method_idx(
    dex: &DexFile,
    class_idx: usize,
    method_idx_in_class: usize,
) -> Result<u32> {
    let mut n = 0usize;
    for class_result in dex.class_defs() {
        let Ok(class_def) = class_result else {
            n += 1;
            continue;
        };
        if n != class_idx {
            n += 1;
            continue;
        }
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            return Err(DexDecompilerError::Parse(format!(
                "class_idx {} has no class_data",
                class_idx
            )));
        };
        let all: Vec<&EncodedMethod> = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
            .collect();
        let encoded = all.get(method_idx_in_class).ok_or_else(|| {
            DexDecompilerError::Parse(format!(
                "method_idx {} out of range (class has {} methods)",
                method_idx_in_class,
                all.len()
            ))
        })?;
        return Ok(encoded.method_idx);
    }
    Err(DexDecompilerError::Parse(format!(
        "class_idx {} out of range",
        class_idx
    )))
}

/// One-pass reverse call index: callee method_ids → incoming invoke edges (capped per callee).
///
/// Build once per DEX and reuse with [`find_method_call_traces_with_index`] — rebuilding this
/// for every sink is the main cost of entry-point reachability.
#[derive(Debug, Clone, Default)]
pub struct ReverseCallIndex {
    rev: std::collections::HashMap<u32, Vec<CompactInEdge>>,
}

impl ReverseCallIndex {
    pub fn build(dex: &DexFile) -> Self {
        Self::build_with_progress(dex, |_, _| {})
    }

    /// Like [`build`], but invokes `on_progress(classes_done, classes_total)` periodically.
    pub fn build_with_progress(dex: &DexFile, on_progress: impl FnMut(usize, usize)) -> Self {
        Self {
            rev: build_reverse_call_index(dex, on_progress),
        }
    }

    pub fn incoming_count(&self) -> usize {
        self.rev.len()
    }
}

/// One-pass reverse call index: callee method_ids → incoming invoke edges (capped per callee).
fn build_reverse_call_index(
    dex: &DexFile,
    mut on_progress: impl FnMut(usize, usize),
) -> std::collections::HashMap<u32, Vec<CompactInEdge>> {
    use std::collections::HashMap;
    let mut rev: HashMap<u32, Vec<CompactInEdge>> = HashMap::new();
    let total = dex.header.class_defs_size as usize;
    let mut last_report = 0usize;

    for (class_idx, class_result) in dex.class_defs().enumerate() {
        let done = class_idx + 1;
        // ~2% steps or every 250 classes, whichever is more frequent for small DEXes.
        let step = (total / 50).max(250).min(2000).max(1);
        if done == total || done - last_report >= step {
            on_progress(done, total);
            last_report = done;
        }
        let Ok(class_def) = class_result else {
            continue;
        };
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            continue;
        };
        let all: Vec<&EncodedMethod> = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
            .collect();
        for (method_idx_in_class, encoded) in all.into_iter().enumerate() {
            if encoded.code_off == 0 {
                continue;
            }
            let Ok(code) = dex.get_code_item(encoded.code_off) else {
                continue;
            };
            let insns = code.insns_slice(&dex.data);
            let Ok(decoded) = decode_all(insns, 0) else {
                continue;
            };
            for ins in decoded {
                let Some(callee) = resolved_invoke_method_index(dex, insns, ins.offset, ins.opcode)
                else {
                    continue;
                };
                let entry = rev.entry(callee).or_default();
                if entry.len() >= CALL_TRACE_MAX_FANIN {
                    continue;
                }
                // Prefer unique callers (first invoke site wins).
                if entry
                    .iter()
                    .any(|e| e.caller_method_idx == encoded.method_idx)
                {
                    continue;
                }
                entry.push(CompactInEdge {
                    caller_method_idx: encoded.method_idx,
                    class_idx: class_idx as u32,
                    method_idx_in_class: method_idx_in_class as u32,
                    offset: ins.offset,
                    opcode: ins.opcode,
                });
            }
        }
    }
    rev
}

fn frame_from_method(
    dex: &DexFile,
    method_idx: u32,
    class_idx: usize,
    method_idx_in_class: usize,
    offset: Option<u32>,
    invoke_kind: Option<String>,
) -> CallTraceFrame {
    let (class_name, method_name, method_descriptor) = method_descriptor_from_dex(dex, method_idx)
        .unwrap_or_else(|_| ("?".into(), "?".into(), "()".into()));
    CallTraceFrame {
        class_name,
        method_name,
        method_descriptor,
        class_idx,
        method_idx_in_class,
        method_idx,
        offset,
        invoke_kind,
    }
}

/// Build reverse call traces ending at `callee_method_idx` (paths are root → … → target).
pub fn find_method_call_traces(
    dex: &DexFile,
    callee_method_idx: u32,
) -> Result<MethodCallTracesInfo> {
    let index = ReverseCallIndex::build(dex);
    find_method_call_traces_with_index(dex, callee_method_idx, &index)
}

/// Like [`find_method_call_traces`], but reuses a precomputed [`ReverseCallIndex`].
pub fn find_method_call_traces_with_index(
    dex: &DexFile,
    callee_method_idx: u32,
    index: &ReverseCallIndex,
) -> Result<MethodCallTracesInfo> {
    if callee_method_idx >= dex.header.method_ids_size {
        return Err(DexDecompilerError::Parse(format!(
            "method index {} out of range (size {})",
            callee_method_idx, dex.header.method_ids_size
        )));
    }
    let (target_class_name, target_method_name, target_descriptor) =
        method_descriptor_from_dex(dex, callee_method_idx)?;

    let rev = &index.rev;
    let mut paths = Vec::new();
    let mut truncated = false;

    // Locate UI indices for the target (best-effort).
    let (target_class_idx, target_method_idx_in_class) = {
        let mut found = (0usize, 0usize);
        'outer: for (ci, class_result) in dex.class_defs().enumerate() {
            let Ok(class_def) = class_result else {
                continue;
            };
            let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
                continue;
            };
            for (mi, encoded) in class_data
                .direct_methods
                .iter()
                .chain(class_data.virtual_methods.iter())
                .enumerate()
            {
                if encoded.method_idx == callee_method_idx {
                    found = (ci, mi);
                    break 'outer;
                }
            }
        }
        found
    };

    let target_frame = frame_from_method(
        dex,
        callee_method_idx,
        target_class_idx,
        target_method_idx_in_class,
        None,
        None,
    );

    fn walk(
        dex: &DexFile,
        rev: &std::collections::HashMap<u32, Vec<CompactInEdge>>,
        current: u32,
        path_to_target: &[CallTraceFrame], // current … target (current first)
        depth: usize,
        paths: &mut Vec<CallTracePath>,
        truncated: &mut bool,
        on_path: &mut std::collections::HashSet<u32>,
    ) {
        if paths.len() >= CALL_TRACE_MAX_PATHS {
            *truncated = true;
            return;
        }
        let incoming = rev.get(&current).map(|v| v.as_slice()).unwrap_or(&[]);
        let mut preferred: Vec<&CompactInEdge> = incoming
            .iter()
            .filter(|e| {
                method_descriptor_from_dex(dex, e.caller_method_idx)
                    .map(|(c, _, _)| !crate::detectors::is_library_class(&c))
                    .unwrap_or(true)
            })
            .collect();
        if preferred.is_empty() {
            preferred = incoming.iter().collect();
        }

        if preferred.is_empty() || depth >= CALL_TRACE_MAX_DEPTH {
            let mut frames = path_to_target.to_vec();
            frames.reverse();
            paths.push(CallTracePath { frames });
            return;
        }

        let mut expanded = false;
        for edge in preferred.into_iter().take(CALL_TRACE_MAX_FANIN) {
            if paths.len() >= CALL_TRACE_MAX_PATHS {
                *truncated = true;
                break;
            }
            if !on_path.insert(edge.caller_method_idx) {
                continue;
            }
            expanded = true;
            let caller_frame = frame_from_method(
                dex,
                edge.caller_method_idx,
                edge.class_idx as usize,
                edge.method_idx_in_class as usize,
                Some(edge.offset),
                Some(invoke_kind_name(edge.opcode).to_string()),
            );
            let mut next_path = Vec::with_capacity(path_to_target.len() + 1);
            next_path.push(caller_frame);
            next_path.extend_from_slice(path_to_target);
            walk(
                dex,
                rev,
                edge.caller_method_idx,
                &next_path,
                depth + 1,
                paths,
                truncated,
                on_path,
            );
            on_path.remove(&edge.caller_method_idx);
        }
        if !expanded {
            let mut frames = path_to_target.to_vec();
            frames.reverse();
            paths.push(CallTracePath { frames });
        }
    }

    let mut on_path = std::collections::HashSet::new();
    on_path.insert(callee_method_idx);
    walk(
        dex,
        rev,
        callee_method_idx,
        &[target_frame],
        0,
        &mut paths,
        &mut truncated,
        &mut on_path,
    );

    // Prefer longer paths first (deeper reverse chains).
    paths.sort_by(|a, b| b.frames.len().cmp(&a.frames.len()));

    Ok(MethodCallTracesInfo {
        target_method_idx: callee_method_idx,
        target_class_name,
        target_method_name,
        target_descriptor,
        paths,
        truncated,
        max_depth: CALL_TRACE_MAX_DEPTH,
    })
}

/// Resolve UI class/method indices, then build reverse call traces.
pub fn find_method_call_traces_by_class_method(
    dex: &DexFile,
    class_idx: usize,
    method_idx_in_class: usize,
) -> Result<MethodCallTracesInfo> {
    let method_idx = resolve_encoded_method_idx(dex, class_idx, method_idx_in_class)?;
    find_method_call_traces(dex, method_idx)
}

/// One field get/put site.
#[derive(Debug, Clone, Serialize)]
pub struct FieldXref {
    pub class_name: String,
    pub method_name: String,
    pub class_idx: usize,
    pub method_idx_in_class: usize,
    pub offset: u32,
    pub access_kind: String,
    pub field_idx: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldXrefsInfo {
    pub field_idx: u32,
    pub field_class: String,
    pub field_name: String,
    pub field_type: String,
    pub xrefs: Vec<FieldXref>,
    pub truncated: bool,
}

const FIELD_XREFS_CAP: usize = 500;

fn field_index(insns: &[u8], offset: u32, opcode: u8) -> Option<u32> {
    // iget/iput/sget/sput family: field idx at +2 (u16), except some range forms.
    let o = offset as usize;
    let is_field = matches!(
        opcode,
        0x52..=0x6d // iget* / iput* / sget* / sput*
    );
    if !is_field || o + 4 > insns.len() {
        return None;
    }
    Some(u16::from_le_bytes([insns[o + 2], insns[o + 3]]) as u32)
}

fn field_access_kind(opcode: u8) -> &'static str {
    match opcode {
        0x52..=0x58 => "iget",
        0x59..=0x5f => "iput",
        0x60..=0x66 => "sget",
        0x67..=0x6d => "sput",
        _ => "field",
    }
}

/// Find all iget/iput/sget/sput sites for a field_ids index.
pub fn find_field_xrefs(dex: &DexFile, field_idx: u32) -> Result<FieldXrefsInfo> {
    if field_idx >= dex.header.field_ids_size {
        return Err(DexDecompilerError::Parse(format!(
            "field index {} out of range",
            field_idx
        )));
    }
    let fi = dex
        .get_field_info(field_idx)
        .map_err(|e| DexDecompilerError::Parse(e.to_string()))?;
    let field_class = java::descriptor_to_java(&fi.class);
    let field_name = fi.name.to_string();
    let field_type = java::descriptor_to_java(&fi.typ);

    let mut xrefs = Vec::new();
    let mut truncated = false;
    'classes: for (class_idx, class_result) in dex.class_defs().enumerate() {
        let Ok(class_def) = class_result else {
            continue;
        };
        let Ok(class_type) = dex.get_type(class_def.class_idx) else {
            continue;
        };
        let class_name = java::descriptor_to_java(&class_type);
        let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
            continue;
        };
        let all: Vec<&EncodedMethod> = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
            .collect();
        for (method_idx_in_class, encoded) in all.into_iter().enumerate() {
            if encoded.code_off == 0 {
                continue;
            }
            let Ok(method_info) = dex.get_method_info(encoded.method_idx) else {
                continue;
            };
            let Ok(code) = dex.get_code_item(encoded.code_off) else {
                continue;
            };
            let insns = code.insns_slice(&dex.data);
            let Ok(decoded) = decode_all(insns, 0) else {
                continue;
            };
            for ins in decoded {
                let Some(fidx) = field_index(insns, ins.offset, ins.opcode) else {
                    continue;
                };
                if fidx != field_idx {
                    continue;
                }
                if xrefs.len() >= FIELD_XREFS_CAP {
                    truncated = true;
                    break 'classes;
                }
                xrefs.push(FieldXref {
                    class_name: class_name.clone(),
                    method_name: method_info.name.to_string(),
                    class_idx,
                    method_idx_in_class,
                    offset: ins.offset,
                    access_kind: field_access_kind(ins.opcode).to_string(),
                    field_idx,
                });
            }
        }
    }

    Ok(FieldXrefsInfo {
        field_idx,
        field_class,
        field_name,
        field_type,
        xrefs,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::invoke_method_index;

    #[test]
    fn invoke_virtual_reads_method_index() {
        // invoke-virtual {v0}, method@0x1234  — F35c
        let bytes = [0x6e, 0x10, 0x34, 0x12, 0x00, 0x00];
        assert_eq!(invoke_method_index(&bytes, 0, 0x6e), Some(0x1234));
    }

    #[test]
    fn invoke_static_range_reads_method_index() {
        let bytes = [0x77, 0x02, 0xcd, 0xab, 0x00, 0x00];
        assert_eq!(invoke_method_index(&bytes, 0, 0x77), Some(0xabcd));
    }

    #[test]
    fn invoke_custom_skipped() {
        let bytes = [0xfc, 0x10, 0x34, 0x12, 0x00, 0x00];
        assert_eq!(invoke_method_index(&bytes, 0, 0xfc), None);
    }
}
