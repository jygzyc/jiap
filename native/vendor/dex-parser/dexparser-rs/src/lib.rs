//! Pure Rust DEX file format parser.
//!
//! Parses Android DEX files: header, string_ids, type_ids, proto_ids,
//! field_ids, method_ids, class_defs, class_data_item, code_item.
//!
//! # Example
//!
//! ```no_run
//! use dex_parser::{DexFile, DexHelper};
//!
//! let bytes = std::fs::read("classes.dex").unwrap();
//! let dex = DexFile::parse(&bytes).unwrap();
//! let helper = DexHelper::from_dex(&dex);
//!
//! for class in helper.classes() {
//!     let c = class.unwrap();
//!     println!("CLASS {}", c.name);
//! }
//! for method in helper.methods() {
//!     if let Ok(m) = method {
//!         println!("METHOD {} {}", m.info.class, m.info.name);
//!     }
//! }
//! ```

pub mod error;
pub mod leb128;
pub mod dex;

pub use error::{DexError, Result};
pub use dex::{
    CallSiteInfo, CallSiteValue, ClassData, ClassDef, ClassInfo, CodeItem, DebugInfo,
    DexCallSites, DexFile, DexHeader, DexHelper, DexStrings, DexTypes, DexProtos, DexFields,
    DexMethods, EncodedMethod, FieldId, FieldInfo, FieldInfoItem, FieldKind, MethodHandleItem,
    MethodId, MethodInfo, MethodInfoItem, MethodType, ProtoId, DEX_MAGIC, NO_INDEX, is_dex,
    parse_debug_info,
};

#[cfg(test)]
mod test_helpers {
    /// Minimal valid DEX with no classes (header + map only).
    pub fn minimal_dex_bytes() -> Vec<u8> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::minimal_dex_bytes;

    #[test]
    fn parse_minimal_dex() {
        let data = minimal_dex_bytes();
        let dex = DexFile::parse(&data).unwrap();
        assert_eq!(dex.header.file_size, 0x80);
        assert_eq!(dex.header.class_defs_size, 0);
        assert_eq!(dex.header.string_ids_size, 0);
    }

    #[test]
    fn dex_helper_classes_empty() {
        let data = minimal_dex_bytes();
        let dex = DexFile::parse(&data).unwrap();
        let helper = DexHelper::from_dex(&dex);
        let count: usize = helper.classes().filter_map(Result::ok).count();
        assert_eq!(count, 0);
    }

    #[test]
    fn dex_helper_strings_empty() {
        let data = minimal_dex_bytes();
        let dex = DexFile::parse(&data).unwrap();
        let helper = DexHelper::from_dex(&dex);
        let count: usize = helper.strings().filter_map(Result::ok).count();
        assert_eq!(count, 0);
    }

    #[test]
    fn dex_helper_methods_empty() {
        let data = minimal_dex_bytes();
        let dex = DexFile::parse(&data).unwrap();
        let helper = DexHelper::from_dex(&dex);
        let count: usize = helper.methods().filter_map(Result::ok).count();
        assert_eq!(count, 0);
    }

    #[test]
    fn dex_helper_fields_empty() {
        let data = minimal_dex_bytes();
        let dex = DexFile::parse(&data).unwrap();
        let helper = DexHelper::from_dex(&dex);
        let count: usize = helper.fields().filter_map(Result::ok).count();
        assert_eq!(count, 0);
    }

    #[test]
    fn parse_invalid_too_short() {
        let short = vec![0u8; 4];
        assert!(DexFile::parse(&short).is_err());
    }

    #[test]
    fn parse_invalid_magic() {
        let mut data = minimal_dex_bytes();
        data[0] = b'X';
        assert!(DexFile::parse(&data).is_err());
    }

    #[test]
    fn is_dex_detection() {
        let data = minimal_dex_bytes();
        assert!(is_dex(&data));
        assert!(is_dex(&data[..4]));
        assert!(!is_dex(&[]));
        assert!(!is_dex(&[0u8; 4]));
        assert!(!is_dex(b"dex ")); // space, not newline
    }
}
