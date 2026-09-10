// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
use super::MethodReference;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisLocation {
    Class(String),
    Method(MethodReference),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisDiagnostic {
    InvalidGenericSignature {
        location: AnalysisLocation,
        signature: String,
        offset: usize,
    },
    InconsistentGenericSignature {
        location: AnalysisLocation,
        signature: String,
        reason: String,
    },
}
