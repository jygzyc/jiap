//! Issues and traces (SAPP-style presentation data).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceFrame {
    pub class_name: String,
    pub method_name: String,
    /// Instruction offset within the method (method-relative), if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    pub kind: String,
    pub description: String,
    /// Recovered dest URL or `extra:<key>` breadcrumb.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
    /// `field:Class.field` breadcrumb when the hop is heap-sensitive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Issue {
    pub rule_code: u32,
    pub rule_name: String,
    #[serde(default)]
    pub description: String,
    pub source_kind: String,
    pub sink_kind: String,
    /// Method where source and sink traces meet (trace root).
    pub callable: String,
    pub trace: Vec<TraceFrame>,
}
