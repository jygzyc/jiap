//! Region maker: structured control flow (SESE-style regions) from CFG.
//!
//! Builds a region tree (Block, Seq, If, Loop) from MethodCfg, then emission
//! walks the tree to produce Java if/else and while.

use crate::decompile::cfg::{BlockEnd, BlockId, MethodCfg};
use std::collections::HashSet;

/// Single-entry single-exit style region for structured emission.
#[derive(Debug, Clone)]
pub enum Region {
    /// A single basic block (by id).
    Block(BlockId),
    /// Sequential list of regions (no branching between them).
    Seq(Vec<Region>),
    /// Conditional: if (condition) then_branch else else_branch.
    If {
        condition: String,
        then_branch: Box<Region>,
        else_branch: Box<Region>,
    },
    /// Loop: while (true) { body }. Header block is the loop header; body includes it.
    Loop { header: BlockId, body: Box<Region> },
    /// Switch: switch (condition) { case v: case_body ... default: default_body }.
    Switch {
        condition: String,
        cases: Vec<(i32, Box<Region>)>,
        default: Box<Region>,
    },
}

/// Returns true if the region has no substantive content (empty seq or seq of empty regions).
/// Used to omit useless `} else { }` in emission.
pub fn region_is_empty(region: &Region) -> bool {
    match region {
        Region::Seq(children) => children.is_empty() || children.iter().all(region_is_empty),
        Region::Block(_) | Region::If { .. } | Region::Loop { .. } | Region::Switch { .. } => false,
    }
}

/// Like region_is_empty but uses the CFG to treat a Block as empty when that block has no
/// substantive instructions (or only a trailing goto). This allows flipping
/// `if (cond) { } else { body }` / goto-only skip targets to `if (!cond) { body }`.
pub fn region_is_empty_with_cfg(region: &Region, cfg: &MethodCfg) -> bool {
    match region {
        Region::Block(bid) => cfg
            .blocks
            .get(*bid)
            .map(block_is_effectively_empty)
            .unwrap_or(false),
        Region::Seq(children) => {
            children.is_empty() || children.iter().all(|c| region_is_empty_with_cfg(c, cfg))
        }
        Region::If { .. } | Region::Loop { .. } | Region::Switch { .. } => false,
    }
}

/// True when a CFG block has no real work: no instructions, or only the terminating goto.
fn block_is_effectively_empty(b: &crate::decompile::cfg::CfgBlock) -> bool {
    if b.instruction_offsets.is_empty() {
        return true;
    }
    // `if-eqz` skip targets are often a lone `goto` to the join — treat as empty so we can
    // emit `if (x != null) { body }` instead of `if (x == null) { goto } else { body }`.
    matches!(b.end, BlockEnd::Goto(_)) && b.instruction_offsets.len() <= 1
}

/// Returns true if the region contains an If (at any depth).
pub fn region_contains_if(region: &Region) -> bool {
    match region {
        Region::If { .. } | Region::Switch { .. } => true,
        Region::Seq(children) => children.iter().any(region_contains_if),
        Region::Loop { body, .. } => region_contains_if(body),
        Region::Block(_) => false,
    }
}

/// Returns true if the region contains a Loop (at any depth).
/// Used to prefer "if (cond) { short/return } else { loop }" by swapping when then has loop and else doesn't.
pub fn region_contains_loop(region: &Region) -> bool {
    match region {
        Region::Block(_) => false,
        Region::Seq(children) => children.iter().any(region_contains_loop),
        Region::If {
            then_branch,
            else_branch,
            ..
        } => region_contains_loop(then_branch) || region_contains_loop(else_branch),
        Region::Loop { .. } => true,
        Region::Switch { cases, default, .. } => {
            cases.iter().any(|(_, r)| region_contains_loop(r)) || region_contains_loop(default)
        }
    }
}

/// Strict break pattern: `Seq([Block(header), If { … }])` only.
pub fn loop_body_break_pattern(body: &Region, header: BlockId) -> Option<(&str, &Region, &Region)> {
    let Region::Seq(children) = body else {
        return None;
    };
    if children.len() < 2 {
        return None;
    }
    match (&children[0], &children[1]) {
        (
            Region::Block(bid),
            Region::If {
                condition,
                then_branch,
                else_branch,
            },
        ) if *bid == header => {
            // Empty-then + non-empty-else is do-while / while(true) territory, not break-while.
            if region_is_empty(then_branch) && !region_is_empty(else_branch) {
                return None;
            }
            Some((condition.as_str(), else_branch, then_branch))
        }
        _ => None,
    }
}

/// Trailing exit-if break pattern (handles nested `Seq` wrappers). Used for mis-peeled
/// empty do-while tails (e.g. sum2d inner index loop) — not for general loop emission.
pub fn loop_body_break_pattern_trailing(
    body: &Region,
    header: BlockId,
) -> Option<(&str, &Region, &Region)> {
    let Region::Seq(children) = body else {
        return None;
    };
    if children.is_empty() {
        return None;
    }
    match children.first()? {
        Region::Block(bid) if *bid == header => {}
        _ => return None,
    }
    let suffix = &children[1..];
    let (cond, else_b, then_b) = trailing_exit_if(suffix)?;
    if let Some(middle) = prefix_before_trailing_if(suffix) {
        if region_contains_loop(&middle) {
            return None;
        }
    }
    if region_contains_loop(else_b) || region_contains_loop(then_b) {
        return None;
    }
    Some((cond, else_b, then_b))
}

fn trailing_exit_if(suffix: &[Region]) -> Option<(&str, &Region, &Region)> {
    if suffix.is_empty() {
        return None;
    }
    match suffix.last()? {
        Region::If {
            condition,
            then_branch,
            else_branch,
        } => Some((condition.as_str(), else_branch, then_branch)),
        Region::Seq(inner) => trailing_exit_if(inner),
        _ => None,
    }
}

/// Middle regions between loop header block and trailing exit-if (for emission inside the while body).
pub fn loop_body_break_middle(body: &Region, header: BlockId) -> Option<Region> {
    let Region::Seq(children) = body else {
        return None;
    };
    if children.is_empty() {
        return None;
    }
    match children.first()? {
        Region::Block(bid) if *bid == header => {}
        _ => return None,
    }
    prefix_before_trailing_if(&children[1..])
}

fn prefix_before_trailing_if(suffix: &[Region]) -> Option<Region> {
    if suffix.is_empty() {
        return Some(Region::Seq(vec![]));
    }
    match suffix.last()? {
        Region::If { .. } => {
            if suffix.len() == 1 {
                Some(Region::Seq(vec![]))
            } else {
                Some(Region::Seq(suffix[..suffix.len() - 1].to_vec()))
            }
        }
        Region::Seq(inner) => {
            let inner_prefix = prefix_before_trailing_if(inner)?;
            if suffix.len() == 1 {
                Some(inner_prefix)
            } else {
                let mut parts: Vec<Region> = suffix[..suffix.len() - 1].to_vec();
                if !region_is_empty(&inner_prefix) {
                    parts.push(inner_prefix);
                }
                Some(Region::Seq(parts))
            }
        }
        _ => None,
    }
}

/// Pretty-print a region tree with block ids (L0-2 debug helper).
pub fn format_region_debug(region: &Region, cfg: &MethodCfg) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    fn walk(r: &Region, cfg: &MethodCfg, d: usize, out: &mut String) {
        match r {
            Region::Block(bid) => {
                let end = cfg
                    .blocks
                    .get(*bid)
                    .map(|b| format!("{:?}", b.end))
                    .unwrap_or_default();
                let _ = writeln!(out, "{:indent$}Block({bid}) {end}", "", indent = d * 2);
            }
            Region::Seq(v) => {
                let _ = writeln!(out, "{:indent$}Seq({})", "", v.len(), indent = d * 2);
                for c in v {
                    walk(c, cfg, d + 1, out);
                }
            }
            Region::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let _ = writeln!(out, "{:indent$}If({condition})", "", indent = d * 2);
                walk(then_branch, cfg, d + 1, out);
                walk(else_branch, cfg, d + 1, out);
            }
            Region::Loop { header, body } => {
                let _ = writeln!(out, "{:indent$}Loop(header={header})", "", indent = d * 2);
                walk(body, cfg, d + 1, out);
            }
            Region::Switch {
                condition,
                cases,
                default,
            } => {
                let _ = writeln!(
                    out,
                    "{:indent$}Switch({condition}, {} cases)",
                    "",
                    cases.len(),
                    indent = d * 2
                );
                for (val, body) in cases {
                    let _ = writeln!(out, "{:indent$}case {val}:", "", indent = d * 2);
                    walk(body, cfg, d + 1, out);
                }
                let _ = writeln!(out, "{:indent$}default:", "", indent = d * 2);
                walk(default, cfg, d + 1, out);
            }
        }
    }
    walk(region, cfg, 0, &mut out);
    let mut ids = Vec::new();
    collect_block_ids(region, &mut ids);
    let _ = writeln!(out, "region blocks: {ids:?}");
    out
}

fn collect_block_ids(region: &Region, out: &mut Vec<BlockId>) {
    match region {
        Region::Block(bid) => out.push(*bid),
        Region::Seq(v) => v.iter().for_each(|c| collect_block_ids(c, out)),
        Region::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_block_ids(then_branch, out);
            collect_block_ids(else_branch, out);
        }
        Region::Loop { body, .. } => collect_block_ids(body, out),
        Region::Switch { cases, default, .. } => {
            cases.iter().for_each(|(_, r)| collect_block_ids(r, out));
            collect_block_ids(default, out);
        }
    }
}

/// Exit tail after a bogus empty do-while — real loop body (assignments), not a lone return.
pub fn region_has_non_return_work(region: &Region, cfg: &MethodCfg) -> bool {
    match region {
        Region::Seq(children) => children.iter().any(|c| region_has_non_return_work(c, cfg)),
        Region::Block(bid) => cfg.blocks.get(*bid).is_some_and(|b| {
            if b.instruction_offsets.len() > 1 {
                return true;
            }
            !matches!(b.end, crate::decompile::cfg::BlockEnd::Exit)
        }),
        Region::If {
            then_branch,
            else_branch,
            ..
        } => {
            region_has_non_return_work(then_branch, cfg)
                || region_has_non_return_work(else_branch, cfg)
        }
        Region::Loop { body, .. } => region_has_non_return_work(body, cfg),
        Region::Switch { cases, default, .. } => {
            cases
                .iter()
                .any(|(_, r)| region_has_non_return_work(r, cfg))
                || region_has_non_return_work(default, cfg)
        }
    }
}

/// True when a peeled do-while body contains more than the loop header block alone.
pub fn loop_has_substantial_body(do_body: &Region, header: BlockId, cfg: &MethodCfg) -> bool {
    match do_body {
        Region::Seq(children) => children.iter().any(|c| match c {
            Region::Block(b) => *b != header && !region_is_empty_with_cfg(c, cfg),
            other => !region_is_empty_with_cfg(other, cfg),
        }),
        other => !region_is_empty_with_cfg(other, cfg),
    }
}

/// First exit block for loop emission `break` wiring (L2-2).
pub fn loop_exit_break_target(body: &Region, header: BlockId, cfg: &MethodCfg) -> Option<BlockId> {
    if let Some((_, _, then_branch)) = loop_body_break_pattern(body, header) {
        if !region_is_empty(then_branch) {
            return first_block(then_branch);
        }
    }
    cfg.loop_exit_targets(header).iter().copied().min()
}

fn trim_trailing_exit_if_suffix(suffix: Region) -> Region {
    match suffix {
        Region::Seq(ref children) if children.is_empty() => Region::Seq(vec![]),
        Region::Seq(children) => {
            let last_idx = children.len() - 1;
            match &children[last_idx] {
                Region::If {
                    condition,
                    else_branch,
                    ..
                } => {
                    let mut new_children = children[..last_idx].to_vec();
                    new_children.push(Region::If {
                        condition: condition.clone(),
                        then_branch: Box::new(Region::Seq(vec![])),
                        else_branch: else_branch.clone(),
                    });
                    Region::Seq(new_children)
                }
                Region::Seq(_) => {
                    let mut new_children = children.clone();
                    new_children[last_idx] =
                        trim_trailing_exit_if_suffix(children[last_idx].clone());
                    Region::Seq(new_children)
                }
                _ => Region::Seq(children),
            }
        }
        Region::If {
            condition,
            else_branch,
            ..
        } => Region::If {
            condition: condition.clone(),
            then_branch: Box::new(Region::Seq(vec![])),
            else_branch: else_branch.clone(),
        },
        other => other,
    }
}

fn trim_trailing_exit_if(body: &Region, header: BlockId) -> Option<Region> {
    let Region::Seq(children) = body else {
        return None;
    };
    if children.is_empty() {
        return None;
    }
    match children.first()? {
        Region::Block(bid) if *bid == header => {}
        _ => return None,
    }
    let suffix = if children.len() > 1 {
        trim_trailing_exit_if_suffix(Region::Seq(children[1..].to_vec()))
    } else {
        Region::Seq(vec![])
    };
    Some(Region::Seq(vec![children[0].clone(), suffix]))
}

fn should_peel_tail(then_branch: &Region, header: BlockId) -> bool {
    fn contains_other_loop(r: &Region, header: BlockId) -> bool {
        match r {
            Region::Loop { header: h, .. } if *h != header => true,
            Region::Seq(v) => v.iter().any(|c| contains_other_loop(c, header)),
            Region::If {
                then_branch,
                else_branch,
                ..
            } => {
                contains_other_loop(then_branch, header) || contains_other_loop(else_branch, header)
            }
            _ => false,
        }
    }
    contains_other_loop(then_branch, header)
}

fn peel_one_loop(region: Region, cfg: &MethodCfg) -> Option<Vec<Region>> {
    let Region::Loop { header, body } = region else {
        return None;
    };
    let (_, _, then_branch) = loop_body_break_pattern(&body, header)?;
    if region_is_empty(then_branch) || !should_peel_tail(then_branch, header) {
        return None;
    }
    let exit_start = first_block(then_branch)?;
    if cfg.natural_loop_blocks(header).contains(&exit_start) {
        return None;
    }
    let trimmed_body = trim_trailing_exit_if(&body, header)?;
    let main = Region::Loop {
        header,
        body: Box::new(peel_sequential_loop_tails(trimmed_body, cfg)),
    };
    let tail = peel_sequential_loop_tails(then_branch.clone(), cfg);
    let mut parts = vec![main];
    match tail {
        Region::Seq(ts) => parts.extend(ts),
        t if !region_is_empty(&t) => parts.push(t),
        _ => {}
    }
    Some(parts)
}

/// L1-2 / L1-4: promote exit tails (tail loops, return) from loop bodies to sequential siblings.
pub fn peel_sequential_loop_tails(region: Region, cfg: &MethodCfg) -> Region {
    match region {
        Region::Seq(children) => {
            let mut out = Vec::new();
            for c in children {
                match peel_one_loop(c.clone(), cfg) {
                    Some(parts) => out.extend(parts),
                    None => out.push(peel_sequential_loop_tails(c, cfg)),
                }
            }
            if out.len() == 1 {
                out.into_iter().next().unwrap()
            } else {
                Region::Seq(out)
            }
        }
        Region::Loop { .. } => {
            if let Some(parts) = peel_one_loop(region.clone(), cfg) {
                if parts.len() == 1 {
                    parts.into_iter().next().unwrap()
                } else {
                    Region::Seq(parts)
                }
            } else {
                region
            }
        }
        Region::If {
            condition,
            then_branch,
            else_branch,
        } => Region::If {
            condition,
            then_branch: Box::new(peel_sequential_loop_tails(*then_branch, cfg)),
            else_branch: Box::new(peel_sequential_loop_tails(*else_branch, cfg)),
        },
        Region::Switch {
            condition,
            cases,
            default,
        } => Region::Switch {
            condition,
            cases: cases
                .into_iter()
                .map(|(v, r)| (v, Box::new(peel_sequential_loop_tails(*r, cfg))))
                .collect(),
            default: Box::new(peel_sequential_loop_tails(*default, cfg)),
        },
        other => other,
    }
}

/// L3-2: sequential exit tests in a loop prefix → `(condition, exit_region)` chain.
pub fn loop_prefix_multi_exit_ifs(region: &Region, header: BlockId) -> Vec<(String, Region)> {
    let Some(middle) = loop_body_break_middle(region, header) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_exit_ifs(&middle, &mut out);
    out
}

fn collect_exit_ifs(region: &Region, out: &mut Vec<(String, Region)>) {
    match region {
        Region::Seq(children) => {
            for c in children {
                collect_exit_ifs(c, out);
            }
        }
        Region::If {
            condition,
            then_branch,
            else_branch,
        } if !region_is_empty(then_branch) => {
            out.push((condition.clone(), then_branch.as_ref().clone()));
            if !region_is_empty(else_branch) {
                collect_exit_ifs(else_branch, out);
            }
        }
        Region::If { else_branch, .. } => collect_exit_ifs(else_branch, out),
        _ => {}
    }
}

fn resolve_then_start(
    cfg: &MethodCfg,
    branch_target: BlockId,
    fall_through: BlockId,
    loop_header: Option<BlockId>,
    emitted: &HashSet<BlockId>,
) -> BlockId {
    if loop_header == Some(branch_target) {
        return branch_target;
    }
    if cfg.is_loop_exit_target(branch_target, loop_header) {
        let chain = cfg.exit_chain_head(branch_target, loop_header, emitted);
        if !emitted.contains(&chain) && chain != fall_through {
            return chain;
        }
    }
    cfg.blocks_that_fall_through_to(branch_target)
        .into_iter()
        .find(|&bid| {
            !emitted.contains(&bid)
                && bid != fall_through
                && !cfg.reachable_from(fall_through, bid, loop_header)
        })
        .unwrap_or(branch_target)
}

/// Used for emit-time `} else if (` instead of `} else { if (`.
pub fn as_single_if(region: &Region) -> Option<(&str, &Region, &Region)> {
    match region {
        Region::If {
            condition,
            then_branch,
            else_branch,
        } => Some((
            condition.as_str(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        )),
        Region::Seq(children) => {
            let mut found: Option<&Region> = None;
            for c in children {
                if region_is_empty(c) {
                    continue;
                }
                if found.is_some() {
                    return None;
                }
                found = Some(c);
            }
            found.and_then(as_single_if)
        }
        _ => None,
    }
}

/// Do-while pattern: loop body is `Seq([Block(header), …middle…, If { condition, then, else }])`
/// with trailing exit-if (empty else = back-edge / continue).
/// Returns `(exit_condition, do_body, then_branch)` for
/// `do { do_body } while (!exit_condition); then_branch`.
///
/// Prefers do-while whenever there is real middle body (len ≥ 3). Classic while
/// `Seq([Block(header), If])` is handled separately by [`loop_body_break_pattern`].
pub fn loop_body_do_while_pattern(
    body: &Region,
    header: BlockId,
) -> Option<(&str, Region, &Region)> {
    let Region::Seq(children) = body else {
        return None;
    };
    if children.len() < 3 {
        return None;
    }
    match &children[0] {
        Region::Block(bid) if *bid == header => {}
        _ => return None,
    }
    let last = children.last()?;
    let Region::If {
        condition,
        then_branch,
        else_branch,
    } = last
    else {
        return None;
    };
    // Empty else = fall into loop back-edge; or else is empty-ish continue.
    if !region_is_empty(else_branch) {
        // Allow else that is only an empty Seq / empty Block placeholder.
        match else_branch.as_ref() {
            Region::Seq(c) if c.iter().all(region_is_empty) => {}
            Region::Block(_) => return None,
            _ => return None,
        }
    }
    let do_body = Region::Seq(children[..children.len() - 1].to_vec());
    Some((condition.as_str(), do_body, then_branch.as_ref()))
}

/// Looser do-while: last region in Seq is exit-If with empty else, and header appears
/// as first block somewhere in the prefix (handles nested Seq wrappers).
pub fn loop_body_do_while_loose(body: &Region, header: BlockId) -> Option<(&str, Region, &Region)> {
    if let Some(r) = loop_body_do_while_pattern(body, header) {
        return Some(r);
    }
    let Region::Seq(children) = body else {
        return None;
    };
    if children.len() < 2 {
        return None;
    }
    let last = children.last()?;
    let Region::If {
        condition,
        then_branch,
        else_branch,
    } = last
    else {
        return None;
    };
    if !region_is_empty(else_branch) {
        return None;
    }
    let prefix = Region::Seq(children[..children.len() - 1].to_vec());
    // Header must be reachable as first block of prefix.
    if first_block(&prefix) != Some(header) && !region_contains_block(&prefix, header) {
        return None;
    }
    Some((condition.as_str(), prefix, then_branch.as_ref()))
}

/// Bottom-tested do-while: `if (continue) goto header` with empty taken arm and real
/// work on fall-through — `do { … } while (continue); exit_tail`.
pub fn loop_body_do_while_exit_in_else(
    body: &Region,
    header: BlockId,
) -> Option<(&str, Region, &Region)> {
    let Region::Seq(children) = body else {
        return None;
    };
    if children.is_empty() {
        return None;
    }
    let Region::Block(first) = &children[0] else {
        return None;
    };
    if *first != header {
        return None;
    }
    let (cond, else_branch, if_idx) = trailing_continue_if(children)?;
    let do_body = Region::Seq(children[..if_idx].to_vec());
    Some((cond, do_body, else_branch))
}

/// Find a trailing `If(empty then, else exit)`; returns condition, else branch, and If index.
fn trailing_continue_if(children: &[Region]) -> Option<(&str, &Region, usize)> {
    if let Some(Region::If {
        condition,
        then_branch,
        else_branch,
    }) = children.last()
    {
        if region_is_empty(then_branch) && !region_is_empty(else_branch) {
            return Some((condition.as_str(), else_branch.as_ref(), children.len() - 1));
        }
    }
    if let Some(Region::Seq(inner)) = children.last() {
        let (cond, else_branch, _inner_idx) = trailing_continue_if(inner)?;
        return Some((cond, else_branch, children.len() - 1));
    }
    None
}

fn region_contains_block(region: &Region, id: BlockId) -> bool {
    match region {
        Region::Block(b) => *b == id,
        Region::Seq(c) => c.iter().any(|r| region_contains_block(r, id)),
        Region::If {
            then_branch,
            else_branch,
            ..
        } => region_contains_block(then_branch, id) || region_contains_block(else_branch, id),
        Region::Loop { body, header, .. } => *header == id || region_contains_block(body, id),
        Region::Switch { cases, default, .. } => {
            cases.iter().any(|(_, r)| region_contains_block(r, id))
                || region_contains_block(default, id)
        }
    }
}

/// First Block(block_id) in depth-first order; used to get loop exit block for break emission.
pub fn first_block(region: &Region) -> Option<BlockId> {
    match region {
        Region::Block(bid) => Some(*bid),
        Region::Seq(children) => children.iter().find_map(first_block),
        Region::If {
            then_branch,
            else_branch,
            ..
        } => first_block(then_branch).or_else(|| first_block(else_branch)),
        Region::Loop { body, .. } => first_block(body),
        Region::Switch { cases, default, .. } => cases
            .first()
            .and_then(|(_, r)| first_block(r))
            .or_else(|| first_block(default)),
    }
}

/// Last Block(block_id) in a Seq (depth-first, last child); used to detect for-loop update tail.
pub fn last_block(region: &Region) -> Option<BlockId> {
    match region {
        Region::Block(bid) => Some(*bid),
        Region::Seq(children) => children.last().and_then(last_block),
        Region::If { .. } | Region::Loop { .. } | Region::Switch { .. } => None,
    }
}

/// If region is a Seq ending with a single Block, returns (prefix region without that block, that block id).
pub fn split_tail_block(region: &Region) -> Option<(Region, BlockId)> {
    match region {
        Region::Seq(children) if !children.is_empty() => {
            let last = children.last()?;
            if let Region::Block(bid) = last {
                let prefix: Vec<Region> = children[..children.len() - 1].to_vec();
                Some((Region::Seq(prefix), *bid))
            } else {
                None
            }
        }
        Region::Block(bid) => Some((Region::Seq(vec![]), *bid)),
        _ => None,
    }
}

/// If region is a Seq ending with two Blocks, returns (prefix, second-to-last block id, last block id).
pub fn split_tail_two_blocks(region: &Region) -> Option<(Region, BlockId, BlockId)> {
    match region {
        Region::Seq(children) if children.len() >= 2 => {
            let last = children.last()?;
            let second = children.get(children.len() - 2)?;
            match (second, last) {
                (Region::Block(bid1), Region::Block(bid2)) => {
                    let prefix: Vec<Region> = children[..children.len() - 2].to_vec();
                    Some((Region::Seq(prefix), *bid1, *bid2))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// For-loop pattern: when we have Seq([Block(init), Loop { header, body }]) and body is
/// Seq([Block(header), If { condition, then_branch, else_branch }]), and else_branch ends with
/// either (1) a single Block (update+goto) or (2) two Blocks (update block, goto-only block),
/// returns (init_block_id, header, condition, body_without_update, then_branch, update_block_id).
pub fn for_loop_pattern(
    seq: &[Region],
) -> Option<(BlockId, BlockId, &str, Region, &Region, BlockId)> {
    if seq.len() != 2 {
        return None;
    }
    let (init_region, loop_region) = (&seq[0], &seq[1]);
    let init_block = match init_region {
        Region::Block(bid) => *bid,
        _ => return None,
    };
    let (header, body) = match loop_region {
        Region::Loop { header, body } => (*header, body.as_ref()),
        _ => return None,
    };
    let (condition, else_branch, then_branch) = loop_body_break_pattern(body, header)?;
    let (body_without_update, update_block) =
        if let Some((prefix, update_bid, _back_bid)) = split_tail_two_blocks(else_branch) {
            (prefix, update_bid)
        } else if let Some((prefix, single_bid)) = split_tail_block(else_branch) {
            (prefix, single_bid)
        } else {
            return None;
        };
    Some((
        init_block,
        header,
        condition,
        body_without_update,
        then_branch,
        update_block,
    ))
}

/// Build a region tree from the CFG starting at entry.
/// Uses the same structure as the current emit (loop headers -> Loop, conditionals -> If, rest -> Seq).
pub fn build_regions(cfg: &MethodCfg, entry: BlockId) -> Option<Region> {
    let mut emitted = HashSet::new();
    build_regions_rec(cfg, entry, None, &mut emitted, None)
}

/// Like [`build_regions`] but promotes sequential tail loops to siblings (L1-2 / L1-4).
pub fn build_regions_with_peeled_tails(cfg: &MethodCfg, entry: BlockId) -> Option<Region> {
    build_regions(cfg, entry).map(|r| peel_sequential_loop_tails(r, cfg))
}

/// Like build_regions but only includes blocks in `allowed`. Used for try body (only try-range blocks)
/// or catch body (only handler-range blocks). Successors outside `allowed` are treated as exit.
pub fn build_regions_filtered(
    cfg: &MethodCfg,
    entry: BlockId,
    allowed: &HashSet<BlockId>,
) -> Option<Region> {
    if !allowed.contains(&entry) {
        return None;
    }
    let mut emitted = HashSet::new();
    build_regions_rec(cfg, entry, None, &mut emitted, Some(allowed))
}

fn build_if_branches(
    cfg: &MethodCfg,
    then_start: BlockId,
    fall_through: BlockId,
    loop_header: Option<BlockId>,
    emitted: &mut HashSet<BlockId>,
    allowed: Option<&HashSet<BlockId>>,
    exit_target: bool,
) -> (Region, Region) {
    if exit_target {
        let then_r = build_regions_rec(cfg, then_start, loop_header, emitted, allowed)
            .unwrap_or_else(|| Region::Seq(vec![]));
        let else_r = build_regions_rec(cfg, fall_through, loop_header, emitted, allowed)
            .unwrap_or_else(|| Region::Seq(vec![]));
        (then_r, else_r)
    } else {
        // Prefer fall-through (else) before branch (then) so shared tails aren't
        // claimed by the taken branch first. Matches typical if-eqz skip layouts.
        let else_r = build_regions_rec(cfg, fall_through, loop_header, emitted, allowed)
            .unwrap_or_else(|| Region::Seq(vec![]));
        let then_r = build_regions_rec(cfg, then_start, loop_header, emitted, allowed)
            .unwrap_or_else(|| Region::Seq(vec![]));
        (then_r, else_r)
    }
}

fn build_if_branches_until(
    cfg: &MethodCfg,
    then_start: BlockId,
    fall_through: BlockId,
    stop_at: &HashSet<BlockId>,
    loop_header: Option<BlockId>,
    emitted: &mut HashSet<BlockId>,
    allowed: Option<&HashSet<BlockId>>,
    exit_target: bool,
) -> (Region, Region) {
    if exit_target {
        let then_r = if stop_at.contains(&then_start) || emitted.contains(&then_start) {
            Region::Seq(vec![])
        } else {
            build_regions_rec_until(cfg, then_start, stop_at, loop_header, emitted, allowed)
                .unwrap_or_else(|| Region::Seq(vec![]))
        };
        let else_r = if stop_at.contains(&fall_through) || emitted.contains(&fall_through) {
            Region::Seq(vec![])
        } else {
            build_regions_rec_until(cfg, fall_through, stop_at, loop_header, emitted, allowed)
                .unwrap_or_else(|| Region::Seq(vec![]))
        };
        (then_r, else_r)
    } else {
        let else_r = if stop_at.contains(&fall_through) || emitted.contains(&fall_through) {
            Region::Seq(vec![])
        } else {
            build_regions_rec_until(cfg, fall_through, stop_at, loop_header, emitted, allowed)
                .unwrap_or_else(|| Region::Seq(vec![]))
        };
        let then_r = if stop_at.contains(&then_start) || emitted.contains(&then_start) {
            Region::Seq(vec![])
        } else {
            build_regions_rec_until(cfg, then_start, stop_at, loop_header, emitted, allowed)
                .unwrap_or_else(|| Region::Seq(vec![]))
        };
        (then_r, else_r)
    }
}

fn build_regions_rec(
    cfg: &MethodCfg,
    block_id: BlockId,
    loop_header: Option<BlockId>,
    emitted: &mut HashSet<BlockId>,
    allowed: Option<&HashSet<BlockId>>,
) -> Option<Region> {
    if emitted.contains(&block_id) {
        return None;
    }
    if let Some(allowed_set) = allowed {
        if !allowed_set.contains(&block_id) {
            return None;
        }
    }
    let block = &cfg.blocks[block_id];
    let is_loop_header = cfg.loop_headers.contains(&block_id);

    if is_loop_header && loop_header != Some(block_id) {
        emitted.insert(block_id);
        let body = build_loop_body(cfg, block_id, emitted, allowed);
        return Some(Region::Loop {
            header: block_id,
            body: Box::new(body),
        });
    }

    emitted.insert(block_id);
    let is_back_edge = matches!(&block.end, BlockEnd::Goto(t) if loop_header == Some(*t));
    let block_region = Region::Block(block_id);
    if is_back_edge {
        return Some(block_region);
    }

    match &block.end {
        BlockEnd::Exit => Some(block_region),
        BlockEnd::FallThrough => {
            let ft = cfg.fall_through_block(block_id)?;
            if allowed.map(|a| !a.contains(&ft)).unwrap_or(false) {
                return Some(block_region);
            }
            let next = build_regions_rec(cfg, ft, loop_header, emitted, allowed)?;
            Some(Region::Seq(vec![block_region, next]))
        }
        BlockEnd::Goto(t) => {
            if allowed.map(|a| !a.contains(t)).unwrap_or(false) {
                return Some(block_region);
            }
            let next = build_regions_rec(cfg, *t, loop_header, emitted, allowed)?;
            Some(Region::Seq(vec![block_region, next]))
        }
        BlockEnd::Conditional {
            condition,
            branch_target,
            fall_through,
        } => {
            // Classic if-eqz / if-nez skip: fall-through body rejoins at branch_target
            // (e.g. `if (url == null) skip; setLoginUrl();` then shared `new Intent()`).
            // Prefer this over expanding then_start backward, which can pull the fall-through
            // body into the taken branch and drop the shared tail into the wrong arm.
            // Only when fall-through has real work — a lone `goto join` is not a skip-body.
            let fall_block = &cfg.blocks[*fall_through];
            let fall_has_work = !block_is_effectively_empty(fall_block);
            let exit_target = cfg.is_loop_exit_target(*branch_target, loop_header);
            let skip_join = fall_has_work
                && *branch_target != *fall_through
                && loop_header != Some(*branch_target)
                && !exit_target
                && !emitted.contains(branch_target)
                && cfg.reachable_from(*fall_through, *branch_target, loop_header)
                && !cfg.reachable_from(*branch_target, *fall_through, loop_header);

            // If branch_target is the loop header (back-edge), don't pull in fall-through predecessors
            // or we'd steal the loop body (e.g. in-loop Swap) into the then branch and leave else empty.
            // Otherwise, if some block falls through to branch_target (e.g. Swap → return), start
            // then_branch from that block so we emit the full exit path.
            let then_start =
                resolve_then_start(cfg, *branch_target, *fall_through, loop_header, emitted);

            // Diamond: else reaches join and then does not reach else → join is shared tail.
            // Prefer skip_join (branch_target) when the fall-through reaches the taken target.
            let diamond_join = if skip_join {
                Some(*branch_target)
            } else if exit_target {
                None
            } else if then_start != *fall_through
                && !emitted.contains(&then_start)
                && cfg.reachable_from(*fall_through, then_start, loop_header)
                && !cfg.reachable_from(then_start, *fall_through, loop_header)
            {
                Some(then_start)
            } else {
                None
            };

            if let Some(join_id) = diamond_join {
                let stop_at: HashSet<BlockId> = std::iter::once(join_id).collect();
                let else_r = build_regions_rec_until(
                    cfg,
                    *fall_through,
                    &stop_at,
                    loop_header,
                    emitted,
                    allowed,
                )
                .unwrap_or_else(|| Region::Seq(vec![]));
                let then_r = Region::Seq(vec![]);
                // Emit the join block alone. Do NOT follow its goto/fall-through here —
                // those successors belong to the enclosing region (e.g. the next else-if arm).
                // Following the join was stealing later arms and leaving this if's body empty.
                let after = if emitted.contains(&join_id) {
                    Region::Seq(vec![])
                } else if block_is_effectively_empty(&cfg.blocks[join_id]) {
                    emitted.insert(join_id);
                    Region::Seq(vec![])
                } else {
                    emitted.insert(join_id);
                    Region::Block(join_id)
                };
                return Some(Region::Seq(vec![
                    block_region,
                    Region::If {
                        condition: condition.clone(),
                        then_branch: Box::new(then_r),
                        else_branch: Box::new(else_r),
                    },
                    after,
                ]));
            }

            // Prefer fall-through (else) before branch (then) for join/skip layouts;
            // for loop exits, build the taken (exit) branch first so tail blocks aren't stolen.
            let (then_r, else_r) = build_if_branches(
                cfg,
                then_start,
                *fall_through,
                loop_header,
                emitted,
                allowed,
                exit_target,
            );
            Some(Region::Seq(vec![
                block_region,
                Region::If {
                    condition: condition.clone(),
                    then_branch: Box::new(then_r),
                    else_branch: Box::new(else_r),
                },
            ]))
        }
        BlockEnd::Switch {
            condition,
            cases,
            default_block,
        } => {
            let stop_at: HashSet<BlockId> = cases
                .iter()
                .map(|(_, bid)| *bid)
                .chain(std::iter::once(*default_block))
                .collect();
            let case_regions: Vec<(i32, Box<Region>)> = cases
                .iter()
                .filter_map(|(val, bid)| {
                    let r =
                        build_regions_rec_until(cfg, *bid, &stop_at, loop_header, emitted, allowed)
                            .unwrap_or_else(|| Region::Seq(vec![]));
                    Some((*val, Box::new(r)))
                })
                .collect();
            let default_r = build_regions_rec(cfg, *default_block, loop_header, emitted, allowed)
                .unwrap_or_else(|| Region::Seq(vec![]));
            Some(Region::Seq(vec![
                block_region,
                Region::Switch {
                    condition: condition.clone(),
                    cases: case_regions,
                    default: Box::new(default_r),
                },
            ]))
        }
    }
}

/// Build region from block_id until we hit a block in stop_at (exclusive). Used for switch case bodies.
fn build_regions_rec_until(
    cfg: &MethodCfg,
    block_id: BlockId,
    stop_at: &HashSet<BlockId>,
    loop_header: Option<BlockId>,
    emitted: &mut HashSet<BlockId>,
    allowed: Option<&HashSet<BlockId>>,
) -> Option<Region> {
    if stop_at.contains(&block_id) {
        return None;
    }
    if emitted.contains(&block_id) {
        return None;
    }
    if let Some(allowed_set) = allowed {
        if !allowed_set.contains(&block_id) {
            return None;
        }
    }
    let block = &cfg.blocks[block_id];
    emitted.insert(block_id);
    let block_region = Region::Block(block_id);
    let is_back_edge = matches!(&block.end, BlockEnd::Goto(t) if loop_header == Some(*t));
    if is_back_edge {
        return Some(block_region);
    }
    match &block.end {
        BlockEnd::Exit => Some(block_region),
        BlockEnd::FallThrough => {
            let ft = match cfg.fall_through_block(block_id) {
                Some(id) if !stop_at.contains(&id) => id,
                _ => return Some(block_region),
            };
            if allowed.map(|a| !a.contains(&ft)).unwrap_or(false) {
                return Some(block_region);
            }
            let next = build_regions_rec_until(cfg, ft, stop_at, loop_header, emitted, allowed)?;
            Some(Region::Seq(vec![block_region, next]))
        }
        BlockEnd::Goto(t) => {
            if stop_at.contains(t) {
                return Some(block_region);
            }
            if allowed.map(|a| !a.contains(t)).unwrap_or(false) {
                return Some(block_region);
            }
            let next = build_regions_rec_until(cfg, *t, stop_at, loop_header, emitted, allowed)?;
            Some(Region::Seq(vec![block_region, next]))
        }
        BlockEnd::Conditional {
            condition,
            branch_target,
            fall_through,
        } => {
            // Same skip-join handling as `build_regions_rec`. Nested conditionals often live
            // under an outer `build_regions_rec_until` (e.g. whole method body under a top-level
            // scheme check) — without this, fall-through skip bodies are lost.
            let fall_has_work = !block_is_effectively_empty(&cfg.blocks[*fall_through]);
            let exit_target = cfg.is_loop_exit_target(*branch_target, loop_header);
            let skip_join = fall_has_work
                && *branch_target != *fall_through
                && loop_header != Some(*branch_target)
                && !exit_target
                && !emitted.contains(branch_target)
                && !stop_at.contains(branch_target)
                && !stop_at.contains(fall_through)
                && cfg.reachable_from(*fall_through, *branch_target, loop_header)
                && !cfg.reachable_from(*branch_target, *fall_through, loop_header);

            if skip_join {
                let mut nested_stop = stop_at.clone();
                nested_stop.insert(*branch_target);
                let else_r = build_regions_rec_until(
                    cfg,
                    *fall_through,
                    &nested_stop,
                    loop_header,
                    emitted,
                    allowed,
                )
                .unwrap_or_else(|| Region::Seq(vec![]));
                // Join block only (no follow) — successors continue below.
                let after = if emitted.contains(branch_target) {
                    Region::Seq(vec![])
                } else if block_is_effectively_empty(&cfg.blocks[*branch_target]) {
                    emitted.insert(*branch_target);
                    Region::Seq(vec![])
                } else {
                    emitted.insert(*branch_target);
                    Region::Block(*branch_target)
                };
                let mut parts = vec![
                    block_region,
                    Region::If {
                        condition: condition.clone(),
                        then_branch: Box::new(Region::Seq(vec![])),
                        else_branch: Box::new(else_r),
                    },
                ];
                match &after {
                    Region::Seq(v) if v.is_empty() => {}
                    _ => parts.push(after),
                }
                // Continue only via fall-through from a non-empty join. Do not follow a
                // join's `goto` — that target is a shared exit / next else-if arm owned by
                // the parent region.
                let cont = match &cfg.blocks[*branch_target].end {
                    BlockEnd::FallThrough => cfg
                        .fall_through_block(*branch_target)
                        .filter(|t| !stop_at.contains(t) && !emitted.contains(t)),
                    _ => None,
                };
                if let Some(next) = cont {
                    if let Some(r) =
                        build_regions_rec_until(cfg, next, stop_at, loop_header, emitted, allowed)
                    {
                        parts.push(r);
                    }
                }
                return Some(Region::Seq(parts));
            }

            // Fall-through first for join layouts; exit targets build taken branch first.
            let (then_r, else_r) = build_if_branches_until(
                cfg,
                *branch_target,
                *fall_through,
                stop_at,
                loop_header,
                emitted,
                allowed,
                exit_target,
            );
            Some(Region::Seq(vec![
                block_region,
                Region::If {
                    condition: condition.clone(),
                    then_branch: Box::new(then_r),
                    else_branch: Box::new(else_r),
                },
            ]))
        }
        BlockEnd::Switch { .. } => Some(block_region),
    }
}

fn build_loop_body(
    cfg: &MethodCfg,
    header_id: BlockId,
    emitted: &mut HashSet<BlockId>,
    allowed: Option<&HashSet<BlockId>>,
) -> Region {
    let block = &cfg.blocks[header_id];
    let block_reg = Region::Block(header_id);
    match &block.end {
        BlockEnd::Exit => block_reg,
        BlockEnd::FallThrough => {
            if let Some(ft) = cfg.fall_through_block(header_id) {
                let next = build_regions_rec(cfg, ft, Some(header_id), emitted, allowed)
                    .unwrap_or_else(|| Region::Seq(vec![]));
                return Region::Seq(vec![block_reg, next]);
            }
            block_reg
        }
        BlockEnd::Goto(t) if *t == header_id => block_reg,
        BlockEnd::Goto(t) => {
            let next = build_regions_rec(cfg, *t, Some(header_id), emitted, allowed)
                .unwrap_or_else(|| Region::Seq(vec![]));
            Region::Seq(vec![block_reg, next])
        }
        BlockEnd::Conditional {
            condition,
            branch_target,
            fall_through,
        } => {
            // If some block falls through to branch_target (e.g. Swap block → return block),
            // start then_branch from that block so we emit the full exit path. Exclude blocks
            // that are reachable from the loop body (fall_through), or we'd pick the wrong block
            // (e.g. the in-loop block that ends at the same address as the exit block start).
            let then_start =
                resolve_then_start(cfg, *branch_target, *fall_through, Some(header_id), emitted);
            let exit_target = cfg.is_loop_exit_target(*branch_target, Some(header_id));
            let (then_r, else_r) = build_if_branches(
                cfg,
                then_start,
                *fall_through,
                Some(header_id),
                emitted,
                allowed,
                exit_target,
            );
            Region::Seq(vec![
                block_reg,
                Region::If {
                    condition: condition.clone(),
                    then_branch: Box::new(then_r),
                    else_branch: Box::new(else_r),
                },
            ])
        }
        BlockEnd::Switch { .. } => block_reg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dex_bytecode::decode_all;

    fn condition_for(_ins: &dex_bytecode::Instruction) -> String {
        "v0 == 0".to_string()
    }

    #[test]
    fn region_linear_single_block() {
        let bytecode: &[u8] = &[0x0e, 0x00]; // return-void
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        let r = build_regions(&cfg, cfg.entry);
        assert!(r.is_some());
        let r = r.unwrap();
        assert!(matches!(r, Region::Block(_) | Region::Seq(_)));
    }

    #[test]
    fn region_if_else() {
        let bytecode: &[u8] = &[
            0x38, 0x00, 0x04, 0x00, // if-eqz +4 -> 8
            0x28, 0x02, // goto +2 -> 8
            0x0e, 0x00, 0x0e, 0x00,
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        let r = build_regions(&cfg, cfg.entry);
        assert!(r.is_some());
        let r = r.unwrap();
        let has_if = match &r {
            Region::Seq(s) => s.iter().any(|x| matches!(x, Region::If { .. })),
            _ => false,
        };
        assert!(has_if, "expected If region in {:?}", r);
    }

    #[test]
    fn region_loop() {
        let bytecode: &[u8] = &[
            0x12, 0x00, 0x38, 0x00, 0x05, 0x00, // if-eqz +5 -> 12
            0x28, 0xfe, // goto -2 -> 2
            0x00, 0x00, 0x00, 0x00, 0x0e, 0x00,
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        let r = build_regions(&cfg, cfg.entry);
        assert!(r.is_some());
        let r = r.unwrap();
        let has_loop = contains_loop(&r);
        assert!(
            has_loop,
            "expected region tree to contain Loop, got {:?}",
            r
        );
    }

    fn contains_loop(r: &Region) -> bool {
        match r {
            Region::Loop { .. } => true,
            Region::Seq(s) => s.iter().any(contains_loop),
            Region::If {
                then_branch,
                else_branch,
                ..
            } => contains_loop(then_branch) || contains_loop(else_branch),
            Region::Block(_) | Region::Switch { .. } => false,
        }
    }

    #[test]
    fn region_do_while_bottom_continue() {
        let bytecode: &[u8] = &[
            0x12, 0x00, 0xd8, 0x00, 0x00, 0x01, 0x34, 0x10, 0xfe, 0xff, 0x0f, 0x00,
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        assert!(
            cfg.loop_headers.contains(&1),
            "block 1 should be loop header, headers={:?} edges={:?}",
            cfg.loop_headers,
            cfg.successor_edges()
        );
        let r = build_regions(&cfg, cfg.entry).expect("build_regions");
        assert!(contains_loop(&r), "expected Loop in {r:?}");
        let (header, body) = match &r {
            Region::Seq(s) => s.iter().find_map(|c| match c {
                Region::Loop { header, body } => Some((*header, body.as_ref())),
                _ => None,
            }),
            Region::Loop { header, body } => Some((*header, body.as_ref())),
            _ => None,
        }
        .expect("loop");
        assert!(
            loop_body_do_while_exit_in_else(body, header).is_some(),
            "expected bottom-continue do-while in {body:?}"
        );
    }

    #[test]
    fn trailing_continue_if_nested() {
        let inner = Region::Seq(vec![
            Region::Block(2),
            Region::If {
                condition: "i < n".into(),
                then_branch: Box::new(Region::Seq(vec![])),
                else_branch: Box::new(Region::Block(3)),
            },
        ]);
        let body = Region::Seq(vec![Region::Block(1), inner]);
        assert!(loop_body_do_while_exit_in_else(&body, 1).is_some());
    }

    /// Partition-style: exit path = (block A) + (block B return).
    /// The first If's then_branch (exit path) must include BOTH blocks (A and B).
    #[test]
    fn region_loop_exit_path_two_blocks() {
        // Same layout as test_decompiler_loop_exit_path_two_blocks: const/4; if-eqz +5 (target 12); goto -2; nop; const/4 v0,1; return v0
        let bytecode: &[u8] = &[
            0x12, 0x00, // const/4 v0, 0
            0x38, 0x00, 0x05, 0x00, // if-eqz v0, +5 -> target 12
            0x28, 0xfe, // goto -2 -> target 2
            0x00, 0x00, // nop
            0x12, 0x01, // const/4 v0, 1  (exit block 1)
            0x0f, 0x00, // return v0     (exit block 2)
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        let r = build_regions(&cfg, cfg.entry).expect("build_regions");
        fn block_count(r: &Region) -> usize {
            match r {
                Region::Block(_) => 1,
                Region::Seq(s) => s.iter().map(block_count).sum(),
                Region::If {
                    then_branch,
                    else_branch,
                    ..
                } => block_count(then_branch) + block_count(else_branch),
                Region::Loop { body, .. } => block_count(body),
                Region::Switch { cases, default, .. } => {
                    cases.iter().map(|(_, r)| block_count(r)).sum::<usize>() + block_count(default)
                }
            }
        }
        fn first_if_then_branch(r: &Region) -> Option<&Region> {
            match r {
                Region::If { then_branch, .. } => Some(then_branch),
                Region::Seq(s) => s.iter().find_map(first_if_then_branch),
                Region::Loop { body, .. } => first_if_then_branch(body.as_ref()),
                Region::Block(_) | Region::Switch { .. } => None,
            }
        }
        let then_branch = first_if_then_branch(&r).expect("region tree should contain an If");
        let then_blocks = block_count(then_branch);
        assert!(
            then_blocks >= 2,
            "exit path (then_branch) must contain at least 2 blocks (e.g. const/4 + return), got {}; then_branch = {:?}; full region = {:?}",
            then_blocks,
            then_branch,
            r
        );
    }

    /// Exit from loop A into loop B (with return) must preserve both loops in the region tree.
    #[test]
    fn region_loop_exit_then_second_loop() {
        // Loop1: header 2, body exits to header 11 when v0==0; Loop2 at 11 exits to return at 19.
        // Layout mirrors merge tail: main loop back-edge + sequential tail loop + return.
        let bytecode: &[u8] = &[
            0x12, 0x00, // 0: const/4 v0, 0  (init — not part of loop under test)
            0x28, 0x09, // 2: goto +9 -> 20 (enter loop1 header)
            0x00, 0x00, // 4: nop padding
            0x38, 0x00, 0x08, 0x00, // 6: if-eqz v0, +8 -> 22 (exit to loop2 header)
            0x12, 0x00, // 10: const/4 v0, 0 (loop1 body)
            0x28, 0xf8, // 12: goto -8 -> 6  (back — treat block 6 as header via back edge)
            0x00, 0x00, // 14: nop
            0x38, 0x00, 0x04, 0x00, // 16: if-eqz v0, +4 -> 24 (loop2 exit to return)
            0x12, 0x01, // 20: const/4 v0, 1 (loop2 body)
            0x28, 0xf8, // 22: goto -8 -> 16 (back to loop2 header)
            0x12, 0x02, // 24: const/4 v0, 2
            0x0f, 0x00, // 26: return v0
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        let r = build_regions(&cfg, cfg.entry).expect("build_regions");
        fn contains_loop_header(r: &Region, header: BlockId) -> bool {
            match r {
                Region::Loop { header: h, body } => {
                    *h == header || contains_loop_header(body, header)
                }
                Region::Seq(v) => v.iter().any(|c| contains_loop_header(c, header)),
                Region::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    contains_loop_header(then_branch, header)
                        || contains_loop_header(else_branch, header)
                }
                Region::Block(_) | Region::Switch { .. } => false,
            }
        }
        fn contains_return(r: &Region, cfg: &MethodCfg) -> bool {
            match r {
                Region::Block(bid) => {
                    matches!(cfg.blocks[*bid].end, BlockEnd::Exit)
                }
                Region::Seq(v) => v.iter().any(|c| contains_return(c, cfg)),
                Region::If {
                    then_branch,
                    else_branch,
                    ..
                } => contains_return(then_branch, cfg) || contains_return(else_branch, cfg),
                Region::Loop { body, .. } => contains_return(body, cfg),
                Region::Switch { cases, default, .. } => {
                    cases.iter().any(|(_, c)| contains_return(c, cfg))
                        || contains_return(default, cfg)
                }
            }
        }
        // At least one loop header from the CFG must appear, and the return block must be reachable in the tree.
        assert!(
            cfg.loop_headers
                .iter()
                .any(|h| contains_loop_header(&r, *h)),
            "region tree should contain a loop; region = {r:?}"
        );
        assert!(
            contains_return(&r, &cfg),
            "region tree should include return block; region = {r:?}"
        );
    }

    /// L1-4: peeled merge region tree has sequential tail loop headers as siblings.
    #[test]
    fn region_merge_peeled_sequential_tail_loops() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/decompiler_fixtures/classes.dex");
        let data = std::fs::read(&path).unwrap();
        let dex = crate::parse_dex(&data).unwrap();
        let class_name = "com.androguard.decompilefixtures.AlgorithmFixtures";
        let mut merge_code = None;
        for class_def in dex.class_defs().flatten() {
            let class_type = dex.get_type(class_def.class_idx).unwrap();
            if crate::java::descriptor_to_java(&class_type) != class_name {
                continue;
            }
            let Some(cd) = dex.get_class_data(&class_def).unwrap() else {
                continue;
            };
            for enc in cd.direct_methods.iter().chain(cd.virtual_methods.iter()) {
                let info = dex.get_method_info(enc.method_idx).unwrap();
                if info.name == "merge" {
                    merge_code = Some(dex.get_code_item(enc.code_off).unwrap());
                    break;
                }
            }
        }
        let code = merge_code.unwrap();
        let insns_bytes = code.insns_slice(&*dex.data);
        let insns = decode_all(insns_bytes, 0).unwrap();
        let cfg = MethodCfg::build(&insns, insns_bytes, 0, &|_| "cond".into());
        let peeled = build_regions_with_peeled_tails(&cfg, cfg.entry).expect("regions");
        let debug = format_region_debug(&peeled, &cfg);
        assert!(
            debug.contains("Loop(header=11)") && debug.contains("Loop(header=15)"),
            "peeled merge should surface tail loops; got:\n{debug}"
        );
        fn count_loops(r: &Region) -> usize {
            match r {
                Region::Loop { body, .. } => 1 + count_loops(body),
                Region::Seq(v) => v.iter().map(count_loops).sum(),
                Region::If {
                    then_branch,
                    else_branch,
                    ..
                } => count_loops(then_branch) + count_loops(else_branch),
                Region::Switch { cases, default, .. } => {
                    cases.iter().map(|(_, b)| count_loops(b)).sum::<usize>() + count_loops(default)
                }
                Region::Block(_) => 0,
            }
        }
        assert!(
            count_loops(&peeled) >= 3,
            "peeled merge should contain ≥3 loops; tree:\n{debug}"
        );
    }

    /// Minimal three-loop kernel: main loop → tail loop → return.
    #[test]
    fn region_three_loop_kernel() {
        let bytecode: &[u8] = &[
            0x38, 0x00, 0x10, 0x00, // 0: if-eqz v0, +16 -> exit loop2 @ 32
            0x38, 0x01, 0x04, 0x00, // 4: if-eqz v1, +4 -> loop2 head @ 12
            0x12, 0x02, // 8: main body
            0x28, 0xf6, // 10: goto -10 -> 0
            0x38, 0x00, 0x06, 0x00, // 12: if-eqz v0, +6 -> return @ 24
            0x12, 0x03, // 16: tail1 body
            0x28, 0xfa, // 18: goto -6 -> 12
            0x12, 0x04, // 20: return prep
            0x0f, 0x00, // 22: return v0
            0x00, 0x00, 0x00, 0x00, // padding
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        let r = build_regions(&cfg, cfg.entry).expect("build_regions");
        fn has_return(r: &Region, cfg: &MethodCfg) -> bool {
            match r {
                Region::Block(b) => matches!(cfg.blocks[*b].end, BlockEnd::Exit),
                Region::Seq(v) => v.iter().any(|c| has_return(c, cfg)),
                Region::If {
                    then_branch,
                    else_branch,
                    ..
                } => has_return(then_branch, cfg) || has_return(else_branch, cfg),
                Region::Loop { body, .. } => has_return(body, cfg),
                Region::Switch { cases, default, .. } => {
                    cases.iter().any(|(_, c)| has_return(c, cfg)) || has_return(default, cfg)
                }
            }
        }
        assert!(
            has_return(&r, &cfg),
            "three-loop kernel must retain return; {r:?}"
        );
    }

    #[test]
    fn format_cfg_and_region_debug_smoke() {
        let bytecode: &[u8] = &[
            0x12, 0x00, 0x38, 0x00, 0x05, 0x00, 0x28, 0xfe, 0x00, 0x00, 0x12, 0x01, 0x0f, 0x00,
        ];
        let instructions = decode_all(bytecode, 0).unwrap();
        let cfg = MethodCfg::build(&instructions, bytecode, 0, &condition_for);
        let cfg_dump = cfg.format_debug();
        assert!(cfg_dump.contains("blocks=") && cfg_dump.contains("edges="));
        let r = build_regions(&cfg, cfg.entry).unwrap();
        let region_dump = format_region_debug(&r, &cfg);
        assert!(region_dump.contains("Loop(") || region_dump.contains("If("));
    }
}
