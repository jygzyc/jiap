//! Patching and minimal encoding of Dalvik bytecode.
//!
//! Use for rewriting branch targets or emitting simple instructions without a full assembler.

use core::convert::TryInto;

use crate::error::DexError;
use crate::opcodes::{format_length, get_opcode_entry, get_payload_kind, Format};

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
fn write_u16(data: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    data[offset..offset + 2].copy_from_slice(&bytes);
}

#[inline]
fn write_i16(data: &mut [u8], offset: usize, value: i16) {
    write_u16(data, offset, value as u16);
}

#[inline]
fn write_i32(data: &mut [u8], offset: usize, value: i32) {
    let bytes = value.to_le_bytes();
    data[offset..offset + 4].copy_from_slice(&bytes);
}

/// Patches the branch instruction at `from_offset` so it jumps to `to_offset` (byte offset).
/// Supports F10t (goto), F20t (goto/16), F21t (if-*z), F22t (if-*), F30t (goto/32).
/// Returns an error if the instruction is not a branch or the relative offset is out of range.
pub fn patch_branch_target(
    data: &mut [u8],
    from_offset: usize,
    to_offset: u32,
) -> Result<(), DexError> {
    let unit =
        read_u16(data, from_offset).ok_or_else(|| DexError::invalid("truncated at branch"))?;
    let op = unit as u8;
    if unit > 0xFF && (op == 0x00 || op == 0xFF) && get_payload_kind(unit).is_some() {
        return Err(DexError::invalid("payload is not a branch"));
    }
    let entry = get_opcode_entry(op);
    let len = format_length(entry.format) as usize;
    if from_offset + len > data.len() {
        return Err(DexError::invalid("branch instruction extends past buffer"));
    }
    let from_i = from_offset as i32;
    let to_i = to_offset as i32;
    // Dalvik: target = from + rel_units * 2 (relative to the branch instruction itself).
    let rel_units = (to_i - from_i) / 2;
    match entry.format {
        Format::F10t => {
            let rel = rel_units as i8;
            if rel_units != rel as i32 {
                return Err(DexError::invalid_owned(format!(
                    "F10t branch offset {} units out of s8 range",
                    rel_units
                )));
            }
            data[from_offset + 1] = rel as u8;
        }
        Format::F20t => {
            let rel = rel_units as i16;
            if rel_units != rel as i32 {
                return Err(DexError::invalid_owned(format!(
                    "F20t branch offset {} units out of s16 range",
                    rel_units
                )));
            }
            write_i16(data, from_offset + 2, rel);
        }
        Format::F21t | Format::F22t => {
            let rel = rel_units as i16;
            if rel_units != rel as i32 {
                return Err(DexError::invalid_owned(format!(
                    "F21t/F22t branch offset {} units out of s16 range",
                    rel_units
                )));
            }
            write_i16(data, from_offset + 2, rel);
        }
        Format::F30t => {
            write_i32(data, from_offset + 2, rel_units);
        }
        _ => {
            return Err(DexError::invalid_owned(format!(
                "opcode 0x{:02x} is not a patchable branch",
                op
            )));
        }
    }
    Ok(())
}

/// Encodes a single `nop` instruction (2 bytes).
#[inline]
pub fn encode_nop() -> [u8; 2] {
    [0x00, 0x00]
}

/// Encodes a single `return-void` instruction (2 bytes).
#[inline]
pub fn encode_return_void() -> [u8; 2] {
    [0x0e, 0x00]
}

/// Encodes a `goto` (F10t) to a relative offset in 16-bit units.
/// `rel_units`: signed 8-bit; target = instruction_address + rel_units * 2.
#[inline]
pub fn encode_goto(rel_units: i8) -> [u8; 2] {
    [0x28, rel_units as u8]
}
