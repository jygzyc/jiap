//! string_ids and string_data_item (MUTF-8).

use crate::error::{DexError, Result};
use crate::leb128::{read_u32, read_uleb128};

/// string_id_item: offset to string_data. We only store offsets; string data is in data section.
#[derive(Clone, Debug)]
pub struct DexStrings {
    /// For each string_id index, offset into file to string_data_item.
    offsets: Vec<u32>,
}

impl DexStrings {
    pub fn parse(data: &[u8], header: &super::DexHeader) -> Result<Self> {
        let n = header.string_ids_size as usize;
        let off = header.string_ids_off as usize;
        if n == 0 {
            return Ok(Self { offsets: vec![] });
        }
        let size_needed = off + n * 4;
        if data.len() < size_needed {
            return Err(DexError::Truncated("string_ids".into()));
        }
        let mut offsets = Vec::with_capacity(n);
        for i in 0..n {
            let o = read_u32(data, off + i * 4).ok_or(DexError::Truncated("string_id_item".into()))?;
            offsets.push(o);
        }
        Ok(Self { offsets })
    }

    /// Get string at index. Reads string_data_item: uleb128 utf16_size, then MUTF-8 data until 0.
    pub fn get(&self, data: &[u8], idx: u32) -> Result<String> {
        let idx = idx as usize;
        let offset = *self.offsets.get(idx).ok_or(DexError::Truncated("string index".into()))? as usize;
        if offset >= data.len() {
            return Err(DexError::Truncated("string_data_off".into()));
        }
        let (_utf16_size, n) = read_uleb128(data, offset).ok_or(DexError::Truncated("utf16_size".into()))?;
        let start = offset + n;
        let mut end = start;
        while end < data.len() && data[end] != 0 {
            end += 1;
        }
        let bytes = &data[start..end];
        decode_mutf8(bytes).map_err(DexError::Parse)
    }
}

/// Decode MUTF-8 to String. Handles 1/2/3 byte sequences and surrogate pairs.
fn decode_mutf8(bytes: &[u8]) -> std::result::Result<String, String> {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let (ch, advance) = if b & 0x80 == 0 {
            (b as char, 1)
        } else if b & 0xe0 == 0xc0 {
            if i + 1 >= bytes.len() {
                return Err("truncated MUTF-8".into());
            }
            let c = (((b & 0x1f) as u32) << 6) | ((bytes[i + 1] & 0x3f) as u32);
            (char::from_u32(c).unwrap_or('\u{fffd}'), 2)
        } else if b & 0xf0 == 0xe0 {
            if i + 2 >= bytes.len() {
                return Err("truncated MUTF-8".into());
            }
            let c = (((b & 0x0f) as u32) << 12)
                | (((bytes[i + 1] & 0x3f) as u32) << 6)
                | ((bytes[i + 2] & 0x3f) as u32);
            (char::from_u32(c).unwrap_or('\u{fffd}'), 3)
        } else {
            return Err("invalid MUTF-8".into());
        };
        out.push(ch);
        i += advance;
    }
    Ok(out)
}
