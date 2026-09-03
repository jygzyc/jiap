//! F31t: op AA BBBBBBBB — vAA and 32-bit signed branch offset (6 bytes).
//! Opcodes: fill-array-data (0x26), packed-switch (0x2b), sparse-switch (0x2c).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i32(v: i32) -> [u8; 4] {
    (v as u32).to_le_bytes()
}

#[test]
fn f31t_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F31t {
            continue;
        }
        for (aa, bbbbbbbb) in [(0u8, 0i32), (1, 2), (255, -1)] {
            let mut bytecode = vec![op, aa, 0, 0, 0, 0];
            bytecode[2..6].copy_from_slice(&le_i32(bbbbbbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins.operands().starts_with(&alloc::format!("v{}, ", aa)));
            assert!(ins.operands().contains('h'));
            assert_eq!(ins.length(), 6);
        }
    }
}
