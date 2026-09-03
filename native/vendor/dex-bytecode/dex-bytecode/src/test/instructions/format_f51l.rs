//! F51l: op AA BBBBBBBBBBBBBBBB — vAA and 64-bit literal (10 bytes).
//! Opcodes: const-wide (0x18).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u64(v: u64) -> [u8; 8] {
    v.to_le_bytes()
}

#[test]
fn f51l_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F51l {
            continue;
        }
        for (aa, literal) in [(0u8, 0u64), (1, 1), (255, 0xffff_ffff_ffff_ffff)] {
            let mut bytecode = vec![op, aa, 0, 0, 0, 0, 0, 0, 0, 0];
            bytecode[2..10].copy_from_slice(&le_u64(literal));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins.operands().starts_with(&alloc::format!("v{}, ", aa)));
            assert_eq!(ins.length(), 10);
        }
    }
}

#[test]
fn f51l_const_wide_values() {
    let bytecode = [0x18u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const-wide");
    assert_eq!(ins.operands(), "v0, 0");
}
