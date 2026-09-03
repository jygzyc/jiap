//! class_data_item: uleb128 sizes, then encoded_field and encoded_method.

use crate::error::{DexError, Result};
use crate::leb128::read_uleb128;
use super::ClassDef;

#[derive(Clone, Debug)]
pub struct EncodedField {
    pub field_idx: u32,
    pub access_flags: u32,
}

#[derive(Clone, Debug)]
pub struct EncodedMethod {
    pub method_idx: u32,
    pub access_flags: u32,
    pub code_off: u32,
}

#[derive(Clone, Debug)]
pub struct ClassData {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<EncodedMethod>,
    pub virtual_methods: Vec<EncodedMethod>,
}

impl ClassData {
    pub fn parse(data: &[u8], class_def: &ClassDef) -> Result<Option<Self>> {
        if class_def.class_data_off == 0 {
            return Ok(None);
        }
        let mut off = class_def.class_data_off as usize;
        if off >= data.len() {
            return Err(DexError::Truncated("class_data_off".into()));
        }

        let (static_fields_size, n) = read_uleb128(data, off).ok_or(DexError::Truncated("static_fields_size".into()))?;
        off += n;
        let (instance_fields_size, n) = read_uleb128(data, off).ok_or(DexError::Truncated("instance_fields_size".into()))?;
        off += n;
        let (direct_methods_size, n) = read_uleb128(data, off).ok_or(DexError::Truncated("direct_methods_size".into()))?;
        off += n;
        let (virtual_methods_size, n) = read_uleb128(data, off).ok_or(DexError::Truncated("virtual_methods_size".into()))?;
        off += n;

        let read_fields = |count: u32, data: &[u8], off: &mut usize| -> Result<Vec<EncodedField>> {
            let mut prev_idx = 0u32;
            let mut out = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let (idx_diff, n) = read_uleb128(data, *off).ok_or(DexError::Truncated("field_idx_diff".into()))?;
                *off += n;
                prev_idx = prev_idx.wrapping_add(idx_diff);
                let (access_flags, n) = read_uleb128(data, *off).ok_or(DexError::Truncated("field access_flags".into()))?;
                *off += n;
                out.push(EncodedField { field_idx: prev_idx, access_flags });
            }
            Ok(out)
        };

        let static_fields = read_fields(static_fields_size, data, &mut off)?;
        let instance_fields = read_fields(instance_fields_size, data, &mut off)?;

        let read_methods = |count: u32, data: &[u8], off: &mut usize| -> Result<Vec<EncodedMethod>> {
            let mut prev_idx = 0u32;
            let mut out = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let (idx_diff, n) = read_uleb128(data, *off).ok_or(DexError::Truncated("method_idx_diff".into()))?;
                *off += n;
                prev_idx = prev_idx.wrapping_add(idx_diff);
                let (access_flags, n) = read_uleb128(data, *off).ok_or(DexError::Truncated("method access_flags".into()))?;
                *off += n;
                let (code_off, n) = read_uleb128(data, *off).ok_or(DexError::Truncated("method code_off".into()))?;
                *off += n;
                out.push(EncodedMethod { method_idx: prev_idx, access_flags, code_off });
            }
            Ok(out)
        };

        let direct_methods = read_methods(direct_methods_size, data, &mut off)?;
        let virtual_methods = read_methods(virtual_methods_size, data, &mut off)?;

        Ok(Some(ClassData {
            static_fields,
            instance_fields,
            direct_methods,
            virtual_methods,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::ClassDef;

    #[test]
    fn class_data_none_when_offset_zero() {
        let class_def = ClassDef {
            class_idx: 0,
            access_flags: 0,
            superclass_idx: 0,
            interfaces_off: 0,
            source_file_idx: 0,
            annotations_off: 0,
            class_data_off: 0,
            static_values_off: 0,
        };
        let data = vec![0u8; 0];
        assert!(matches!(ClassData::parse(&data, &class_def).unwrap(), None));
    }

    #[test]
    fn class_data_empty_sections() {
        // At offset 1: four uleb128 zeros (static/instance/direct/virtual sizes)
        let data = vec![0u8, 0u8, 0u8, 0u8, 0u8]; // index 1..5 are the four zeros
        let class_def = ClassDef {
            class_idx: 0,
            access_flags: 0,
            superclass_idx: 0,
            interfaces_off: 0,
            source_file_idx: 0,
            annotations_off: 0,
            class_data_off: 1,
            static_values_off: 0,
        };
        let cd = ClassData::parse(&data, &class_def).unwrap();
        let cd = cd.expect("class_data_off != 0");
        assert!(cd.static_fields.is_empty());
        assert!(cd.instance_fields.is_empty());
        assert!(cd.direct_methods.is_empty());
        assert!(cd.virtual_methods.is_empty());
    }
}
