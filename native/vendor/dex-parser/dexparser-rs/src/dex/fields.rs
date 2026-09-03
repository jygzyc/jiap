//! field_ids: class_idx (ushort), type_idx (ushort), name_idx (uint).

use crate::error::{DexError, Result};
use crate::leb128::{read_u32, read_u16};
use super::DexHeader;

#[derive(Clone, Debug)]
pub struct FieldId {
    pub class_idx: u16,
    pub type_idx: u16,
    pub name_idx: u32,
}

#[derive(Clone, Debug)]
pub struct DexFields {
    items: Vec<FieldId>,
}

impl DexFields {
    pub fn parse(data: &[u8], header: &DexHeader) -> Result<Self> {
        let n = header.field_ids_size as usize;
        let off = header.field_ids_off as usize;
        if n == 0 {
            return Ok(Self { items: vec![] });
        }
        let size_needed = off + n * 8;
        if data.len() < size_needed {
            return Err(DexError::Truncated("field_ids".into()));
        }
        let mut items = Vec::with_capacity(n);
        for i in 0..n {
            let base = off + i * 8;
            let class_idx = read_u16(data, base).ok_or(DexError::Truncated("field class_idx".into()))?;
            let type_idx = read_u16(data, base + 2).ok_or(DexError::Truncated("field type_idx".into()))?;
            let name_idx = read_u32(data, base + 4).ok_or(DexError::Truncated("field name_idx".into()))?;
            items.push(FieldId {
                class_idx,
                type_idx,
                name_idx,
            });
        }
        Ok(Self { items })
    }

    pub fn get(&self, idx: u32) -> Result<&FieldId> {
        self.items.get(idx as usize).ok_or_else(|| DexError::Truncated("field index".into()))
    }
}
