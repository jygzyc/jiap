//! F10x: op — no operands (2 bytes). Opcodes: nop (0x00), return-void (0x0e).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

#[test]
fn f10x_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F10x {
            continue;
        }
        let bytecode = [op, 0u8];
        let ins = decode_one(&bytecode[..], 0).unwrap();
        assert_eq!(ins.opcode(), op);
        assert_eq!(ins.mnemonic(), entry.mnemonic);
        assert_eq!(ins.operands(), "");
        assert_eq!(ins.length(), 2);
    }
}

#[test]
fn f10x_nop() {
    let ins = decode_one(&[0x00u8, 0x00], 0).unwrap();
    assert_eq!(ins.mnemonic(), "nop");
    assert_eq!(ins.operands(), "");
}

#[test]
fn f10x_return_void() {
    let ins = decode_one(&[0x0eu8, 0x00], 0).unwrap();
    assert_eq!(ins.mnemonic(), "return-void");
    assert_eq!(ins.operands(), "");
}
