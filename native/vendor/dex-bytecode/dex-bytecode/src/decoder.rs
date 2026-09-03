//! Linear-sweep Dalvik instruction decoder.
//! Matches androguard LinearSweepAlgorithm.get_instructions.

use alloc::vec::Vec;
use core::convert::TryInto;

use crate::error::DexError;
use crate::instruction::{Instruction, RefKind};
use crate::opcodes::{format_length, get_opcode_entry, get_payload_kind, Format, PayloadKind};
use crate::resolve::ResolveRef;

/// Read little-endian u16 from `data` at `offset`. Returns None if out of bounds.
#[inline]
fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let end = offset + 2;
    if end > data.len() {
        return None;
    }
    let bytes: [u8; 2] = data[offset..end].try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

#[inline]
fn read_i16(data: &[u8], offset: usize) -> Option<i16> {
    read_u16(data, offset).map(|u| u as i16)
}

#[inline]
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let end = offset + 4;
    if end > data.len() {
        return None;
    }
    let bytes: [u8; 4] = data[offset..end].try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

#[inline]
fn read_i32(data: &[u8], offset: usize) -> Option<i32> {
    read_u32(data, offset).map(|u| u as i32)
}

#[inline]
fn read_u64(data: &[u8], offset: usize) -> Option<u64> {
    let end = offset + 8;
    if end > data.len() {
        return None;
    }
    let bytes: [u8; 8] = data[offset..end].try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

/// Decode a single instruction at `offset` in `data`. Returns the instruction and its length.
pub fn decode_one(data: &[u8], offset: usize) -> Result<Instruction, DexError> {
    let unit = read_u16(data, offset)
        .ok_or_else(|| DexError::invalid("truncated at first 16-bit unit"))?;
    let op_byte = unit as u8;

    // Payload pseudo-instructions: 0x0100, 0x0200, 0x0300 (high byte 0x01/0x02/0x03, low 0x00)
    if unit > 0xFF && (op_byte == 0x00 || op_byte == 0xFF) {
        if let Some(payload) = get_payload_kind(unit) {
            return decode_payload(data, offset, payload);
        }
    }

    let entry = get_opcode_entry(op_byte);
    let len = format_length(entry.format) as usize;

    if entry.format == Format::F00x {
        return Err(DexError::invalid_owned(format!(
            "unused or invalid opcode 0x{:02x}",
            op_byte
        )));
    }

    if offset + len > data.len() {
        return Err(DexError::invalid("instruction extends past buffer"));
    }

    let ref_fmt = |kind: RefKind, idx: u32| Instruction::format_ref(kind, idx);
    decode_format(data, offset, op_byte, entry, len, ref_fmt)
}

/// Decode a single instruction at `offset` with reference resolution.
/// Indices are resolved to display strings when the resolver returns `Some`.
pub fn decode_one_with_resolver<R: ResolveRef>(
    data: &[u8],
    offset: usize,
    resolver: &R,
) -> Result<Instruction, DexError> {
    let unit = read_u16(data, offset)
        .ok_or_else(|| DexError::invalid("truncated at first 16-bit unit"))?;
    let op_byte = unit as u8;

    if unit > 0xFF && (op_byte == 0x00 || op_byte == 0xFF) {
        if let Some(payload) = get_payload_kind(unit) {
            return decode_payload(data, offset, payload);
        }
    }

    let entry = get_opcode_entry(op_byte);
    let len = format_length(entry.format) as usize;

    if entry.format == Format::F00x {
        return Err(DexError::invalid_owned(format!(
            "unused or invalid opcode 0x{:02x}",
            op_byte
        )));
    }

    if offset + len > data.len() {
        return Err(DexError::invalid("instruction extends past buffer"));
    }

    let ref_fmt = |kind: RefKind, idx: u32| {
        resolver
            .resolve(kind, idx)
            .unwrap_or_else(|| Instruction::format_ref(kind, idx))
    };
    decode_format(data, offset, op_byte, entry, len, ref_fmt)
}

fn decode_format<F>(
    data: &[u8],
    offset: usize,
    opcode: u8,
    entry: &crate::opcodes::OpcodeEntry,
    len: usize,
    ref_fmt: F,
) -> Result<Instruction, DexError>
where
    F: Fn(RefKind, u32) -> String,
{
    use crate::opcodes::Format;
    use alloc::format;

    let ref_fmt_primary = |idx: u32| ref_fmt(entry.ref_kind, idx);

    let operands = match entry.format {
        Format::F10x => String::new(),
        Format::F10t => {
            let aa = high_byte(data, offset) as i8;
            format!("{:+03x}h", aa)
        }
        Format::F11n => {
            let byte = data[offset + 1] as i8;
            let a = (data[offset + 1] & 0x0F) as u32;
            let b = byte >> 4; // 4-bit sign-extended
            format!("v{}, {}", a, b)
        }
        Format::F11x => {
            let aa = high_byte(data, offset);
            format!("v{}", aa)
        }
        Format::F12x => {
            let (a, b) = ab_from_first(data, offset);
            format!("v{}, v{}", a, b)
        }
        Format::F20t => {
            let aaaa = read_i16(data, offset + 2).unwrap_or(0);
            format!("{:+05x}h", aaaa)
        }
        Format::F20bc => {
            let aa = data[offset + 1];
            let bbbb = read_u16(data, offset + 2).unwrap_or(0);
            format!("{}, {}", aa, bbbb)
        }
        Format::F21c => {
            let aa = data[offset + 1];
            let bbbb = read_u16(data, offset + 2).unwrap_or(0) as u32;
            format!("v{}, {}", aa, ref_fmt_primary(bbbb))
        }
        Format::F21h => {
            let aa = data[offset + 1];
            let bbbb = read_i16(data, offset + 2).unwrap_or(0);
            let val: i64 = if opcode == 0x15 {
                (bbbb as i64) << 16
            } else if opcode == 0x19 {
                (bbbb as i64) << 48
            } else {
                bbbb as i64
            };
            format!("v{}, {}", aa, val)
        }
        Format::F21s => {
            let aa = data[offset + 1];
            let bbbb = read_i16(data, offset + 2).unwrap_or(0);
            format!("v{}, {}", aa, bbbb)
        }
        Format::F21t => {
            let aa = data[offset + 1];
            let bbbb = read_i16(data, offset + 2).unwrap_or(0);
            format!("v{}, {:+04x}h", aa, bbbb)
        }
        Format::F22b => {
            let aa = data[offset + 1];
            let bb = data[offset + 2];
            let cc = data[offset + 3] as i8;
            format!("v{}, v{}, {}", aa, bb, cc)
        }
        Format::F22c => {
            let (a, b) = ab_from_first(data, offset);
            let cccc = read_u16(data, offset + 2).unwrap_or(0) as u32;
            format!("v{}, v{}, {}", a, b, ref_fmt_primary(cccc))
        }
        Format::F22s => {
            let (a, b) = ab_from_first(data, offset);
            let cccc = read_i16(data, offset + 2).unwrap_or(0);
            format!("v{}, v{}, {}", a, b, cccc)
        }
        Format::F22t => {
            let (a, b) = ab_from_first(data, offset);
            let cccc = read_i16(data, offset + 2).unwrap_or(0);
            format!("v{}, v{}, {:+04x}h", a, b, cccc)
        }
        Format::F22cs => {
            let (a, b) = ab_from_first(data, offset);
            let cccc = read_u16(data, offset + 2).unwrap_or(0) as u32;
            format!("v{}, v{}, {}", a, b, ref_fmt_primary(cccc))
        }
        Format::F23x => {
            let aa = data[offset + 1];
            let bb = data[offset + 2];
            let cc = data[offset + 3];
            format!("v{}, v{}, v{}", aa, bb, cc)
        }
        Format::F30t => {
            let aaaaaaaa = read_i32(data, offset + 2).unwrap_or(0);
            format!("{:+08x}h", aaaaaaaa)
        }
        Format::F31c => {
            let aa = data[offset + 1];
            let bbbbbbbb = read_u32(data, offset + 2).unwrap_or(0);
            format!("v{}, {}", aa, ref_fmt_primary(bbbbbbbb))
        }
        Format::F31i => {
            let aa = data[offset + 1];
            let bbbbbbbb = read_i32(data, offset + 2).unwrap_or(0);
            format!("v{}, {}", aa, bbbbbbbb)
        }
        Format::F31t => {
            let aa = data[offset + 1];
            let bbbbbbbb = read_i32(data, offset + 2).unwrap_or(0);
            format!("v{}, {:+08x}h", aa, bbbbbbbb)
        }
        Format::F32x => {
            let aaaa = read_u16(data, offset + 2).unwrap_or(0);
            let bbbb = read_u16(data, offset + 4).unwrap_or(0);
            format!("v{}, v{}", aaaa, bbbb)
        }
        Format::F35c | Format::F35mi | Format::F35ms => {
            let (a, g, c, d, e, f) = decode_35c(data, offset);
            let bbbb = read_u16(data, offset + 2).unwrap_or(0) as u32;
            let kind = ref_fmt_primary(bbbb);
            let regs = format_35_regs(a, c, d, e, f, g);
            if regs.is_empty() {
                kind
            } else {
                format!("{}, {}", regs, kind)
            }
        }
        Format::F3rc | Format::F3rmi | Format::F3rms => {
            let aa = data[offset + 1];
            let bbbb = read_u16(data, offset + 2).unwrap_or(0) as u32;
            let cccc = read_u16(data, offset + 4).unwrap_or(0);
            let nnnn = (cccc as u32).saturating_add(aa as u32).saturating_sub(1) as u16;
            let kind = ref_fmt_primary(bbbb);
            if cccc == nnnn {
                format!("v{}, {}", cccc, kind)
            } else {
                format!("v{} ... v{}, {}", cccc, nnnn, kind)
            }
        }
        Format::F40sc => {
            let bbbbbbbb = read_u32(data, offset + 2).unwrap_or(0);
            let aaaa = read_u16(data, offset + 6).unwrap_or(0);
            format!("{}, {}", aaaa, ref_fmt_primary(bbbbbbbb))
        }
        Format::F41c => {
            let bbbbbbbb = read_u32(data, offset + 2).unwrap_or(0);
            let aaaa = read_u16(data, offset + 6).unwrap_or(0);
            format!("v{}, {}", aaaa, ref_fmt_primary(bbbbbbbb))
        }
        Format::F45cc => {
            let (a, g, c, d, e, f) = decode_35c(data, offset);
            let bbbb = read_u16(data, offset + 2).unwrap_or(0) as u32;
            let hhhh = read_u16(data, offset + 6).unwrap_or(0) as u32;
            let regs = format_35_regs(a, c, d, e, f, g);
            let method_str = ref_fmt(entry.ref_kind, bbbb);
            let proto_str = ref_fmt(RefKind::MethodProto, hhhh);
            if regs.is_empty() {
                format!("{}, {}", method_str, proto_str)
            } else {
                format!("{}, {}, {}", regs, method_str, proto_str)
            }
        }
        Format::F4rcc => {
            let aa = data[offset + 1];
            let bbbb = read_u16(data, offset + 2).unwrap_or(0) as u32;
            let cccc = read_u16(data, offset + 4).unwrap_or(0);
            let hhhh = read_u16(data, offset + 6).unwrap_or(0) as u32;
            let nnnn = (cccc as u32).saturating_add(aa as u32).saturating_sub(1) as u16;
            let method_str = ref_fmt(entry.ref_kind, bbbb);
            let proto_str = ref_fmt(RefKind::MethodProto, hhhh);
            format!("v{} .. v{} {} {}", cccc, nnnn, method_str, proto_str)
        }
        Format::F51l => {
            let aa = data[offset + 1];
            let raw = read_u64(data, offset + 2).unwrap_or(0);
            format!("v{}, {}", aa, raw as i64)
        }
        Format::F52c => {
            let cccccccc = read_u32(data, offset + 2).unwrap_or(0);
            let aaaa = read_u16(data, offset + 6).unwrap_or(0);
            let bbbb = read_u16(data, offset + 8).unwrap_or(0);
            format!("v{}, v{}, {}", aaaa, bbbb, ref_fmt_primary(cccccccc))
        }
        Format::F5rc => {
            let bbbbbbbb = read_u32(data, offset + 2).unwrap_or(0);
            let aaaa = read_u16(data, offset + 6).unwrap_or(0);
            let cccc = read_u16(data, offset + 8).unwrap_or(0);
            let nnnn = (cccc as u32).saturating_add(aaaa as u32).saturating_sub(1) as u16;
            let kind = ref_fmt_primary(bbbbbbbb);
            if cccc == nnnn {
                format!("v{}, {}", cccc, kind)
            } else {
                format!("v{} ... v{}, {}", cccc, nnnn, kind)
            }
        }
        Format::F22x => {
            let aa = data[offset + 1];
            let bbbb = read_u16(data, offset + 2).unwrap_or(0);
            format!("v{}, v{}", aa, bbbb)
        }
        Format::F00x => String::new(),
    };

    Ok(Instruction::new(
        offset as u32,
        len as u32,
        opcode,
        entry.mnemonic,
        operands,
    ))
}

#[inline]
fn high_byte(data: &[u8], offset: usize) -> u8 {
    data.get(offset + 1).copied().unwrap_or(0)
}

#[inline]
fn ab_from_first(data: &[u8], offset: usize) -> (u8, u8) {
    let w = read_u16(data, offset).unwrap_or(0);
    let a = ((w >> 8) & 0x0F) as u8;
    let b = ((w >> 12) & 0x0F) as u8;
    (a, b)
}

#[inline]
fn decode_35c(data: &[u8], offset: usize) -> (u8, u8, u8, u8, u8, u8) {
    let w1 = read_u16(data, offset).unwrap_or(0);
    let w2 = read_u16(data, offset + 4).unwrap_or(0);
    let a = ((w1 >> 12) & 0x0F) as u8;
    let g = ((w1 >> 8) & 0x0F) as u8;
    let c = (w2 & 0x0F) as u8;
    let d = ((w2 >> 4) & 0x0F) as u8;
    let e = ((w2 >> 8) & 0x0F) as u8;
    let f = ((w2 >> 12) & 0x0F) as u8;
    (a, g, c, d, e, f)
}

fn format_35_regs(a: u8, c: u8, d: u8, e: u8, f: u8, g: u8) -> String {
    match a {
        0 => String::new(),
        1 => format!("v{}", c),
        2 => format!("v{}, v{}", c, d),
        3 => format!("v{}, v{}, v{}", c, d, e),
        4 => format!("v{}, v{}, v{}, v{}", c, d, e, f),
        5 => format!("v{}, v{}, v{}, v{}, v{}", c, d, e, f, g),
        _ => String::new(),
    }
}

fn decode_payload(data: &[u8], offset: usize, kind: PayloadKind) -> Result<Instruction, DexError> {
    let (name, len) = match kind {
        PayloadKind::PackedSwitch => {
            // ident=0x0100, size, first_key, then size x targets (4 bytes each)
            if data.len() < offset + 8 {
                return Err(DexError::invalid("truncated packed-switch-payload"));
            }
            let size = read_u16(data, offset + 2).unwrap_or(0) as usize;
            let len = 4 + 4 + size * 4; // 2 units header + size*4 bytes
            ("packed-switch-payload", len)
        }
        PayloadKind::SparseSwitch => {
            // ident=0x0200, size, then size keys + size targets (4 bytes each)
            if data.len() < offset + 4 {
                return Err(DexError::invalid("truncated sparse-switch-payload"));
            }
            let size = read_u16(data, offset + 2).unwrap_or(0) as usize;
            let len = 4 + size * 4 + size * 4;
            ("sparse-switch-payload", len)
        }
        PayloadKind::FillArrayData => {
            // ident=0x0300, element_width (2), size (4), then (size*element_width) bytes padded to 16-bit
            if data.len() < offset + 8 {
                return Err(DexError::invalid("truncated fill-array-data-payload"));
            }
            let elem_width = read_u16(data, offset + 2).unwrap_or(0) as usize;
            let size = read_u32(data, offset + 4).unwrap_or(0) as usize;
            let data_len = size * elem_width;
            let padded = (data_len + 1) & !1; // round up to 2 bytes
            let len = 8 + padded;
            ("fill-array-data-payload", len)
        }
    };
    if offset + len > data.len() {
        return Err(DexError::invalid("payload extends past buffer"));
    }
    Ok(Instruction::new(
        offset as u32,
        len as u32,
        0,
        name,
        String::new(),
    ))
}

/// Linear-sweep decoder: decodes all instructions in `data` (bytecode only, no size limit).
/// Stops at first error or when buffer is exhausted.
pub fn decode_all(data: &[u8], start_offset: usize) -> Result<Vec<Instruction>, DexError> {
    decode_all_with_resolver(data, start_offset, &NoResolver)
}

/// Like [`decode_all`] but uses `resolver` to resolve string/type/field/method indices.
pub fn decode_all_with_resolver<R: ResolveRef>(
    data: &[u8],
    start_offset: usize,
    resolver: &R,
) -> Result<Vec<Instruction>, DexError> {
    let mut out = Vec::new();
    let remaining = data.len().saturating_sub(start_offset);
    out.reserve(remaining / 2);
    let mut offset = start_offset;
    while offset < data.len() {
        let ins = decode_one_with_resolver(data, offset, resolver)?;
        let len = ins.length() as usize;
        out.push(ins);
        offset += len;
    }
    Ok(out)
}

/// Dummy resolver used when no external resolver is provided (always falls back to `kind@index`).
struct NoResolver;

impl ResolveRef for NoResolver {
    fn resolve(&self, _kind: RefKind, _index: u32) -> Option<String> {
        None
    }
}

/// Iterator over decoded instructions (stops on first error).
pub struct Decoder<'a> {
    data: &'a [u8],
    offset: usize,
    size_16bit_units: Option<usize>, // if set, max byte = size_16bit_units * 2
}

impl<'a> Decoder<'a> {
    /// Decode from raw bytecode. `data` is the instruction bytes; decoding runs until buffer end
    /// or (if `size_16bit_units` is given) until `size_16bit_units * 2` bytes are consumed.
    pub fn new(data: &'a [u8], start_offset: usize, size_16bit_units: Option<usize>) -> Self {
        Self {
            data,
            offset: start_offset,
            size_16bit_units,
        }
    }

    /// Decode from bytecode with no size limit (use full buffer).
    pub fn with_ip(data: &'a [u8], ip: u64, _options: u32) -> Result<Self, DexError> {
        Ok(Self {
            data,
            offset: ip as usize,
            size_16bit_units: None,
        })
    }

    #[inline]
    pub const fn ip(&self) -> u64 {
        self.offset as u64
    }
}

impl<'a> Iterator for Decoder<'a> {
    type Item = Result<Instruction, DexError>;

    fn next(&mut self) -> Option<Self::Item> {
        let max = self
            .size_16bit_units
            .map(|s| s * 2)
            .unwrap_or(self.data.len());
        if self.offset >= max || self.offset >= self.data.len() {
            return None;
        }
        match decode_one(self.data, self.offset) {
            Ok(ins) => {
                self.offset += ins.length() as usize;
                Some(Ok(ins))
            }
            Err(e) => Some(Err(e)),
        }
    }
}
