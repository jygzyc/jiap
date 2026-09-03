//! Load / merge taint configuration (models + rules).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::models::{PropagationModel, Rule, SanitizerModel, SinkModel, SourceModel};
use crate::error::{DexDecompilerError, Result};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaintConfig {
    #[serde(default)]
    pub sources: Vec<SourceModel>,
    #[serde(default)]
    pub sinks: Vec<SinkModel>,
    #[serde(default)]
    pub propagations: Vec<PropagationModel>,
    #[serde(default)]
    pub sanitizers: Vec<SanitizerModel>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl TaintConfig {
    pub fn from_json_str(s: &str) -> Result<Self> {
        serde_json::from_str(s)
            .map_err(|e| DexDecompilerError::Decompilation(format!("taint config: {e}")))
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| DexDecompilerError::Decompilation(format!("read taint config: {e}")))?;
        Self::from_json_str(&text)
    }

    /// Merge another config (append models/rules).
    pub fn merge(&mut self, other: TaintConfig) {
        self.sources.extend(other.sources);
        self.sinks.extend(other.sinks);
        self.propagations.extend(other.propagations);
        self.sanitizers.extend(other.sanitizers);
        self.rules.extend(other.rules);
    }

    pub fn find_source(&self, method_ref: &str) -> Option<&SourceModel> {
        self.sources
            .iter()
            .find(|s| super::models::pattern_matches(method_ref, &s.patterns))
    }

    /// All matching source models. A callback may seed more than one argument/kind.
    pub fn find_sources(&self, method_ref: &str) -> Vec<&SourceModel> {
        self.sources
            .iter()
            .filter(|s| super::models::pattern_matches(method_ref, &s.patterns))
            .collect()
    }

    pub fn find_sink(&self, method_ref: &str) -> Option<&SinkModel> {
        self.sinks
            .iter()
            .find(|s| super::models::pattern_matches(method_ref, &s.patterns))
    }

    pub fn find_propagations(&self, method_ref: &str) -> Vec<&PropagationModel> {
        self.propagations
            .iter()
            .filter(|p| super::models::pattern_matches(method_ref, &p.patterns))
            .collect()
    }

    pub fn find_sanitizer(&self, method_ref: &str) -> Option<&SanitizerModel> {
        self.sanitizers
            .iter()
            .find(|s| super::models::pattern_matches(method_ref, &s.patterns))
    }

    pub fn matching_rules(&self, source_kind: &str, sink_kind: &str) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| {
                r.sources.iter().any(|s| s == source_kind || s == "*")
                    && r.sinks.iter().any(|s| s == sink_kind || s == "*")
            })
            .collect()
    }
}
