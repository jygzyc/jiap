//! Dalvik opcode table: opcode -> format, mnemonic, and reference kind.
//! Matches androguard/core/dex/__init__.py DALVIK_OPCODES_FORMAT.

use crate::instruction::RefKind;

/// Format identifier for instruction decoding (number of 16-bit units and layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Format {
    F00x,  // unused / invalid
    F10x,  // op
    F10t,  // op AA (s8 offset)
    F11n,  // op A|B (nibble const)
    F11x,  // op AA
    F12x,  // B|A|op
    F20t,  // op ØØØØ AAAA
    F20bc, // op AA BBBB
    F21c,  // op AA BBBB
    F21h,  // op AA BBBB (special shift)
    F21s,  // op AA BBBB (s16)
    F21t,  // op AA BBBB (branch)
    F22b,  // op AA BB CC
    F22x,  // op AA BBBB (16-bit register)
    F22c,  // B|A|op CCCC
    F22s,  // B|A|op CCCC (s16)
    F22t,  // B|A|op CCCC (branch)
    F22cs, // B|A|op CCCC
    F23x,  // op AA BB CC
    F30t,  // op ØØØØ AAAAAAAA
    F31c,  // op AA BBBBBBBB
    F31i,  // op AA BBBBBBBB
    F31t,  // op AA BBBBBBBB (branch)
    F32x,  // op ØØØØ AAAA BBBB
    F35c,  // A|G|op BBBB C|D|E|F
    F35mi, // A|G|op BBBB C|D|E|F
    F35ms, // A|G|op BBBB C|D|E|F
    F3rc,  // op AA BBBB CCCC
    F3rmi, // op AA BBBB CCCC
    F3rms, // op AA BBBB CCCC
    F40sc, // op BBBBBBBB AAAA
    F41c,  // op BBBBBBBB AAAA
    F45cc, // op A|G BBBB C|D|E|F HHHH
    F4rcc, // op AA BBBB CCCC HHHH
    F51l,  // op AA BBBBBBBBBBBBBBBB
    F52c,  // op CCCCCCCC AAAA BBBB
    F5rc,  // op BBBBBBBB AAAA CCCC
}

/// Payload pseudo-instruction identifier (high 16-bit = 0x01, 0x02, 0x03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    PackedSwitch,  // 0x0100
    SparseSwitch,  // 0x0200
    FillArrayData, // 0x0300
}

/// Entry in the opcode table.
#[derive(Debug, Clone, Copy)]
pub struct OpcodeEntry {
    pub format: Format,
    pub mnemonic: &'static str,
    pub ref_kind: RefKind,
}

impl OpcodeEntry {
    const fn new(format: Format, mnemonic: &'static str, ref_kind: RefKind) -> Self {
        Self {
            format,
            mnemonic,
            ref_kind,
        }
    }
}

/// Instruction length in bytes for each format (standard instructions only; payloads vary).
#[inline]
pub fn format_length(f: Format) -> u32 {
    match f {
        Format::F00x => 0,
        Format::F10x | Format::F10t | Format::F11n | Format::F11x | Format::F12x => 2,
        Format::F20t
        | Format::F20bc
        | Format::F21c
        | Format::F21h
        | Format::F21s
        | Format::F21t
        | Format::F22b
        | Format::F22x
        | Format::F22c
        | Format::F22s
        | Format::F22t
        | Format::F22cs
        | Format::F23x => 4,
        Format::F30t
        | Format::F31c
        | Format::F31i
        | Format::F31t
        | Format::F32x
        | Format::F35c
        | Format::F35mi
        | Format::F35ms
        | Format::F3rc
        | Format::F3rmi
        | Format::F3rms => 6,
        Format::F40sc | Format::F41c | Format::F45cc | Format::F4rcc => 8,
        Format::F51l | Format::F52c | Format::F5rc => 10,
    }
}

/// Opcode table for standard Dalvik opcodes (0x00..=0xFF).
/// Index = opcode byte (low byte of first 16-bit unit).
pub static OPCODE_TABLE: [OpcodeEntry; 256] = [
    // 0x00
    OpcodeEntry::new(Format::F10x, "nop", RefKind::None),
    OpcodeEntry::new(Format::F12x, "move", RefKind::None),
    OpcodeEntry::new(Format::F22x, "move/from16", RefKind::None),
    OpcodeEntry::new(Format::F32x, "move/16", RefKind::None),
    OpcodeEntry::new(Format::F12x, "move-wide", RefKind::None),
    OpcodeEntry::new(Format::F22x, "move-wide/from16", RefKind::None),
    OpcodeEntry::new(Format::F32x, "move-wide/16", RefKind::None),
    OpcodeEntry::new(Format::F12x, "move-object", RefKind::None),
    OpcodeEntry::new(Format::F22x, "move-object/from16", RefKind::None),
    OpcodeEntry::new(Format::F32x, "move-object/16", RefKind::None),
    OpcodeEntry::new(Format::F11x, "move-result", RefKind::None),
    OpcodeEntry::new(Format::F11x, "move-result-wide", RefKind::None),
    OpcodeEntry::new(Format::F11x, "move-result-object", RefKind::None),
    OpcodeEntry::new(Format::F11x, "move-exception", RefKind::None),
    OpcodeEntry::new(Format::F10x, "return-void", RefKind::None),
    OpcodeEntry::new(Format::F11x, "return", RefKind::None),
    // 0x10
    OpcodeEntry::new(Format::F11x, "return-wide", RefKind::None),
    OpcodeEntry::new(Format::F11x, "return-object", RefKind::None),
    OpcodeEntry::new(Format::F11n, "const/4", RefKind::None),
    OpcodeEntry::new(Format::F21s, "const/16", RefKind::None),
    OpcodeEntry::new(Format::F31i, "const", RefKind::None),
    OpcodeEntry::new(Format::F21h, "const/high16", RefKind::None),
    OpcodeEntry::new(Format::F21s, "const-wide/16", RefKind::None),
    OpcodeEntry::new(Format::F31i, "const-wide/32", RefKind::None),
    OpcodeEntry::new(Format::F51l, "const-wide", RefKind::None),
    OpcodeEntry::new(Format::F21h, "const-wide/high16", RefKind::None),
    OpcodeEntry::new(Format::F21c, "const-string", RefKind::String),
    OpcodeEntry::new(Format::F31c, "const-string/jumbo", RefKind::String),
    OpcodeEntry::new(Format::F21c, "const-class", RefKind::Type),
    OpcodeEntry::new(Format::F11x, "monitor-enter", RefKind::None),
    OpcodeEntry::new(Format::F11x, "monitor-exit", RefKind::None),
    OpcodeEntry::new(Format::F21c, "check-cast", RefKind::Type),
    // 0x20
    OpcodeEntry::new(Format::F22c, "instance-of", RefKind::Type),
    OpcodeEntry::new(Format::F12x, "array-length", RefKind::None),
    OpcodeEntry::new(Format::F21c, "new-instance", RefKind::Type),
    OpcodeEntry::new(Format::F22c, "new-array", RefKind::Type),
    OpcodeEntry::new(Format::F35c, "filled-new-array", RefKind::Type),
    OpcodeEntry::new(Format::F3rc, "filled-new-array/range", RefKind::Type),
    OpcodeEntry::new(Format::F31t, "fill-array-data", RefKind::None),
    OpcodeEntry::new(Format::F11x, "throw", RefKind::None),
    OpcodeEntry::new(Format::F10t, "goto", RefKind::None),
    OpcodeEntry::new(Format::F20t, "goto/16", RefKind::None),
    OpcodeEntry::new(Format::F30t, "goto/32", RefKind::None),
    OpcodeEntry::new(Format::F31t, "packed-switch", RefKind::None),
    OpcodeEntry::new(Format::F31t, "sparse-switch", RefKind::None),
    OpcodeEntry::new(Format::F23x, "cmpl-float", RefKind::None),
    OpcodeEntry::new(Format::F23x, "cmpg-float", RefKind::None),
    OpcodeEntry::new(Format::F23x, "cmpl-double", RefKind::None),
    // 0x30
    OpcodeEntry::new(Format::F23x, "cmpg-double", RefKind::None),
    OpcodeEntry::new(Format::F23x, "cmp-long", RefKind::None),
    OpcodeEntry::new(Format::F22t, "if-eq", RefKind::None),
    OpcodeEntry::new(Format::F22t, "if-ne", RefKind::None),
    OpcodeEntry::new(Format::F22t, "if-lt", RefKind::None),
    OpcodeEntry::new(Format::F22t, "if-ge", RefKind::None),
    OpcodeEntry::new(Format::F22t, "if-gt", RefKind::None),
    OpcodeEntry::new(Format::F22t, "if-le", RefKind::None),
    OpcodeEntry::new(Format::F21t, "if-eqz", RefKind::None),
    OpcodeEntry::new(Format::F21t, "if-nez", RefKind::None),
    OpcodeEntry::new(Format::F21t, "if-ltz", RefKind::None),
    OpcodeEntry::new(Format::F21t, "if-gez", RefKind::None),
    OpcodeEntry::new(Format::F21t, "if-gtz", RefKind::None),
    OpcodeEntry::new(Format::F21t, "if-lez", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    // 0x40
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aget", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aget-wide", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aget-object", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aget-boolean", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aget-byte", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aget-char", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aget-short", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aput", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aput-wide", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aput-object", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aput-boolean", RefKind::None),
    // 0x50
    OpcodeEntry::new(Format::F23x, "aput-byte", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aput-char", RefKind::None),
    OpcodeEntry::new(Format::F23x, "aput-short", RefKind::None),
    OpcodeEntry::new(Format::F22c, "iget", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iget-wide", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iget-object", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iget-boolean", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iget-byte", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iget-char", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iget-short", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iput", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iput-wide", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iput-object", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iput-boolean", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iput-byte", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iput-char", RefKind::Field),
    OpcodeEntry::new(Format::F22c, "iput-short", RefKind::Field),
    // 0x60
    OpcodeEntry::new(Format::F21c, "sget", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sget-wide", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sget-object", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sget-boolean", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sget-byte", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sget-char", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sget-short", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sput", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sput-wide", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sput-object", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sput-boolean", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sput-byte", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sput-char", RefKind::Field),
    OpcodeEntry::new(Format::F21c, "sput-short", RefKind::Field),
    OpcodeEntry::new(Format::F35c, "invoke-virtual", RefKind::Method),
    OpcodeEntry::new(Format::F35c, "invoke-super", RefKind::Method),
    // 0x70
    OpcodeEntry::new(Format::F35c, "invoke-direct", RefKind::Method),
    OpcodeEntry::new(Format::F35c, "invoke-static", RefKind::Method),
    OpcodeEntry::new(Format::F35c, "invoke-interface", RefKind::Method),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F3rc, "invoke-virtual/range", RefKind::Method),
    OpcodeEntry::new(Format::F3rc, "invoke-super/range", RefKind::Method),
    OpcodeEntry::new(Format::F3rc, "invoke-direct/range", RefKind::Method),
    OpcodeEntry::new(Format::F3rc, "invoke-static/range", RefKind::Method),
    OpcodeEntry::new(Format::F3rc, "invoke-interface/range", RefKind::Method),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F12x, "neg-int", RefKind::None),
    OpcodeEntry::new(Format::F12x, "not-int", RefKind::None),
    OpcodeEntry::new(Format::F12x, "neg-long", RefKind::None),
    OpcodeEntry::new(Format::F12x, "not-long", RefKind::None),
    OpcodeEntry::new(Format::F12x, "neg-float", RefKind::None),
    // 0x80
    OpcodeEntry::new(Format::F12x, "neg-double", RefKind::None),
    OpcodeEntry::new(Format::F12x, "int-to-long", RefKind::None),
    OpcodeEntry::new(Format::F12x, "int-to-float", RefKind::None),
    OpcodeEntry::new(Format::F12x, "int-to-double", RefKind::None),
    OpcodeEntry::new(Format::F12x, "long-to-int", RefKind::None),
    OpcodeEntry::new(Format::F12x, "long-to-float", RefKind::None),
    OpcodeEntry::new(Format::F12x, "long-to-double", RefKind::None),
    OpcodeEntry::new(Format::F12x, "float-to-int", RefKind::None),
    OpcodeEntry::new(Format::F12x, "float-to-long", RefKind::None),
    OpcodeEntry::new(Format::F12x, "float-to-double", RefKind::None),
    OpcodeEntry::new(Format::F12x, "double-to-int", RefKind::None),
    OpcodeEntry::new(Format::F12x, "double-to-long", RefKind::None),
    OpcodeEntry::new(Format::F12x, "double-to-float", RefKind::None),
    OpcodeEntry::new(Format::F12x, "int-to-byte", RefKind::None),
    OpcodeEntry::new(Format::F12x, "int-to-char", RefKind::None),
    OpcodeEntry::new(Format::F12x, "int-to-short", RefKind::None),
    // 0x90
    OpcodeEntry::new(Format::F23x, "add-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "sub-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "mul-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "div-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "rem-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "and-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "or-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "xor-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "shl-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "shr-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "ushr-int", RefKind::None),
    OpcodeEntry::new(Format::F23x, "add-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "sub-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "mul-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "div-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "rem-long", RefKind::None),
    // 0xA0
    OpcodeEntry::new(Format::F23x, "and-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "or-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "xor-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "shl-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "shr-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "ushr-long", RefKind::None),
    OpcodeEntry::new(Format::F23x, "add-float", RefKind::None),
    OpcodeEntry::new(Format::F23x, "sub-float", RefKind::None),
    OpcodeEntry::new(Format::F23x, "mul-float", RefKind::None),
    OpcodeEntry::new(Format::F23x, "div-float", RefKind::None),
    OpcodeEntry::new(Format::F23x, "rem-float", RefKind::None),
    OpcodeEntry::new(Format::F23x, "add-double", RefKind::None),
    OpcodeEntry::new(Format::F23x, "sub-double", RefKind::None),
    OpcodeEntry::new(Format::F23x, "mul-double", RefKind::None),
    OpcodeEntry::new(Format::F23x, "div-double", RefKind::None),
    OpcodeEntry::new(Format::F23x, "rem-double", RefKind::None),
    // 0xB0
    OpcodeEntry::new(Format::F12x, "add-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "sub-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "mul-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "div-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "rem-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "and-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "or-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "xor-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "shl-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "shr-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "ushr-int/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "add-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "sub-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "mul-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "div-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "rem-long/2addr", RefKind::None),
    // 0xC0
    OpcodeEntry::new(Format::F12x, "and-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "or-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "xor-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "shl-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "shr-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "ushr-long/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "add-float/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "sub-float/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "mul-float/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "div-float/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "rem-float/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "add-double/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "sub-double/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "mul-double/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "div-double/2addr", RefKind::None),
    OpcodeEntry::new(Format::F12x, "rem-double/2addr", RefKind::None),
    // 0xD0
    OpcodeEntry::new(Format::F22s, "add-int/lit16", RefKind::None),
    OpcodeEntry::new(Format::F22s, "rsub-int", RefKind::None),
    OpcodeEntry::new(Format::F22s, "mul-int/lit16", RefKind::None),
    OpcodeEntry::new(Format::F22s, "div-int/lit16", RefKind::None),
    OpcodeEntry::new(Format::F22s, "rem-int/lit16", RefKind::None),
    OpcodeEntry::new(Format::F22s, "and-int/lit16", RefKind::None),
    OpcodeEntry::new(Format::F22s, "or-int/lit16", RefKind::None),
    OpcodeEntry::new(Format::F22s, "xor-int/lit16", RefKind::None),
    OpcodeEntry::new(Format::F22b, "add-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "rsub-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "mul-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "div-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "rem-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "and-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "or-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "xor-int/lit8", RefKind::None),
    // 0xE0
    OpcodeEntry::new(Format::F22b, "shl-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "shr-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F22b, "ushr-int/lit8", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    // 0xF0
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F00x, "unused", RefKind::None),
    OpcodeEntry::new(Format::F45cc, "invoke-polymorphic", RefKind::MethodProto),
    OpcodeEntry::new(
        Format::F4rcc,
        "invoke-polymorphic/range",
        RefKind::MethodProto,
    ),
    OpcodeEntry::new(Format::F35c, "invoke-custom", RefKind::CallSite),
    OpcodeEntry::new(Format::F3rc, "invoke-custom/range", RefKind::CallSite),
    OpcodeEntry::new(Format::F21c, "const-method-handle", RefKind::Method),
    OpcodeEntry::new(Format::F21c, "const-method-type", RefKind::MethodProto),
];

/// Returns the opcode entry for a given opcode byte (0x00..=0xFF).
#[inline]
pub fn get_opcode_entry(opcode: u8) -> &'static OpcodeEntry {
    &OPCODE_TABLE[opcode as usize]
}

/// Returns payload kind for 16-bit unit 0x0100, 0x0200, 0x0300 (ident in high byte).
pub fn get_payload_kind(unit: u16) -> Option<PayloadKind> {
    match unit {
        0x0100 => Some(PayloadKind::PackedSwitch),
        0x0200 => Some(PayloadKind::SparseSwitch),
        0x0300 => Some(PayloadKind::FillArrayData),
        _ => None,
    }
}
