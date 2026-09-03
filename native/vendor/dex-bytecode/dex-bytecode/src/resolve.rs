//! Pluggable reference resolution for Dalvik bytecode.
//!
//! Implement [`ResolveRef`] to supply string/type/field/method names from an external
//! source (e.g. a DEX file parsed by another tool). The decoder will call your
//! implementation when formatting operands so that indices are shown as symbols.

use crate::instruction::RefKind;

/// Resolves constant-pool indices to display strings.
///
/// Implement this trait (or pass a closure adapter) so that the decoder can show
/// e.g. `Lpkg/Foo;->bar(I)V` instead of `method@5`. Return `None` to fall back
/// to the default `kind@index` format.
///
/// # Example (from another tool that parses DEX)
///
/// ```ignore
/// struct DexResolver { /* string_ids, type_ids, method_ids, ... */ }
///
/// impl ResolveRef for DexResolver {
///     fn resolve(&self, kind: RefKind, index: u32) -> Option<String> {
///         match kind {
///             RefKind::String => self.get_string(index),
///             RefKind::Type => self.get_type(index),
///             RefKind::Method => self.get_method(index),
///             RefKind::Field => self.get_field(index),
///             RefKind::MethodProto => self.get_proto(index),
///             _ => None,
///         }
///     }
/// }
///
/// let instructions = decode_all(bytecode, 0, Some(&resolver))?;
/// ```
pub trait ResolveRef {
    /// Return a display string for the given reference, or `None` to use default formatting.
    fn resolve(&self, kind: RefKind, index: u32) -> Option<String>;
}

/// Adapter so a closure can be used as a resolver without defining a type.
pub struct FnResolver<F>(pub F);

impl<F> ResolveRef for FnResolver<F>
where
    F: Fn(RefKind, u32) -> Option<String>,
{
    fn resolve(&self, kind: RefKind, index: u32) -> Option<String> {
        (self.0)(kind, index)
    }
}
