//! code_item: registers_size, ins_size, outs_size, tries_size, debug_info_off, insns_size, insns[], padding?, tries?, handlers?

use crate::error::{DexError, Result};
use crate::leb128::{read_u32, read_u16};

#[derive(Clone, Debug)]
pub struct CodeItem {
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub tries_size: u16,
    pub debug_info_off: u32,
    /// Size in 16-bit code units (bytes = insns_size * 2).
    pub insns_size: u32,
    /// Offset into file where insns array starts (after code_item header).
    pub insns_off: usize,
}

impl CodeItem {
    /// Parse code_item at given file offset.
    pub fn parse(data: &[u8], code_off: u32) -> Result<Self> {
        let off = code_off as usize;
        if data.len() < off + 16 {
            return Err(DexError::Truncated("code_item header".into()));
        }
        let registers_size = read_u16(data, off).ok_or(DexError::Truncated("registers_size".into()))?;
        let ins_size = read_u16(data, off + 2).ok_or(DexError::Truncated("ins_size".into()))?;
        let outs_size = read_u16(data, off + 4).ok_or(DexError::Truncated("outs_size".into()))?;
        let tries_size = read_u16(data, off + 6).ok_or(DexError::Truncated("tries_size".into()))?;
        let debug_info_off = read_u32(data, off + 8).ok_or(DexError::Truncated("debug_info_off".into()))?;
        let insns_size = read_u32(data, off + 12).ok_or(DexError::Truncated("insns_size".into()))?;
        let insns_off = off + 16;
        let insns_bytes = (insns_size as usize).saturating_mul(2);
        if data.len() < insns_off + insns_bytes {
            return Err(DexError::Truncated("code_item insns".into()));
        }
        Ok(Self {
            registers_size,
            ins_size,
            outs_size,
            tries_size,
            debug_info_off,
            insns_size,
            insns_off,
        })
    }

    /// Byte slice of instructions for this code_item (insns array only).
    pub fn insns_slice<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        let len = (self.insns_size as usize).saturating_mul(2);
        let end = (self.insns_off + len).min(data.len());
        &data[self.insns_off..end]
    }

    /// Size of insns in 16-bit units.
    pub fn insns_size_units(&self) -> usize {
        self.insns_size as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_item_parse_minimal() {
        // Minimal code_item: 2 registers, 0 ins/outs, 0 tries, 0 debug_info, 1 insn (2 bytes)
        let mut data = vec![0u8; 18];
        data[0..2].copy_from_slice(&(2u16).to_le_bytes()); // registers_size
        data[2..4].copy_from_slice(&0u16.to_le_bytes()); // ins_size
        data[4..6].copy_from_slice(&0u16.to_le_bytes()); // outs_size
        data[6..8].copy_from_slice(&0u16.to_le_bytes()); // tries_size
        data[8..12].copy_from_slice(&0u32.to_le_bytes()); // debug_info_off
        data[12..16].copy_from_slice(&(1u32).to_le_bytes()); // insns_size (1 unit = 2 bytes)
        data[16..18].copy_from_slice(&[0x0e, 0x00]); // return-void
        let code = CodeItem::parse(&data, 0).unwrap();
        assert_eq!(code.registers_size, 2);
        assert_eq!(code.insns_size, 1);
        assert_eq!(code.insns_off, 16);
        assert_eq!(code.insns_slice(&data), &[0x0e, 0x00]);
    }

    #[test]
    fn code_item_parse_truncated_header() {
        let data = vec![0u8; 8];
        assert!(CodeItem::parse(&data, 0).is_err());
    }

    #[test]
    fn code_item_parse_truncated_insns() {
        let mut data = vec![0u8; 16];
        data[12..16].copy_from_slice(&(100u32).to_le_bytes()); // insns_size = 100, but only 0 bytes after
        assert!(CodeItem::parse(&data, 0).is_err());
    }
}
