//! call_site_ids / method_handles (DEX 038+) for invoke-custom / lambdas.
//!
//! Located via `map_list` (`TYPE_CALL_SITE_ID_ITEM` = 0x0007,
//! `TYPE_METHOD_HANDLE_ITEM` = 0x0008).

use crate::error::{DexError, Result};
use crate::leb128::{read_u16, read_u32, read_uleb128};

use super::DexHeader;

const TYPE_CALL_SITE_ID_ITEM: u16 = 0x0007;
const TYPE_METHOD_HANDLE_ITEM: u16 = 0x0008;

/// One resolved call-site (bootstrap + dynamic name/type + static args).
#[derive(Clone, Debug)]
pub struct CallSiteInfo {
    /// Index into method_handles.
    pub bootstrap_handle_idx: u32,
    /// Dynamic method name (SAM name for lambdas, e.g. `accept`).
    pub method_name: String,
    /// Proto index for the factory / invokedynamic type.
    pub proto_idx: u32,
    /// Extra static bootstrap arguments (LambdaMetafactory: MethodType, MethodHandle, …).
    pub extra: Vec<CallSiteValue>,
}

/// Encoded value subset used in call_site_item arrays.
#[derive(Clone, Debug)]
pub enum CallSiteValue {
    MethodHandle { handle_type: u16, id: u16 },
    MethodType(u32),
    String(String),
    Type(u32),
    Int(i32),
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct DexCallSites {
    /// Offsets of call_site_item for each call_site_id index.
    call_site_offs: Vec<u32>,
    /// Raw method_handle_item table (8 bytes each).
    method_handles: Vec<MethodHandleItem>,
}

#[derive(Clone, Debug)]
pub struct MethodHandleItem {
    pub handle_type: u16,
    pub field_or_method_id: u16,
}

impl DexCallSites {
    /// Parse call sites / method handles from the DEX map_list (empty if absent).
    pub fn parse(data: &[u8], header: &DexHeader) -> Result<Self> {
        let map = parse_map_list(data, header.map_off)?;
        let mut call_site_offs = Vec::new();
        let mut method_handles = Vec::new();
        for item in map {
            match item.type_code {
                TYPE_CALL_SITE_ID_ITEM => {
                    call_site_offs = parse_call_site_ids(data, item.offset, item.size)?;
                }
                TYPE_METHOD_HANDLE_ITEM => {
                    method_handles = parse_method_handles(data, item.offset, item.size)?;
                }
                _ => {}
            }
        }
        Ok(Self {
            call_site_offs,
            method_handles,
        })
    }

    pub fn len(&self) -> usize {
        self.call_site_offs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.call_site_offs.is_empty()
    }

    pub fn get_method_handle(&self, idx: u32) -> Option<&MethodHandleItem> {
        self.method_handles.get(idx as usize)
    }

    /// Resolve a call_site_id index into structured info.
    pub fn get_call_site(
        &self,
        data: &[u8],
        get_string: &dyn Fn(u32) -> Result<String>,
        idx: u32,
    ) -> Result<CallSiteInfo> {
        let off = *self
            .call_site_offs
            .get(idx as usize)
            .ok_or_else(|| DexError::Truncated(format!("call_site_id {}", idx)))?
            as usize;
        let values = parse_encoded_array(data, off)?;
        if values.len() < 3 {
            return Err(DexError::Truncated("call_site_item too short".into()));
        }
        let bootstrap_handle_idx = match &values[0] {
            Encoded::MethodHandle(h) => *h,
            _ => {
                return Err(DexError::Truncated(
                    "call_site bootstrap must be method_handle".into(),
                ))
            }
        };
        let method_name = match &values[1] {
            Encoded::String(sidx) => get_string(*sidx)?,
            _ => {
                return Err(DexError::Truncated(
                    "call_site name must be string".into(),
                ))
            }
        };
        let proto_idx = match &values[2] {
            Encoded::MethodType(p) => *p,
            _ => {
                return Err(DexError::Truncated(
                    "call_site type must be method_type".into(),
                ))
            }
        };
        let mut extra = Vec::new();
        for v in values.into_iter().skip(3) {
            extra.push(match v {
                Encoded::MethodHandle(h) => {
                    if let Some(mh) = self.get_method_handle(h) {
                        CallSiteValue::MethodHandle {
                            handle_type: mh.handle_type,
                            id: mh.field_or_method_id,
                        }
                    } else {
                        CallSiteValue::Other
                    }
                }
                Encoded::MethodType(p) => CallSiteValue::MethodType(p),
                Encoded::String(s) => CallSiteValue::String(get_string(s).unwrap_or_default()),
                Encoded::Type(t) => CallSiteValue::Type(t),
                Encoded::Int(i) => CallSiteValue::Int(i),
                _ => CallSiteValue::Other,
            });
        }
        Ok(CallSiteInfo {
            bootstrap_handle_idx,
            method_name,
            proto_idx,
            extra,
        })
    }

    /// First MethodHandle in extra args that looks like an implementation method
    /// (LambdaMetafactory: MethodType, MethodHandle, MethodType, …).
    pub fn impl_method_id(info: &CallSiteInfo) -> Option<u16> {
        for v in &info.extra {
            if let CallSiteValue::MethodHandle { handle_type, id } = v {
                // 0–3 field, 4–8 method (see dex method_handle_type).
                if *handle_type >= 4 {
                    return Some(*id);
                }
            }
        }
        None
    }
}

struct MapItem {
    type_code: u16,
    size: u32,
    offset: u32,
}

fn parse_map_list(data: &[u8], map_off: u32) -> Result<Vec<MapItem>> {
    let off = map_off as usize;
    if off + 4 > data.len() {
        return Err(DexError::Truncated("map_list".into()));
    }
    let size = read_u32(data, off).ok_or(DexError::Truncated("map_list size".into()))? as usize;
    let mut out = Vec::with_capacity(size);
    for i in 0..size {
        let base = off + 4 + i * 12;
        if base + 12 > data.len() {
            return Err(DexError::Truncated("map_item".into()));
        }
        let type_code =
            read_u16(data, base).ok_or(DexError::Truncated("map_item type".into()))?;
        let item_size =
            read_u32(data, base + 4).ok_or(DexError::Truncated("map_item size".into()))?;
        let item_off =
            read_u32(data, base + 8).ok_or(DexError::Truncated("map_item offset".into()))?;
        out.push(MapItem {
            type_code,
            size: item_size,
            offset: item_off,
        });
    }
    Ok(out)
}

fn parse_call_site_ids(data: &[u8], offset: u32, size: u32) -> Result<Vec<u32>> {
    let off = offset as usize;
    let n = size as usize;
    if off + n * 4 > data.len() {
        return Err(DexError::Truncated("call_site_ids".into()));
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let o = read_u32(data, off + i * 4)
            .ok_or(DexError::Truncated("call_site_id_item".into()))?;
        out.push(o);
    }
    Ok(out)
}

fn parse_method_handles(data: &[u8], offset: u32, size: u32) -> Result<Vec<MethodHandleItem>> {
    let off = offset as usize;
    let n = size as usize;
    if off + n * 8 > data.len() {
        return Err(DexError::Truncated("method_handles".into()));
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let base = off + i * 8;
        let handle_type =
            read_u16(data, base).ok_or(DexError::Truncated("method_handle type".into()))?;
        let field_or_method_id =
            read_u16(data, base + 4).ok_or(DexError::Truncated("method_handle id".into()))?;
        out.push(MethodHandleItem {
            handle_type,
            field_or_method_id,
        });
    }
    Ok(out)
}

#[derive(Clone, Debug)]
enum Encoded {
    MethodHandle(u32),
    MethodType(u32),
    String(u32),
    Type(u32),
    Int(i32),
    Other,
}

fn parse_encoded_array(data: &[u8], off: usize) -> Result<Vec<Encoded>> {
    let mut pos = off;
    let (size, n) =
        read_uleb128(data, pos).ok_or(DexError::Truncated("encoded_array size".into()))?;
    pos += n;
    let mut out = Vec::with_capacity(size as usize);
    for _ in 0..size {
        let (v, np) = parse_encoded_value(data, pos)?;
        pos = np;
        out.push(v);
    }
    Ok(out)
}

fn parse_encoded_value(data: &[u8], mut pos: usize) -> Result<(Encoded, usize)> {
    if pos >= data.len() {
        return Err(DexError::Truncated("encoded_value".into()));
    }
    let header = data[pos];
    pos += 1;
    let value_type = header & 0x1f;
    let value_arg = (header >> 5) as usize;
    match value_type {
        0x04 => {
            // int
            let mut buf = [0u8; 4];
            for i in 0..=value_arg.min(3) {
                buf[i] = *data
                    .get(pos)
                    .ok_or(DexError::Truncated("encoded int".into()))?;
                pos += 1;
            }
            Ok((Encoded::Int(i32::from_le_bytes(buf)), pos))
        }
        0x17 => {
            let (idx, np) = read_indexed(data, pos, value_arg)?;
            Ok((Encoded::String(idx), np))
        }
        0x18 => {
            let (idx, np) = read_indexed(data, pos, value_arg)?;
            Ok((Encoded::Type(idx), np))
        }
        0x15 => {
            // METHOD_TYPE → proto_id
            let (idx, np) = read_indexed(data, pos, value_arg)?;
            Ok((Encoded::MethodType(idx), np))
        }
        0x16 => {
            // METHOD_HANDLE
            let (idx, np) = read_indexed(data, pos, value_arg)?;
            Ok((Encoded::MethodHandle(idx), np))
        }
        0x1c => {
            // array — skip nested
            let (size, n) =
                read_uleb128(data, pos).ok_or(DexError::Truncated("nested array".into()))?;
            pos += n;
            for _ in 0..size {
                let (_, np) = parse_encoded_value(data, pos)?;
                pos = np;
            }
            Ok((Encoded::Other, pos))
        }
        0x1e => Ok((Encoded::Other, pos)), // null
        0x1f => Ok((Encoded::Other, pos)), // boolean in header
        _ => {
            // skip value_arg+1 bytes for unknown fixed-size
            if value_type <= 0x1b {
                pos += value_arg + 1;
            }
            Ok((Encoded::Other, pos))
        }
    }
}

fn read_indexed(data: &[u8], mut pos: usize, value_arg: usize) -> Result<(u32, usize)> {
    let mut buf = [0u8; 4];
    for i in 0..=value_arg.min(3) {
        buf[i] = *data
            .get(pos)
            .ok_or(DexError::Truncated("encoded index".into()))?;
        pos += 1;
    }
    Ok((u32::from_le_bytes(buf), pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_yields_empty_call_sites() {
        // Minimal DEX map with only header map item is enough if we pass empty sections.
        // Just ensure parse_map_list doesn't panic on tiny input.
        let mut data = vec![0u8; 16];
        data[0..4].copy_from_slice(&1u32.to_le_bytes()); // size=1
        // type=0x0000, unused, size=1, offset=0
        data[4..6].copy_from_slice(&0u16.to_le_bytes());
        data[8..12].copy_from_slice(&1u32.to_le_bytes());
        data[12..16].copy_from_slice(&0u32.to_le_bytes());
        let map = parse_map_list(&data, 0).unwrap();
        assert_eq!(map.len(), 1);
    }
}
