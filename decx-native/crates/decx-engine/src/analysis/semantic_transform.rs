// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
use crate::ir::{SemanticContext, SemanticMethod};

pub trait SemanticTransform<Input>
where
    Input: SemanticContext,
{
    type Output: SemanticContext;
    type Error: std::error::Error;

    fn transform(
        &mut self,
        method: SemanticMethod<Input>,
    ) -> Result<SemanticMethod<Self::Output>, Self::Error>;
}
