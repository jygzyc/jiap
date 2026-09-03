//! Value-flow and data tainting: where a specific value is read/written in the method.
//!
//! Uses reaching definitions over the CFG. Given a seed (instruction offset + register),
//! returns all program points that **read** that value and all points that **write** (define) it.

use super::cfg::{BlockId, MethodCfg};
use super::read_write::instruction_reads_writes;
use dex_bytecode::Instruction;
use std::collections::{HashMap, HashSet};

/// Map from instruction offset (invoke only) to resolved method ref. Used for PendingIntent and sink analysis.
pub type InvokeMethodMap = HashMap<u32, String>;

/// Owns CFG, read/write map, API return sources, and invoke→method ref map for tainting and PendingIntent analysis.
pub struct ValueFlowAnalysisOwned {
    pub cfg: MethodCfg,
    pub rw_map: InstructionRwMap,
    /// Conservative exceptional control-flow edges from protected blocks to catch handlers.
    pub exceptional_edges: Vec<(BlockId, BlockId)>,
    /// For each (offset, reg) of a move-result that receives an invoke return, the method ref (e.g. "FusedLocationProviderClient.getLastLocation").
    pub api_return_sources: Vec<((u32, u32), String)>,
    /// For each invoke offset, the resolved method ref (e.g. "android.app.PendingIntent.getActivity").
    pub invoke_method_map: InvokeMethodMap,
    /// Disassembly label per instruction offset: `"mnemonic operands"`.
    pub insn_at: HashMap<u32, String>,
    /// `code_item.registers_size` (total virtual registers).
    pub registers_size: u32,
    /// `code_item.ins_size` (incoming argument register slots, including `this`).
    pub ins_size: u32,
}

impl ValueFlowAnalysisOwned {
    /// Human-readable instruction at `offset`, or empty.
    pub fn insn_label(&self, offset: u32) -> String {
        self.insn_at.get(&offset).cloned().unwrap_or_default()
    }

    /// Build a value-flow analysis (reaching defs, def-use, use-def). The returned analysis borrows this struct.
    pub fn analysis(&self) -> ValueFlowAnalysis<'_> {
        ValueFlowAnalysis::new_with_exceptional_edges(
            &self.cfg,
            &self.rw_map,
            &self.exceptional_edges,
        )
    }

    /// Follow an incoming method argument, which has no defining DEX instruction.
    pub fn value_flow_from_entry_reg(&self, reg: u32) -> ValueFlowResult {
        const ENTRY_DEF: u32 = u32::MAX;
        let mut cfg = self.cfg.clone();
        let Some(entry) = cfg.blocks.get_mut(cfg.entry) else {
            return ValueFlowResult::default();
        };
        entry.instruction_offsets.insert(0, ENTRY_DEF);
        let mut rw_map = self.rw_map.clone();
        rw_map.insert(ENTRY_DEF, (vec![], vec![reg]));
        ValueFlowAnalysis::new_with_exceptional_edges(&cfg, &rw_map, &self.exceptional_edges)
            .value_flow_from_seed(ENTRY_DEF, reg)
    }

    /// Run value-flow from every move-result that receives the return of an invoke whose method ref
    /// contains any of the given patterns (e.g. "getLastLocation" or "FusedLocationProviderClient.getLastLocation").
    /// Merges reads and writes from all matching seeds (union, deduped).
    pub fn value_flow_from_api_sources<P: AsRef<str>>(&self, patterns: &[P]) -> ValueFlowResult {
        let seeds: Vec<(u32, u32)> = self
            .api_return_sources
            .iter()
            .filter(|(_, method_ref)| patterns.iter().any(|p| method_ref.contains(p.as_ref())))
            .map(|&((offset, reg), _)| (offset, reg))
            .collect();
        let mut reads = HashSet::new();
        let mut writes = HashSet::new();
        let analysis = self.analysis();
        for (offset, reg) in seeds {
            let result = analysis.value_flow_from_seed(offset, reg);
            reads.extend(result.reads);
            writes.extend(result.writes);
        }
        ValueFlowResult {
            reads: reads.into_iter().collect(),
            writes: writes.into_iter().collect(),
        }
    }
}

/// Per-instruction read/write sets: offset -> (regs_read, regs_written).
pub type InstructionRwMap = HashMap<u32, (Vec<u32>, Vec<u32>)>;

/// Extract method ref from resolved invoke operands (e.g. "v0, pkg.Clz.m(I)V" -> "pkg.Clz.m").
fn extract_invoke_method_ref(ops_resolved: &str) -> Option<String> {
    let inner = ops_resolved.trim();
    if inner.is_empty() {
        return None;
    }
    let mut depth = 0u32;
    let mut last_comma_at = None;
    for (i, c) in inner.chars().enumerate() {
        match c {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => last_comma_at = Some(i),
            _ => {}
        }
    }
    // Zero-argument invokes contain only the method reference and therefore no
    // top-level comma (for example `pkg.Origin.source()`).
    let method_ref = last_comma_at
        .map(|split_at| inner[split_at + 1..].trim())
        .unwrap_or(inner);
    let method_name = method_ref.split('(').next().unwrap_or(method_ref).trim();
    Some(method_name.to_string())
}

/// Build map from invoke offset to resolved method ref (e.g. "android.app.PendingIntent.getActivity").
/// Used for PendingIntent and sink analysis.
pub fn build_invoke_method_map<F>(
    instructions: &[Instruction],
    base_off: u32,
    resolve_operands: F,
) -> InvokeMethodMap
where
    F: Fn(&str) -> String,
{
    let mut map = InvokeMethodMap::new();
    for ins in instructions {
        let offset = (ins.offset as u32).wrapping_add(base_off);
        let m = ins.mnemonic();
        if m.starts_with("invoke-") {
            let resolved = resolve_operands(ins.operands());
            if let Some(method_ref) = extract_invoke_method_ref(&resolved) {
                map.insert(offset, method_ref);
            }
        }
    }
    map
}

/// Build list of (offset, reg) for each move-result that receives the return of an invoke, with the method ref.
/// Used to taint returns of Android API methods (e.g. FusedLocationProviderClient.getLastLocation).
pub fn build_api_return_sources<F>(
    instructions: &[Instruction],
    base_off: u32,
    resolve_operands: F,
) -> Vec<((u32, u32), String)>
where
    F: Fn(&str) -> String,
{
    let mut out = Vec::new();
    let mut last_invoke_method: Option<String> = None;
    for ins in instructions {
        let offset = (ins.offset as u32).wrapping_add(base_off);
        let m = ins.mnemonic();
        let resolved = resolve_operands(ins.operands());
        if m.starts_with("invoke-") {
            last_invoke_method = extract_invoke_method_ref(&resolved);
        } else if m.starts_with("move-result") {
            let (_, writes) = instruction_reads_writes(m, &resolved);
            if let Some(method_ref) = last_invoke_method.take() {
                // move-result-wide defines both register slots; either slot can be
                // consumed by subsequent wide argument handling.
                for reg in writes {
                    out.push(((offset, reg), method_ref.clone()));
                }
            }
        } else {
            last_invoke_method = None;
        }
    }
    out
}

/// Build the read/write map for all instructions. Resolve operands using the given closure.
pub fn build_instruction_rw_map<F>(
    instructions: &[Instruction],
    base_off: u32,
    resolve_operands: F,
) -> InstructionRwMap
where
    F: Fn(&str) -> String,
{
    let mut map = InstructionRwMap::new();
    for ins in instructions {
        let offset = (ins.offset as u32).wrapping_add(base_off);
        let resolved = resolve_operands(ins.operands());
        let (reads, writes) = instruction_reads_writes(ins.mnemonic(), &resolved);
        map.insert(offset, (reads, writes));
    }
    map
}

/// Build offset → `"mnemonic operands"` labels for analysis evidence.
pub fn build_insn_labels<F>(
    instructions: &[Instruction],
    base_off: u32,
    resolve_operands: F,
) -> HashMap<u32, String>
where
    F: Fn(&str) -> String,
{
    let mut map = HashMap::new();
    for ins in instructions {
        let offset = (ins.offset as u32).wrapping_add(base_off);
        let resolved = resolve_operands(ins.operands());
        let label = if resolved.is_empty() {
            ins.mnemonic().to_string()
        } else {
            format!("{} {}", ins.mnemonic(), resolved)
        };
        map.insert(offset, label);
    }
    map
}

/// Result of value-flow from a seed: all (offset, reg) that read the value, and all that write it.
#[derive(Debug, Clone, Default)]
pub struct ValueFlowResult {
    /// Program points that **read** the value (use it).
    pub reads: Vec<(u32, u32)>,
    /// Program points that **write** the value (define it; includes the seed and copies).
    pub writes: Vec<(u32, u32)>,
}

/// Reaching definitions and def-use / use-def for a method.
pub struct ValueFlowAnalysis<'a> {
    rw_map: &'a InstructionRwMap,
    /// For each (offset, reg) use, the set of (offset', reg') defs that can reach it.
    use_def: HashMap<(u32, u32), HashSet<(u32, u32)>>,
    /// For each (offset', reg') def, the set of (offset, reg) uses it reaches.
    def_use: HashMap<(u32, u32), HashSet<(u32, u32)>>,
}

impl<'a> ValueFlowAnalysis<'a> {
    /// Build reaching definitions and def-use / use-def from CFG and per-instruction read/write map.
    pub fn new(cfg: &'a MethodCfg, rw_map: &'a InstructionRwMap) -> Self {
        Self::new_with_exceptional_edges(cfg, rw_map, &[])
    }

    /// Build reaching definitions including throw/catch control-flow edges.
    pub fn new_with_exceptional_edges(
        cfg: &'a MethodCfg,
        rw_map: &'a InstructionRwMap,
        exceptional_edges: &[(BlockId, BlockId)],
    ) -> Self {
        let predecessors = Self::predecessors(cfg, exceptional_edges);
        let (use_def, def_use) = Self::reaching_defs(cfg, rw_map, &predecessors);
        Self {
            rw_map,
            use_def,
            def_use,
        }
    }

    fn predecessors(
        cfg: &MethodCfg,
        exceptional_edges: &[(BlockId, BlockId)],
    ) -> HashMap<BlockId, Vec<BlockId>> {
        let mut pred: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        for (from, to) in cfg
            .successor_edges()
            .into_iter()
            .chain(exceptional_edges.iter().copied())
        {
            pred.entry(to).or_default().push(from);
        }
        pred
    }

    /// Compute reaching definitions: at each (offset, reg) use, which (offset', reg') defs reach it.
    fn reaching_defs(
        cfg: &MethodCfg,
        rw_map: &InstructionRwMap,
        predecessors: &HashMap<BlockId, Vec<BlockId>>,
    ) -> (
        HashMap<(u32, u32), HashSet<(u32, u32)>>,
        HashMap<(u32, u32), HashSet<(u32, u32)>>,
    ) {
        type RegDefs = HashMap<u32, HashSet<(u32, u32)>>; // reg -> set of (offset, reg) defs

        let mut out_block: HashMap<BlockId, RegDefs> = HashMap::new();
        let mut use_def: HashMap<(u32, u32), HashSet<(u32, u32)>> = HashMap::new();
        let mut def_use: HashMap<(u32, u32), HashSet<(u32, u32)>> = HashMap::new();

        let mut changed = true;
        while changed {
            changed = false;
            for bid in 0..cfg.blocks.len() {
                let block = &cfg.blocks[bid];
                let mut in_state: RegDefs = predecessors
                    .get(&bid)
                    .map(|preds| {
                        let mut merged: RegDefs = HashMap::new();
                        for &p in preds {
                            if let Some(out) = out_block.get(&p) {
                                for (reg, defs) in out {
                                    merged.entry(*reg).or_default().extend(defs.iter().copied());
                                }
                            }
                        }
                        merged
                    })
                    .unwrap_or_default();

                for &off in &block.instruction_offsets {
                    let (reads, writes) = match rw_map.get(&off) {
                        Some(rw) => rw.clone(),
                        None => continue,
                    };
                    for reg in &reads {
                        let key = (off, *reg);
                        let defs = in_state.get(reg).cloned().unwrap_or_default();
                        if !defs.is_empty() {
                            use_def.insert(key, defs.clone());
                            for &d in &defs {
                                def_use.entry(d).or_default().insert(key);
                            }
                        }
                    }
                    for reg in &writes {
                        in_state.insert(*reg, HashSet::from([(off, *reg)]));
                    }
                }

                let prev = out_block.insert(bid, in_state);
                if prev.as_ref() != out_block.get(&bid) {
                    changed = true;
                }
            }
        }

        (use_def, def_use)
    }

    /// All (offset, reg) that **read** the value defined at (seed_offset, seed_reg),
    /// and all (offset, reg) that **write** that same value (seed + transitive copies).
    /// Propagation: the value flows through moves (v_dst = v_src), so all such defs and
    /// all their uses are included.
    pub fn value_flow_from_seed(&self, seed_offset: u32, seed_reg: u32) -> ValueFlowResult {
        let seed = (seed_offset, seed_reg);

        // Writes: seed + all defs that get their value from the seed (transitively).
        // E.g. move v1,v0 then move v2,v1 → (0,v0), (2,v1), (4,v2) all write the same value.
        let mut written_set: HashSet<(u32, u32)> = HashSet::from([seed]);
        let mut changed = true;
        while changed {
            changed = false;
            for (&(off, _use_reg), sources) in &self.use_def {
                if sources.is_disjoint(&written_set) {
                    continue;
                }
                // At instruction off we read a reg defined by something in written_set (value flows here).
                if let Some((_read_regs, write_regs)) = self.rw_map.get(&off) {
                    for &reg in write_regs {
                        if written_set.insert((off, reg)) {
                            changed = true;
                        }
                    }
                }
            }
        }
        let writes: Vec<(u32, u32)> = written_set.into_iter().collect();

        // Reads: all (offset, reg) that use the value (direct use of seed or use of any copy).
        let mut read_set: HashSet<(u32, u32)> = HashSet::new();
        for &def in &writes {
            if let Some(uses) = self.def_use.get(&def) {
                read_set.extend(uses.iter().copied());
            }
        }
        let reads: Vec<(u32, u32)> = read_set.into_iter().collect();

        ValueFlowResult { reads, writes }
    }

    /// Def-use: for def (offset, reg), return all (offset_use, reg_use) that use that value.
    pub fn def_use(&self, def_offset: u32, def_reg: u32) -> Vec<(u32, u32)> {
        self.def_use
            .get(&(def_offset, def_reg))
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Use-def: for use (offset, reg), return all (offset_def, reg_def) that can define it.
    pub fn use_def(&self, use_offset: u32, use_reg: u32) -> Vec<(u32, u32)> {
        self.use_def
            .get(&(use_offset, use_reg))
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dex_bytecode::Instruction;

    fn condition_no_branch(_: &Instruction) -> String {
        "true".into()
    }

    /// Linear bytecode: const/4 v0,0 (0..2); move v1,v0 (2..4); return v1 (4..6).
    fn linear_const_move_return_bytecode() -> Vec<u8> {
        vec![
            0x12, 0x00, // const/4 v0, 0  -> offset 0
            0x01, 0x01, // move v1, v0    -> offset 2
            0x0f, 0x01, // return v1      -> offset 4
        ]
    }

    /// Bytecode with transitive copy: const/4 v0,0; move v1,v0; move v2,v1; return v2.
    fn transitive_copy_bytecode() -> Vec<u8> {
        vec![
            0x12, 0x00, // const/4 v0, 0  -> 0
            0x01, 0x01, // move v1, v0    -> 2
            0x01, 0x12, // move v2, v1    -> 4
            0x0f, 0x02, // return v2      -> 6
        ]
    }

    /// Bytecode: const/4 v0,0; move v1,v0; invoke-virtual {v1}, method@0 (value passed to function).
    fn const_move_invoke_bytecode() -> Vec<u8> {
        vec![
            0x12, 0x00, // const/4 v0, 0  -> 0
            0x01, 0x01, // move v1, v0    -> 2
            0x6e, 0x10, 0x00, 0x00, 0x01, 0x00, // invoke-virtual {v1}, method@0  -> 4
        ]
    }

    /// Bytecode: one value flows to BOTH invoke (param) and return.
    /// const v0; move v1,v0; invoke v1; move v2,v0; return v2.
    fn param_and_return_bytecode() -> Vec<u8> {
        vec![
            0x12, 0x00, // const/4 v0, 0  -> 0
            0x01, 0x01, // move v1, v0    -> 2
            0x6e, 0x10, 0x00, 0x00, 0x01, 0x00, // invoke-virtual {v1}  -> 4
            0x01, 0x02, // move v2, v0    -> 10
            0x0f, 0x02, // return v2      -> 12
        ]
    }

    /// Bytecode: value returned from callee (move-result) flows to return.
    /// invoke ...; move-result v0; move v1,v0; return v1.
    fn move_result_to_return_bytecode() -> Vec<u8> {
        vec![
            0x6e, 0x10, 0x00, 0x00, 0x00, 0x00, // invoke-virtual {v0}  -> 0
            0x0a, 0x00, // move-result v0  -> 6
            0x01, 0x01, // move v1, v0    -> 8
            0x0f, 0x01, // return v1      -> 10
        ]
    }

    fn build_cfg_and_rw(bytecode: &[u8]) -> (MethodCfg, InstructionRwMap) {
        let instructions = dex_bytecode::decode_all(bytecode, 0).expect("decode");
        let base_off = 0u32;
        let rw_map = build_instruction_rw_map(&instructions, base_off, |s| s.to_string());
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_no_branch);
        (cfg, rw_map)
    }

    /// Get instruction offsets in order from the entry block.
    fn entry_block_offsets(cfg: &MethodCfg) -> Vec<u32> {
        cfg.blocks[cfg.entry].instruction_offsets.clone()
    }

    #[test]
    fn value_flow_seed_const_move_return() {
        let bytecode = linear_const_move_return_bytecode();
        let (cfg, rw_map) = build_cfg_and_rw(&bytecode);
        let offsets = entry_block_offsets(&cfg);
        assert!(offsets.len() >= 3, "expected at least 3 instructions");

        let off_const = offsets[0];
        let off_move = offsets[1];
        let off_return = offsets[2];

        let analysis = ValueFlowAnalysis::new(&cfg, &rw_map);
        let result = analysis.value_flow_from_seed(off_const, 0);

        let writes_set: HashSet<_> = result.writes.iter().copied().collect();
        assert!(
            writes_set.contains(&(off_const, 0)),
            "writes should contain seed, got {:?}",
            result.writes
        );
        assert!(
            writes_set.contains(&(off_move, 1)),
            "writes should contain copy, got {:?}",
            result.writes
        );
        assert_eq!(
            result.writes.len(),
            2,
            "expected 2 writes, got {:?}",
            result.writes
        );

        let reads_set: HashSet<_> = result.reads.iter().copied().collect();
        assert!(reads_set.contains(&(off_move, 0)));
        assert!(reads_set.contains(&(off_return, 1)));
        assert_eq!(
            result.reads.len(),
            2,
            "expected 2 reads, got {:?}",
            result.reads
        );
    }

    #[test]
    fn value_flow_transitive_copy() {
        let bytecode = transitive_copy_bytecode();
        let (cfg, rw_map) = build_cfg_and_rw(&bytecode);
        let offsets = entry_block_offsets(&cfg);
        assert!(offsets.len() >= 4, "expected at least 4 instructions");

        let off_const = offsets[0];
        let off_move1 = offsets[1];
        let off_move2 = offsets[2];
        let off_return = offsets[3];

        let analysis = ValueFlowAnalysis::new(&cfg, &rw_map);
        let result = analysis.value_flow_from_seed(off_const, 0);

        let writes_set: HashSet<_> = result.writes.iter().copied().collect();
        assert!(writes_set.contains(&(off_const, 0)));
        assert!(writes_set.contains(&(off_move1, 1)));
        assert!(writes_set.contains(&(off_move2, 2)));
        assert_eq!(
            result.writes.len(),
            3,
            "expected 3 writes, got {:?}",
            result.writes
        );

        let reads_set: HashSet<_> = result.reads.iter().copied().collect();
        assert!(reads_set.contains(&(off_move1, 0)));
        assert!(reads_set.contains(&(off_move2, 1)));
        assert!(reads_set.contains(&(off_return, 2)));
        assert_eq!(
            result.reads.len(),
            3,
            "expected 3 reads, got {:?}",
            result.reads
        );
    }

    #[test]
    fn def_use_and_use_def() {
        let bytecode = linear_const_move_return_bytecode();
        let (cfg, rw_map) = build_cfg_and_rw(&bytecode);
        let offsets = entry_block_offsets(&cfg);

        let off_const = offsets[0];
        let off_move = offsets[1];
        let off_return = offsets[2];

        let analysis = ValueFlowAnalysis::new(&cfg, &rw_map);

        let uses = analysis.def_use(off_const, 0);
        assert!(
            uses.contains(&(off_move, 0)),
            "def (const) should reach use at move, got {:?}",
            uses
        );

        let defs = analysis.use_def(off_return, 1);
        assert!(
            defs.contains(&(off_move, 1)),
            "use (return v1) should be reached by move v1, got {:?}",
            defs
        );
    }

    /// Tainting tracks value when it is **returned**: seed at const, value flows to move then to return.
    #[test]
    fn value_flow_tracks_return_use() {
        let bytecode = linear_const_move_return_bytecode();
        let (cfg, rw_map) = build_cfg_and_rw(&bytecode);
        let offsets = entry_block_offsets(&cfg);
        assert!(offsets.len() >= 3);
        let off_return = offsets[2];

        let analysis = ValueFlowAnalysis::new(&cfg, &rw_map);
        let result = analysis.value_flow_from_seed(offsets[0], 0);

        // The value is read at return v1 (v1 holds the tracked value).
        assert!(
            result
                .reads
                .iter()
                .any(|&(off, reg)| off == off_return && reg == 1),
            "reads should contain return (offset, v1), got reads={:?}",
            result.reads
        );
    }

    /// Tainting tracks value when it is **passed to a function**: seed at const, value flows to move then to invoke arg.
    #[test]
    fn value_flow_tracks_value_passed_to_invoke() {
        let bytecode = const_move_invoke_bytecode();
        let (cfg, rw_map) = build_cfg_and_rw(&bytecode);
        let offsets = entry_block_offsets(&cfg);
        assert!(offsets.len() >= 3, "expected const, move, invoke");
        let off_invoke = offsets[2];

        let analysis = ValueFlowAnalysis::new(&cfg, &rw_map);
        let result = analysis.value_flow_from_seed(offsets[0], 0);

        // Writes: const v0, move v1.
        let writes_set: HashSet<_> = result.writes.iter().copied().collect();
        assert!(
            writes_set.contains(&(offsets[0], 0)),
            "writes should contain const v0"
        );
        assert!(
            writes_set.contains(&(offsets[1], 1)),
            "writes should contain move v1"
        );

        // The value is read at invoke (v1 passed as argument).
        assert!(
            result.reads.iter().any(|&(off, reg)| off == off_invoke && reg == 1),
            "reads should contain invoke (offset, v1) when value is passed to function, got reads={:?}",
            result.reads
        );
    }

    /// Complex: one value (e.g. param or sensitive source) flows to BOTH a function call (param) and return.
    /// const v0; move v1,v0; invoke v1 (param); move v2,v0; return v2.
    #[test]
    fn value_flow_complex_param_and_return() {
        let bytecode = param_and_return_bytecode();
        let (cfg, rw_map) = build_cfg_and_rw(&bytecode);
        let offsets = entry_block_offsets(&cfg);
        assert!(
            offsets.len() >= 5,
            "expected const, move, invoke, move, return"
        );
        let off_invoke = offsets[2];
        let off_return = offsets[4];

        let analysis = ValueFlowAnalysis::new(&cfg, &rw_map);
        let result = analysis.value_flow_from_seed(offsets[0], 0);

        let reads_set: HashSet<_> = result.reads.iter().copied().collect();
        let writes_set: HashSet<_> = result.writes.iter().copied().collect();

        // Value flows to invoke (passed as param).
        assert!(
            reads_set.contains(&(off_invoke, 1)),
            "reads should contain invoke (value passed as param), got {:?}",
            result.reads
        );
        // Value also flows to return.
        assert!(
            reads_set.contains(&(off_return, 2)),
            "reads should contain return (value returned), got {:?}",
            result.reads
        );
        // Writes: seed v0, copy v1 (for invoke), copy v2 (for return).
        assert!(writes_set.contains(&(offsets[0], 0)));
        assert!(writes_set.contains(&(offsets[1], 1)));
        assert!(writes_set.contains(&(offsets[3], 2)));
    }

    /// Complex: value returned from a callee (move-result) propagates to this method's return.
    /// invoke ...; move-result v0; move v1,v0; return v1.
    #[test]
    fn value_flow_complex_return_from_callee_to_return() {
        let bytecode = move_result_to_return_bytecode();
        let (cfg, rw_map) = build_cfg_and_rw(&bytecode);
        let offsets = entry_block_offsets(&cfg);
        assert!(
            offsets.len() >= 4,
            "expected invoke, move-result, move, return"
        );
        let off_move_result = offsets[1];
        let off_return = offsets[3];

        let analysis = ValueFlowAnalysis::new(&cfg, &rw_map);
        // Seed: value received from callee (move-result v0).
        let result = analysis.value_flow_from_seed(off_move_result, 0);

        let reads_set: HashSet<_> = result.reads.iter().copied().collect();
        let writes_set: HashSet<_> = result.writes.iter().copied().collect();

        // Value from callee flows through move to return.
        assert!(
            writes_set.contains(&(off_move_result, 0)),
            "writes should contain move-result (value from callee)"
        );
        assert!(
            writes_set.contains(&(offsets[2], 1)),
            "writes should contain move v1 (copy of callee return)"
        );
        assert!(
            reads_set.contains(&(off_return, 1)),
            "reads should contain return v1 (value propagated from callee return)"
        );
    }

    #[test]
    fn exceptional_predecessor_preserves_definition_in_handler() {
        use super::super::cfg::{BlockEnd, CfgBlock};

        let cfg = MethodCfg {
            blocks: vec![
                CfgBlock {
                    start_offset: 0,
                    end_offset: 2,
                    end: BlockEnd::Exit,
                    instruction_offsets: vec![0],
                },
                CfgBlock {
                    start_offset: 10,
                    end_offset: 12,
                    end: BlockEnd::Exit,
                    instruction_offsets: vec![10],
                },
            ],
            block_by_start: HashMap::from([(0, 0), (10, 1)]),
            loop_headers: HashSet::new(),
            entry: 0,
            folded_const_offsets: HashSet::new(),
        };
        let rw_map = HashMap::from([(0, (vec![], vec![0])), (10, (vec![0], vec![]))]);

        let without_exception = ValueFlowAnalysis::new(&cfg, &rw_map);
        assert!(without_exception
            .value_flow_from_seed(0, 0)
            .reads
            .is_empty());

        let with_exception =
            ValueFlowAnalysis::new_with_exceptional_edges(&cfg, &rw_map, &[(0, 1)]);
        assert!(with_exception
            .value_flow_from_seed(0, 0)
            .reads
            .contains(&(10, 0)));
    }

    #[test]
    fn incoming_parameter_has_synthetic_entry_definition() {
        let bytecode = vec![
            0x01, 0x01, // move v1, v0
            0x0f, 0x01, // return v1
        ];
        let (cfg, rw_map) = build_cfg_and_rw(&bytecode);
        let owned = ValueFlowAnalysisOwned {
            cfg,
            rw_map,
            exceptional_edges: vec![],
            api_return_sources: vec![],
            invoke_method_map: HashMap::new(),
            insn_at: HashMap::new(),
            registers_size: 2,
            ins_size: 1,
        };
        let flow = owned.value_flow_from_entry_reg(0);
        assert!(flow.reads.contains(&(0, 0)));
        assert!(flow.writes.contains(&(0, 1)));
        assert!(flow.reads.contains(&(2, 1)));
    }
}
