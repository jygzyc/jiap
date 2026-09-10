// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
//! Finite Kotlin syntax recovery proven over Semantic IR.

mod conditions;
mod expression;
mod loops;
pub(super) mod primitives;

use crate::analysis::SemanticTransform;
use crate::ir::{analysis::TypeHierarchy, SemanticMethod, SourceSemantics, SourceSyntaxSemantics};

pub struct SourceSyntaxRecovery<'a> {
    expressions: expression::ExpressionSyntax,
    loops: loops::KotlinLoopSyntax<'a>,
    conditions: conditions::PathConditionSyntax,
}

pub struct KotlinValueSyntax<'a> {
    expressions: expression::ExpressionSyntax,
    loops: loops::KotlinLoopSyntax<'a>,
    conditions: conditions::PathConditionSyntax,
}

impl<'a> KotlinValueSyntax<'a> {
    pub fn new(hierarchy: &'a dyn TypeHierarchy) -> Self {
        Self {
            expressions: expression::ExpressionSyntax::default(),
            loops: loops::KotlinLoopSyntax::new(hierarchy),
            conditions: conditions::PathConditionSyntax,
        }
    }

    pub fn apply(
        &mut self,
        method: &mut SemanticMethod<SourceSyntaxSemantics>,
    ) -> Result<bool, crate::ir::SemanticFoldError> {
        let (body, state) = method.parts_mut();
        let recovery = self.expressions.apply(body, state.types())?;
        let loops = self.loops.apply(body)?;
        Ok(recovery.changed || loops)
    }

    pub fn reduce_conditions(
        &mut self,
        method: &mut SemanticMethod<SourceSyntaxSemantics>,
    ) -> Result<bool, crate::ir::SemanticFoldError> {
        let (body, state) = method.parts_mut();
        self.conditions.apply(body, state.types())
    }
}

impl<'a> SourceSyntaxRecovery<'a> {
    pub fn new(hierarchy: &'a dyn TypeHierarchy) -> Self {
        Self {
            expressions: expression::ExpressionSyntax::default(),
            loops: loops::KotlinLoopSyntax::new(hierarchy),
            conditions: conditions::PathConditionSyntax,
        }
    }
}

impl SemanticTransform<SourceSemantics> for SourceSyntaxRecovery<'_> {
    type Output = SourceSyntaxSemantics;
    type Error = crate::ir::SemanticFoldError;

    fn transform(
        &mut self,
        mut method: SemanticMethod<SourceSemantics>,
    ) -> Result<SemanticMethod<Self::Output>, Self::Error> {
        let (body, state) = method.parts_mut();
        let expressions = self.expressions.apply(body, state.types())?;
        state
            .types_mut()
            .refine_booleans(expressions.boolean_variables);
        self.loops.apply(body)?;
        self.conditions.apply(body, state.types())?;
        Ok(method.into_source_syntax())
    }
}
