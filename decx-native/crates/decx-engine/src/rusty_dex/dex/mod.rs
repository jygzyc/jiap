// decx-engine rusty_dex module: DEX parser (decx-native).
// Adapted from rusty-rs/rusty-dex 0.2.0 (Apache-2.0): https://github.com/rusty-rs/rusty-dex
// Baseline: the copy bundled in the asLody/dexdec v1.0.2 workspace.
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
pub mod access_flags;
pub mod classes;
pub mod code_item;
pub mod debug_info;
pub mod declarations;
pub mod encoded_value;
pub mod fields;
pub mod file;
pub mod header;
pub mod instructions;
pub mod methods;
pub mod opcodes;
pub mod protos;
pub mod reader;
pub mod references;
pub mod strings;
pub mod types;
