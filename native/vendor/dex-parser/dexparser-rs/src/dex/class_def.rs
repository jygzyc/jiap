//! class_def_item: class_idx, access_flags, superclass_idx, interfaces_off, etc.

use crate::error::{DexError, Result};
use crate::leb128::read_u32;
use super::DexHeader;

/// NO_INDEX = 0xffffffff for superclass_idx / interfaces / source_file / etc.
pub const NO_INDEX: u32 = 0xffff_ffff;

#[derive(Clone, Debug)]
pub struct ClassDef {
    pub class_idx: u32,
    pub access_flags: u32,
    pub superclass_idx: u32,
    pub interfaces_off: u32,
    pub source_file_idx: u32,
    pub annotations_off: u32,
    pub class_data_off: u32,
    pub static_values_off: u32,
}

impl ClassDef {
    pub fn parse(data: &[u8], header: &DexHeader, idx: u32) -> Result<Self> {
        let off = (header.class_defs_off as usize) + (idx as usize) * 32;
        if data.len() < off + 32 {
            return Err(DexError::Truncated("class_def_item".into()));
        }
        let class_idx = read_u32(data, off).ok_or(DexError::Truncated("class_def class_idx".into()))?;
        let access_flags = read_u32(data, off + 4).ok_or(DexError::Truncated("class_def access_flags".into()))?;
        let superclass_idx = read_u32(data, off + 8).ok_or(DexError::Truncated("class_def superclass_idx".into()))?;
        let interfaces_off = read_u32(data, off + 12).ok_or(DexError::Truncated("class_def interfaces_off".into()))?;
        let source_file_idx = read_u32(data, off + 16).ok_or(DexError::Truncated("class_def source_file_idx".into()))?;
        let annotations_off = read_u32(data, off + 20).ok_or(DexError::Truncated("class_def annotations_off".into()))?;
        let class_data_off = read_u32(data, off + 24).ok_or(DexError::Truncated("class_def class_data_off".into()))?;
        let static_values_off = read_u32(data, off + 28).ok_or(DexError::Truncated("class_def static_values_off".into()))?;
        Ok(Self {
            class_idx,
            access_flags,
            superclass_idx,
            interfaces_off,
            source_file_idx,
            annotations_off,
            class_data_off,
            static_values_off,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex::DexHeader;

    fn minimal_header_with_class_defs() -> (Vec<u8>, DexHeader) {
        let mut data = vec![0u8; 0x70 + 32];
        data[0..4].copy_from_slice(&[0x64, 0x65, 0x78, 0x0a]);
        data[4..8].copy_from_slice(b"035\0");
        data[36..40].copy_from_slice(&(0x70u32).to_le_bytes());
        data[40..44].copy_from_slice(&(0x1234_5678u32).to_le_bytes());
        data[96..100].copy_from_slice(&(1u32).to_le_bytes()); // class_defs_size = 1
        data[100..104].copy_from_slice(&(0x70u32).to_le_bytes()); // class_defs_off = 0x70
        let header = DexHeader::parse(&data).unwrap();
        (data, header)
    }

    #[test]
    fn class_def_parse_no_index_superclass() {
        let (mut data, header) = minimal_header_with_class_defs();
        // class_def at 0x70: class_idx=0, access_flags=1, superclass_idx=NO_INDEX, ...
        data[0x70..0x74].copy_from_slice(&(0u32).to_le_bytes());
        data[0x74..0x78].copy_from_slice(&(1u32).to_le_bytes());
        data[0x78..0x7c].copy_from_slice(&(NO_INDEX).to_le_bytes());
        data[0x7c..0x90].copy_from_slice(&[0u8; 20]);
        let class_def = ClassDef::parse(&data, &header, 0).unwrap();
        assert_eq!(class_def.class_idx, 0);
        assert_eq!(class_def.superclass_idx, NO_INDEX);
    }

    #[test]
    fn class_def_parse_truncated() {
        let (data, header) = minimal_header_with_class_defs();
        let short = &data[0..0x80];
        assert!(ClassDef::parse(short, &header, 0).is_err());
    }
}
