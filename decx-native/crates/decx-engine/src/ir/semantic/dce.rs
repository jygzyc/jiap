// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md
use super::{SemanticFoldError, SemanticFolder, SemanticNode, SemanticStatementKind};

/// Eliminates effect statements whose complete expression tree is proven
/// source-observable-effect free.
pub struct SemanticDeadCodeElimination {
    changed: bool,
}

impl SemanticDeadCodeElimination {
    pub fn apply(root: &mut SemanticNode) -> Result<bool, SemanticFoldError> {
        let body = std::mem::replace(root, SemanticNode::Empty);
        let mut elimination = Self { changed: false };
        *root = elimination.fold_node(body)?;
        Ok(elimination.changed)
    }
}

impl SemanticFolder for SemanticDeadCodeElimination {
    type Error = SemanticFoldError;

    fn finish_node(&mut self, mut node: SemanticNode) -> Result<SemanticNode, Self::Error> {
        if let SemanticNode::BasicBlock(block) = &mut node {
            block.statements.retain(|statement| {
                let removable = matches!(
                    &statement.kind,
                    SemanticStatementKind::Instruction(instruction)
                        if instruction.result.is_none()
                ) && statement.effects().is_pure();
                self.changed |= removable;
                !removable
            });
        }
        Ok(node)
    }
}
