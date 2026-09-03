//! Taint model / rule types (Mariana Trench–inspired JSON schema).

use serde::{Deserialize, Serialize};

/// Where taint enters or leaves a modeled method.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Port {
    /// Return value (after move-result).
    Return,
    /// `this` / first register of an instance invoke (argument index 0).
    This,
    /// Positional argument (0 = this for instance methods, or first static arg).
    Argument {
        #[serde(default)]
        index: u32,
    },
}

impl Default for Port {
    fn default() -> Self {
        Port::Return
    }
}

/// Optional access-path hint (MT-style); currently used as a breadcrumb only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessPath {
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceModel {
    /// Substrings matched against resolved invoke method refs (e.g. `getIntent`).
    pub patterns: Vec<String>,
    #[serde(default)]
    pub port: Port,
    /// Taint kind produced (e.g. `ActivityUserInput`).
    pub kind: String,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SinkModel {
    pub patterns: Vec<String>,
    #[serde(default)]
    pub port: Port,
    pub kind: String,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Taint-in / taint-out (propagation / passthrough).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PropagationModel {
    pub patterns: Vec<String>,
    pub from: Port,
    pub to: Port,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SanitizerModel {
    pub patterns: Vec<String>,
    /// Kind names to clear; `"*"` clears all kinds.
    #[serde(default)]
    pub kinds: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub code: u32,
    #[serde(default)]
    pub description: String,
    pub sources: Vec<String>,
    pub sinks: Vec<String>,
}

pub(crate) fn pattern_matches(method_ref: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| method_ref.contains(p.as_str()))
}
