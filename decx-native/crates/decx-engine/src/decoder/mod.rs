// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// MODIFIED for decx-native: dependency shims as root modules, batch/caching
// APIs, streaming iterator signatures, rustc-compat fixes, extra tests.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
//! Instruction Decoder
//!
//! This module decodes Dalvik bytecode into IR instructions from the raw
//! `MethodCode` format.

pub mod method_decoder;

pub use method_decoder::{DecodeResult, MethodDecoder};
