//! F45cc: A|G BBBB C|D|E|F HHHH — 0–5 regs, method ref, proto ref (8 bytes).
//! Opcodes: invoke-polymorphic (0xfa).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn f45cc_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F45cc {
            continue;
        }
        let mut bytecode = vec![op, 0, 0, 0, 0, 0, 0, 0];
        bytecode[2..4].copy_from_slice(&le_u16(0));
        bytecode[6..8].copy_from_slice(&le_u16(0));
        let ins = decode_one(&bytecode[..], 0).unwrap();
        assert_eq!(ins.opcode(), op);
        assert_eq!(ins.mnemonic(), entry.mnemonic);
        assert_eq!(ins.length(), 8);
    }
}

#[test]
fn f45cc_invoke_polymorphic() {
    let bytecode = [0xfau8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00];
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "invoke-polymorphic");
    assert!(ins.operands().contains("proto@1"));
}
