// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
//! Visualization module for dexdec
//!
//! This module provides visualization capabilities for various IR structures,
//! including CFG visualization in DOT format.

pub mod cfg_dot;

pub use cfg_dot::{method_to_dot, method_to_text};
