//! Intra-procedural abstract interpretation: register-level taint tokens
//! with origin + call-edge witness, producing per-method summaries
//! (param → sink / param → static escape / param → return) plus direct
//! local findings. Flow-insensitive (two linear passes), context-insensitive,
//! token sets capped to keep large methods bounded.

use crate::body::{MethodBody, Op};
use crate::dispatch::{is_dispatch_kind, DispatchIndex};
use crate::model::{PropMode, Rules};
use std::collections::{BTreeMap, BTreeSet};

/// where a taint token was born
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Origin {
    /// a source rule fired at (method, pc)
    Src { rule: usize, method: String, pc: usize },
    /// seeded parameter slot of the analyzed method
    Param { method: String, slot: usize },
}

/// a taint token: origin + inter-procedural route (call edges taken so far)
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Token {
    pub origin: Origin,
    /// (caller method, invoke pc) frames appended as taint crosses calls
    pub hops: Vec<(String, usize)>,
}

/// per-token blowup caps
const MAX_TOKENS_PER_REG: usize = 6;
const MAX_HOPS: usize = 8;

fn push_hop(t: &Token, m: &str, pc: usize) -> Token {
    let mut t = t.clone();
    if t.hops.len() < MAX_HOPS && t.hops.last().map(|(h, _)| h.as_str()) != Some(m) {
        t.hops.push((m.to_string(), pc));
    }
    t
}

/// sink hit witnessed at (method, pc) calling `api`
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SinkWit {
    pub rule: usize,
    pub pc: usize,
    pub api: String,
}

/// inter-procedural summary consumed by callers
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// tainted param slot reaches a sink (possibly deeper, materialized here)
    pub param_sinks: BTreeMap<usize, BTreeSet<SinkWit>>,
    /// tainted param slot escapes into a static field
    pub param_statics: BTreeMap<usize, BTreeSet<String>>,
    /// tainted param slot taints the return value
    pub ret_from_params: BTreeSet<usize>,
    /// a local source taints the return value: tokens callers adopt on
    /// move-result of any call to this method
    pub ret_tokens: BTreeSet<Token>,
}

/// a finished taint path
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Finding {
    pub src_rule: usize,
    pub src_method: String,
    pub src_pc: usize,
    pub src_api: String,
    pub sink_rule: usize,
    pub sink_method: String,
    pub sink_pc: usize,
    pub sink_api: String,
    pub hops: Vec<(String, usize)>,
}

pub struct LocalOut {
    pub summary: Summary,
    pub findings: BTreeSet<Finding>,
    /// static fields this method pollutes with concrete source tokens
    pub static_publishes: BTreeMap<String, BTreeSet<Token>>,
}

fn record_sink(
    rules: &Rules,
    sink_method: &str,
    findings: &mut BTreeSet<Finding>,
    summary: &mut Summary,
    rule: usize,
    pc: usize,
    api: &str,
    tokens: impl Iterator<Item = Token>,
) {
    for tok in tokens {
        match &tok.origin {
            Origin::Src { rule: sr, method, pc: spc } => {
                findings.insert(Finding {
                    src_rule: *sr,
                    src_method: method.clone(),
                    src_pc: *spc,
                    src_api: rules.sources[*sr].label.clone(),
                    sink_rule: rule,
                    sink_method: sink_method.to_string(),
                    sink_pc: pc,
                    sink_api: api.to_string(),
                    hops: tok.hops.clone(),
                });
            }
            Origin::Param { slot, .. } => {
                summary
                    .param_sinks
                    .entry(*slot)
                    .or_default()
                    .insert(SinkWit { rule, pc, api: api.to_string() });
            }
        }
    }
}

/// analyze one method body against current summaries + static-field taint.
pub fn analyze(
    body: &MethodBody,
    rules: &Rules,
    summaries: &BTreeMap<String, Summary>,
    statics: &BTreeMap<String, BTreeSet<Token>>,
    dispatch: &DispatchIndex,
) -> LocalOut {
    let m = &body.sig;
    let nregs = body.registers_size as usize;
    let mut t: Vec<BTreeSet<Token>> = vec![BTreeSet::new(); nregs.max(1)];

    // seed params (context-insensitive: all slots, each its own origin)
    let base = body.param_base() as usize;
    for slot in 0..body.ins_size as usize {
        if base + slot < t.len() {
            t[base + slot].insert(Token {
                origin: Origin::Param { method: m.clone(), slot },
                hops: Vec::new(),
            });
        }
    }

    // param-anchored sources (AppShark-style callback entries): the declared
    // parameter of a matching callback override is born tainted
    if let Some((rule, pos)) = rules.param_source_for(m) {
        if let Some((_, slot)) = walk_params(m, pos) {
            let reg = base + slot;
            if reg < t.len() {
                insert_capped(
                    &mut t[reg],
                    Token {
                        origin: Origin::Src { rule, method: m.clone(), pc: 0 },
                        hops: Vec::new(),
                    },
                );
            }
        }
    }

    let mut summary = Summary::default();
    let mut findings = BTreeSet::new();
    let mut publishes: BTreeMap<String, BTreeSet<Token>> = BTreeMap::new();

    for _pass in 0..2 {
        // taint staged by an invoke for its move-result register
        let mut stage: BTreeSet<Token> = BTreeSet::new();
        for op in &body.ops {
            // defensive: obfuscated/invalid dex may reference registers outside
            // the frame after best-effort decoding — skip such ops entirely
            if op.max_reg().is_some_and(|m| m as usize >= t.len()) {
                if !matches!(op, Op::Invoke { .. }) {
                    stage.clear();
                }
                continue;
            }
            match op {
                Op::MoveResult { dst, .. } => {
                    let dsti = *dst as usize;
                    t[dsti].clear();
                    for tok in std::mem::take(&mut stage) {
                        insert_capped(&mut t[dsti], tok);
                    }
                    continue;
                }
                Op::Move { dst, src, .. } => {
                    reg_merge(&mut t, *dst as usize, *src as usize);
                }
                Op::Const { dst, .. } => {
                    t[*dst as usize].clear();
                }
                Op::Invoke { pc, callee, args, kind } => {
                    // dispatch candidates: the referenced signature plus every
                    // same-key override for virtual/interface/super/polymorphic
                    // invokes (conservative, type-analysis-free resolution)
                    let candidates: Vec<String> = if is_dispatch_kind(kind) {
                        dispatch.fanout(callee)
                    } else {
                        vec![callee.clone()]
                    };
                    // 1. arg_to_recv propagators: fold other args into the receiver
                    if matches!(rules.match_propagator(callee), Some(PropMode::ArgToRecv)) && args.len() > 1 {
                        let mut recv = t[args[0] as usize].clone();
                        for a in &args[1..] {
                            merge_into(&mut recv, &t[*a as usize]);
                        }
                        t[args[0] as usize] = recv;
                    }
                    // 2. sink hits over all dispatch candidates (an override
                    // may hold the sink the base-class call site misses)
                    for cand in &candidates {
                        if let Some(rule) = rules.match_sink(cand) {
                            let hit: Vec<Token> = args
                                .iter()
                                .flat_map(|a| t[*a as usize].iter().cloned())
                                .collect();
                            if !hit.is_empty() {
                                record_sink(
                                    rules,
                                    m,
                                    &mut findings,
                                    &mut summary,
                                    rule,
                                    *pc,
                                    cand,
                                    hit.clone().into_iter(),
                                );
                            }
                        }
                    }
                    // 3. result staging: source / propagator / sanitizer / filled array.
                    //    Sources fan out over candidates; propagators and
                    //    sanitizers stay exact-match (over-applying a sanitizer
                    //    would silently kill findings).
                    if callee == "#filled-new-array" {
                        for a in args {
                            merge_into(&mut stage, &t[*a as usize]);
                        }
                    } else {
                        let mut fired_source = false;
                        for cand in &candidates {
                            if let Some(rule) = rules.match_source(cand) {
                                stage.insert(Token {
                                    origin: Origin::Src { rule, method: m.clone(), pc: *pc },
                                    hops: Vec::new(),
                                });
                                fired_source = true;
                            }
                        }
                        if !fired_source {
                            if let Some(mode) = rules.match_propagator(callee) {
                        match mode {
                            // "returns this" style: fold back into recv AND
                            // stage it for a chained move-result
                            PropMode::ArgToRecv => {
                                if let Some(r) = args.first() {
                                    merge_into(&mut stage, t[*r as usize].iter());
                                }
                            }
                            PropMode::RecvToRet => {
                                if let Some(r) = args.first() {
                                    merge_into(&mut stage, &t[*r as usize]);
                                }
                            }
                            PropMode::RecvOrArgToRet | PropMode::ArgToRet => {
                                for a in args {
                                    merge_into(&mut stage, &t[*a as usize]);
                                }
                            }
                            }
                        }
                    }
                    }
                    if rules.is_sanitizer(callee) {
                        stage.clear();
                    }
                    // 4. inter-procedural: apply every dispatch candidate's summary
                    if !callee.starts_with('#') {
                        for cand in &candidates {
                            let Some(sum) = summaries.get(cand.as_str()) else { continue };
                            let has_recv = *kind != "static";
                            for (pos, a) in args.iter().enumerate() {
                                let arg_tokens: Vec<Token> = t[*a as usize].iter().cloned().collect();
                                if arg_tokens.is_empty() {
                                    continue;
                                }
                                let Some(slot) = slot_of_position(cand, pos, has_recv) else {
                                    continue;
                                };
                                if let Some(wits) = sum.param_sinks.get(&slot) {
                                    for w in wits {
                                        let toks = arg_tokens.iter().map(|tok| push_hop(tok, m, *pc));
                                        record_sink(
                                            rules,
                                            cand,
                                            &mut findings,
                                            &mut summary,
                                            w.rule,
                                            w.pc,
                                            &w.api,
                                            toks,
                                        );
                                    }
                                }
                                if let Some(fields) = sum.param_statics.get(&slot) {
                                    for f in fields {
                                        for tok in &arg_tokens {
                                            match &tok.origin {
                                                Origin::Src { .. } => {
                                                    publishes
                                                        .entry(f.clone())
                                                        .or_default()
                                                        .insert(push_hop(tok, m, *pc));
                                                }
                                                Origin::Param { slot: p, .. } => {
                                                    summary
                                                        .param_statics
                                                        .entry(*p)
                                                        .or_default()
                                                        .insert(f.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                                if sum.ret_from_params.contains(&slot) {
                                    for tok in &arg_tokens {
                                        stage.insert(push_hop(tok, m, *pc));
                                    }
                                }
                            }
                            // callee returns locally-sourced taint
                            for rt in &sum.ret_tokens {
                                stage.insert(push_hop(rt, m, *pc));
                            }
                        }
                    }
                }
                Op::FieldGet { dst, field, obj, pc } => match obj {
                    None => {
                        // static read: global taint first, then field-source rules
                        if let Some(toks) = statics.get(field) {
                            for tok in toks {
                                insert_capped(&mut t[*dst as usize], tok.clone());
                            }
                        } else if let Some(rule) = rules.match_field_source(field) {
                            insert_capped(
                                &mut t[*dst as usize],
                                Token {
                                    origin: Origin::Src { rule, method: m.clone(), pc: *pc },
                                    hops: Vec::new(),
                                },
                            );
                        }
                    }
                    Some(o) => {
                        reg_merge(&mut t, *dst as usize, *o as usize);
                        if let Some(rule) = rules.match_field_source(field) {
                            insert_capped(
                                &mut t[*dst as usize],
                                Token {
                                    origin: Origin::Src { rule, method: m.clone(), pc: *pc },
                                    hops: Vec::new(),
                                },
                            );
                        }
                    }
                },
                Op::FieldPut { src, field, obj, pc } => {
                    let toks: Vec<Token> = t[*src as usize].iter().cloned().collect();
                    if !toks.is_empty() {
                        if let Some(rule) = rules.match_field_sink(field) {
                            record_sink(rules, m, &mut findings, &mut summary, rule, *pc, field, toks.clone().into_iter());
                        }
                        for tok in &toks {
                            match &tok.origin {
                                Origin::Src { .. } => {
                                    publishes.entry(field.clone()).or_default().insert(tok.clone());
                                }
                                Origin::Param { slot, .. } => {
                                    summary
                                        .param_statics
                                        .entry(*slot)
                                        .or_default()
                                        .insert(field.clone());
                                }
                            }
                        }
                    }
                    if let Some(o) = obj {
                        reg_merge(&mut t, *o as usize, *src as usize);
                    }
                }
                Op::ArrayGet { dst, arr, .. } => reg_merge(&mut t, *dst as usize, *arr as usize),
                Op::ArrayPut { src, arr, .. } => reg_merge(&mut t, *arr as usize, *src as usize),
                Op::BinOp { dst, a, b, .. } => {
                    reg_merge(&mut t, *dst as usize, *a as usize);
                    reg_merge(&mut t, *dst as usize, *b as usize);
                }
                Op::UnOp { dst, src, .. } => reg_merge(&mut t, *dst as usize, *src as usize),
                Op::Return { src: Some(r), .. } => {
                    for tok in &t[*r as usize] {
                        match &tok.origin {
                            Origin::Src { .. } => {
                                summary.ret_tokens.insert(tok.clone());
                            }
                            Origin::Param { slot, .. } => {
                                summary.ret_from_params.insert(*slot);
                            }
                        }
                    }
                }
                Op::Return { src: None, .. } | Op::Other { .. } => {}
            }
            // a staged result only survives into an immediately following move-result
            if !matches!(op, Op::Invoke { .. }) {
                stage.clear();
            }
        }
    }

    LocalOut {
        summary,
        findings,
        static_publishes: publishes,
    }
}

fn merge_into<'a>(dst: &mut BTreeSet<Token>, src: impl IntoIterator<Item = &'a Token>) {
    for tok in src {
        insert_capped(dst, tok.clone());
    }
}

/// t[dst] ∪= t[src] — clones the source side first to satisfy the borrow checker
fn reg_merge(t: &mut [BTreeSet<Token>], dst: usize, src: usize) {
    if dst == src || src >= t.len() || dst >= t.len() {
        return;
    }
    let snap = t[src].clone();
    merge_into(&mut t[dst], snap.iter());
}

fn insert_capped(set: &mut BTreeSet<Token>, tok: Token) {
    if set.contains(&tok) {
        return;
    }
    if set.len() >= MAX_TOKENS_PER_REG {
        if let Some(last) = set.iter().next_back().cloned() {
            if last <= tok {
                return;
            }
            set.pop_last();
        }
    }
    set.insert(tok);
}

/// map caller argument position -> callee parameter slot, walking the
/// callee descriptor (wide J/D params occupy two slots). Receiver (when
/// present) is slot 0.
fn slot_of_position(callee_sig: &str, pos: usize, has_receiver: bool) -> Option<usize> {
    if has_receiver {
        if pos == 0 {
            return Some(0);
        }
        walk_params(callee_sig, pos - 1).map(|(_, slot)| slot + 1)
    } else {
        walk_params(callee_sig, pos).map(|(_, slot)| slot)
    }
}

/// nth parameter (0-based) -> (is_wide, slot offset from params start)
fn walk_params(callee_sig: &str, want: usize) -> Option<(bool, usize)> {
    let open = callee_sig.find('(')?;
    let close = callee_sig.rfind(')')?;
    if close <= open {
        return None;
    }
    let mut types = &callee_sig[open + 1..close];
    let mut pos = 0usize;
    let mut slot = 0usize;
    while !types.is_empty() {
        let c = types.as_bytes()[0];
        let (wide, len) = match c {
            b'J' | b'D' => (true, 1),
            b'V' | b'Z' | b'B' | b'C' | b'S' | b'I' | b'F' => (false, 1),
            b'L' => (false, types.find(';').map(|i| i + 1).unwrap_or(1)),
            b'[' => {
                let n_arr = types.len() - types.trim_start_matches('[').len();
                let elem = &types[n_arr..];
                let el = if elem.starts_with('L') {
                    elem.find(';').map(|i| i + 1).unwrap_or(1)
                } else {
                    1
                };
                (false, n_arr + el)
            }
            _ => (false, 1),
        };
        if pos == want {
            return Some((wide, slot));
        }
        pos += 1;
        slot += if wide { 2 } else { 1 };
        types = &types[len..];
    }
    None
}
