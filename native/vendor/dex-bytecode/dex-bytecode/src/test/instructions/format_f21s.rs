//! F21s: op AA BBBB — vAA and 16-bit signed constant (4 bytes).
//! Opcodes: const/16 (0x13), const-wide/16 (0x16).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i16(v: i16) -> [u8; 2] {
    (v as u16).to_le_bytes()
}

#[test]
fn f21s_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F21s {
            continue;
        }
        for (aa, bbbb) in [(0u8, 0i16), (1, 1), (255, -1), (0, 0x7fff), (0, -0x8000)] {
            let mut bytecode = vec![op, aa, 0, 0];
            bytecode[2..4].copy_from_slice(&le_i16(bbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.operands(), alloc::format!("v{}, {}", aa, bbbb));
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f21s_const16_min_max() {
    let ins = decode_one(&[0x13u8, 0x00, 0x00, 0x80], 0).unwrap(); // const/16 v0, -32768
    assert_eq!(ins.mnemonic(), "const/16");
    assert_eq!(ins.operands(), "v0, -32768");
}
