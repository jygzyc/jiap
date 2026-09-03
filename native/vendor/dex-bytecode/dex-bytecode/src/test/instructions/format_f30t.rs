//! F30t: op ØØØØ AAAAAAAA — 32-bit signed branch offset in 16-bit units (6 bytes).
//! Opcodes: goto/32 (0x2a).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i32(v: i32) -> [u8; 4] {
    (v as u32).to_le_bytes()
}

#[test]
fn f30t_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F30t {
            continue;
        }
        for aaaaaaaa in [0i32, 1, -1, 0x7fff_ffff, -0x8000_0000] {
            let mut bytecode = vec![op, 0, 0, 0, 0, 0];
            bytecode[2..6].copy_from_slice(&le_i32(aaaaaaaa));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.length(), 6);
        }
    }
}
