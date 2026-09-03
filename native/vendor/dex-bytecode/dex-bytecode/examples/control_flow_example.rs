//! Example: control-flow — branch targets, labels, basic blocks.
//!
//! Run with: `cargo run -p dex-bytecode --example control_flow_example`

use dex_bytecode::{basic_blocks, branch_targets, collect_branch_targets, decode_all};

fn main() {
    // Small method: goto +3; nop; nop; return-void (branch target at offset 6)
    let bytecode: &[u8] = &[
        0x28, 0x03, 0x00, 0x00, // goto +3 (target = 6); nop
        0x00, 0x00, // nop
        0x00, 0x00, // nop
        0x0e, 0x00, // return-void
    ];

    let instructions = decode_all(bytecode, 0).unwrap();
    println!("Instructions:");
    for ins in &instructions {
        println!(
            "  {:08x}  {} {}",
            ins.offset,
            ins.mnemonic(),
            ins.operands()
        );
    }

    let targets = collect_branch_targets(&instructions, bytecode, 0);
    println!("\nBranch targets (for labels): {:?}", targets);

    println!("\nBranch targets per instruction:");
    for ins in &instructions {
        let off = ins.offset as usize;
        let t = branch_targets(bytecode, off);
        if !t.is_empty() {
            println!("  {:08x}  {} -> {:?}", off, ins.mnemonic(), t);
        }
    }

    let blocks = basic_blocks(&instructions, bytecode, 0);
    println!("\nBasic blocks ({}):", blocks.len());
    for (i, bb) in blocks.iter().enumerate() {
        println!(
            "  block {}: 0x{:08x}..0x{:08x}  successors: {:?}",
            i, bb.start_offset, bb.end_offset, bb.successors
        );
    }
}
