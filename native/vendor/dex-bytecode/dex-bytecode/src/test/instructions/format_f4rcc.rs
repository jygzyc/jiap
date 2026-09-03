//! F4rcc: op AA BBBB CCCC HHHH — register range, method ref, proto ref (8 bytes).
//! Opcodes: invoke-polymorphic/range (0xfb).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn f4rcc_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F4rcc {
            continue;
        }
        let mut bytecode = vec![op, 1, 0, 0, 0, 0, 0, 0];
        bytecode[2..4].copy_from_slice(&le_u16(0));
        bytecode[4..6].copy_from_slice(&le_u16(0));
        bytecode[6..8].copy_from_slice(&le_u16(0));
        let ins = decode_one(&bytecode[..], 0).unwrap();
        assert_eq!(ins.opcode(), op);
        assert_eq!(ins.mnemonic(), entry.mnemonic);
        assert_eq!(ins.length(), 8);
    }
}

#[test]
fn f4rcc_invoke_polymorphic_range() {
    let bytecode = [0xfbu8, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "invoke-polymorphic/range");
    assert!(ins.operands().contains("v0"));
}
