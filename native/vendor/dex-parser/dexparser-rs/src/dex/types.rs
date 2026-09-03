//! type_ids: descriptor_idx (into string_ids).

use crate::error::{DexError, Result};
use crate::leb128::read_u32;
use super::strings::DexStrings;

#[derive(Clone, Debug)]
pub struct DexTypes {
    /// For each type_id index, string_id index of descriptor.
    descriptor_idxs: Vec<u32>,
}

impl DexTypes {
    pub fn parse(data: &[u8], header: &super::DexHeader) -> Result<Self> {
        let n = header.type_ids_size as usize;
        let off = header.type_ids_off as usize;
        if n == 0 {
            return Ok(Self { descriptor_idxs: vec![] });
        }
        let size_needed = off + n * 4;
        if data.len() < size_needed {
            return Err(DexError::Truncated("type_ids".into()));
        }
        let mut descriptor_idxs = Vec::with_capacity(n);
        for i in 0..n {
            let idx = read_u32(data, off + i * 4).ok_or(DexError::Truncated("type_id_item".into()))?;
            descriptor_idxs.push(idx);
        }
        Ok(Self { descriptor_idxs })
    }

    pub fn get(&self, data: &[u8], strings: &DexStrings, idx: u32) -> Result<String> {
        let idx = idx as usize;
        let string_idx = *self.descriptor_idxs.get(idx).ok_or(DexError::Truncated("type index".into()))?;
        strings.get(data, string_idx)
    }
}
