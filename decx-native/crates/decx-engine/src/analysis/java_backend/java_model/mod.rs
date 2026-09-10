// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
mod class;
pub(super) mod method;
mod source_abi;

#[cfg(test)]
pub(super) use class::JavaClassDeclaration;
pub(super) use class::{JavaClassKind, JavaClassModel, JavaFieldDeclaration};
pub(super) use method::{JavaMethodDeclaration, JavaMethodModel, MethodBodyOptions};
pub(super) use source_abi::OuterInstanceField;
pub(crate) use source_abi::{FunctionObjectClass, JavaSourceAbi};
