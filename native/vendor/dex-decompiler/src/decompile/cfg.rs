//! Control-flow graph for method body: basic blocks, loop detection, if/else structure.
//! Used to emit structured Java (if/else, while/for).

use dex_bytecode::{basic_blocks, explicit_successors, Instruction};
use std::collections::{HashMap, HashSet};

/// Parse packed/sparse-switch payload at expected offset; return (case_value, target_byte_offset) for each case.
/// Target offset is in the same space as block start_offset (byte offset in code buffer).
fn parse_switch_payload(data: &[u8], switch_ins_off: usize) -> Option<Vec<(i32, u32)>> {
    if switch_ins_off + 6 > data.len() {
        return None;
    }
    let rel_units = i32::from_le_bytes(
        data[switch_ins_off + 2..switch_ins_off + 6]
            .try_into()
            .unwrap_or([0, 0, 0, 0]),
    );
    let try_payload = |off: usize| -> Option<(u16, u16)> {
        if off + 4 > data.len() {
            return None;
        }
        let id = u16::from_le_bytes(data[off..off + 2].try_into().unwrap_or([0, 0]));
        let sz = u16::from_le_bytes(data[off + 2..off + 4].try_into().unwrap_or([0, 0]));
        if id == 0x0100 || id == 0x0200 {
            Some((id, sz))
        } else {
            None
        }
    };
    let cand_a = (switch_ins_off as i32 + rel_units * 2) as usize;
    let cand_b = (switch_ins_off as i32 + 2 + rel_units * 2) as usize;
    let mut payload_off = None;
    for &off in &[cand_a, cand_b] {
        if off + 4 <= data.len() && try_payload(off).is_some() {
            payload_off = Some(off);
            break;
        }
    }
    let payload_off = payload_off.or_else(|| {
        let start = cand_a.saturating_sub(8);
        let end = (cand_a + 8).min(data.len().saturating_sub(4));
        (start..=end).find(|&off| off + 4 <= data.len() && try_payload(off).is_some())
    })?;
    let (ident, size) = try_payload(payload_off)?;
    let size = size as usize;
    let mut out = Vec::with_capacity(size);
    match ident {
        0x0100 => {
            if payload_off + 8 + size * 4 > data.len() {
                return None;
            }
            let first_key = i32::from_le_bytes(
                data[payload_off + 4..payload_off + 8]
                    .try_into()
                    .unwrap_or([0, 0, 0, 0]),
            );
            let targets_base = payload_off + 8;
            for i in 0..size {
                let rel = i32::from_le_bytes(
                    data[targets_base + i * 4..targets_base + i * 4 + 4]
                        .try_into()
                        .unwrap_or([0, 0, 0, 0]),
                );
                // Target in 16-bit units from switch instruction; same as dex-bytecode: base + rel*2.
                let target_byte = ((switch_ins_off as i32) + rel * 2) as u32;
                out.push((first_key + i as i32, target_byte));
            }
        }
        0x0200 => {
            if payload_off + 4 + size * 8 > data.len() {
                return None;
            }
            let keys_base = payload_off + 4;
            let targets_base = payload_off + 4 + size * 4;
            for i in 0..size {
                let key = i32::from_le_bytes(
                    data[keys_base + i * 4..keys_base + i * 4 + 4]
                        .try_into()
                        .unwrap_or([0, 0, 0, 0]),
                );
                let rel = i32::from_le_bytes(
                    data[targets_base + i * 4..targets_base + i * 4 + 4]
                        .try_into()
                        .unwrap_or([0, 0, 0, 0]),
                );
                let target_byte = ((switch_ins_off as i32) + rel * 2) as u32;
                out.push((key, target_byte));
            }
        }
        _ => return None,
    }
    // basic_blocks uses byte offsets in the buffer; ensure we don't exceed slice length
    for (_, t) in out.iter_mut() {
        if *t as usize > data.len() {
            return None;
        }
    }
    Some(out)
}

/// Block index in the CFG (entry = 0 when blocks are sorted by start_offset).
pub type BlockId = usize;

/// Kind of control flow at the end of a block.
#[derive(Debug, Clone)]
pub enum BlockEnd {
    /// Fall-through to next block (no branch, or return/throw).
    FallThrough,
    /// Unconditional branch (goto) to target block.
    Goto(BlockId),
    /// Conditional branch: if condition then branch_target else fall_through.
    Conditional {
        condition: String,
        branch_target: BlockId,
        fall_through: BlockId,
    },
    /// Packed/sparse switch: (case value -> block), default block.
    Switch {
        /// Variable/expression used in switch (e.g. "local0").
        condition: String,
        /// (case value, target block) in payload order.
        cases: Vec<(i32, BlockId)>,
        /// Block for default (fall-through from switch instruction).
        default_block: BlockId,
    },
    /// Return or throw; no successor.
    Exit,
}

/// Extended block with resolved successors and end kind.
#[derive(Debug, Clone)]
pub struct CfgBlock {
    #[allow(dead_code)]
    pub start_offset: u32,
    pub end_offset: u32,
    pub end: BlockEnd,
    /// Instruction byte offsets (in code buffer) that belong to this block.
    pub instruction_offsets: Vec<u32>,
}

/// Method-level CFG: blocks, entry, loop headers.
#[derive(Debug, Clone)]
pub struct MethodCfg {
    pub blocks: Vec<CfgBlock>,
    /// Map: byte offset (block start) -> BlockId.
    pub block_by_start: HashMap<u32, BlockId>,
    /// Block ids that are targets of back edges (loop headers).
    pub loop_headers: HashSet<BlockId>,
    /// Entry block id (block containing first instruction).
    pub entry: BlockId,
    /// Instruction offsets of const values folded into conditions (skip during emission).
    pub folded_const_offsets: HashSet<u32>,
}

impl MethodCfg {
    /// Build CFG from decoded instructions and bytecode. base_offset is 0 when using insns_slice.
    pub fn build(
        instructions: &[Instruction],
        data: &[u8],
        base_offset: usize,
        condition_for_offset: &impl Fn(&Instruction) -> String,
    ) -> Self {
        let raw_blocks = basic_blocks(instructions, data, base_offset);
        if raw_blocks.is_empty() {
            return Self {
                blocks: vec![],
                block_by_start: HashMap::new(),
                loop_headers: HashSet::new(),
                entry: 0,
                folded_const_offsets: HashSet::new(),
            };
        }

        let mut block_by_start: HashMap<u32, BlockId> = HashMap::new();
        for (i, b) in raw_blocks.iter().enumerate() {
            block_by_start.insert(b.start_offset, i);
        }

        let entry = *block_by_start.get(&(base_offset as u32)).unwrap_or(&0);

        let mut blocks: Vec<CfgBlock> = Vec::with_capacity(raw_blocks.len());
        for (_i, rb) in raw_blocks.iter().enumerate() {
            let start = rb.start_offset;
            let end = rb.end_offset;

            let instruction_offsets: Vec<u32> = instructions
                .iter()
                .filter(|ins| {
                    let off = (ins.offset as usize) + base_offset;
                    off >= start as usize && off < end as usize
                })
                .map(|ins| (ins.offset as usize + base_offset) as u32)
                .collect();

            let last_ins = instruction_offsets.last().and_then(|&off| {
                instructions
                    .iter()
                    .find(|ins| (ins.offset as usize) + base_offset == off as usize)
            });

            // Prefer BasicBlock.fallthrough_to (absent for goto / exit).
            let fall_through_id = rb
                .fallthrough_to
                .and_then(|off| block_by_start.get(&off).copied());
            let branch_target_ids: Vec<BlockId> = rb
                .successors
                .iter()
                .filter_map(|&off| {
                    block_by_start.get(&off).copied().or_else(|| {
                        // Target may land mid-block if data is odd; map to containing block.
                        raw_blocks.iter().position(|b| {
                            let b_end = if b.end_offset == u32::MAX {
                                usize::MAX
                            } else {
                                b.end_offset as usize
                            };
                            (b.start_offset as usize..b_end).contains(&(off as usize))
                        })
                    })
                })
                .collect();

            let end_final = if let Some(ins) = last_ins {
                let m = ins.mnemonic();
                // Look for packed/sparse-switch anywhere in the block (e.g. block may be const, packed-switch, goto).
                let switch_ins = instruction_offsets.iter().find_map(|&off| {
                    let ins_here = instructions
                        .iter()
                        .find(|i| (i.offset as usize) + base_offset == off as usize)?;
                    if ins_here.mnemonic() == "packed-switch"
                        || ins_here.mnemonic() == "sparse-switch"
                    {
                        Some((ins_here, (off as usize).saturating_sub(base_offset)))
                    } else {
                        None
                    }
                });
                if let Some((switch_ins, ins_off_in_slice)) = switch_ins {
                    if let Some(case_targets) = parse_switch_payload(data, ins_off_in_slice) {
                        // Default block: if the next instruction after the switch is a goto, default is goto target; else fall-through.
                        let next_off_abs = (base_offset + ins_off_in_slice + 6) as u32;
                        let default_id = if instruction_offsets.contains(&next_off_abs) {
                            instructions
                                .iter()
                                .find(|i| {
                                    (i.offset as usize) + base_offset == next_off_abs as usize
                                })
                                .and_then(|i| {
                                    if i.mnemonic().starts_with("goto") {
                                        branch_target_ids.first().copied()
                                    } else {
                                        None
                                    }
                                })
                        } else {
                            None
                        }
                        .or_else(|| {
                            let key = (ins_off_in_slice + 6) as u32;
                            block_by_start.get(&key).copied()
                        })
                        .or_else(|| fall_through_id)
                        .unwrap_or(entry);
                        let cases: Vec<(i32, BlockId)> = case_targets
                            .into_iter()
                            .filter_map(|(val, off)| {
                                let bid = block_by_start.get(&off).copied().or_else(|| {
                                    raw_blocks
                                        .iter()
                                        .position(|b| {
                                            let end = if b.end_offset == u32::MAX {
                                                usize::MAX
                                            } else {
                                                b.end_offset as usize
                                            };
                                            (b.start_offset as usize..end).contains(&(off as usize))
                                        })
                                        .map(|i| i as BlockId)
                                })?;
                                Some((val, bid))
                            })
                            .collect();
                        if !cases.is_empty() {
                            blocks.push(CfgBlock {
                                start_offset: start,
                                end_offset: end,
                                end: BlockEnd::Switch {
                                    condition: condition_for_offset(switch_ins),
                                    cases,
                                    default_block: default_id,
                                },
                                instruction_offsets: instruction_offsets.clone(),
                            });
                            continue;
                        }
                    }
                }
                match m {
                    "return-void" | "return" | "return-wide" | "return-object" | "throw" => {
                        BlockEnd::Exit
                    }
                    "goto" | "goto/16" | "goto/32" => {
                        // Prefer the last instruction's own target (not other branches in the block).
                        let goto_tid = last_ins.and_then(|gi| {
                            let off = (gi.offset as usize) + base_offset;
                            explicit_successors(data, off)
                                .into_iter()
                                .next()
                                .and_then(|toff| {
                                    block_by_start.get(&toff).copied().or_else(|| {
                                        raw_blocks.iter().position(|b| {
                                            let b_end = if b.end_offset == u32::MAX {
                                                usize::MAX
                                            } else {
                                                b.end_offset as usize
                                            };
                                            (b.start_offset as usize..b_end)
                                                .contains(&(toff as usize))
                                        })
                                    })
                                })
                        });
                        if let Some(tid) = goto_tid.or_else(|| branch_target_ids.first().copied()) {
                            BlockEnd::Goto(tid)
                        } else {
                            fall_through_id
                                .map(|_| BlockEnd::FallThrough)
                                .unwrap_or(BlockEnd::Exit)
                        }
                    }
                    "if-eq" | "if-ne" | "if-lt" | "if-ge" | "if-gt" | "if-le" | "if-eqz"
                    | "if-nez" | "if-ltz" | "if-gez" | "if-gtz" | "if-lez" => {
                        let cond = condition_for_offset(ins);
                        let then_id = branch_target_ids.first().copied().unwrap_or(entry);
                        let else_id = fall_through_id.unwrap_or(entry);
                        BlockEnd::Conditional {
                            condition: cond,
                            branch_target: then_id,
                            fall_through: else_id,
                        }
                    }
                    "packed-switch" | "sparse-switch" => {
                        let ins_off_in_slice = instruction_offsets
                            .last()
                            .copied()
                            .map(|o| (o as usize).saturating_sub(base_offset))
                            .unwrap_or(0);
                        if let Some(case_targets) = parse_switch_payload(data, ins_off_in_slice) {
                            let default_off = (ins_off_in_slice + 6) as u32;
                            let default_id = block_by_start
                                .get(&default_off)
                                .copied()
                                .or_else(|| fall_through_id)
                                .unwrap_or(entry);
                            let cases: Vec<(i32, BlockId)> = case_targets
                                .into_iter()
                                .filter_map(|(val, off)| {
                                    let bid = block_by_start.get(&off).copied().or_else(|| {
                                        raw_blocks
                                            .iter()
                                            .position(|b| {
                                                let end = if b.end_offset == u32::MAX {
                                                    usize::MAX
                                                } else {
                                                    b.end_offset as usize
                                                };
                                                (b.start_offset as usize..end)
                                                    .contains(&(off as usize))
                                            })
                                            .map(|i| i as BlockId)
                                    })?;
                                    Some((val, bid))
                                })
                                .collect();
                            if !cases.is_empty() {
                                BlockEnd::Switch {
                                    condition: condition_for_offset(ins),
                                    cases,
                                    default_block: default_id,
                                }
                            } else {
                                fall_through_id
                                    .map(|_| BlockEnd::FallThrough)
                                    .unwrap_or(BlockEnd::Exit)
                            }
                        } else {
                            fall_through_id
                                .map(|_| BlockEnd::FallThrough)
                                .unwrap_or(BlockEnd::Exit)
                        }
                    }
                    _ => fall_through_id
                        .map(|_| BlockEnd::FallThrough)
                        .unwrap_or(BlockEnd::Exit),
                }
            } else {
                fall_through_id
                    .map(|_| BlockEnd::FallThrough)
                    .unwrap_or(BlockEnd::Exit)
            };

            blocks.push(CfgBlock {
                start_offset: start,
                end_offset: end,
                end: end_final,
                instruction_offsets,
            });
        }

        // Detect loop headers: blocks that are targets of back edges (edge from B to A where A.start < B.start).
        let mut loop_headers = HashSet::new();
        for (_i, rb) in raw_blocks.iter().enumerate() {
            for &succ_off in &rb.successors {
                if let Some(&succ_id) = block_by_start.get(&succ_off) {
                    let succ_start = raw_blocks[succ_id].start_offset;
                    let my_start = rb.start_offset;
                    if succ_start < my_start {
                        loop_headers.insert(succ_id);
                    }
                }
            }
        }

        Self {
            blocks,
            block_by_start,
            loop_headers,
            entry,
            folded_const_offsets: HashSet::new(),
        }
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Split existing blocks at instruction-aligned offsets (notably catch entries).
    ///
    /// The bytecode branch CFG cannot discover exception-handler leaders on its own.
    /// This preserves normal successors while making each handler address targetable.
    pub fn split_at_offsets(&mut self, offsets: &[u32]) {
        if self.blocks.is_empty() || offsets.is_empty() {
            return;
        }
        let old_blocks = self.blocks.clone();
        let old_entry = self.entry;
        let old_loops = self.loop_headers.clone();
        let mut old_to_first = vec![0usize; old_blocks.len()];
        let mut old_to_last = vec![0usize; old_blocks.len()];
        let mut blocks = Vec::new();

        for (old_id, block) in old_blocks.iter().enumerate() {
            let mut cuts: Vec<u32> = offsets
                .iter()
                .copied()
                .filter(|offset| {
                    *offset > block.start_offset
                        && *offset < block.end_offset
                        && block.instruction_offsets.contains(offset)
                })
                .collect();
            cuts.sort_unstable();
            cuts.dedup();
            let mut starts = vec![block.start_offset];
            starts.extend(cuts);
            old_to_first[old_id] = blocks.len();
            for (index, start) in starts.iter().copied().enumerate() {
                let end = starts.get(index + 1).copied().unwrap_or(block.end_offset);
                let instruction_offsets = block
                    .instruction_offsets
                    .iter()
                    .copied()
                    .filter(|offset| *offset >= start && *offset < end)
                    .collect();
                blocks.push(CfgBlock {
                    start_offset: start,
                    end_offset: end,
                    end: if index + 1 == starts.len() {
                        block.end.clone()
                    } else {
                        BlockEnd::FallThrough
                    },
                    instruction_offsets,
                });
            }
            old_to_last[old_id] = blocks.len() - 1;
        }

        let remap = |old: BlockId| old_to_first.get(old).copied().unwrap_or(old);
        for old_id in 0..old_blocks.len() {
            let last = old_to_last[old_id];
            blocks[last].end = match blocks[last].end.clone() {
                BlockEnd::Goto(target) => BlockEnd::Goto(remap(target)),
                BlockEnd::Conditional {
                    condition,
                    branch_target,
                    fall_through,
                } => BlockEnd::Conditional {
                    condition,
                    branch_target: remap(branch_target),
                    fall_through: remap(fall_through),
                },
                BlockEnd::Switch {
                    condition,
                    cases,
                    default_block,
                } => BlockEnd::Switch {
                    condition,
                    cases: cases
                        .into_iter()
                        .map(|(value, target)| (value, remap(target)))
                        .collect(),
                    default_block: remap(default_block),
                },
                other => other,
            };
        }

        self.entry = remap(old_entry);
        self.loop_headers = old_loops.into_iter().map(remap).collect();
        self.block_by_start = blocks
            .iter()
            .enumerate()
            .map(|(id, block)| (block.start_offset, id))
            .collect();
        self.blocks = blocks;
    }

    /// All (from_block_id, to_block_id) edges for graph visualization.
    pub fn successor_edges(&self) -> Vec<(BlockId, BlockId)> {
        let mut edges = Vec::new();
        for (from_id, block) in self.blocks.iter().enumerate() {
            match &block.end {
                BlockEnd::FallThrough => {
                    if let Some(ft) = self.fall_through_block(from_id) {
                        edges.push((from_id, ft));
                    }
                }
                BlockEnd::Goto(t) => edges.push((from_id, *t)),
                BlockEnd::Conditional {
                    branch_target,
                    fall_through,
                    ..
                } => {
                    edges.push((from_id, *branch_target));
                    edges.push((from_id, *fall_through));
                }
                BlockEnd::Switch {
                    cases,
                    default_block,
                    ..
                } => {
                    for (_, bid) in cases {
                        edges.push((from_id, *bid));
                    }
                    edges.push((from_id, *default_block));
                }
                BlockEnd::Exit => {}
            }
        }
        edges
    }

    /// Conservative edges from every protected block to each applicable catch handler.
    ///
    /// DEX does not encode which instruction throws, so all blocks overlapping the try
    /// interval are connected. Reaching-definitions then preserves values across catch paths.
    pub fn exceptional_edges_for_range(
        &self,
        try_start: u32,
        try_end: u32,
        handler_starts: &[u32],
    ) -> Vec<(BlockId, BlockId)> {
        let handlers: Vec<BlockId> = handler_starts
            .iter()
            .filter_map(|offset| self.block_id_at_offset(*offset))
            .collect();
        let mut edges = Vec::new();
        for (from, block) in self.blocks.iter().enumerate() {
            let overlaps = block.start_offset < try_end && block.end_offset > try_start;
            if !overlaps {
                continue;
            }
            for &to in &handlers {
                if from != to && !edges.contains(&(from, to)) {
                    edges.push((from, to));
                }
            }
        }
        edges
    }

    pub fn fall_through_block(&self, block_id: BlockId) -> Option<BlockId> {
        let end = self.blocks[block_id].end_offset;
        self.block_by_start.get(&end).copied()
    }

    /// Blocks that fall through to the given block (predecessors by fall-through only).
    /// Used to include the full exit path when branch_target is e.g. a return-only block
    /// and the real exit (e.g. Swap + return) is the block that falls through to it.
    pub fn blocks_that_fall_through_to(&self, target: BlockId) -> Vec<BlockId> {
        let mut preds = Vec::new();
        for bid in 0..self.blocks.len() {
            if self.fall_through_block(bid) == Some(target) {
                preds.push(bid);
            }
        }
        preds
    }

    /// Block id that contains the given byte offset (start_offset <= offset < end_offset).
    pub fn block_id_at_offset(&self, byte_offset: u32) -> Option<BlockId> {
        for (bid, block) in self.blocks.iter().enumerate() {
            let end = block.end_offset;
            if block.start_offset <= byte_offset && (byte_offset < end || end == u32::MAX) {
                return Some(bid);
            }
        }
        None
    }

    /// True if `target` is reachable from `start` without entering `exclude` (e.g. loop header).
    pub fn reachable_from(
        &self,
        start: BlockId,
        target: BlockId,
        exclude: Option<BlockId>,
    ) -> bool {
        use std::collections::HashSet;
        let mut visited = HashSet::new();
        let mut stack = vec![start];
        while let Some(bid) = stack.pop() {
            if bid == target {
                return true;
            }
            if visited.contains(&bid) || exclude == Some(bid) {
                continue;
            }
            visited.insert(bid);
            let block = &self.blocks[bid];
            match &block.end {
                BlockEnd::FallThrough => {
                    if let Some(ft) = self.fall_through_block(bid) {
                        stack.push(ft);
                    }
                }
                BlockEnd::Goto(t) => stack.push(*t),
                BlockEnd::Conditional {
                    branch_target,
                    fall_through,
                    ..
                } => {
                    stack.push(*branch_target);
                    stack.push(*fall_through);
                }
                BlockEnd::Switch {
                    cases,
                    default_block,
                    ..
                } => {
                    for (_, bid) in cases {
                        stack.push(*bid);
                    }
                    stack.push(*default_block);
                }
                BlockEnd::Exit => {}
            }
        }
        false
    }

    /// Blocks in the natural loop for `header` (back-edge targets → header, excluding exit paths).
    pub fn natural_loop_blocks(&self, header: BlockId) -> HashSet<BlockId> {
        let mut body = HashSet::new();
        body.insert(header);
        let back_edges: Vec<BlockId> = self
            .successor_edges()
            .iter()
            .filter(|(_, to)| *to == header)
            .map(|(from, _)| *from)
            .collect();
        for tail in back_edges {
            let mut stack = vec![tail];
            let mut visited = HashSet::new();
            while let Some(bid) = stack.pop() {
                if bid == header || !visited.insert(bid) {
                    continue;
                }
                body.insert(bid);
                for (pred, succ) in self.successor_edges() {
                    if succ == bid {
                        stack.push(pred);
                    }
                }
            }
        }
        body
    }

    /// True when `target` is outside the natural loop for `loop_header` (exit / tail code).
    pub fn is_loop_exit_target(&self, target: BlockId, loop_header: Option<BlockId>) -> bool {
        let Some(header) = loop_header else {
            return false;
        };
        if target == header {
            return false;
        }
        !self.natural_loop_blocks(header).contains(&target)
    }

    /// All blocks targeted by edges leaving the natural loop body (exit edge targets).
    pub fn loop_exit_targets(&self, header: BlockId) -> HashSet<BlockId> {
        let body = self.natural_loop_blocks(header);
        let mut exits = HashSet::new();
        for (from, to) in self.successor_edges() {
            if body.contains(&from) && !body.contains(&to) {
                exits.insert(to);
            }
        }
        exits
    }

    /// Walk fall-through predecessors outside the loop to include multi-block exit chains
    /// (e.g. `const/4; return` where the branch targets the return block).
    pub fn exit_chain_head(
        &self,
        exit: BlockId,
        loop_header: Option<BlockId>,
        emitted: &HashSet<BlockId>,
    ) -> BlockId {
        let mut head = exit;
        loop {
            let mut extended = None;
            for (pred, succ) in self.successor_edges() {
                if succ != head || emitted.contains(&pred) {
                    continue;
                }
                if loop_header == Some(pred) {
                    continue;
                }
                if self.fall_through_block(pred) != Some(head) {
                    continue;
                }
                if self.is_loop_exit_target(head, loop_header)
                    && !self.is_loop_exit_target(pred, loop_header)
                {
                    continue;
                }
                extended = Some(pred);
                break;
            }
            match extended {
                Some(p) => head = p,
                None => break,
            }
        }
        head
    }

    /// Pretty-print CFG blocks, edges, and loop headers (L0-1 debug helper).
    pub fn format_debug(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "blocks={} loop_headers={:?}",
            self.block_count(),
            self.loop_headers
        );
        for (i, b) in self.blocks.iter().enumerate() {
            let _ = writeln!(
                out,
                "  block {i}: off={} end={:?} ins={}",
                b.start_offset,
                b.end,
                b.instruction_offsets.len()
            );
        }
        let _ = writeln!(out, "edges={:?}", self.successor_edges());
        for &h in &self.loop_headers {
            let _ = writeln!(out, "  loop {h} exits={:?}", self.loop_exit_targets(h));
        }
        out
    }

    /// True if any block ends with return/throw.
    pub fn has_return_block(&self) -> bool {
        self.blocks.iter().any(|b| matches!(b.end, BlockEnd::Exit))
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockEnd, MethodCfg};
    use dex_bytecode::decode_all;

    fn condition_for(_ins: &dex_bytecode::Instruction) -> String {
        "v0 == 0".to_string()
    }

    /// Single return-void: entry block ends in Exit (basic_blocks may split after branch/return).
    #[test]
    fn cfg_single_return_void() {
        let bytecode: &[u8] = &[0x0e, 0x00]; // return-void
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        assert!(cfg.block_count() >= 1);
        assert!(matches!(cfg.blocks[cfg.entry].end, BlockEnd::Exit));
        assert!(cfg.loop_headers.is_empty());
    }

    /// if-eqz v0, +4; goto +2; return-void; return-void
    /// Blocks: [0-4) if-eqz, [4-6) goto, [6-8) return-void, [8-10) return-void.
    /// Then-branch target = 8, else fall-through = 4.
    #[test]
    fn cfg_if_else_two_branches() {
        // if-eqz v0, +4 (21t: target = 0+4*2 = 8); goto +2 (target 4+2*2 = 8); return-void at 6; return-void at 8
        let bytecode: &[u8] = &[
            0x38, 0x00, 0x04, 0x00, // if-eqz v0, +4 -> target byte 8
            0x28, 0x02, // goto +2 -> target byte 8
            0x0e, 0x00, // return-void at 6
            0x0e, 0x00, // return-void at 8
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        assert!(cfg.block_count() >= 2);
        let has_conditional = cfg
            .blocks
            .iter()
            .any(|b| matches!(&b.end, BlockEnd::Conditional { .. }));
        assert!(
            has_conditional,
            "expected at least one conditional block (if-eqz)"
        );
    }

    /// natural_loop_blocks for merge-style CFG: main loop is blocks 1-10, not tail headers 11/15/19.
    #[test]
    fn natural_loop_blocks_merge_shape() {
        // Minimal back-edge loop: header at block 0, body 1, back from 1 to 0; exit block 2.
        let bytecode: &[u8] = &[
            0x38, 0x00, 0x04, 0x00, // 0: if-eqz -> 8 (exit block 2)
            0x12, 0x00, // 4: body
            0x28, 0xfc, // 6: goto -4 -> 0
            0x0e, 0x00, // 8: return-void (outside loop)
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        let header = cfg.entry;
        let body = cfg.natural_loop_blocks(header);
        assert!(body.contains(&header));
        assert!(
            !body.contains(&(header + 2)),
            "return block should not be in loop body"
        );
    }

    /// Loop: const/4; if-eqz (exit); goto back to if-eqz block. Back edge target is loop header.
    #[test]
    fn cfg_loop_back_edge() {
        // const/4 v0,0 (0..2); if-eqz v0,+5 (2..6) target 12; goto -2 (6..8) target 2; nop nop (8..12); return-void (12..14)
        let bytecode: &[u8] = &[
            0x12, 0x00, // const/4 v0, 0
            0x38, 0x00, 0x05, 0x00, // if-eqz v0, +5 -> target byte 12
            0x28, 0xfe, // goto -2 -> target byte 2
            0x00, 0x00, 0x00, 0x00, // nop, nop
            0x0e, 0x00, // return-void at 12
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        assert!(
            !cfg.loop_headers.is_empty(),
            "goto back to earlier block should create loop header"
        );
    }

    /// Linear sequence: const/4, return (value-return). At least one block ends in Exit.
    #[test]
    fn cfg_linear_then_return() {
        let bytecode: &[u8] = &[
            0x12, 0x00, // const/4 v0, 0
            0x0f, 0x00, // return v0
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        assert!(cfg.block_count() >= 1);
        let has_exit = cfg.blocks.iter().any(|b| matches!(b.end, BlockEnd::Exit));
        assert!(
            has_exit,
            "linear method with return should have an Exit block"
        );
    }
}
