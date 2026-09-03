//! F21c: op AA BBBB — vAA and 16-bit index (string/type/field/method) (4 bytes).
//! Opcodes: const-string, const-class, check-cast, new-instance, sget*, sput*, const-method-handle, const-method-type.

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn f21c_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F21c {
            continue;
        }
        for (aa, bbbb) in [(0u8, 0u16), (1, 1), (255, 0xffff)] {
            let mut bytecode = vec![op, aa, 0, 0];
            bytecode[2..4].copy_from_slice(&le_u16(bbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins.operands().starts_with(&alloc::format!("v{}, ", aa)));
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f21c_const_string_ref_format() {
    let bytecode = [0x1au8, 0x00, 0x0f, 0x00]; // const-string v0, string@15
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const-string");
    assert_eq!(ins.operands(), "v0, string@15");
}
