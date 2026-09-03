//! Lightweight JSON export of decompiled class inventory (P2-4).

use serde::Serialize;

#[derive(Serialize)]
pub struct DexJsonExport {
    pub classes: Vec<ClassJson>,
}

#[derive(Serialize)]
pub struct ClassJson {
    pub name: String,
    pub fields: Vec<FieldJson>,
    pub methods: Vec<MethodJson>,
}

#[derive(Serialize)]
pub struct FieldJson {
    pub name: String,
    #[serde(rename = "type")]
    pub typ: String,
    pub static_field: bool,
}

#[derive(Serialize)]
pub struct MethodJson {
    pub name: String,
    pub descriptor: String,
    pub access_flags: u32,
    /// Decompiled Java body (optional; empty when bodies are omitted).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub java: String,
}
