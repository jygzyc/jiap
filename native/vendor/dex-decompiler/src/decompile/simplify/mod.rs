//! Post-process decompiled method body to simplify invoke + move-result + return patterns.

mod cleanup;
mod conditions;
mod expr;
mod format;
mod loops;
mod pipeline;
mod repair;
mod string_switch;
mod try_catch;
mod util;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

pub use format::{normalize_java_indent, simplify_synchronized_blocks};
pub use pipeline::simplify_method_body;
pub use string_switch::{java_string_hash_code, restore_string_switch};
pub use try_catch::merge_duplicate_finally;
