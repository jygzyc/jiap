//! Instruction walker: linear scan over code units, decode operands for xref/emitter use.

use crate::opcodes::{fmt_len, invoke_kind, Fmt, OPCODES};

#[derive(Default, Clone)]
pub struct Insn {
    pub pc: usize,          // offset in code units
    pub opcode: u8,
    pub name: &'static str,
    pub regs: Vec<u8>,      // register operands (best effort)
    pub lit: i64,           // literal/branch offset
    pub str_idx: Option<u32>,
    pub type_idx: Option<u32>,
    pub field_idx: Option<u32>,
    pub method_idx: Option<u32>,
    pub invoke_kind: Option<&'static str>,
    pub branch_target: Option<usize>, // absolute pc in units
}

fn s11(b: u8) -> i64 {
    (b as i8) as i64
}
fn s16(w: u16) -> i64 {
    (w as i16) as i64
}
fn s32(w0: u16, w1: u16) -> i64 {
    (((w0 as u32) | ((w1 as u32) << 16)) as i32) as i64
}

/// Walk instructions. Unknown/unassigned opcodes are treated as 1 unit
/// (robustness over strictness); payload pseudo-ops sized by their headers.
pub fn walk(insns: &[u16]) -> Vec<Insn> {
    let mut out = Vec::new();
    let mut pc = 0usize;
    while pc < insns.len() {
        let w = insns[pc];
        let op = (w & 0xff) as u8;
        let mut insn = Insn {
            pc,
            opcode: op,
            ..Default::default()
        };
        // payload pseudo-instructions (nop with ident in high byte)
        if op == 0x00 && pc + 1 < insns.len() {
            let ident = insns[pc] >> 8;
            match ident {
                0x01 => {
                    // packed-switch-payload: ident,size,first_key(2u), targets
                    let size = insns[pc + 1] as usize;
                    let units = 4 + size * 2;
                    out.push(Insn {
                        pc,
                        opcode: op,
                        name: "packed-switch-payload",
                        lit: units as i64,
                        ..Default::default()
                    });
                    pc += units;
                    continue;
                }
                0x02 => {
                    let size = insns[pc + 1] as usize;
                    let units = 2 + size * 4;
                    out.push(Insn {
                        pc,
                        opcode: op,
                        name: "sparse-switch-payload",
                        lit: units as i64,
                        ..Default::default()
                    });
                    pc += units;
                    continue;
                }
                0x03 => {
                    // fill-array-data: ident, elem_width u16, size u32, data
                    let w_ = insns[pc + 1] as usize;
                    let size = (insns[pc + 2] as usize) | ((insns[pc + 3] as usize) << 16);
                    let bytes = size * w_;
                    let units = 4 + (bytes + 1) / 2;
                    out.push(Insn {
                        pc,
                        opcode: op,
                        name: "fill-array-data-payload",
                        lit: units as i64,
                        ..Default::default()
                    });
                    pc += units;
                    continue;
                }
                _ => {}
            }
        }
        let (name, fmt) = OPCODES[op as usize];
        insn.name = name;
        let units = fmt_len(fmt);
        let get = |i: usize| -> u16 { insns.get(pc + i).copied().unwrap_or(0) };
        match fmt {
            Fmt::F21c => {
                let regs = [(w >> 8) as u8];
                insn.regs = regs.to_vec();
                let idx = get(1) as u32;
                match op {
                    0x1a => insn.str_idx = Some(idx),         // const-string
                    0x1c => insn.type_idx = Some(idx),        // const-class
                    0x1f => insn.type_idx = Some(idx),        // check-cast
                    0x22 => insn.type_idx = Some(idx),        // new-instance
                    0x60..=0x6d => insn.field_idx = Some(idx), // sget/sput
                    0xfc | 0xfd => insn.method_idx = Some(idx),
                    _ => {}
                }
            }
            Fmt::F31c => {
                insn.regs = vec![(w >> 8) as u8];
                let idx = (get(1) as u32) | ((get(2) as u32) << 16);
                if op == 0x1b {
                    insn.str_idx = Some(idx);
                }
            }
            Fmt::F22c => {
                // B|A|op CCCC: A = bits 8..11 (first register operand, e.g. iget dst),
                // B = bits 12..15 (second, e.g. iget object)
                insn.regs = vec![((w >> 8) & 0xf) as u8, ((w >> 12) & 0xf) as u8];
                let idx = get(1) as u32;
                match op {
                    0x20 | 0x23 => insn.type_idx = Some(idx), // instance-of/new-array
                    0x52..=0x5f => insn.field_idx = Some(idx), // iget/iput
                    _ => {}
                }
            }
            Fmt::F35c => {
                // A|G|op BBBB FEDC: A=(w>>12), G=((w>>8)&0xf), BBBB=get(1) method idx,
                // FEDC nibbles of get(2): C=low, D, E, F=high
                insn.method_idx = Some(get(1) as u32);
                insn.invoke_kind = invoke_kind(op);
                let a = (w >> 12) & 0xf;
                let g = (w >> 8) & 0xf;
                let w2 = get(2);
                insn.regs = vec![
                    (w2 & 0xf) as u8,
                    ((w2 >> 4) & 0xf) as u8,
                    ((w2 >> 8) & 0xf) as u8,
                    ((w2 >> 12) & 0xf) as u8,
                    g as u8,
                ];
                insn.lit = a as i64; // arg count
            }
            Fmt::F3rc => {
                // AA|op BBBB CCCC: AA=(w>>8) count, BBBB=get(1) method idx, CCCC=get(2) first reg
                insn.method_idx = Some(get(1) as u32);
                insn.invoke_kind = invoke_kind(op);
                insn.lit = ((w >> 8) & 0xff) as i64;
                insn.regs = vec![(get(2) & 0xff) as u8];
            }
            Fmt::F45cc => {
                // A|G|op BBBB F|E|D|C HHHH (invoke-custom style polymorphic)
                insn.method_idx = Some(get(1) as u32);
                insn.invoke_kind = invoke_kind(op);
                let g = (w >> 8) & 0xf;
                let a = (w >> 12) & 0xf;
                let w2 = get(2);
                insn.regs = vec![
                    (w2 & 0xf) as u8,
                    ((w2 >> 4) & 0xf) as u8,
                    ((w2 >> 8) & 0xf) as u8,
                    ((w2 >> 12) & 0xf) as u8,
                    g as u8,
                ];
                insn.lit = a as i64;
            }
            Fmt::F4rcc => {
                // AA|op BBBB HHHH CCCC
                insn.method_idx = Some(get(1) as u32);
                insn.invoke_kind = invoke_kind(op);
                insn.lit = ((w >> 8) & 0xff) as i64;
                insn.regs = vec![(get(3) & 0xff) as u8];
            }
            Fmt::F10t => {
                insn.lit = s11((w >> 8) as u8);
                insn.branch_target = Some(((pc as i64) + insn.lit) as usize);
            }
            Fmt::F20t | Fmt::F21t | Fmt::F22t => {
                let off = s16(get(1));
                insn.lit = off;
                insn.branch_target = Some(((pc as i64) + off) as usize);
                insn.regs = match fmt {
                    // ØØ|op: no register
                    Fmt::F20t => vec![],
                    // AA|op: 8-bit register
                    Fmt::F21t => vec![(w >> 8) as u8],
                    // B|A|op: A = bits 8..11, B = bits 12..15
                    _ => vec![((w >> 8) & 0xf) as u8, ((w >> 12) & 0xf) as u8],
                };
            }
            Fmt::F30t => {
                let off = s32(get(1), get(2));
                insn.lit = off;
                insn.branch_target = Some(((pc as i64) + off) as usize);
            }
            Fmt::F11n => {
                // B|A|op: A = bits 8..11 (register), B = bits 12..15 (signed literal)
                insn.regs = vec![((w >> 8) & 0xf) as u8];
                let b = ((w >> 12) & 0xf) as i64;
                insn.lit = if b >= 8 { b - 16 } else { b };
            }
            Fmt::F21s | Fmt::F21h => {
                insn.regs = vec![(w >> 8) as u8];
                insn.lit = s16(get(1));
                if fmt == Fmt::F21h {
                    insn.lit = insn.lit << 16;
                }
            }
            Fmt::F31i => {
                insn.regs = vec![(w >> 8) as u8];
                insn.lit = s32(get(1), get(2));
            }
            Fmt::F51l => {
                insn.regs = vec![(w >> 8) as u8];
                let lo = ((get(1) as u64) | ((get(2) as u64) << 16)) & 0xffff_ffff;
                let hi = ((get(3) as u64) | ((get(4) as u64) << 16)) & 0xffff_ffff;
                insn.lit = ((lo | (hi << 32)) as i64) as i64;
            }
            Fmt::F12x => {
                // B|A|op: A = bits 8..11, B = bits 12..15
                insn.regs = vec![((w >> 8) & 0xf) as u8, ((w >> 12) & 0xf) as u8];
            }
            Fmt::F11x => {
                insn.regs = vec![(w >> 8) as u8];
            }
            Fmt::F23x => {
                insn.regs = vec![(w >> 8) as u8, (get(1) & 0xff) as u8, (get(1) >> 8) as u8];
            }
            Fmt::F22x => {
                // AA|op BBBB: move/from16 (dst AA 8-bit, src BBBB 16-bit, truncated to u8)
                insn.regs = vec![(w >> 8) as u8, (get(1) & 0xff) as u8];
            }
            Fmt::F32x => {
                // ØØ|op AAAA BBBB: move/16 (dst AAAA unit 1, src BBBB unit 2; both truncated to u8)
                insn.regs = vec![(get(1) & 0xff) as u8, (get(2) & 0xff) as u8];
            }
            Fmt::F31t => {
                // AA|op BBBB: fill-array-data / switch targets (base reg only)
                insn.regs = vec![(w >> 8) as u8];
            }
            Fmt::F22b => {
                // AA|op BB CC: dst AA, src BB, literal CC (signed)
                insn.regs = vec![(w >> 8) as u8, (get(1) & 0xff) as u8];
                insn.lit = (get(1) >> 8) as i8 as i64;
            }
            Fmt::F22s => {
                // A|op|B BBBB: A = bits 8..11, B = bits 12..15, literal BBBB
                insn.regs = vec![((w >> 8) & 0xf) as u8, ((w >> 12) & 0xf) as u8];
                insn.lit = s16(get(1));
            }
            _ => {}
        }
        out.push(insn);
        pc += units.max(1);
    }
    out
}

/// Basic block leaders for CFG listing.
pub fn basic_blocks(insns: &[Insn], total_units: usize) -> Vec<(usize, usize)> {
    let mut leaders: Vec<usize> = Vec::new();
    if !insns.is_empty() {
        leaders.push(0);
    }
    for i in insns {
        if let Some(t) = i.branch_target {
            leaders.push(t);
        }
    }
    leaders.sort_unstable();
    leaders.dedup();
    let mut out = Vec::new();
    for (i, &l) in leaders.iter().enumerate() {
        let end = leaders.get(i + 1).copied().unwrap_or(total_units);
        if l < end {
            out.push((l, end));
        }
    }
    out
}
