//! proto_ids: shorty_idx, return_type_idx, parameters_off.

use crate::error::{DexError, Result};
use crate::leb128::{read_u32, read_u16};
use super::{DexTypes, strings::DexStrings};

#[derive(Clone, Debug)]
pub struct ProtoId {
    #[allow(dead_code)]
    pub shorty_idx: u32,
    pub return_type_idx: u32,
    pub parameters_off: u32,
}

#[derive(Clone, Debug)]
pub struct DexProtos {
    items: Vec<ProtoId>,
}

impl DexProtos {
    pub fn parse(data: &[u8], header: &super::DexHeader) -> Result<Self> {
        let n = header.proto_ids_size as usize;
        let off = header.proto_ids_off as usize;
        if n == 0 {
            return Ok(Self { items: vec![] });
        }
        let size_needed = off + n * 12;
        if data.len() < size_needed {
            return Err(DexError::Truncated("proto_ids".into()));
        }
        let mut items = Vec::with_capacity(n);
        for i in 0..n {
            let base = off + i * 12;
            let shorty_idx = read_u32(data, base).ok_or(DexError::Truncated("proto shorty_idx".into()))?;
            let return_type_idx = read_u32(data, base + 4).ok_or(DexError::Truncated("proto return_type_idx".into()))?;
            let parameters_off = read_u32(data, base + 8).ok_or(DexError::Truncated("proto parameters_off".into()))?;
            items.push(ProtoId {
                shorty_idx,
                return_type_idx,
                parameters_off,
            });
        }
        Ok(Self { items })
    }

    pub fn get_proto(
        &self,
        data: &[u8],
        types: &DexTypes,
        strings: &DexStrings,
        idx: u32,
    ) -> Result<(String, Vec<String>)> {
        let p = self.items.get(idx as usize).ok_or(DexError::Truncated("proto index".into()))?;
        let return_type = types.get(data, strings, p.return_type_idx)?;
        let mut params = Vec::new();
        if p.parameters_off != 0 {
            let off = p.parameters_off as usize;
            if data.len() < off + 4 {
                return Err(DexError::Truncated("type_list".into()));
            }
            let size = read_u32(data, off).ok_or(DexError::Truncated("type_list size".into()))? as usize;
            for i in 0..size {
                let type_idx = read_u16(data, off + 4 + i * 2).ok_or(DexError::Truncated("type_item".into()))? as u32;
                params.push(types.get(data, strings, type_idx)?);
            }
        }
        Ok((return_type, params))
    }
}
