//! `MethodBody`: the taint solver's input IR. Extracted from decoded DEX
//! instructions (decx-core `code::walk`) with all pool indices resolved to
//! strings, so the solver (and its tests) never touch a `Dex`.

use decx_core::code::{walk, Insn};
use decx_core::dex::Dex;
use decx_core::Project;

/// one taint-relevant operation; registers are destination/src indices
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// dst <- src (move*, check-cast)
    Move { pc: usize, dst: u8, src: u8 },
    /// dst <- constant / freshly allocated object: kills taint
    Const { pc: usize, dst: u8 },
    /// result register of the immediately preceding invoke
    MoveResult { pc: usize, dst: u8 },
    /// callee full sig + argument registers in call order (receiver first
    /// for virtual/interface/super/direct invokes); kind = invoke dispatch
    /// kind ("static" has no receiver slot)
    Invoke { pc: usize, callee: String, args: Vec<u8>, kind: &'static str },
    /// dst <- field read (obj None => static)
    FieldGet { pc: usize, dst: u8, field: String, obj: Option<u8> },
    /// field write (obj None => static)
    FieldPut { pc: usize, src: u8, field: String, obj: Option<u8> },
    /// dst <- arr[idx]
    ArrayGet { pc: usize, dst: u8, arr: u8 },
    /// arr[idx] <- src
    ArrayPut { pc: usize, src: u8, arr: u8 },
    /// dst <- a op b (binops, cmps, instance-of, array-length as src=b)
    BinOp { pc: usize, dst: u8, a: u8, b: u8 },
    /// dst <- op src (unops)
    UnOp { pc: usize, dst: u8, src: u8 },
    /// return (void => None)
    Return { pc: usize, src: Option<u8> },
    /// no taint effect (branches, switches, monitor, throw, payloads...)
    Other { pc: usize },
}

impl Op {
    /// highest register index this op touches; `None` when it touches none.
    /// Lets the solver skip ops referencing registers outside the frame
    /// (obfuscated or malformed dex).
    pub fn max_reg(&self) -> Option<u8> {
        Some(match self {
            Op::Move { dst, src, .. } => (*dst).max(*src),
            Op::Const { dst, .. } | Op::MoveResult { dst, .. } => *dst,
            Op::Return { src, .. } => (*src)?,
            Op::Invoke { args, .. } => *args.iter().max()?,
            Op::FieldGet { dst, obj, .. } => obj.map_or(*dst, |o| (*dst).max(o)),
            Op::FieldPut { src, obj, .. } => obj.map_or(*src, |o| (*src).max(o)),
            Op::ArrayGet { dst, arr, .. } => (*dst).max(*arr),
            Op::ArrayPut { src, arr, .. } => (*src).max(*arr),
            Op::BinOp { dst, a, b, .. } => (*dst).max(*a).max(*b),
            Op::UnOp { dst, src, .. } => (*dst).max(*src),
            Op::Other { .. } => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct MethodBody {
    /// full signature `Lcls;->name(args)ret`
    pub sig: String,
    pub registers_size: u16,
    /// number of incoming parameter registers (slots at the register top)
    pub ins_size: u16,
    pub ops: Vec<Op>,
}

impl MethodBody {
    /// first incoming parameter register (params occupy [param_base, registers_size))
    pub fn param_base(&self) -> u16 {
        self.registers_size.saturating_sub(self.ins_size)
    }

    /// parameter slot index for an incoming register
    pub fn param_slot(&self, reg: u16) -> Option<usize> {
        let base = self.param_base();
        if reg >= base && reg < self.registers_size {
            Some((reg - base) as usize)
        } else {
            None
        }
    }
}

fn invoke_args(ins: &Insn) -> Vec<u8> {
    let count = ins.lit.max(0) as usize;
    match ins.regs.len() {
        // F35c/F45cc: regs = [C, D, E, F, G]
        5 => ins.regs.iter().take(count.min(5)).copied().collect(),
        // F3rc/F4rcc: regs = [first]; range = first .. first+count
        1 => {
            let first = ins.regs[0] as u16;
            (0..count.min(256))
                .map(|i| (first + i as u16) as u8)
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Build the Op stream for one method body.
pub fn lower(dex: &Dex, sig: &str, code: &decx_core::dex::CodeItem) -> MethodBody {
    let insns = walk(&code.insns);
    let mut ops = Vec::with_capacity(insns.len());
    for i in &insns {
        let pc = i.pc;
        let op = i.opcode;
        let name = i.name;
        let op_out = match op {
            0x01..=0x09 | 0x0d => {
                // moves (incl. from16/16) + move-exception (kills)
                if i.regs.len() == 2 {
                    if op == 0x0d {
                        Op::Const { pc, dst: i.regs[0] }
                    } else {
                        Op::Move { pc, dst: i.regs[0], src: i.regs[1] }
                    }
                } else {
                    Op::Other { pc }
                }
            }
            0x0a..=0x0c => Op::MoveResult { pc, dst: i.regs.first().copied().unwrap_or(0) },
            0x0e => Op::Return { pc, src: None },
            0x0f..=0x11 => Op::Return { pc, src: i.regs.first().copied() },
            0x12..=0x1c | 0x22 => Op::Const { pc, dst: i.regs.first().copied().unwrap_or(0) },
            0x1f => Op::Move {
                pc,
                dst: i.regs.first().copied().unwrap_or(0),
                src: i.regs.get(1).copied().unwrap_or(0),
            },
            0x20 | 0x21 | 0x2d..=0x31 | 0x90..=0xe2 => {
                // binop family (23x/12x/22s/22b layouts all carry [dst, src...])
                if i.regs.len() >= 2 {
                    Op::BinOp {
                        pc,
                        dst: i.regs[0],
                        a: i.regs[1],
                        b: i.regs.get(2).copied().unwrap_or(i.regs[1]),
                    }
                } else {
                    Op::Other { pc }
                }
            }
            0x23 => Op::UnOp {
                pc,
                dst: i.regs.first().copied().unwrap_or(0),
                src: i.regs.get(1).copied().unwrap_or(0),
            },
            0x25 | 0x26 => Op::Invoke {
                // filled-new-array / filled-new-array/range: result lands in the
                // move-result register; modeled as a call with no rules
                pc,
                callee: "#filled-new-array".to_string(),
                args: invoke_args(i),
                kind: "static",
            },
            0x44..=0x51 => {
                // aget: regs [dst, arr, idx] / aput: regs [src, arr, idx]
                if name.starts_with("aget") {
                    Op::ArrayGet {
                        pc,
                        dst: i.regs.first().copied().unwrap_or(0),
                        arr: i.regs.get(1).copied().unwrap_or(0),
                    }
                } else {
                    Op::ArrayPut {
                        pc,
                        src: i.regs.first().copied().unwrap_or(0),
                        arr: i.regs.get(1).copied().unwrap_or(0),
                    }
                }
            }
            0x52..=0x5f => {
                let field = i.field_idx.map(|f| dex.field_full(f)).unwrap_or_default();
                if name.starts_with("iget") {
                    Op::FieldGet {
                        pc,
                        dst: i.regs.first().copied().unwrap_or(0),
                        field,
                        obj: i.regs.get(1).copied(),
                    }
                } else {
                    Op::FieldPut {
                        pc,
                        src: i.regs.first().copied().unwrap_or(0),
                        field,
                        obj: i.regs.get(1).copied(),
                    }
                }
            }
            0x60..=0x6d => {
                let field = i.field_idx.map(|f| dex.field_full(f)).unwrap_or_default();
                if name.starts_with("sget") {
                    Op::FieldGet { pc, dst: i.regs.first().copied().unwrap_or(0), field, obj: None }
                } else {
                    Op::FieldPut { pc, src: i.regs.first().copied().unwrap_or(0), field, obj: None }
                }
            }
            0x6e..=0x72 | 0x74..=0x78 | 0xfa..=0xfd => {
                let callee = i
                    .method_idx
                    .map(|m| dex.method_full(m))
                    .unwrap_or_default();
                let kind = i.invoke_kind.unwrap_or("virtual");
                Op::Invoke { pc, callee, args: invoke_args(i), kind }
            }
            0x7b..=0x8f => {
                if i.regs.len() >= 2 {
                    Op::UnOp { pc, dst: i.regs[0], src: i.regs[1] }
                } else {
                    Op::Other { pc }
                }
            }
            _ => Op::Other { pc },
        };
        ops.push(op_out);
    }
    MethodBody {
        sig: sig.to_string(),
        registers_size: code.registers_size,
        ins_size: code.ins_size,
        ops,
    }
}

/// Extract every method body with code from the project. Methods matching
/// rule excludes or exceeding `max_insns` are skipped (returns them nowhere).
pub fn extract_all(project: &Project, excluded: impl Fn(&str) -> bool, max_insns: usize) -> Vec<MethodBody> {
    let mut out = Vec::new();
    for dex in &project.dexes {
        for def in &dex.class_defs {
            let cd = dex.class_data(def);
            for section in [&cd.direct_methods, &cd.virtual_methods] {
                for m in section {
                    let sig = dex.method_full(m.method_idx);
                    if excluded(&sig) {
                        continue;
                    }
                    let Some(code) = dex.code_item(m.code_off) else {
                        continue;
                    };
                    if code.insns.len() > max_insns {
                        continue;
                    }
                    out.push(lower(dex, &sig, &code));
                }
            }
        }
    }
    out
}
