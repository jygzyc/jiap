//! method_ids: class_idx (ushort), proto_idx (ushort), name_idx (uint).

use crate::error::{DexError, Result};
use crate::leb128::{read_u32, read_u16};
use super::DexHeader;

#[derive(Clone, Debug)]
pub struct MethodId {
    pub class_idx: u16,
    pub proto_idx: u16,
    pub name_idx: u32,
}

#[derive(Clone, Debug)]
pub struct DexMethods {
    items: Vec<MethodId>,
}

impl DexMethods {
    pub fn parse(data: &[u8], header: &DexHeader) -> Result<Self> {
        let n = header.method_ids_size as usize;
        let off = header.method_ids_off as usize;
        if n == 0 {
            return Ok(Self { items: vec![] });
        }
        let size_needed = off + n * 8;
        if data.len() < size_needed {
            return Err(DexError::Truncated("method_ids".into()));
        }
        let mut items = Vec::with_capacity(n);
        for i in 0..n {
            let base = off + i * 8;
            let class_idx = read_u16(data, base).ok_or(DexError::Truncated("method class_idx".into()))?;
            let proto_idx = read_u16(data, base + 2).ok_or(DexError::Truncated("method proto_idx".into()))?;
            let name_idx = read_u32(data, base + 4).ok_or(DexError::Truncated("method name_idx".into()))?;
            items.push(MethodId {
                class_idx,
                proto_idx,
                name_idx,
            });
        }
        Ok(Self { items })
    }

    pub fn get(&self, idx: u32) -> Result<&MethodId> {
        self.items.get(idx as usize).ok_or_else(|| DexError::Truncated("method index".into()))
    }
}
