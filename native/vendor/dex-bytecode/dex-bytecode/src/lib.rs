mod control_flow;
mod decoder;
mod error;
mod instruction;
mod opcodes;
mod patch;
mod resolve;

#[cfg(test)]
pub(crate) mod test;

extern crate alloc;

pub use control_flow::{
    basic_blocks, branch_target_offsets, branch_targets, cfg_edges, collect_branch_targets,
    exception_edges, explicit_successors, format_catch_line, is_unconditional_branch, BasicBlock,
    TryCatchEntry,
};
pub use decoder::{
    decode_all, decode_all_with_resolver, decode_one, decode_one_with_resolver, Decoder,
};
pub use error::DexError;
pub use instruction::{Instruction, RefKind};
pub use opcodes::{get_opcode_entry, Format, OpcodeEntry, PayloadKind};
pub use patch::{encode_goto, encode_nop, encode_return_void, patch_branch_target};
pub use resolve::{FnResolver, ResolveRef};
