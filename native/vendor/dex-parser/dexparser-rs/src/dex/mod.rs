//! DEX file format parser.
//! Parses header, string_ids, type_ids, proto_ids, field_ids, method_ids,
//! class_defs, class_data_item, code_item per Android DEX format.

mod header;
mod strings;
mod types;
mod protos;
mod fields;
mod methods;
mod class_def;
mod class_data;
mod code_item;
mod debug_info;
mod call_sites;

pub use header::{DexHeader, DEX_MAGIC, is_dex};
pub use strings::DexStrings;
pub use types::DexTypes;
pub use protos::{DexProtos, ProtoId};
pub use fields::{DexFields, FieldId};
pub use methods::{DexMethods, MethodId};
pub use class_def::{ClassDef, NO_INDEX};
pub use class_data::{ClassData, EncodedField, EncodedMethod};
pub use code_item::CodeItem;
pub use debug_info::{parse_debug_info, DebugInfo};
pub use call_sites::{CallSiteInfo, CallSiteValue, DexCallSites, MethodHandleItem};

use crate::error::Result;
use std::sync::Arc;

/// Parsed DEX file: raw bytes + parsed indices.
///
/// Uses [`Arc`] so the same parse can be shared across threads (parallel
/// decompile / taint / detectors).
#[derive(Clone)]
pub struct DexFile {
    pub data: Arc<[u8]>,
    pub header: DexHeader,
    pub strings: DexStrings,
    pub types: DexTypes,
    pub protos: DexProtos,
    pub fields: DexFields,
    pub methods: DexMethods,
    /// DEX 038+ call sites / method handles (may be empty).
    pub call_sites: DexCallSites,
}

impl DexFile {
    /// Parse a DEX file from raw bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let data: Arc<[u8]> = Arc::from(data.to_vec().into_boxed_slice());
        let header = DexHeader::parse(&data)?;
        let strings = DexStrings::parse(&data, &header)?;
        let types = DexTypes::parse(&data, &header)?;
        let protos = DexProtos::parse(&data, &header)?;
        let fields = DexFields::parse(&data, &header)?;
        let methods = DexMethods::parse(&data, &header)?;
        let call_sites = DexCallSites::parse(&data, &header)?;
        Ok(Self {
            data: Arc::clone(&data),
            header,
            strings,
            types,
            protos,
            fields,
            methods,
            call_sites,
        })
    }

    /// Resolve `invoke-custom` call_site_id to structured info (if present).
    pub fn get_call_site(&self, idx: u32) -> Result<CallSiteInfo> {
        self.call_sites
            .get_call_site(&*self.data, &|s| self.get_string(s), idx)
    }

    /// Get string by string_id index.
    pub fn get_string(&self, idx: u32) -> Result<String> {
        self.strings.get(&*self.data, idx)
    }

    /// Get type descriptor by type_id index (e.g. "I", "Ljava/lang/String;").
    pub fn get_type(&self, idx: u32) -> Result<String> {
        self.types.get(&*self.data, &self.strings, idx)
    }

    /// Get class_def by index in class_defs list.
    pub fn get_class_def(&self, idx: u32) -> Result<ClassDef> {
        ClassDef::parse(&self.data, &self.header, idx)
    }

    /// Iterate over all class definitions.
    pub fn class_defs(&self) -> impl Iterator<Item = Result<ClassDef>> + '_ {
        (0..self.header.class_defs_size).map(move |i| self.get_class_def(i))
    }

    /// Get class_data for a class_def (static/instance fields, direct/virtual methods).
    pub fn get_class_data(&self, class_def: &ClassDef) -> Result<Option<ClassData>> {
        ClassData::parse(&self.data, class_def)
    }

    /// Get code_item at file offset (from encoded_method.code_off).
    pub fn get_code_item(&self, code_off: u32) -> Result<CodeItem> {
        CodeItem::parse(&self.data, code_off)
    }

    /// Parse `debug_info_item` at the given file offset (`0` → empty).
    pub fn get_debug_info(&self, debug_info_off: u32) -> Result<DebugInfo> {
        if debug_info_off == 0 {
            return Ok(DebugInfo::default());
        }
        parse_debug_info(&self.data, debug_info_off, &|idx| self.get_string(idx))
    }

    /// Convenience: debug info for a code_item (empty if `debug_info_off == 0`).
    pub fn debug_info_for_code(&self, code: &CodeItem) -> Result<DebugInfo> {
        self.get_debug_info(code.debug_info_off)
    }

    /// Get method info: class type, name, proto (return + params).
    pub fn get_method_info(&self, method_idx: u32) -> Result<MethodInfo> {
        let method_id = self.methods.get(method_idx)?;
        let class_name = self.get_type(method_id.class_idx as u32)?;
        let name = self.get_string(method_id.name_idx)?;
        let (return_type, params) = self.protos.get_proto(
            &*self.data,
            &self.types,
            &self.strings,
            method_id.proto_idx as u32,
        )?;
        Ok(MethodInfo {
            class: class_name,
            name,
            return_type,
            params,
        })
    }

    /// Get field info: class, name, type.
    pub fn get_field_info(&self, field_idx: u32) -> Result<FieldInfo> {
        let field_id = self.fields.get(field_idx)?;
        let class_name = self.get_type(field_id.class_idx as u32)?;
        let name = self.get_string(field_id.name_idx)?;
        let typ = self.get_type(field_id.type_idx as u32)?;
        Ok(FieldInfo {
            class: class_name,
            name,
            typ,
        })
    }
}

/// Method identity: class, name, return type, parameter types.
#[derive(Clone, Debug)]
pub struct MethodInfo {
    pub class: String,
    pub name: String,
    pub return_type: String,
    pub params: Vec<String>,
}

/// Field identity: class, name, type.
#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub class: String,
    pub name: String,
    pub typ: String,
}

/// Helper that provides high-level iterators over classes, methods, and fields
/// (similar to the Python DEXHelper).
#[derive(Clone)]
pub struct DexHelper {
    dex: Arc<DexFile>,
}

impl DexHelper {
    /// Create a helper from a parsed DEX file.
    pub fn from_dex(dex: &DexFile) -> Self {
        Self {
            dex: Arc::new(dex.clone()),
        }
    }

    /// Reference to the underlying DEX file.
    pub fn dex(&self) -> &DexFile {
        &self.dex
    }

    /// Iterate over class names (class descriptor + superclass descriptor).
    pub fn classes(&self) -> impl Iterator<Item = Result<ClassInfo>> + '_ {
        let dex = self.dex.clone();
        (0..dex.header.class_defs_size).map(move |idx| {
            let class_def = dex.get_class_def(idx)?;
            let name = dex.get_type(class_def.class_idx)?;
            let superclass_name = if class_def.superclass_idx != NO_INDEX {
                dex.get_type(class_def.superclass_idx)?
            } else {
                String::new()
            };
            Ok(ClassInfo {
                name,
                superclass_name,
                class_def,
            })
        })
    }

    /// Iterate over all methods (direct then virtual) with resolved names and protos.
    pub fn methods(&self) -> impl Iterator<Item = Result<MethodInfoItem>> + '_ {
        let dex = self.dex.clone();
        MethodsIter {
            dex,
            class_idx: 0,
            in_direct: true,
            method_idx_in_class: 0,
            prev_method_idx: 0,
            done: false,
        }
    }

    /// Iterate over all fields (static then instance).
    pub fn fields(&self) -> impl Iterator<Item = Result<FieldInfoItem>> + '_ {
        let dex = self.dex.clone();
        FieldsIter {
            dex,
            class_idx: 0,
            in_static: true,
            field_idx_in_class: 0,
            prev_field_idx: 0,
            done: false,
        }
    }

    /// Iterate over all strings in the DEX.
    pub fn strings(&self) -> impl Iterator<Item = Result<String>> + '_ {
        let dex = self.dex.clone();
        (0..dex.header.string_ids_size).map(move |idx| dex.get_string(idx))
    }
}

/// Class info with name and superclass name.
#[derive(Clone, Debug)]
pub struct ClassInfo {
    pub name: String,
    pub superclass_name: String,
    pub class_def: ClassDef,
}

/// Method with resolved info and optional code.
#[derive(Clone, Debug)]
pub struct MethodInfoItem {
    pub info: MethodInfo,
    pub method_type: MethodType,
    pub method_idx: u32,
    pub code_off: u32,
    pub code_item: Option<CodeItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodType {
    Direct,
    Virtual,
}

/// Field with resolved info.
#[derive(Clone, Debug)]
pub struct FieldInfoItem {
    pub info: FieldInfo,
    pub field_kind: FieldKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Static,
    Instance,
}

struct MethodsIter {
    dex: Arc<DexFile>,
    class_idx: u32,
    in_direct: bool,
    method_idx_in_class: u32,
    prev_method_idx: u32,
    done: bool,
}

impl Iterator for MethodsIter {
    type Item = Result<MethodInfoItem>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            let class_def = match self.dex.get_class_def(self.class_idx) {
                Ok(c) => c,
                Err(e) => return Some(Err(e)),
            };
            let class_data = match self.dex.get_class_data(&class_def) {
                Ok(Some(cd)) => cd,
                Ok(None) => {
                    self.class_idx += 1;
                    self.in_direct = true;
                    self.method_idx_in_class = 0;
                    self.prev_method_idx = 0;
                    if self.class_idx >= self.dex.header.class_defs_size {
                        self.done = true;
                        return None;
                    }
                    continue;
                }
                Err(e) => return Some(Err(e)),
            };

            let (methods, method_type) = if self.in_direct {
                (&class_data.direct_methods, MethodType::Direct)
            } else {
                (&class_data.virtual_methods, MethodType::Virtual)
            };

            if (self.method_idx_in_class as usize) >= methods.len() {
                if self.in_direct {
                    self.in_direct = false;
                    self.method_idx_in_class = 0;
                    self.prev_method_idx = 0;
                } else {
                    self.class_idx += 1;
                    self.in_direct = true;
                    self.method_idx_in_class = 0;
                    self.prev_method_idx = 0;
                    if self.class_idx >= self.dex.header.class_defs_size {
                        self.done = true;
                        return None;
                    }
                }
                continue;
            }

            let enc = &methods[self.method_idx_in_class as usize];
            let method_idx = enc.method_idx;
            self.method_idx_in_class += 1;
            self.prev_method_idx = method_idx;

            let info = match self.dex.get_method_info(method_idx) {
                Ok(i) => i,
                Err(e) => return Some(Err(e)),
            };
            let code_item = if enc.code_off != 0 {
                self.dex.get_code_item(enc.code_off).ok()
            } else {
                None
            };
            return Some(Ok(MethodInfoItem {
                info,
                method_type,
                method_idx,
                code_off: enc.code_off,
                code_item,
            }));
        }
    }
}

struct FieldsIter {
    dex: Arc<DexFile>,
    class_idx: u32,
    in_static: bool,
    field_idx_in_class: u32,
    prev_field_idx: u32,
    done: bool,
}

impl Iterator for FieldsIter {
    type Item = Result<FieldInfoItem>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            let class_def = match self.dex.get_class_def(self.class_idx) {
                Ok(c) => c,
                Err(e) => return Some(Err(e)),
            };
            let class_data = match self.dex.get_class_data(&class_def) {
                Ok(Some(cd)) => cd,
                Ok(None) => {
                    self.class_idx += 1;
                    self.in_static = true;
                    self.field_idx_in_class = 0;
                    self.prev_field_idx = 0;
                    if self.class_idx >= self.dex.header.class_defs_size {
                        self.done = true;
                        return None;
                    }
                    continue;
                }
                Err(e) => return Some(Err(e)),
            };

            let fields = if self.in_static {
                &class_data.static_fields
            } else {
                &class_data.instance_fields
            };
            let field_kind = if self.in_static {
                FieldKind::Static
            } else {
                FieldKind::Instance
            };

            if (self.field_idx_in_class as usize) >= fields.len() {
                if self.in_static {
                    self.in_static = false;
                    self.field_idx_in_class = 0;
                    self.prev_field_idx = 0;
                } else {
                    self.class_idx += 1;
                    self.in_static = true;
                    self.field_idx_in_class = 0;
                    self.prev_field_idx = 0;
                    if self.class_idx >= self.dex.header.class_defs_size {
                        self.done = true;
                        return None;
                    }
                }
                continue;
            }

            let enc = &fields[self.field_idx_in_class as usize];
            self.field_idx_in_class += 1;
            self.prev_field_idx = enc.field_idx;

            let info = match self.dex.get_field_info(enc.field_idx) {
                Ok(i) => i,
                Err(e) => return Some(Err(e)),
            };
            return Some(Ok(FieldInfoItem {
                info,
                field_kind,
            }));
        }
    }
}
