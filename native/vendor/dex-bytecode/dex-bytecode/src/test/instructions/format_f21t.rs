//! F21t: op AA BBBB — vAA and 16-bit signed branch offset (4 bytes).
//! Opcodes: if-eqz, if-nez, if-ltz, if-gez, if-gtz, if-lez (0x38..0x3d).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i16(v: i16) -> [u8; 2] {
    (v as u16).to_le_bytes()
}

#[test]
fn f21t_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F21t {
            continue;
        }
        for (aa, bbbb) in [(0u8, 0i16), (1, 2), (255, -1)] {
            let mut bytecode = vec![op, aa, 0, 0];
            bytecode[2..4].copy_from_slice(&le_i16(bbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins.operands().starts_with(&alloc::format!("v{}, ", aa)));
            assert!(ins.operands().contains('h'));
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f21t_if_eqz() {
    let bytecode = [0x38u8, 0x00, 0x02, 0x00]; // if-eqz v0, +2
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "if-eqz");
    assert!(ins.operands().starts_with("v0, "));
}
