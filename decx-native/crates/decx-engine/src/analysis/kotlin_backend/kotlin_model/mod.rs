// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
mod class;
mod declared_members;
mod default_mask_flow;
mod external_abi;
mod metadata_members;
pub(super) mod method;
mod nullability;
mod source_abi;

#[cfg(test)]
pub(super) use class::KotlinClassDeclaration;
pub(super) use class::{KotlinClassKind, KotlinClassModel, KotlinFieldDeclaration};
pub(super) use declared_members::KotlinDefaultArgumentLayout;
pub(super) use method::{KotlinMethodDeclaration, KotlinMethodModel, MethodBodyOptions};
pub(super) use source_abi::OuterInstanceField;
pub(crate) use source_abi::{FunctionObjectClass, KotlinSourceAbi};
