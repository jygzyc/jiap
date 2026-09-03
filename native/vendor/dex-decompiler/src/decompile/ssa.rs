//! CFG-aware SSA construction with φ-nodes (Cytron et al.).
//!
//! Used for method-level renaming. φ-nodes are stripped before Java emission;
//! versions linked by a φ share one display name via [`phi_canonical_map`].

use std::collections::{HashMap, HashSet};

use super::cfg::{BlockId, MethodCfg};
use super::graph::Graph;
use super::ir::{Expr as IrExpr, Stmt as IrStmt, VarId};

/// Build a string-keyed graph from a [`MethodCfg`] for dominator computation.
fn cfg_to_graph(cfg: &MethodCfg) -> Graph {
    let mut g = Graph::with_entry(&cfg.entry.to_string());
    for bid in 0..cfg.blocks.len() {
        g.add_node(&bid.to_string());
    }
    for (from, to) in cfg.successor_edges() {
        g.add_edge(&from.to_string(), &to.to_string());
    }
    g
}

fn predecessors(cfg: &MethodCfg) -> HashMap<BlockId, Vec<BlockId>> {
    let mut preds: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    for (from, to) in cfg.successor_edges() {
        preds.entry(to).or_default().push(from);
    }
    preds
}

/// Dominance frontiers: DF(b) = blocks where b's dominance "stops".
fn dominance_frontiers(cfg: &MethodCfg) -> HashMap<BlockId, HashSet<BlockId>> {
    let g = cfg_to_graph(cfg);
    let idom = g.immediate_dominators();
    let preds = predecessors(cfg);
    let mut df: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();
    for bid in 0..cfg.blocks.len() {
        df.insert(bid, HashSet::new());
    }
    for (bid, pred_list) in &preds {
        if pred_list.len() < 2 {
            continue;
        }
        for &p in pred_list {
            let mut runner = p;
            let idom_b = idom
                .get(&bid.to_string())
                .and_then(|o| o.as_ref())
                .and_then(|s| s.parse::<BlockId>().ok());
            while Some(runner) != idom_b {
                df.entry(runner).or_default().insert(*bid);
                let idom_runner = idom
                    .get(&runner.to_string())
                    .and_then(|o| o.as_ref())
                    .and_then(|s| s.parse::<BlockId>().ok());
                match idom_runner {
                    Some(d) if d != runner => runner = d,
                    _ => break,
                }
            }
        }
    }
    df
}

fn defs_in_block(stmts: &[IrStmt]) -> HashSet<u32> {
    let mut defs = HashSet::new();
    for s in stmts {
        if let IrStmt::Assign { dst, .. } | IrStmt::Phi { dst, .. } = s {
            defs.insert(dst.reg);
        }
    }
    defs
}

/// Insert φ-nodes and rename variables (Cytron algorithm).
///
/// `blocks` maps each CFG block to its IR (pre-SSA). After this call, statements
/// use SSA `VarId` versions and φ-nodes appear at the start of join blocks.
pub fn construct_ssa(cfg: &MethodCfg, blocks: &mut HashMap<BlockId, Vec<IrStmt>>) {
    if cfg.block_count() == 0 {
        return;
    }
    let df = dominance_frontiers(cfg);
    let preds = predecessors(cfg);

    // Registers defined somewhere.
    let mut all_regs: HashSet<u32> = HashSet::new();
    let mut def_sites: HashMap<u32, HashSet<BlockId>> = HashMap::new();
    for (&bid, stmts) in blocks.iter() {
        for r in defs_in_block(stmts) {
            all_regs.insert(r);
            def_sites.entry(r).or_default().insert(bid);
        }
    }

    // Insert phis.
    for &reg in &all_regs {
        let mut work: Vec<BlockId> = def_sites
            .get(&reg)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut has_phi: HashSet<BlockId> = HashSet::new();
        while let Some(b) = work.pop() {
            for &d in df.get(&b).into_iter().flatten() {
                if has_phi.insert(d) {
                    let incomings: Vec<(BlockId, VarId)> = preds
                        .get(&d)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|p| (p, VarId::new(reg, 0)))
                        .collect();
                    if incomings.len() >= 2 {
                        let phi = IrStmt::Phi {
                            dst: VarId::new(reg, 0),
                            incomings,
                        };
                        blocks.entry(d).or_default().insert(0, phi);
                        work.push(d);
                    }
                }
            }
        }
    }

    // Rename (stack-based).
    let mut next_ver: HashMap<u32, u32> = HashMap::new();
    let mut stacks: HashMap<u32, Vec<u32>> = HashMap::new();
    for &r in &all_regs {
        next_ver.insert(r, 0);
        stacks.insert(r, vec![0]); // version 0 = params / undefined
    }

    fn rename_expr(expr: IrExpr, stacks: &HashMap<u32, Vec<u32>>) -> IrExpr {
        match expr {
            IrExpr::Var(v) => {
                let ver = stacks
                    .get(&v.reg)
                    .and_then(|s| s.last())
                    .copied()
                    .unwrap_or(0);
                IrExpr::Var(VarId::new(v.reg, ver))
            }
            IrExpr::Call { target, args } => IrExpr::Call {
                target: rename_text(&target, stacks),
                args: rename_text(&args, stacks),
            },
            IrExpr::PendingResult => IrExpr::PendingResult,
            IrExpr::Raw(s) => IrExpr::Raw(rename_text(&s, stacks)),
        }
    }

    fn rename_text(s: &str, stacks: &HashMap<u32, Vec<u32>>) -> String {
        // Reuse pass.rs style via a local map of current versions.
        let cur: HashMap<u32, u32> = stacks
            .iter()
            .filter_map(|(r, st)| st.last().copied().map(|v| (*r, v)))
            .collect();
        super::pass::rename_vars_in_text_public(s, &cur)
    }

    fn new_name(
        reg: u32,
        next_ver: &mut HashMap<u32, u32>,
        stacks: &mut HashMap<u32, Vec<u32>>,
    ) -> VarId {
        let v = next_ver.entry(reg).or_insert(0);
        *v += 1;
        let ver = *v;
        stacks.entry(reg).or_default().push(ver);
        VarId::new(reg, ver)
    }

    // Dominator tree children.
    let g = cfg_to_graph(cfg);
    let idom = g.immediate_dominators();
    let mut children: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    for bid in 0..cfg.blocks.len() {
        if let Some(Some(dom)) = idom.get(&bid.to_string()) {
            if let Ok(d) = dom.parse::<BlockId>() {
                children.entry(d).or_default().push(bid);
            }
        }
    }

    fn rename_block(
        bid: BlockId,
        cfg: &MethodCfg,
        blocks: &mut HashMap<BlockId, Vec<IrStmt>>,
        preds: &HashMap<BlockId, Vec<BlockId>>,
        children: &HashMap<BlockId, Vec<BlockId>>,
        next_ver: &mut HashMap<u32, u32>,
        stacks: &mut HashMap<u32, Vec<u32>>,
    ) {
        let mut pushed: Vec<u32> = Vec::new();
        if let Some(stmts) = blocks.get_mut(&bid) {
            for stmt in stmts.iter_mut() {
                match stmt {
                    IrStmt::Phi { dst, .. } => {
                        let nv = new_name(dst.reg, next_ver, stacks);
                        *dst = nv;
                        pushed.push(dst.reg);
                    }
                    IrStmt::Assign { dst, rhs, comment } => {
                        *rhs = rename_expr(rhs.clone(), stacks);
                        let nv = new_name(dst.reg, next_ver, stacks);
                        *dst = nv;
                        pushed.push(dst.reg);
                        let _ = comment;
                    }
                    IrStmt::Expr { expr, .. } => {
                        *expr = rename_expr(expr.clone(), stacks);
                    }
                    IrStmt::Return { value, .. } => {
                        if let Some(v) = value {
                            *v = rename_expr(v.clone(), stacks);
                        }
                    }
                    IrStmt::Raw(s) => {
                        *s = rename_text(s, stacks);
                    }
                }
            }
        }

        // Fill φ incomings on successors.
        for (from, succ) in cfg.successor_edges().into_iter().filter(|(f, _)| *f == bid) {
            let _ = from;
            if let Some(succ_stmts) = blocks.get_mut(&succ) {
                for stmt in succ_stmts.iter_mut() {
                    if let IrStmt::Phi { dst, incomings } = stmt {
                        for (pred, val) in incomings.iter_mut() {
                            if *pred == bid {
                                let ver = stacks
                                    .get(&dst.reg)
                                    .and_then(|s| s.last())
                                    .copied()
                                    .unwrap_or(0);
                                *val = VarId::new(dst.reg, ver);
                            }
                        }
                    }
                }
            }
        }

        for &child in children.get(&bid).into_iter().flatten() {
            rename_block(child, cfg, blocks, preds, children, next_ver, stacks);
        }

        // Pop stack for defs in this block.
        for reg in pushed {
            if let Some(st) = stacks.get_mut(&reg) {
                st.pop();
            }
        }
    }

    rename_block(
        cfg.entry,
        cfg,
        blocks,
        &preds,
        &children,
        &mut next_ver,
        &mut stacks,
    );
}

/// Map each SSA value to a canonical representative (φ webs ∪ identity).
pub fn phi_canonical_map(blocks: &HashMap<BlockId, Vec<IrStmt>>) -> HashMap<VarId, VarId> {
    let mut parent: HashMap<VarId, VarId> = HashMap::new();

    fn find(parent: &mut HashMap<VarId, VarId>, x: VarId) -> VarId {
        let p = parent.get(&x).copied().unwrap_or(x);
        if p != x {
            let root = find(parent, p);
            parent.insert(x, root);
            root
        } else {
            x
        }
    }
    fn union(parent: &mut HashMap<VarId, VarId>, a: VarId, b: VarId) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent.insert(rb, ra);
        }
    }

    for stmts in blocks.values() {
        for s in stmts {
            if let IrStmt::Phi { dst, incomings } = s {
                parent.entry(*dst).or_insert(*dst);
                for (_, v) in incomings {
                    parent.entry(*v).or_insert(*v);
                    union(&mut parent, *dst, *v);
                }
            }
        }
    }

    let keys: Vec<VarId> = parent.keys().copied().collect();
    let mut out = HashMap::new();
    for k in keys {
        out.insert(k, find(&mut parent, k));
    }
    out
}

/// Drop φ-nodes (Java cannot express them). Call after [`phi_canonical_map`].
pub fn strip_phis(blocks: &mut HashMap<BlockId, Vec<IrStmt>>) {
    for stmts in blocks.values_mut() {
        stmts.retain(|s| !matches!(s, IrStmt::Phi { .. }));
    }
}

/// Registers that receive at least one φ (useful for shared naming in restructure mode).
pub fn phi_registers(blocks: &HashMap<BlockId, Vec<IrStmt>>) -> HashSet<u32> {
    let mut regs = HashSet::new();
    for stmts in blocks.values() {
        for s in stmts {
            if let IrStmt::Phi { dst, .. } = s {
                regs.insert(dst.reg);
            }
        }
    }
    regs
}

/// Apply φ-web canonicalization to a name map (share one Java name across versions).
pub fn apply_canonical_names(
    name_map: &mut HashMap<VarId, String>,
    canonical: &HashMap<VarId, VarId>,
) {
    // Ensure each web root has a name (prefer any existing member's name).
    let snapshot: Vec<(VarId, String)> = name_map.iter().map(|(k, v)| (*k, v.clone())).collect();
    for (vid, name) in &snapshot {
        if let Some(canon) = canonical.get(vid) {
            name_map.entry(*canon).or_insert_with(|| name.clone());
        }
    }
    let canon_names: HashMap<VarId, String> = name_map
        .iter()
        .filter_map(|(v, n)| {
            let c = canonical.get(v).copied().unwrap_or(*v);
            if c == *v {
                Some((*v, n.clone()))
            } else {
                None
            }
        })
        .collect();
    for (vid, name) in name_map.iter_mut() {
        let c = canonical.get(vid).copied().unwrap_or(*vid);
        if let Some(cn) = canon_names.get(&c) {
            *name = cn.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile::ir::{Expr, Stmt, VarId};

    #[test]
    fn phi_canonical_unions_incomings() {
        let mut blocks = HashMap::new();
        blocks.insert(
            0,
            vec![Stmt::Phi {
                dst: VarId::new(0, 3),
                incomings: vec![(1, VarId::new(0, 1)), (2, VarId::new(0, 2))],
            }],
        );
        let canon = phi_canonical_map(&blocks);
        assert_eq!(canon.get(&VarId::new(0, 1)), canon.get(&VarId::new(0, 3)));
        assert_eq!(canon.get(&VarId::new(0, 2)), canon.get(&VarId::new(0, 3)));
    }

    #[test]
    fn strip_phis_removes_nodes() {
        let mut blocks = HashMap::new();
        blocks.insert(
            0,
            vec![
                Stmt::Phi {
                    dst: VarId::new(0, 1),
                    incomings: vec![],
                },
                Stmt::Assign {
                    dst: VarId::new(1, 1),
                    rhs: Expr::Raw("0".into()),
                    comment: None,
                },
            ],
        );
        strip_phis(&mut blocks);
        assert_eq!(blocks[&0].len(), 1);
    }

    #[test]
    fn construct_ssa_diamond_inserts_phi() {
        use crate::decompile::cfg::{BlockEnd, CfgBlock, MethodCfg};
        use std::collections::HashSet;

        // Diamond: 0 → 1, 0 → 2, 1 → 3, 2 → 3
        let cfg = MethodCfg {
            blocks: vec![
                CfgBlock {
                    start_offset: 0,
                    end_offset: 2,
                    end: BlockEnd::Conditional {
                        condition: "c".into(),
                        branch_target: 1,
                        fall_through: 2,
                    },
                    instruction_offsets: vec![],
                },
                CfgBlock {
                    start_offset: 2,
                    end_offset: 4,
                    end: BlockEnd::Goto(3),
                    instruction_offsets: vec![],
                },
                CfgBlock {
                    start_offset: 4,
                    end_offset: 6,
                    end: BlockEnd::Goto(3),
                    instruction_offsets: vec![],
                },
                CfgBlock {
                    start_offset: 6,
                    end_offset: 8,
                    end: BlockEnd::Exit,
                    instruction_offsets: vec![],
                },
            ],
            block_by_start: HashMap::new(),
            loop_headers: HashSet::new(),
            entry: 0,
            folded_const_offsets: HashSet::new(),
        };
        let mut blocks = HashMap::new();
        blocks.insert(0, vec![]);
        blocks.insert(
            1,
            vec![Stmt::Assign {
                dst: VarId::new(0, 0),
                rhs: Expr::Raw("1".into()),
                comment: None,
            }],
        );
        blocks.insert(
            2,
            vec![Stmt::Assign {
                dst: VarId::new(0, 0),
                rhs: Expr::Raw("2".into()),
                comment: None,
            }],
        );
        blocks.insert(
            3,
            vec![Stmt::Return {
                value: Some(Expr::Var(VarId::new(0, 0))),
                comment: None,
            }],
        );
        construct_ssa(&cfg, &mut blocks);
        let has_phi = blocks[&3]
            .iter()
            .any(|s| matches!(s, Stmt::Phi { dst, .. } if dst.reg == 0));
        assert!(has_phi, "join block should have φ for v0");
        let regs = phi_registers(&blocks);
        assert!(regs.contains(&0));
    }
}
