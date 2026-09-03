//! Pass framework (jadx visitors equivalent).
//!
//! Passes transform method IR (`Vec<IrStmt>`) in sequence. Built-in passes handle
//! invoke+move-result+return folding and similar patterns.

use crate::decompile::ir::VarId;
use crate::decompile::ir::{Expr as IrExpr, Stmt as IrStmt};
use std::collections::{HashMap, HashSet};

/// A single transformation pass over method IR.
pub trait Pass {
    /// Transform the statement list; may replace, remove, or add statements.
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt>;
}

/// Runs a sequence of passes in order (jadx-style pipeline).
#[derive(Default)]
pub struct PassRunner {
    passes: Vec<Box<dyn Pass + Send>>,
}

impl PassRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pass to the pipeline (runs after previously added passes).
    pub fn add<P: Pass + Send + 'static>(&mut self, pass: P) {
        self.passes.push(Box::new(pass));
    }

    /// Run all passes in order on the given IR.
    pub fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        let mut current = stmts;
        for pass in &self.passes {
            current = pass.run(current);
        }
        current
    }
}

/// Merges invoke + move-result + return into single statements:
/// - `Expr(Call)` + `Assign(reg, PendingResult)` → `Assign(reg, Call)`
/// - `Expr(Raw)` + `Assign(reg, PendingResult)` → `Assign(reg, Raw)` (lambdas)
/// - `Assign(reg, rhs)` + `Return(Var(reg))` → `Return(rhs)` (literals, calls, …)
/// - `Expr(Call)` + `Return(None)` → left as-is (call; return;)
#[derive(Debug, Clone, Copy, Default)]
pub struct InvokeChainPass;

impl Pass for InvokeChainPass {
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        let mut out: Vec<IrStmt> = Vec::with_capacity(stmts.len());
        let mut i = 0usize;
        while i < stmts.len() {
            // Try: Expr(Call) + Assign(reg, PendingResult) + Return(Var(reg)) → Return(Call)
            if i + 2 < stmts.len() {
                if let (
                    IrStmt::Expr {
                        expr: IrExpr::Call { target, args },
                        comment: c1,
                    },
                    IrStmt::Assign {
                        dst,
                        rhs: IrExpr::PendingResult,
                        comment: c2,
                    },
                    IrStmt::Return {
                        value: Some(IrExpr::Var(ret_reg)),
                        comment: c3,
                    },
                ) = (&stmts[i], &stmts[i + 1], &stmts[i + 2])
                {
                    if dst == ret_reg {
                        let comment = merge_comment(
                            c1.as_deref(),
                            merge_comment(c2.as_deref(), c3.as_deref()).as_deref(),
                        );
                        let args = snapshot_invoke_args_overwritten_by_result(args, *dst, &out);
                        out.push(IrStmt::Return {
                            value: Some(IrExpr::Call {
                                target: target.clone(),
                                args,
                            }),
                            comment,
                        });
                        i += 3;
                        continue;
                    }
                }
            }

            // Try: Expr(Call) + Assign(reg, PendingResult) → Assign(reg, Call)
            if i + 1 < stmts.len() {
                if let (
                    IrStmt::Expr {
                        expr: IrExpr::Call { target, args },
                        comment: c1,
                    },
                    IrStmt::Assign {
                        dst,
                        rhs: IrExpr::PendingResult,
                        comment: c2,
                    },
                ) = (&stmts[i], &stmts[i + 1])
                {
                    let comment = merge_comment(c1.as_deref(), c2.as_deref());
                    let args = snapshot_invoke_args_overwritten_by_result(args, *dst, &out);
                    out.push(IrStmt::Assign {
                        dst: *dst,
                        rhs: IrExpr::Call {
                            target: target.clone(),
                            args,
                        },
                        comment,
                    });
                    i += 2;
                    continue;
                }
                // Expr(Raw) + Assign(PendingResult) → Assign(Raw) — lambda / invoke-custom
                if let (
                    IrStmt::Expr {
                        expr: IrExpr::Raw(raw),
                        comment: c1,
                    },
                    IrStmt::Assign {
                        dst,
                        rhs: IrExpr::PendingResult,
                        comment: c2,
                    },
                ) = (&stmts[i], &stmts[i + 1])
                {
                    let comment = merge_comment(c1.as_deref(), c2.as_deref());
                    out.push(IrStmt::Assign {
                        dst: *dst,
                        rhs: IrExpr::Raw(raw.clone()),
                        comment,
                    });
                    i += 2;
                    continue;
                }
            }

            // Try: Assign(reg, any) + Return(Var(reg)) → Return(any)
            // Covers Call and Raw/literals: `result = "bad_name"; return result;` → `return "bad_name";`
            if i + 1 < stmts.len() {
                if let (
                    IrStmt::Assign {
                        dst,
                        rhs,
                        comment: c1,
                    },
                    IrStmt::Return {
                        value: Some(IrExpr::Var(ret_reg)),
                        comment: c2,
                    },
                ) = (&stmts[i], &stmts[i + 1])
                {
                    if dst == ret_reg && !matches!(rhs, IrExpr::PendingResult) {
                        let comment = merge_comment(c1.as_deref(), c2.as_deref());
                        out.push(IrStmt::Return {
                            value: Some(rhs.clone()),
                            comment,
                        });
                        i += 2;
                        continue;
                    }
                }
            }

            out.push(stmts[i].clone());
            i += 1;
        }
        out
    }
}

/// Dalvik `invoke` + `move-result` into an argument register: `v9 = foo(..., v9)`.
/// Substitute the pre-call numeric const so the arg is not the post-call dest.
fn snapshot_invoke_args_overwritten_by_result(args: &str, dst: VarId, prior: &[IrStmt]) -> String {
    let names = if dst.ver == 0 {
        vec![format!("v{}", dst.reg)]
    } else {
        vec![format!("v{}_{}", dst.reg, dst.ver), format!("v{}", dst.reg)]
    };
    if !names.iter().any(|n| ident_in_arg_list(args, n)) {
        return args.to_string();
    }
    let Some(lit) = last_numeric_const_for_reg(prior, dst.reg) else {
        return args.to_string();
    };
    let mut out = args.to_string();
    for name in names {
        out = replace_whole_ident(&out, &name, &lit);
    }
    out
}

fn ident_in_arg_list(args: &str, name: &str) -> bool {
    if name.is_empty() || !args.contains(name) {
        return false;
    }
    replace_whole_ident(args, name, "\u{1}") != args
}

fn last_numeric_const_for_reg(stmts: &[IrStmt], reg: u32) -> Option<String> {
    for s in stmts.iter().rev() {
        if let IrStmt::Assign { dst, rhs, .. } = s {
            if dst.reg != reg {
                continue;
            }
            if let IrExpr::Raw(raw) = rhs {
                if is_numeric_const_rhs(raw) {
                    return Some(raw.trim().to_string());
                }
            }
            // A later non-const def (array, call result) kills the old numeric value.
            return None;
        }
    }
    None
}

fn merge_comment(a: Option<&str>, b: Option<&str>) -> Option<String> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x.to_string()),
        (Some(x), Some(y)) => Some(format!("{} | {}", x, y)),
    }
}

/// Identity pass: returns IR unchanged (useful for testing or default pipeline).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityPass;

impl Pass for IdentityPass {
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        stmts
    }
}

/// Removes assigns whose destination is never used (dead store elimination).
/// When run per-block, only sees uses in that block; use `run_with_used_regs` when
/// emitting CFG so assigns used in other blocks are not removed.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeadAssignPass;

impl Pass for DeadAssignPass {
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        let used = used_var_ids(&stmts);
        stmts
            .into_iter()
            .filter(|s| {
                if matches!(s, IrStmt::Phi { .. }) {
                    return false;
                }
                if let IrStmt::Assign { dst, .. } = s {
                    used.contains(dst)
                } else {
                    true
                }
            })
            .collect()
    }
}

/// Dead-assign using a precomputed set of used *register numbers* (not VarId).
/// Keeps Assign if dst.reg is in used_regs. Use when emitting CFG so assigns
/// whose destination is used in other blocks are not removed.
pub fn run_dead_assign_with_used_regs(stmts: Vec<IrStmt>, used_regs: &HashSet<u32>) -> Vec<IrStmt> {
    stmts
        .into_iter()
        .filter(|s| {
            if matches!(s, IrStmt::Phi { .. }) {
                return false;
            }
            if let IrStmt::Assign { dst, rhs, .. } = s {
                // Keep numeric const defs — register reuse makes used_regs miss SSA versions.
                if matches!(rhs, IrExpr::Raw(r) if is_numeric_const_rhs(r)) {
                    return true;
                }
                used_regs.contains(&dst.reg)
            } else {
                true
            }
        })
        .collect()
}

fn is_numeric_const_rhs(rhs: &str) -> bool {
    let s = rhs.trim();
    if s.is_empty() {
        return false;
    }
    if s.starts_with("0x") || s.starts_with("0X") {
        return s[2..].chars().all(|c| c.is_ascii_hexdigit());
    }
    let mut chars = s.chars();
    if chars.next() == Some('-') {
        return chars.all(|c| c.is_ascii_digit());
    }
    s.chars().all(|c| c.is_ascii_digit())
}

/// Collect all register numbers that are read (used) in the IR.
pub fn used_regs(stmts: &[IrStmt]) -> HashSet<u32> {
    used_var_ids(stmts).iter().map(|v| v.reg).collect()
}

/// Collect all VarIds that are read (used) in the IR (in RHS, Return, Expr).
/// Includes VarIds mentioned in Raw strings and Call args so dead-assign doesn't remove defs that are only used there.
fn used_var_ids(stmts: &[IrStmt]) -> HashSet<VarId> {
    let mut set = HashSet::new();
    for s in stmts {
        match s {
            IrStmt::Assign { rhs, .. } => collect_var_ids_expr(rhs, &mut set),
            IrStmt::Expr { expr, .. } => collect_var_ids_expr(expr, &mut set),
            IrStmt::Return { value: Some(e), .. } => collect_var_ids_expr(e, &mut set),
            IrStmt::Return { value: None, .. } => {}
            IrStmt::Raw(s) => var_ids_in_text(s, &mut set),
            IrStmt::Phi { incomings, .. } => {
                for (_, v) in incomings {
                    set.insert(*v);
                }
            }
        }
    }
    set
}

fn collect_var_ids_expr(expr: &IrExpr, set: &mut HashSet<VarId>) {
    match expr {
        IrExpr::Var(v) => {
            set.insert(*v);
        }
        IrExpr::Call { target, args } => {
            // Receiver lives in `target` (`v0.processLogin`), not only in `args`.
            var_ids_in_text(target, set);
            var_ids_in_text(args, set);
        }
        IrExpr::PendingResult => {}
        IrExpr::Raw(s) => var_ids_in_text(s, set),
    }
}

/// Collect all VarIds mentioned in text (vN or vN_k).
fn var_ids_in_text(s: &str, set: &mut HashSet<VarId>) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'v' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            i += 1;
            let mut reg: u32 = 0;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                reg = reg * 10 + (bytes[i] - b'0') as u32;
                i += 1;
            }
            let ver = if i + 1 < bytes.len() && bytes[i] == b'_' && bytes[i + 1].is_ascii_digit() {
                i += 1;
                let mut v: u32 = 0;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    v = v * 10 + (bytes[i] - b'0') as u32;
                    i += 1;
                }
                v
            } else {
                0
            };
            set.insert(VarId::new(reg, ver));
            continue;
        }
        i += 1;
    }
}

/// Propagate SSA copies (`v0_1 = v3`) so later uses see the source register, then
/// drop the copy assigns. D8's accessor prologue is `move-object v0, p0` before invoke.
#[derive(Debug, Clone, Copy, Default)]
pub struct CopyPropPass;

impl Pass for CopyPropPass {
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        let mut alias: HashMap<VarId, VarId> = HashMap::new();
        for stmt in &stmts {
            if let IrStmt::Assign { dst, rhs, .. } = stmt {
                if let Some(src) = copy_source(rhs) {
                    if src != *dst {
                        alias.insert(*dst, src);
                    }
                }
            }
        }
        if alias.is_empty() {
            return stmts;
        }
        let mut resolved: HashMap<VarId, VarId> = HashMap::new();
        for &dst in alias.keys() {
            let mut cur = dst;
            let mut seen: HashSet<VarId> = HashSet::new();
            while let Some(&next) = alias.get(&cur) {
                if !seen.insert(cur) {
                    break;
                }
                cur = next;
            }
            if cur != dst {
                resolved.insert(dst, cur);
            }
        }
        if resolved.is_empty() {
            return stmts;
        }
        let rewritten: Vec<IrStmt> = stmts
            .iter()
            .map(|stmt| rewrite_stmt(stmt, &resolved))
            .collect();
        (0..stmts.len())
            .filter_map(|idx| {
                if let IrStmt::Assign { dst, .. } = &stmts[idx] {
                    if resolved.contains_key(dst) {
                        if copy_assign_must_keep(idx, *dst, &stmts, &rewritten, &resolved) {
                            // Do not rewrite kept copies: `a = t` must not become `a = b`
                            // after `b = a % b` when t was the old b.
                            return Some(stmts[idx].clone());
                        }
                        return None;
                    }
                }
                Some(rewritten[idx].clone())
            })
            .collect()
    }
}

/// Keep `dst = src` when removing it would break swap patterns (Euclid GCD) or loop-carried slots.
fn copy_assign_must_keep(
    idx: usize,
    dst: VarId,
    stmts: &[IrStmt],
    rewritten: &[IrStmt],
    resolved: &HashMap<VarId, VarId>,
) -> bool {
    let reg = dst.reg;
    if reg_used_in_range(rewritten, idx + 1, rewritten.len(), reg) {
        return true;
    }
    let reads = used_regs(stmts);
    if is_last_assign_reg(stmts, idx, reg)
        && reads.contains(&reg)
        && reg_used_in_range(rewritten, 0, idx, reg)
    {
        return true;
    }
    if let Some(&src) = resolved.get(&dst) {
        let end = first_reg_use_after(stmts, idx, reg).unwrap_or(stmts.len());
        if reg_non_copy_redefined(stmts, idx + 1, end, src.reg) {
            return true;
        }
    }
    false
}

fn rewrite_stmt(stmt: &IrStmt, resolved: &HashMap<VarId, VarId>) -> IrStmt {
    match stmt {
        IrStmt::Assign { dst, rhs, comment } => IrStmt::Assign {
            dst: *dst,
            rhs: rewrite_expr(rhs.clone(), resolved),
            comment: comment.clone(),
        },
        IrStmt::Expr { expr, comment } => IrStmt::Expr {
            expr: rewrite_expr(expr.clone(), resolved),
            comment: comment.clone(),
        },
        IrStmt::Return { value, comment } => IrStmt::Return {
            value: value.clone().map(|e| rewrite_expr(e, resolved)),
            comment: comment.clone(),
        },
        IrStmt::Phi { dst, incomings } => IrStmt::Phi {
            dst: *dst,
            incomings: incomings
                .iter()
                .map(|(b, v)| (*b, resolve_vid(*v, resolved)))
                .collect(),
        },
        IrStmt::Raw(s) => IrStmt::Raw(rewrite_varids_in_text(s, resolved)),
    }
}

fn reg_used_in_range(stmts: &[IrStmt], start: usize, end: usize, reg: u32) -> bool {
    if start >= end || start >= stmts.len() {
        return false;
    }
    used_regs(&stmts[start..end.min(stmts.len())]).contains(&reg)
}

fn is_last_assign_reg(stmts: &[IrStmt], idx: usize, reg: u32) -> bool {
    !stmts[idx + 1..]
        .iter()
        .any(|s| matches!(s, IrStmt::Assign { dst, .. } if dst.reg == reg))
}

fn reg_non_copy_redefined(stmts: &[IrStmt], from: usize, to: usize, reg: u32) -> bool {
    if from >= to || from >= stmts.len() {
        return false;
    }
    stmts[from..to.min(stmts.len())].iter().any(|s| {
        matches!(
            s,
            IrStmt::Assign { dst, rhs, .. }
                if dst.reg == reg && copy_source(rhs).is_none()
        )
    })
}

fn first_reg_use_after(stmts: &[IrStmt], from: usize, reg: u32) -> Option<usize> {
    for (j, s) in stmts.iter().enumerate().skip(from + 1) {
        if used_regs(std::slice::from_ref(s)).contains(&reg) {
            return Some(j);
        }
    }
    None
}

fn copy_source(rhs: &IrExpr) -> Option<VarId> {
    match rhs {
        IrExpr::Var(v) => Some(*v),
        IrExpr::Raw(s) => parse_bare_varid(s.trim()),
        _ => None,
    }
}

fn parse_bare_varid(s: &str) -> Option<VarId> {
    let s = s.trim();
    if !s.starts_with('v') {
        return None;
    }
    let rest = &s[1..];
    let bytes = rest.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let reg: u32 = rest[..i].parse().ok()?;
    if i == rest.len() {
        return Some(VarId::new(reg, 0));
    }
    if bytes[i] != b'_' {
        return None;
    }
    let ver_s = &rest[i + 1..];
    if ver_s.is_empty() || !ver_s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let ver: u32 = ver_s.parse().ok()?;
    Some(VarId::new(reg, ver))
}

fn resolve_vid(v: VarId, map: &HashMap<VarId, VarId>) -> VarId {
    map.get(&v).copied().unwrap_or(v)
}

fn rewrite_expr(expr: IrExpr, map: &HashMap<VarId, VarId>) -> IrExpr {
    match expr {
        IrExpr::Var(v) => IrExpr::Var(resolve_vid(v, map)),
        IrExpr::Call { target, args } => IrExpr::Call {
            target: rewrite_varids_in_text(&target, map),
            args: rewrite_varids_in_text(&args, map),
        },
        IrExpr::Raw(s) => IrExpr::Raw(rewrite_varids_in_text(&s, map)),
        other => other,
    }
}

fn format_varid(v: VarId) -> String {
    if v.ver == 0 {
        format!("v{}", v.reg)
    } else {
        format!("v{}_{}", v.reg, v.ver)
    }
}

fn rewrite_varids_in_text(s: &str, map: &HashMap<VarId, VarId>) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'v' {
            let word_start = i == 0 || {
                let p = bytes[i - 1] as char;
                !(p.is_ascii_alphanumeric() || p == '_')
            };
            if word_start {
                let mut j = i + 1;
                if j < bytes.len() && bytes[j].is_ascii_digit() {
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    let reg: u32 = s[i + 1..j].parse().unwrap_or(0);
                    let mut ver = 0u32;
                    if j < bytes.len()
                        && bytes[j] == b'_'
                        && j + 1 < bytes.len()
                        && bytes[j + 1].is_ascii_digit()
                    {
                        let ver_start = j + 1;
                        j += 1;
                        while j < bytes.len() && bytes[j].is_ascii_digit() {
                            j += 1;
                        }
                        ver = s[ver_start..j].parse().unwrap_or(0);
                    }
                    let vid = VarId::new(reg, ver);
                    if let Some(&repl) = map.get(&vid) {
                        out.push_str(&format_varid(repl));
                        i = j;
                        continue;
                    }
                    out.push_str(&s[i..j]);
                    i = j;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Linear SSA renaming for a flat statement list (no CFG).
///
/// For CFG-aware SSA with φ-nodes, see [`crate::decompile::ssa::construct_ssa`]
/// (used by `--decompilation-mode simple` and for φ-register naming in restructure).
#[derive(Debug, Clone, Copy, Default)]
pub struct SsaRenamePass;

impl Pass for SsaRenamePass {
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        let mut next_ver: HashMap<u32, u32> = HashMap::new();
        let mut cur_ver: HashMap<u32, u32> = HashMap::new();

        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            match stmt {
                IrStmt::Assign {
                    mut dst,
                    rhs,
                    comment,
                } => {
                    let rhs = rename_expr(rhs, &cur_ver);
                    let v = next_ver.entry(dst.reg).or_insert(0);
                    *v += 1;
                    dst.ver = *v;
                    cur_ver.insert(dst.reg, dst.ver);
                    out.push(IrStmt::Assign { dst, rhs, comment });
                }
                IrStmt::Expr { expr, comment } => {
                    out.push(IrStmt::Expr {
                        expr: rename_expr(expr, &cur_ver),
                        comment,
                    });
                }
                IrStmt::Return { value, comment } => {
                    let value = value.map(|e| rename_expr(e, &cur_ver));
                    out.push(IrStmt::Return { value, comment });
                }
                IrStmt::Phi { mut dst, incomings } => {
                    // Flat-list pass: treat phi as a def (should not appear in linear mode).
                    let v = next_ver.entry(dst.reg).or_insert(0);
                    *v += 1;
                    dst.ver = *v;
                    cur_ver.insert(dst.reg, dst.ver);
                    out.push(IrStmt::Phi { dst, incomings });
                }
                IrStmt::Raw(s) => out.push(IrStmt::Raw(rename_vars_in_text(&s, &cur_ver))),
            }
        }
        out
    }
}

fn rename_expr(expr: IrExpr, cur_ver: &HashMap<u32, u32>) -> IrExpr {
    match expr {
        IrExpr::Var(v) => IrExpr::Var(rename_var(v, cur_ver)),
        IrExpr::Call { target, args } => IrExpr::Call {
            target: rename_vars_in_text(&target, cur_ver),
            args: rename_vars_in_text(&args, cur_ver),
        },
        IrExpr::PendingResult => IrExpr::PendingResult,
        IrExpr::Raw(s) => IrExpr::Raw(rename_vars_in_text(&s, cur_ver)),
    }
}

fn rename_var(v: VarId, cur_ver: &HashMap<u32, u32>) -> VarId {
    let ver = cur_ver.get(&v.reg).copied().unwrap_or(0);
    VarId::new(v.reg, ver)
}

/// Best-effort text rewrite: `vN` -> `vN_k` if current version for N is k>0.
/// Leaves already-versioned `vN_k` unchanged.
pub(crate) fn rename_vars_in_text_public(s: &str, cur_ver: &HashMap<u32, u32>) -> String {
    rename_vars_in_text(s, cur_ver)
}

/// Best-effort text rewrite: `vN` -> `vN_k` if current version for N is k>0.
/// Leaves already-versioned `vN_k` unchanged.
fn rename_vars_in_text(s: &str, cur_ver: &HashMap<u32, u32>) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'v' {
            // Ensure word boundary-ish: previous char not [A-Za-z0-9_]
            if i > 0 {
                let p = bytes[i - 1] as char;
                if p.is_ascii_alphanumeric() || p == '_' {
                    out.push('v');
                    i += 1;
                    continue;
                }
            }
            let mut j = i + 1;
            if j >= bytes.len() || !((bytes[j] as char).is_ascii_digit()) {
                out.push('v');
                i += 1;
                continue;
            }
            while j < bytes.len() && ((bytes[j] as char).is_ascii_digit()) {
                j += 1;
            }
            // If already versioned, keep as-is.
            if j < bytes.len() && bytes[j] == b'_' {
                out.push_str(&s[i..j]);
                // consume '_' and following digits
                let mut k = j + 1;
                while k < bytes.len() && ((bytes[k] as char).is_ascii_digit()) {
                    k += 1;
                }
                out.push_str(&s[j..k]);
                i = k;
                continue;
            }
            let reg: u32 = s[i + 1..j].parse().unwrap_or(0);
            let ver = cur_ver.get(&reg).copied().unwrap_or(0);
            if ver == 0 {
                out.push_str(&s[i..j]);
            } else {
                out.push_str(&format!("v{}_{}", reg, ver));
            }
            i = j;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Merge `new Foo()` + `receiver.<init>(args)` → `new Foo(args)`, and remove the bare `<init>` call.
/// Allows intervening simple assignments (e.g. `const-class`) that don't redefine the receiver.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConstructorMergePass;

impl Pass for ConstructorMergePass {
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        let mut out: Vec<IrStmt> = Vec::with_capacity(stmts.len());
        let mut i = 0;
        while i < stmts.len() {
            if let Some((merged, next_i)) = try_merge_constructor(&stmts, i) {
                out.extend(merged);
                i = next_i;
                continue;
            }
            out.push(stmts[i].clone());
            i += 1;
        }
        out
    }
}

fn call_is_init_for_reg(target: &str, reg: u32) -> bool {
    // vN.<init> or vN_k.<init>
    let Some((recv, meth)) = target.rsplit_once('.') else {
        return false;
    };
    if meth != "<init>" {
        return false;
    }
    if let Some(r) = recv.strip_prefix('v') {
        let reg_str = r.split('_').next().unwrap_or(r);
        return reg_str.parse::<u32>().ok() == Some(reg);
    }
    false
}

fn try_merge_constructor(stmts: &[IrStmt], i: usize) -> Option<(Vec<IrStmt>, usize)> {
    let IrStmt::Assign {
        dst,
        rhs: IrExpr::Raw(raw),
        comment,
    } = &stmts[i]
    else {
        return None;
    };
    let class_name = raw
        .strip_prefix("new ")
        .and_then(|s| s.strip_suffix("()"))?;
    let dst_reg = dst.reg;
    let mut intervening: Vec<IrStmt> = Vec::new();
    let mut j = i + 1;
    while j < stmts.len() {
        match &stmts[j] {
            IrStmt::Assign { dst: mid, .. } if mid.reg == dst_reg => return None,
            IrStmt::Expr {
                expr: IrExpr::Call { target, args },
                comment: c2,
            } if call_is_init_for_reg(target, dst_reg) => {
                let merged_comment = merge_comment(comment.as_deref(), c2.as_deref());
                // Inline intervening const-class / const-string temps into constructor args so
                // we don't emit `new Intent(this, local0)` before `local0 = Foo.class`.
                let mut args = args.clone();
                let mut keep_intervening: Vec<IrStmt> = Vec::new();
                for inter in &intervening {
                    if let IrStmt::Assign {
                        dst: idst,
                        rhs: IrExpr::Raw(rval),
                        ..
                    } = inter
                    {
                        let inlined = inline_var_in_args(&args, *idst, rval);
                        if inlined != args {
                            args = inlined;
                            continue;
                        }
                    }
                    keep_intervening.push(inter.clone());
                }
                let mut out = keep_intervening;
                out.push(IrStmt::Assign {
                    dst: *dst,
                    rhs: IrExpr::Raw(format!("new {}({})", class_name, args)),
                    comment: merged_comment,
                });
                return Some((out, j + 1));
            }
            s @ IrStmt::Assign { .. } => {
                intervening.push(s.clone());
                j += 1;
            }
            _ => return None,
        }
    }
    None
}

/// Replace `vN` / `vN_k` for `vid` in a comma-separated arg list with `rval`.
fn inline_var_in_args(args: &str, vid: VarId, rval: &str) -> String {
    let names = if vid.ver == 0 {
        vec![format!("v{}", vid.reg)]
    } else {
        vec![format!("v{}_{}", vid.reg, vid.ver), format!("v{}", vid.reg)]
    };
    let mut out = args.to_string();
    for name in names {
        out = replace_whole_ident(&out, &name, rval);
    }
    out
}

fn replace_whole_ident(text: &str, var: &str, replacement: &str) -> String {
    if var.is_empty() || !text.contains(var) {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let v = var.as_bytes();
    let mut out = String::with_capacity(text.len() + replacement.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + v.len() <= bytes.len() && &bytes[i..i + v.len()] == v {
            let before_ok =
                i == 0 || !(bytes[i - 1] as char).is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            let after_ok = i + v.len() == bytes.len()
                || (!(bytes[i + v.len()] as char).is_ascii_alphanumeric()
                    && bytes[i + v.len()] != b'_');
            if before_ok && after_ok {
                out.push_str(replacement);
                i += v.len();
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Expression simplification: `v0 = v0 + 1` → `v0++`, `v0 = v0 + x` → `v0 += x`.
/// Also simplifies assignments where dst == src (self-assign after copy-prop).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExprSimplifyPass;

impl Pass for ExprSimplifyPass {
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        stmts
            .into_iter()
            .map(|s| match &s {
                IrStmt::Assign {
                    dst,
                    rhs: IrExpr::Raw(raw),
                    comment,
                } => {
                    if let Some(simplified) = simplify_compound_assign(*dst, raw) {
                        IrStmt::Assign {
                            dst: *dst,
                            rhs: IrExpr::Raw(simplified),
                            comment: comment.clone(),
                        }
                    } else {
                        s
                    }
                }
                _ => s,
            })
            .collect()
    }
}

/// Inline `sget` / const defs into `new T[]{ v0_k, … }` (enum `$values()` and similar).
#[derive(Debug, Clone, Copy, Default)]
pub struct InlineFilledArrayPass;

impl Pass for InlineFilledArrayPass {
    fn run(&self, stmts: Vec<IrStmt>) -> Vec<IrStmt> {
        let mut defs: HashMap<VarId, String> = HashMap::new();
        for s in &stmts {
            if let IrStmt::Assign {
                dst,
                rhs: IrExpr::Raw(r),
                ..
            } = s
            {
                if is_inlinable_array_elem_def(r) {
                    defs.insert(*dst, r.trim().to_string());
                }
            }
        }
        if defs.is_empty() {
            return stmts;
        }
        let mut inlined_from: HashSet<VarId> = HashSet::new();
        let mut rewrites: HashMap<usize, String> = HashMap::new();
        for (idx, s) in stmts.iter().enumerate() {
            let raw = match s {
                IrStmt::Assign {
                    rhs: IrExpr::Raw(r),
                    ..
                } => Some(r.as_str()),
                IrStmt::Return {
                    value: Some(IrExpr::Raw(r)),
                    ..
                } => Some(r.as_str()),
                _ => None,
            };
            if let Some(r) = raw {
                if let Some(new_r) = try_inline_array_literal(r, &defs, &mut inlined_from) {
                    rewrites.insert(idx, new_r);
                }
            }
        }
        if rewrites.is_empty() {
            return stmts;
        }
        let mut out: Vec<IrStmt> = Vec::with_capacity(stmts.len());
        for (idx, s) in stmts.iter().enumerate() {
            if let IrStmt::Assign { dst, .. } = s {
                if inlined_from.contains(dst)
                    && !vid_used_outside_inlined_arrays(&stmts, *dst, &rewrites)
                {
                    continue;
                }
            }
            if let Some(new_r) = rewrites.get(&idx) {
                match s {
                    IrStmt::Assign { dst, comment, .. } => {
                        out.push(IrStmt::Assign {
                            dst: *dst,
                            rhs: IrExpr::Raw(new_r.clone()),
                            comment: comment.clone(),
                        });
                    }
                    IrStmt::Return { comment, .. } => {
                        out.push(IrStmt::Return {
                            value: Some(IrExpr::Raw(new_r.clone())),
                            comment: comment.clone(),
                        });
                    }
                    other => out.push(other.clone()),
                }
            } else {
                out.push(s.clone());
            }
        }
        out
    }
}

fn vid_used_outside_inlined_arrays(
    stmts: &[IrStmt],
    vid: VarId,
    array_rewrites: &HashMap<usize, String>,
) -> bool {
    for (idx, s) in stmts.iter().enumerate() {
        if array_rewrites.contains_key(&idx) {
            continue;
        }
        if let IrStmt::Assign { dst, .. } = s {
            if *dst == vid {
                continue;
            }
        }
        let mut used = HashSet::new();
        match s {
            IrStmt::Assign { rhs, .. } => collect_var_ids_expr(rhs, &mut used),
            IrStmt::Expr { expr, .. } => collect_var_ids_expr(expr, &mut used),
            IrStmt::Return { value: Some(e), .. } => collect_var_ids_expr(e, &mut used),
            IrStmt::Raw(t) => var_ids_in_text(t, &mut used),
            _ => {}
        }
        if used.contains(&vid) {
            return true;
        }
    }
    false
}

fn is_inlinable_array_elem_def(r: &str) -> bool {
    let r = r.trim();
    if r.is_empty() || r.contains('(') || r.contains(' ') || r.starts_with("new ") {
        return false;
    }
    if matches!(r, "null" | "true" | "false") {
        return true;
    }
    if r.starts_with('"') || r.starts_with('\'') {
        return true;
    }
    let b = r.as_bytes();
    if b.first()
        .copied()
        .map(|c| c.is_ascii_digit() || c == b'-')
        .unwrap_or(false)
    {
        return true;
    }
    if r.ends_with(".class") {
        return true;
    }
    // Static / enum field path: com.foo.Bar.BAZ
    r.contains('.') && r.split('.').all(|p| !p.is_empty() && is_java_ident_part(p))
}

fn is_java_ident_part(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn try_inline_array_literal(
    r: &str,
    defs: &HashMap<VarId, String>,
    inlined: &mut HashSet<VarId>,
) -> Option<String> {
    let r = r.trim();
    let open = r.find("[]{")?;
    let close = r.rfind('}')?;
    if close <= open + 3 {
        return None;
    }
    let prefix = &r[..open + 3];
    let inner = r[open + 3..close].trim();
    let suffix = &r[close..];
    let elems = split_array_elems(inner);
    let mut changed = false;
    let mut new_elems = Vec::with_capacity(elems.len());
    for e in elems {
        let e_trim = e.trim();
        if let Some(vid) = parse_var_ref_token(e_trim) {
            if let Some(val) = defs.get(&vid) {
                inlined.insert(vid);
                new_elems.push(val.clone());
                changed = true;
                continue;
            }
        }
        new_elems.push(e_trim.to_string());
    }
    if !changed {
        return None;
    }
    Some(format!("{}{}{}", prefix, new_elems.join(", "), suffix))
}

fn parse_var_ref_token(s: &str) -> Option<VarId> {
    let s = s.trim();
    if !s.starts_with('v') {
        return None;
    }
    parse_bare_varid(s).or_else(|| {
        let rest = &s[1..];
        let num: u32 = rest.split('_').next()?.parse().ok()?;
        Some(VarId::new(num, 0))
    })
}

fn split_array_elems(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let bytes = inner.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b',' if depth == 0 => {
                out.push(inner[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start <= inner.len() {
        out.push(inner[start..].to_string());
    }
    out
}

/// Try to simplify `v0 = v0 + 1` → compound assign. Returns the new RHS string.
fn simplify_compound_assign(dst: VarId, raw: &str) -> Option<String> {
    let raw = raw.trim();
    let dst_name = if dst.ver == 0 {
        format!("v{}", dst.reg)
    } else {
        format!("v{}_{}", dst.reg, dst.ver)
    };
    for op in &["+", "-", "*", "/", "%", "&", "|", "^", "<<", ">>", ">>>"] {
        // Pattern: "vN op expr" where vN is the dst
        let prefix = format!("{} {} ", dst_name, op);
        if raw.starts_with(&prefix) {
            let rhs_part = raw[prefix.len()..].trim();
            if rhs_part == "1" && (*op == "+" || *op == "-") {
                return Some(format!("__compound_{}{}_{}", dst_name, op, op));
            }
            return Some(format!("__compound_{} {}= {}", dst_name, op, rhs_part));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile::ir::{Expr as IrExpr, Stmt as IrStmt, VarId};

    #[test]
    fn invoke_chain_merge_assign_then_return() {
        let stmts = vec![
            IrStmt::Expr {
                expr: IrExpr::Call {
                    target: "Foo.bar".into(),
                    args: "v0".into(),
                },
                comment: Some("invoke".into()),
            },
            IrStmt::Assign {
                dst: VarId::new(1, 0),
                rhs: IrExpr::PendingResult,
                comment: Some("move-result".into()),
            },
            IrStmt::Return {
                value: Some(IrExpr::Var(VarId::new(1, 0))),
                comment: Some("return".into()),
            },
        ];
        let out = InvokeChainPass.run(stmts);
        assert_eq!(out.len(), 1);
        match &out[0] {
            IrStmt::Return {
                value: Some(IrExpr::Call { target, args }),
                ..
            } => {
                assert_eq!(target, "Foo.bar");
                assert_eq!(args, "v0");
            }
            _ => panic!("expected single Return(Call), got {:?}", out),
        }
    }

    #[test]
    fn invoke_chain_merge_assign_only() {
        let stmts = vec![
            IrStmt::Expr {
                expr: IrExpr::Call {
                    target: "Baz.qux".into(),
                    args: "v2, v3".into(),
                },
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(0, 0),
                rhs: IrExpr::PendingResult,
                comment: None,
            },
        ];
        let out = InvokeChainPass.run(stmts);
        assert_eq!(out.len(), 1);
        match &out[0] {
            IrStmt::Assign {
                dst,
                rhs: IrExpr::Call { target, args },
                ..
            } if *dst == VarId::new(0, 0) => {
                assert_eq!(target, "Baz.qux");
                assert_eq!(args, "v2, v3");
            }
            _ => panic!("expected single Assign(Call), got {:?}", out),
        }
    }

    #[test]
    fn invoke_chain_snapshots_const_arg_overwritten_by_move_result() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(9, 0),
                rhs: IrExpr::Raw("2".into()),
                comment: None,
            },
            IrStmt::Expr {
                expr: IrExpr::Call {
                    target: "Algo.bfs".into(),
                    args: "g, v0, v9".into(),
                },
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(9, 0),
                rhs: IrExpr::PendingResult,
                comment: None,
            },
        ];
        let out = InvokeChainPass.run(stmts);
        match &out[1] {
            IrStmt::Assign {
                rhs: IrExpr::Call { args, .. },
                ..
            } => {
                assert_eq!(
                    args, "g, v0, 2",
                    "move-result into arg reg must keep pre-call const: {args}"
                );
            }
            other => panic!("expected Assign(Call), got {other:?}"),
        }
    }

    #[test]
    fn invoke_chain_does_not_snapshot_stale_const_after_object_redef() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(5, 0),
                rhs: IrExpr::Raw("5".into()),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(5, 0),
                rhs: IrExpr::Raw("new int[]{3, 1, 4, 1, 5}".into()),
                comment: None,
            },
            IrStmt::Expr {
                expr: IrExpr::Call {
                    target: "Algo.mergeSort".into(),
                    args: "v5".into(),
                },
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(5, 0),
                rhs: IrExpr::PendingResult,
                comment: None,
            },
        ];
        let out = InvokeChainPass.run(stmts);
        match &out[2] {
            IrStmt::Assign {
                rhs: IrExpr::Call { args, .. },
                ..
            } => {
                assert_eq!(
                    args, "v5",
                    "must not replace array arg with stale 5: {args}"
                );
            }
            other => panic!("expected Assign(Call), got {other:?}"),
        }
    }

    #[test]
    fn invoke_chain_fold_literal_assign_return() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(0, 0),
                rhs: IrExpr::Raw("\"bad_name\"".into()),
                comment: None,
            },
            IrStmt::Return {
                value: Some(IrExpr::Var(VarId::new(0, 0))),
                comment: None,
            },
        ];
        let out = InvokeChainPass.run(stmts);
        assert_eq!(out.len(), 1);
        match &out[0] {
            IrStmt::Return {
                value: Some(IrExpr::Raw(s)),
                ..
            } => assert_eq!(s, "\"bad_name\""),
            _ => panic!("expected Return(Raw(\"bad_name\")), got {:?}", out),
        }
    }

    #[test]
    fn runner_runs_passes_in_order() {
        let mut runner = PassRunner::new();
        runner.add(InvokeChainPass);
        let stmts = vec![
            IrStmt::Expr {
                expr: IrExpr::Call {
                    target: "X.y".into(),
                    args: "".into(),
                },
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(0, 0),
                rhs: IrExpr::PendingResult,
                comment: None,
            },
            IrStmt::Return {
                value: Some(IrExpr::Var(VarId::new(0, 0))),
                comment: None,
            },
        ];
        let out = runner.run(stmts);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn ssa_renames_defs_and_uses_linearly() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(0, 0),
                rhs: IrExpr::Raw("0".into()),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(0, 0),
                rhs: IrExpr::Var(VarId::new(0, 0)),
                comment: None,
            },
            IrStmt::Return {
                value: Some(IrExpr::Var(VarId::new(0, 0))),
                comment: None,
            },
        ];
        let out = SsaRenamePass.run(stmts);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].to_java_line(), "v0_1 = 0;");
        assert_eq!(out[1].to_java_line(), "v0_2 = v0_1;");
        assert_eq!(out[2].to_java_line(), "return v0_2;");
    }

    #[test]
    fn dead_assign_keeps_const_class_used_in_aput_raw() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(9, 0),
                rhs: IrExpr::Raw("android.content.Context.class".into()),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(10, 0),
                rhs: IrExpr::Raw("0".into()),
                comment: None,
            },
            IrStmt::Raw("v8[v10] = v9;".into()),
        ];
        let out = DeadAssignPass.run(stmts);
        assert_eq!(
            out.len(),
            3,
            "const-class/index must survive when only used in aput Raw: {:?}",
            out
        );
        let regs = used_regs(&out);
        assert!(regs.contains(&9) && regs.contains(&10), "{:?}", regs);
    }

    #[test]
    fn var_ids_parses_versioned_aput() {
        let mut set = HashSet::new();
        var_ids_in_text("v8_2[v10_2] = v9_2;", &mut set);
        assert!(set.contains(&VarId::new(8, 2)), "{:?}", set);
        assert!(set.contains(&VarId::new(10, 2)), "{:?}", set);
        assert!(set.contains(&VarId::new(9, 2)), "{:?}", set);
        let regs: HashSet<u32> = set.iter().map(|v| v.reg).collect();
        assert!(regs.contains(&9) && regs.contains(&10), "{:?}", regs);
    }

    #[test]
    fn used_regs_includes_call_receiver_in_target() {
        let stmts = vec![IrStmt::Expr {
            expr: IrExpr::Call {
                target: "v0_1.processLogin".into(),
                args: "v1_1, v2_1".into(),
            },
            comment: None,
        }];
        let regs = used_regs(&stmts);
        assert!(
            regs.contains(&0) && regs.contains(&1) && regs.contains(&2),
            "Call target receiver must count as a use: {:?}",
            regs
        );
    }

    #[test]
    fn dead_assign_removes_unused() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(0, 1),
                rhs: IrExpr::Raw("0".into()),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(0, 2),
                rhs: IrExpr::Raw("1".into()),
                comment: None,
            },
            IrStmt::Return {
                value: Some(IrExpr::Var(VarId::new(0, 2))),
                comment: None,
            },
        ];
        let out = DeadAssignPass.run(stmts);
        assert_eq!(
            out.len(),
            2,
            "first assign (v0_1) is dead, should be removed"
        );
        match &out[0] {
            IrStmt::Assign {
                dst,
                rhs: IrExpr::Raw(s),
                ..
            } => {
                assert_eq!(dst.ver, 2);
                assert_eq!(s, "1");
            }
            _ => panic!("expected assign then return"),
        }
        assert!(matches!(out[1], IrStmt::Return { .. }));
    }

    #[test]
    fn dead_assign_keeps_copy_used_as_invoke_receiver() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(0, 1),
                rhs: IrExpr::Raw("v3".into()),
                comment: None,
            },
            IrStmt::Expr {
                expr: IrExpr::Call {
                    target: "v0_1.processLogin".into(),
                    args: "v1_1, v2_1".into(),
                },
                comment: None,
            },
        ];
        let out = DeadAssignPass.run(stmts);
        assert_eq!(out.len(), 2, "receiver copy must not be dropped: {:?}", out);
    }

    #[test]
    fn copy_prop_keeps_euclid_gcd_swap() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(0, 1),
                rhs: IrExpr::Var(VarId::new(2, 0)),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(2, 1),
                rhs: IrExpr::Raw("v1_0 % v2_0".into()),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(1, 1),
                rhs: IrExpr::Var(VarId::new(0, 1)),
                comment: None,
            },
        ];
        let out = CopyPropPass.run(stmts);
        assert_eq!(out.len(), 3, "{:?}", out);
    }

    #[test]
    fn copy_prop_folds_param_moves_into_invoke() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(0, 1),
                rhs: IrExpr::Raw("v3".into()),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(1, 1),
                rhs: IrExpr::Raw("v4".into()),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(2, 1),
                rhs: IrExpr::Raw("v5".into()),
                comment: None,
            },
            IrStmt::Expr {
                expr: IrExpr::Call {
                    target: "v0_1.processLogin".into(),
                    args: "v1_1, v2_1".into(),
                },
                comment: None,
            },
            IrStmt::Return {
                value: None,
                comment: None,
            },
        ];
        let out = CopyPropPass.run(stmts);
        assert_eq!(out.len(), 2, "{:?}", out);
        match &out[0] {
            IrStmt::Expr {
                expr: IrExpr::Call { target, args },
                ..
            } => {
                assert_eq!(target, "v3.processLogin");
                assert_eq!(args, "v4, v5");
            }
            other => panic!("expected Call after copy-prop, got {:?}", other),
        }
        assert!(matches!(out[1], IrStmt::Return { value: None, .. }));
    }

    #[test]
    fn inline_filled_array_inlines_enum_sget_operands() {
        let stmts = vec![
            IrStmt::Assign {
                dst: VarId::new(0, 1),
                rhs: IrExpr::Raw("com.foo.Prompt.A".into()),
                comment: None,
            },
            IrStmt::Assign {
                dst: VarId::new(1, 1),
                rhs: IrExpr::Raw("com.foo.Prompt.B".into()),
                comment: None,
            },
            IrStmt::Return {
                value: Some(IrExpr::Raw("new com.foo.Prompt[]{ v0_1, v1_1 }".into())),
                comment: None,
            },
        ];
        let out = InlineFilledArrayPass.run(stmts);
        assert_eq!(out.len(), 1, "{:?}", out);
        match &out[0] {
            IrStmt::Return {
                value: Some(IrExpr::Raw(r)),
                ..
            } => {
                assert!(r.contains("com.foo.Prompt.A"), "{}", r);
                assert!(r.contains("com.foo.Prompt.B"), "{}", r);
                assert!(!r.contains("v0_1"), "{}", r);
                assert!(!r.contains("v1_1"), "{}", r);
            }
            other => panic!("expected Return(Raw(...)), got {:?}", other),
        }
    }
}
