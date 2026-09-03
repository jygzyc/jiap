//! DEX header_item parsing.

use crate::error::{DexError, Result};
use crate::leb128::read_u32;

/// DEX file magic: "dex\n" (version in bytes 4-7).
pub const DEX_MAGIC: [u8; 4] = [0x64, 0x65, 0x78, 0x0a]; // "dex\n"

/// Returns true if the given bytes look like a DEX file (magic "dex\n" at start).
/// Use this to detect DEX by content when the file has no .dex extension.
#[inline]
pub fn is_dex(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0..4] == DEX_MAGIC
}

/// Parsed header (v035/v037/v038/v039; we use offsets from header).
#[derive(Clone, Debug)]
pub struct DexHeader {
    pub file_size: u32,
    pub header_size: u32,
    pub endian_tag: u32,
    pub link_size: u32,
    pub link_off: u32,
    pub map_off: u32,
    pub string_ids_size: u32,
    pub string_ids_off: u32,
    pub type_ids_size: u32,
    pub type_ids_off: u32,
    pub proto_ids_size: u32,
    pub proto_ids_off: u32,
    pub field_ids_size: u32,
    pub field_ids_off: u32,
    pub method_ids_size: u32,
    pub method_ids_off: u32,
    pub class_defs_size: u32,
    pub class_defs_off: u32,
    pub data_size: u32,
    pub data_off: u32,
}

impl DexHeader {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 0x70 {
            return Err(DexError::Truncated("file shorter than header".into()));
        }
        if &data[0..4] != &DEX_MAGIC {
            return Err(DexError::InvalidMagic);
        }
        let file_size = read_u32(data, 32).ok_or(DexError::Truncated("file_size".into()))?;
        let header_size = read_u32(data, 36).ok_or(DexError::Truncated("header_size".into()))?;
        let endian_tag = read_u32(data, 40).ok_or(DexError::Truncated("endian_tag".into()))?;
        if endian_tag != 0x12345678 {
            return Err(DexError::Parse("unsupported endianness".into()));
        }
        let link_size = read_u32(data, 44).unwrap_or(0);
        let link_off = read_u32(data, 48).unwrap_or(0);
        let map_off = read_u32(data, 52).ok_or(DexError::Truncated("map_off".into()))?;
        let string_ids_size = read_u32(data, 56).ok_or(DexError::Truncated("string_ids_size".into()))?;
        let string_ids_off = read_u32(data, 60).ok_or(DexError::Truncated("string_ids_off".into()))?;
        let type_ids_size = read_u32(data, 64).ok_or(DexError::Truncated("type_ids_size".into()))?;
        let type_ids_off = read_u32(data, 68).ok_or(DexError::Truncated("type_ids_off".into()))?;
        let proto_ids_size = read_u32(data, 72).ok_or(DexError::Truncated("proto_ids_size".into()))?;
        let proto_ids_off = read_u32(data, 76).ok_or(DexError::Truncated("proto_ids_off".into()))?;
        let field_ids_size = read_u32(data, 80).ok_or(DexError::Truncated("field_ids_size".into()))?;
        let field_ids_off = read_u32(data, 84).ok_or(DexError::Truncated("field_ids_off".into()))?;
        let method_ids_size = read_u32(data, 88).ok_or(DexError::Truncated("method_ids_size".into()))?;
        let method_ids_off = read_u32(data, 92).ok_or(DexError::Truncated("method_ids_off".into()))?;
        let class_defs_size = read_u32(data, 96).ok_or(DexError::Truncated("class_defs_size".into()))?;
        let class_defs_off = read_u32(data, 100).ok_or(DexError::Truncated("class_defs_off".into()))?;
        let data_size = read_u32(data, 104).unwrap_or(0);
        let data_off = read_u32(data, 108).unwrap_or(0);

        Ok(Self {
            file_size,
            header_size,
            endian_tag,
            link_size,
            link_off,
            map_off,
            string_ids_size,
            string_ids_off,
            type_ids_size,
            type_ids_off,
            proto_ids_size,
            proto_ids_off,
            field_ids_size,
            field_ids_off,
            method_ids_size,
            method_ids_off,
            class_defs_size,
            class_defs_off,
            data_size,
            data_off,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid DEX: magic + header (0x70 bytes) + map_list. No classes.
    fn minimal_dex_bytes() -> Vec<u8> {
        let mut data = vec![0u8; 0x80];
        data[0..4].copy_from_slice(&[0x64, 0x65, 0x78, 0x0a]);
        data[4..8].copy_from_slice(b"035\0");
        data[32..36].copy_from_slice(&(0x80u32).to_le_bytes());
        data[36..40].copy_from_slice(&(0x70u32).to_le_bytes());
        data[40..44].copy_from_slice(&(0x1234_5678u32).to_le_bytes());
        data[52..56].copy_from_slice(&(0x70u32).to_le_bytes());
        for i in (56..112).step_by(4) {
            data[i..i + 4].copy_from_slice(&0u32.to_le_bytes());
        }
        data[0x70..0x74].copy_from_slice(&(1u32).to_le_bytes());
        data[0x74..0x78].copy_from_slice(&(0u32).to_le_bytes());
        data[0x78..0x7c].copy_from_slice(&(0x70u32).to_le_bytes());
        data[0x7c..0x80].copy_from_slice(&(0u32).to_le_bytes());
        data
    }

    #[test]
    fn header_truncated() {
        let short = vec![0u8; 0x40];
        assert!(DexHeader::parse(&short).is_err());
    }

    #[test]
    fn header_invalid_magic() {
        let mut data = minimal_dex_bytes();
        data[0] = b'x';
        assert!(DexHeader::parse(&data).is_err());
    }

    #[test]
    fn header_invalid_endian() {
        let mut data = minimal_dex_bytes();
        data[40..44].copy_from_slice(&(0x8765_4321u32).to_le_bytes());
        assert!(DexHeader::parse(&data).is_err());
    }

    #[test]
    fn header_parse_minimal() {
        let data = minimal_dex_bytes();
        let h = DexHeader::parse(&data).unwrap();
        assert_eq!(h.file_size, 0x80);
        assert_eq!(h.header_size, 0x70);
        assert_eq!(h.endian_tag, 0x1234_5678);
        assert_eq!(h.map_off, 0x70);
        assert_eq!(h.string_ids_size, 0);
        assert_eq!(h.class_defs_size, 0);
    }
}
