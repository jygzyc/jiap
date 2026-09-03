//! Dalvik instruction representation and operand formatting.
//! Matches androguard Instruction output (mnemonic + get_output).

use alloc::string::String;
use core::fmt;

/// Kind of reference for constant-pool-style operands (string, type, field, method, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum RefKind {
    #[default]
    None,
    String,
    Type,
    Field,
    Method,
    MethodProto,
    CallSite,
    Varies,
}

/// A decoded Dalvik instruction: offset, size, opcode, mnemonic, and operands string.
#[derive(Debug, Clone)]
pub struct Instruction {
    /// Byte offset of this instruction in the code buffer.
    pub offset: u32,
    /// Length in bytes (2, 4, 6, 8, or 10 for standard; variable for payloads).
    pub length: u32,
    /// Raw opcode byte (0x00..=0xFF).
    pub opcode: u8,
    /// Mnemonic (e.g. "move", "invoke-virtual").
    pub mnemonic: &'static str,
    /// Formatted operands (e.g. "v0, v1" or "v0, 0x1234").
    pub operands: String,
}

impl Instruction {
    pub fn new(
        offset: u32,
        length: u32,
        opcode: u8,
        mnemonic: &'static str,
        operands: String,
    ) -> Self {
        Self {
            offset,
            length,
            opcode,
            mnemonic,
            operands,
        }
    }

    /// Instruction length in bytes.
    #[inline]
    pub fn length(&self) -> u32 {
        self.length
    }

    /// Opcode value.
    #[inline]
    pub fn opcode(&self) -> u8 {
        self.opcode
    }

    /// Mnemonic name.
    #[inline]
    pub fn mnemonic(&self) -> &'static str {
        self.mnemonic
    }

    /// Operands as string (e.g. "v0, v1").
    #[inline]
    pub fn operands(&self) -> &str {
        &self.operands
    }

    /// Full disassembly line: mnemonic + " " + operands.
    pub fn disasm_line(&self) -> String {
        if self.operands.is_empty() {
            alloc::string::ToString::to_string(self.mnemonic)
        } else {
            let mut s = String::with_capacity(self.mnemonic.len() + 1 + self.operands.len());
            s.push_str(self.mnemonic);
            s.push(' ');
            s.push_str(&self.operands);
            s
        }
    }

    /// Format reference for display: kind prefix + index (e.g. "string@5", "type@2").
    pub fn format_ref(kind: RefKind, index: u32) -> String {
        let prefix = match kind {
            RefKind::None => return index.to_string(),
            RefKind::String => "string@",
            RefKind::Type => "type@",
            RefKind::Field => "field@",
            RefKind::Method => "method@",
            RefKind::MethodProto => "proto@",
            RefKind::CallSite => "callsite@",
            RefKind::Varies => "ref@",
        };
        let mut s = String::with_capacity(prefix.len() + 4);
        s.push_str(prefix);
        s.push_str(&index.to_string());
        s
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.mnemonic, self.operands)
    }
}
