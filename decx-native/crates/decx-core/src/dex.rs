//! DEX structural parser: header → string pool → type/proto/field/method ids → class defs → code items.

use crate::leb128;

pub struct ProtoId {
    pub shorty_idx: u32,
    pub return_type_idx: u32,
    pub parameters_off: u32,
}

pub struct FieldId {
    pub class_idx: u16,
    pub type_idx: u16,
    pub name_idx: u32,
}

pub struct MethodId {
    pub class_idx: u16,
    pub proto_idx: u16,
    pub name_idx: u32,
}

pub struct ClassDef {
    pub class_idx: u32,
    pub access_flags: u32,
    pub superclass_idx: u32,
    pub interfaces_off: u32,
    pub source_file_idx: u32,
    pub class_data_off: u32,
    pub static_values_off: u32,
}

#[derive(Default, Clone)]
pub struct EncodedField {
    pub field_idx: u32,
    pub access_flags: u32,
}

#[derive(Default, Clone)]
pub struct EncodedMethod {
    pub method_idx: u32,
    pub access_flags: u32,
    pub code_off: u32,
}

#[derive(Default, Clone)]
pub struct ClassData {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<EncodedMethod>,
    pub virtual_methods: Vec<EncodedMethod>,
}

pub struct CodeItem {
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    pub insns: Vec<u16>,
    pub tries_size: u16,
}

pub struct Dex {
    pub strings: Vec<String>,
    pub type_ids: Vec<u32>,
    pub proto_ids: Vec<ProtoId>,
    pub field_ids: Vec<FieldId>,
    pub method_ids: Vec<MethodId>,
    pub class_defs: Vec<ClassDef>,
    data: Vec<u8>,
}

fn u16le(d: &[u8], o: usize) -> Result<u16, String> {
    let b = d
        .get(o..o + 2)
        .ok_or_else(|| format!("dex: read u16 at {o:#x} out of bounds"))?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}
fn u32le(d: &[u8], o: usize) -> Result<u32, String> {
    let b = d
        .get(o..o + 4)
        .ok_or_else(|| format!("dex: read u32 at {o:#x} out of bounds"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// MUTF-8 (CESU-8 style) decode: merge surrogate pairs, lossy fallback.
fn mutf8_to_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = {
        let mut v = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            let u = if b < 0x80 {
                i += 1;
                b as u16
            } else if b & 0xe0 == 0xc0 {
                if i + 1 >= bytes.len() {
                    i += 1;
                    0xfffd
                } else {
                    i += 2;
                    (((b & 0x1f) as u16) << 6) | (bytes[i - 1] & 0x3f) as u16
                }
            } else if b & 0xf0 == 0xe0 {
                if i + 2 >= bytes.len() {
                    i += 1;
                    0xfffd
                } else {
                    i += 3;
                    (((b & 0x0f) as u16) << 12)
                        | (((bytes[i - 2] & 0x3f) as u16) << 6)
                        | (bytes[i - 1] & 0x3f) as u16
                }
            } else {
                i += 1;
                0xfffd
            };
            v.push(u);
        }
        v
    };
    // merge surrogate pairs
    let mut out = String::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let u = units[i];
        if (0xd800..0xdc00).contains(&u) && i + 1 < units.len() && (0xdc00..0xe000).contains(&units[i + 1]) {
            let hi = (u - 0xd800) as u32;
            let lo = (units[i + 1] - 0xdc00) as u32;
            let c = 0x10000 + (hi << 10) + lo;
            out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
            i += 2;
        } else {
            out.push(char::from_u32(u as u32).unwrap_or('\u{FFFD}'));
            i += 1;
        }
    }
    out
}

impl Dex {
    pub fn parse(data: Vec<u8>) -> Result<Dex, String> {
        if data.len() < 0x70 {
            return Err("dex: too small".into());
        }
        if &data[0..4] != b"dex\n" {
            return Err("dex: bad magic".into());
        }
        let string_ids_size = u32le(&data, 0x38)? as usize;
        let string_ids_off = u32le(&data, 0x3c)? as usize;
        let type_ids_size = u32le(&data, 0x40)? as usize;
        let type_ids_off = u32le(&data, 0x44)? as usize;
        let proto_ids_size = u32le(&data, 0x48)? as usize;
        let proto_ids_off = u32le(&data, 0x4c)? as usize;
        let field_ids_size = u32le(&data, 0x50)? as usize;
        let field_ids_off = u32le(&data, 0x54)? as usize;
        let method_ids_size = u32le(&data, 0x58)? as usize;
        let method_ids_off = u32le(&data, 0x5c)? as usize;
        let class_defs_size = u32le(&data, 0x60)? as usize;
        let class_defs_off = u32le(&data, 0x64)? as usize;

        // strings
        let mut strings = Vec::with_capacity(string_ids_size);
        for i in 0..string_ids_size {
            let ido = string_ids_off + i * 4;
            let off = u32le(&data, ido)? as usize;
            let mut p = off;
            let _utf16_len = leb128::read_uleb128(&data, &mut p)? as usize;
            let start = p;
            while p < data.len() && data[p] != 0 {
                p += 1;
            }
            strings.push(mutf8_to_string(&data[start..p]));
        }

        // type ids
        let mut type_ids = Vec::with_capacity(type_ids_size);
        for i in 0..type_ids_size {
            type_ids.push(u32le(&data, type_ids_off + i * 4)?);
        }

        // proto ids
        let mut proto_ids = Vec::with_capacity(proto_ids_size);
        for i in 0..proto_ids_size {
            let o = proto_ids_off + i * 12;
            proto_ids.push(ProtoId {
                shorty_idx: u32le(&data, o)?,
                return_type_idx: u32le(&data, o + 4)?,
                parameters_off: u32le(&data, o + 8)?,
            });
        }

        // field ids
        let mut field_ids = Vec::with_capacity(field_ids_size);
        for i in 0..field_ids_size {
            let o = field_ids_off + i * 8;
            field_ids.push(FieldId {
                class_idx: u16le(&data, o)?,
                type_idx: u16le(&data, o + 2)?,
                name_idx: u32le(&data, o + 4)?,
            });
        }

        // method ids
        let mut method_ids = Vec::with_capacity(method_ids_size);
        for i in 0..method_ids_size {
            let o = method_ids_off + i * 8;
            method_ids.push(MethodId {
                class_idx: u16le(&data, o)?,
                proto_idx: u16le(&data, o + 2)?,
                name_idx: u32le(&data, o + 4)?,
            });
        }

        // class defs
        let mut class_defs = Vec::with_capacity(class_defs_size);
        for i in 0..class_defs_size {
            let o = class_defs_off + i * 32;
            class_defs.push(ClassDef {
                class_idx: u32le(&data, o)?,
                access_flags: u32le(&data, o + 4)?,
                superclass_idx: u32le(&data, o + 8)?,
                interfaces_off: u32le(&data, o + 12)?,
                source_file_idx: u32le(&data, o + 16)?,
                class_data_off: u32le(&data, o + 24)?,
                static_values_off: u32le(&data, o + 28)?,
            });
        }

        Ok(Dex {
            strings,
            type_ids,
            proto_ids,
            field_ids,
            method_ids,
            class_defs,
            data,
        })
    }

    pub fn type_descriptor(&self, type_idx: u32) -> &str {
        match self
            .type_ids
            .get(type_idx as usize)
            .and_then(|si| self.strings.get(*si as usize))
        {
            Some(s) => s.as_str(),
            None => "",
        }
    }

    pub fn str(&self, idx: u32) -> &str {
        self.strings.get(idx as usize).map(|s| s.as_str()).unwrap_or("")
    }

    /// parameter type descriptors of a proto
    pub fn proto_params(&self, proto_idx: u16) -> Vec<&str> {
        let Some(p) = self.proto_ids.get(proto_idx as usize) else {
            return Vec::new();
        };
        if p.parameters_off == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let Ok(size32) = u32le(&self.data, p.parameters_off as usize) else {
            return out;
        };
        let size = size32 as usize;
        for i in 0..size {
            let Ok(ti) = u32le(&self.data, p.parameters_off as usize + 4 + i * 4) else {
                break;
            };
            out.push(self.type_descriptor(ti));
        }
        out
    }

    /// full dalvik signature "Lcls;->name(params)ret"
    pub fn method_full(&self, method_idx: u32) -> String {
        let Some(m) = self.method_ids.get(method_idx as usize) else {
            return format!("<method#{method_idx}>");
        };
        let class = self.type_descriptor(m.class_idx as u32);
        let name = self.str(m.name_idx);
        let ret = self
            .proto_ids
            .get(m.proto_idx as usize)
            .map(|p| self.type_descriptor(p.return_type_idx))
            .unwrap_or("V");
        let params = self.proto_params(m.proto_idx).join("");
        format!("{class}->{name}({params}){ret}")
    }

    pub fn method_class(&self, method_idx: u32) -> &str {
        self.method_ids
            .get(method_idx as usize)
            .map(|m| self.type_descriptor(m.class_idx as u32))
            .unwrap_or("")
    }

    pub fn method_name(&self, method_idx: u32) -> &str {
        self.method_ids
            .get(method_idx as usize)
            .map(|m| self.str(m.name_idx))
            .unwrap_or("")
    }

    pub fn field_full(&self, field_idx: u32) -> String {
        let Some(f) = self.field_ids.get(field_idx as usize) else {
            return format!("<field#{field_idx}>");
        };
        let class = self.type_descriptor(f.class_idx as u32);
        let ftype = self.type_descriptor(f.type_idx as u32);
        let name = self.str(f.name_idx);
        format!("{class}->{name}:{ftype}")
    }

    pub fn class_data(&self, def: &ClassDef) -> ClassData {
        let mut cd = ClassData::default();
        if def.class_data_off == 0 {
            return cd;
        }
        let Ok(mut p) = <usize as TryFrom<u32>>::try_from(def.class_data_off) else {
            return cd;
        };
        let rd = |p: &mut usize| -> u32 {
            leb128::read_uleb128(&self.data, p).unwrap_or(0) as u32
        };
        let static_n = rd(&mut p);
        let instance_n = rd(&mut p);
        let direct_n = rd(&mut p);
        let virtual_n = rd(&mut p);
        let mut read_fields = |p: &mut usize, n: u32, out: &mut Vec<EncodedField>| {
            let mut idx = 0u32;
            for _ in 0..n {
                idx = idx.wrapping_add(rd(p));
                out.push(EncodedField {
                    field_idx: idx,
                    access_flags: rd(p),
                });
            }
        };
        read_fields(&mut p, static_n, &mut cd.static_fields);
        read_fields(&mut p, instance_n, &mut cd.instance_fields);
        let mut read_methods = |p: &mut usize, n: u32, out: &mut Vec<EncodedMethod>| {
            let mut idx = 0u32;
            for _ in 0..n {
                idx = idx.wrapping_add(rd(p));
                out.push(EncodedMethod {
                    method_idx: idx,
                    access_flags: rd(p),
                    code_off: rd(p),
                });
            }
        };
        read_methods(&mut p, direct_n, &mut cd.direct_methods);
        read_methods(&mut p, virtual_n, &mut cd.virtual_methods);
        cd
    }

    /// interfaces (descriptors) of a class def
    pub fn interfaces(&self, def: &ClassDef) -> Vec<&str> {
        if def.interfaces_off == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let Ok(size32) = u32le(&self.data, def.interfaces_off as usize) else {
            return out;
        };
        let size = size32 as usize;
        for i in 0..size {
            let Ok(ti) = u32le(&self.data, def.interfaces_off as usize + 4 + i * 4) else {
                break;
            };
            out.push(self.type_descriptor(ti));
        }
        out
    }

    pub fn code_item(&self, code_off: u32) -> Option<CodeItem> {
        if code_off == 0 {
            return None;
        }
        let o = code_off as usize;
        let registers_size = u16le(&self.data, o).ok()?;
        let ins_size = u16le(&self.data, o + 2).ok()?;
        let outs_size = u16le(&self.data, o + 4).ok()?;
        let tries_size = u16le(&self.data, o + 6).ok()?;
        let insns_size = u32le(&self.data, o + 12).ok()? as usize;
        let mut insns = Vec::with_capacity(insns_size);
        for i in 0..insns_size {
            insns.push(u16le(&self.data, o + 16 + i * 2).ok()?);
        }
        Some(CodeItem {
            registers_size,
            ins_size,
            outs_size,
            tries_size,
            insns,
        })
    }
}
