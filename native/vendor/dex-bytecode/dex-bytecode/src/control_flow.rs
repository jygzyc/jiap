//! Control-flow helpers: branch targets, labels, basic blocks.
//!
//! Branch targets and labels can be computed from decoded instructions and raw
//! bytecode so that another tool can display `goto :L0010` instead of raw offsets.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::convert::TryInto;

use crate::instruction::Instruction;
use crate::opcodes::{format_length, get_opcode_entry, get_payload_kind, Format};

#[inline]
fn read_u16_dec(data: &[u8], offset: usize) -> Option<u16> {
    if offset + 2 > data.len() {
        return None;
    }
    let bytes: [u8; 2] = data[offset..offset + 2].try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

#[inline]
fn read_i16_dec(data: &[u8], offset: usize) -> Option<i16> {
    read_u16_dec(data, offset).map(|u| u as i16)
}

#[inline]
fn read_i32_dec(data: &[u8], offset: usize) -> Option<i32> {
    if offset + 4 > data.len() {
        return None;
    }
    let bytes: [u8; 4] = data[offset..offset + 4].try_into().ok()?;
    Some(i32::from_le_bytes(bytes))
}

/// Returns the branch target byte offsets for the instruction at `offset` in `data`.
/// Offsets are in 16-bit code units in Dalvik; we return byte offsets.
///
/// - Simple branches (goto, if-*): one target.
/// - packed-switch / sparse-switch: target is the payload offset (first 16-bit unit of payload).
/// - Non-branch instructions: empty.
pub fn branch_targets(data: &[u8], offset: usize) -> Vec<u32> {
    let unit = match read_u16_dec(data, offset) {
        Some(u) => u,
        None => return Vec::new(),
    };
    let op_byte = unit as u8;

    // Payload: target is the payload start (for switch, payload follows)
    if unit > 0xFF && (op_byte == 0x00 || op_byte == 0xFF) {
        if get_payload_kind(unit).is_some() {
            return alloc::vec![offset as u32];
        }
    }

    let entry = get_opcode_entry(op_byte);
    let len = format_length(entry.format) as usize;
    if offset + len > data.len() {
        return Vec::new();
    }

    // Dalvik: destination = instruction_address + signed_offset (in 16-bit code units).
    // See https://source.android.com/docs/core/runtime/dalvik-bytecode
    let offset_i = offset as i32;
    match entry.format {
        Format::F10t => {
            let aa = data.get(offset + 1).copied().unwrap_or(0) as i8;
            let target = offset_i + (aa as i32) * 2;
            if target >= 0 {
                return alloc::vec![target as u32];
            }
        }
        Format::F20t => {
            let aaaa = read_i16_dec(data, offset + 2).unwrap_or(0) as i32;
            let target = offset_i + aaaa * 2;
            if target >= 0 {
                return alloc::vec![target as u32];
            }
        }
        Format::F21t => {
            let bbbb = read_i16_dec(data, offset + 2).unwrap_or(0) as i32;
            let target = offset_i + bbbb * 2;
            if target >= 0 {
                return alloc::vec![target as u32];
            }
        }
        Format::F22t => {
            let cccc = read_i16_dec(data, offset + 2).unwrap_or(0) as i32;
            let target = offset_i + cccc * 2;
            if target >= 0 {
                return alloc::vec![target as u32];
            }
        }
        Format::F30t => {
            let aaaaaaaa = read_i32_dec(data, offset + 2).unwrap_or(0);
            let target = offset_i + aaaaaaaa * 2;
            if target >= 0 {
                return alloc::vec![target as u32];
            }
        }
        Format::F31t => {
            // packed-switch / sparse-switch: branch offset in 16-bit units points to payload
            let bbbbbbbb = read_i32_dec(data, offset + 2).unwrap_or(0);
            let target = offset_i + bbbbbbbb * 2;
            if target >= 0 && (target as usize) < data.len() {
                return alloc::vec![target as u32];
            }
        }
        _ => {}
    }
    Vec::new()
}

/// Returns explicit CFG successor offsets for the instruction at `offset` in `data`.
///
/// - For simple branches (goto, if-*): returns the branch target.
/// - For packed-/sparse-switch: returns all case target offsets (expanded from the payload).
/// - For fill-array-data: returns empty (payload is data, not a control-flow edge).
/// - For other instructions: empty.
///
/// Notes:
/// - Returned offsets are byte offsets within `data`.
/// - This intentionally excludes fallthrough; use it to build block boundaries / jump edges.
pub fn explicit_successors(data: &[u8], offset: usize) -> Vec<u32> {
    let unit = match read_u16_dec(data, offset) {
        Some(u) => u,
        None => return Vec::new(),
    };
    let op = unit as u8;

    // Payload pseudo-instructions (packed/sparse switch payloads, fill-array-data payload)
    // are not executed as code; they have no CFG successors.
    if unit > 0xFF && (op == 0x00 || op == 0xFF) && get_payload_kind(unit).is_some() {
        return Vec::new();
    }

    // fill-array-data uses 31t to reference a payload, but it's not a control-flow edge.
    if op == 0x26 {
        return Vec::new();
    }

    // Switch: expand payload case targets.
    if op == 0x2b || op == 0x2c {
        // Payload location is encoded like 31t (signed 32-bit in 16-bit units).
        let rel_units = read_i32_dec(data, offset + 2).unwrap_or(0);
        // Payload address is relative to the switch instruction (same as other branches).
        let payload_i = (offset as i32) + rel_units * 2;
        if payload_i < 0 {
            return Vec::new();
        }
        let payload = payload_i as usize;
        return switch_case_targets(data, offset, payload);
    }

    // Default: use existing branch target logic (covers goto/if-*).
    branch_targets(data, offset)
}

fn switch_case_targets(data: &[u8], switch_offset: usize, payload_offset: usize) -> Vec<u32> {
    // Payload ident is a u16: 0x0100 packed, 0x0200 sparse.
    let ident = match read_u16_dec(data, payload_offset) {
        Some(v) => v,
        None => return Vec::new(),
    };
    let size = match read_u16_dec(data, payload_offset + 2) {
        Some(v) => v as usize,
        None => return Vec::new(),
    };

    // Targets are relative to the switch instruction address in 16-bit code units.
    let mut out = Vec::new();
    let base = switch_offset as i32;

    match ident {
        0x0100 => {
            // packed-switch-payload:
            // ident (2), size (2), first_key (4), then size x targets (4 bytes each)
            let targets_base = payload_offset + 8;
            for i in 0..size {
                let rel = match read_i32_dec(data, targets_base + i * 4) {
                    Some(v) => v,
                    None => break,
                };
                let t = base + rel * 2;
                if t >= 0 {
                    out.push(t as u32);
                }
            }
        }
        0x0200 => {
            // sparse-switch-payload:
            // ident (2), size (2), size keys (4 each), then size targets (4 each)
            let targets_base = payload_offset + 4 + size * 4;
            for i in 0..size {
                let rel = match read_i32_dec(data, targets_base + i * 4) {
                    Some(v) => v,
                    None => break,
                };
                let t = base + rel * 2;
                if t >= 0 {
                    out.push(t as u32);
                }
            }
        }
        _ => {}
    }

    // Dedup + sort (stable order) for nicer consumers.
    let mut set = BTreeSet::new();
    for t in out {
        set.insert(t);
    }
    set.into_iter().collect()
}

/// Collects all branch target offsets from a decoded instruction list and raw bytecode.
/// Use with a base offset if the code buffer starts at a non-zero offset.
pub fn collect_branch_targets(
    instructions: &[Instruction],
    data: &[u8],
    base_offset: usize,
) -> BTreeSet<u32> {
    let mut targets = BTreeSet::new();
    for ins in instructions {
        let off = (ins.offset as usize) + base_offset;
        for t in branch_targets(data, off) {
            targets.insert(t);
        }
    }
    targets
}

/// Decodes bytecode starting at `start` and returns all branch target offsets (relative to
/// the decoded region). Useful for displaying labels in a disassembler.
pub fn branch_target_offsets(
    data: &[u8],
    start: usize,
) -> Result<BTreeSet<u32>, crate::error::DexError> {
    if start >= data.len() {
        return Ok(BTreeSet::new());
    }
    let slice = &data[start..];
    let instructions = crate::decoder::decode_all(slice, 0)?;
    Ok(collect_branch_targets(&instructions, slice, 0))
}

/// Returns true if the instruction at `offset` is an unconditional branch (goto, goto/16, goto/32).
/// Such instructions have no fallthrough successor.
#[inline]
pub fn is_unconditional_branch(data: &[u8], offset: usize) -> bool {
    let unit = match read_u16_dec(data, offset) {
        Some(u) => u,
        None => return false,
    };
    let op = unit as u8;
    // goto (0x28), goto/16 (0x29), goto/32 (0x2a)
    op == 0x28 || op == 0x29 || op == 0x2a
}

/// A basic block: contiguous instructions from `start_offset` to `end_offset` (exclusive),
/// with optional successor offsets (branch targets) and optional fallthrough.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// Start byte offset of the block.
    pub start_offset: u32,
    /// End byte offset (exclusive).
    pub end_offset: u32,
    /// Successor byte offsets (explicit branch/jump targets), deduplicated and sorted.
    pub successors: Vec<u32>,
    /// If present, the block falls through to this byte offset (start of the next block).
    /// Absent when the block ends with an unconditional branch (e.g. goto) or has no next block.
    pub fallthrough_to: Option<u32>,
}

/// Try/catch range for one handler. Fill from DEX `encoded_catch_handler` in another tool.
#[derive(Debug, Clone)]
pub struct TryCatchEntry {
    /// Start byte offset of the protected range (inclusive).
    pub start_offset: u32,
    /// End byte offset of the protected range (exclusive).
    pub end_offset: u32,
    /// Byte offset of the handler (catch block).
    pub handler_offset: u32,
    /// Exception type index (e.g. in type_ids). `None` = catch-all.
    pub type_index: Option<u32>,
}

/// Formats a single `.catch` line for disassembly. `type_name` can be resolved by another tool
/// from `entry.type_index` (e.g. `Ljava/lang/Exception;`); use `None` for catch-all.
#[inline]
pub fn format_catch_line(entry: &TryCatchEntry, type_name: Option<&str>) -> String {
    let ty = type_name.unwrap_or("all");
    alloc::format!(
        ".catch {} {{ 0x{:08x} .. 0x{:08x} }} :L{:08x}",
        ty,
        entry.start_offset,
        entry.end_offset,
        entry.handler_offset
    )
}

/// Returns exception edges (from_block_start, handler_offset) for the CFG.
/// For each basic block whose range [start_offset, end_offset) overlaps any try range
/// [start_offset, end_offset), adds an edge from that block's start to the handler.
/// Use with `cfg_edges` and optionally DOT export to include exception flow.
pub fn exception_edges(entries: &[TryCatchEntry], blocks: &[BasicBlock]) -> Vec<(u32, u32)> {
    let mut edges = Vec::new();
    for b in blocks {
        for e in entries {
            let try_start = e.start_offset as u64;
            let try_end = e.end_offset as u64;
            let block_start = b.start_offset as u64;
            let block_end = if b.end_offset == u32::MAX {
                u64::MAX
            } else {
                b.end_offset as u64
            };
            if block_start < try_end && block_end > try_start {
                edges.push((b.start_offset, e.handler_offset));
            }
        }
    }
    edges
}

/// Builds basic blocks from decoded instructions and raw bytecode.
/// Block boundaries: code start, branch targets, and the instruction after any branch.
/// Each block's `successors` list is deduplicated and sorted for stable iteration.
pub fn basic_blocks(
    instructions: &[Instruction],
    data: &[u8],
    base_offset: usize,
) -> Vec<BasicBlock> {
    // CFG-aware targets (switch expanded; fill-array-data ignored).
    let mut targets = BTreeSet::new();
    for ins in instructions {
        let off = (ins.offset as usize) + base_offset;
        for t in explicit_successors(data, off) {
            targets.insert(t);
        }
    }
    let mut block_starts: BTreeSet<u32> = BTreeSet::new();
    block_starts.insert(base_offset as u32);
    for &t in &targets {
        block_starts.insert(t);
    }
    for ins in instructions {
        let start = (ins.offset as usize) + base_offset;
        let end = start + ins.length as usize;
        let succ = explicit_successors(data, start);
        for t in &succ {
            block_starts.insert(*t);
        }
        // Conditional / branch instructions start a block; fall-through after them starts another.
        if !succ.is_empty() {
            block_starts.insert(start as u32);
            block_starts.insert(end as u32);
        }
    }

    let starts: Vec<u32> = block_starts.into_iter().collect();
    let mut blocks = Vec::new();
    for i in 0..starts.len() {
        let start_offset = starts[i];
        let end_offset = starts.get(i + 1).copied().unwrap_or(u32::MAX);
        let block_instructions: Vec<_> = instructions
            .iter()
            .filter(|ins| {
                let off = (ins.offset as usize) + base_offset;
                let start = start_offset as usize;
                let end = if end_offset == u32::MAX {
                    usize::MAX
                } else {
                    end_offset as usize
                };
                off >= start && off < end
            })
            .collect();
        let successor_set: BTreeSet<u32> = block_instructions
            .iter()
            .flat_map(|ins| explicit_successors(data, (ins.offset as usize) + base_offset))
            .collect();
        let successors: Vec<u32> = successor_set.into_iter().collect();
        let last_ins_offset = block_instructions
            .last()
            .map(|ins| (ins.offset as usize) + base_offset);
        let ends_with_unconditional = last_ins_offset
            .map(|off| is_unconditional_branch(data, off))
            .unwrap_or(false);
        let fallthrough_to = if end_offset != u32::MAX && !ends_with_unconditional {
            Some(end_offset)
        } else {
            None
        };
        blocks.push(BasicBlock {
            start_offset,
            end_offset,
            successors,
            fallthrough_to,
        });
    }
    blocks
}

/// Returns all CFG edges (from_offset, to_offset) including fallthrough.
/// Use this for full control-flow graph visualization (e.g. DOT export).
pub fn cfg_edges(instructions: &[Instruction], data: &[u8], base_offset: usize) -> Vec<(u32, u32)> {
    let blocks = basic_blocks(instructions, data, base_offset);
    let mut edges = Vec::new();
    for b in &blocks {
        for &to in &b.successors {
            edges.push((b.start_offset, to));
        }
        if let Some(to) = b.fallthrough_to {
            edges.push((b.start_offset, to));
        }
    }
    edges
}
