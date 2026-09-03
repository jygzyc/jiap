//! F32x: op ØØØØ AAAA BBBB — vAAAA and vBBBB (6 bytes).
//! Opcodes: move/16 (0x03), move-wide/16 (0x06), move-object/16 (0x09).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn f32x_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F32x {
            continue;
        }
        for (aaaa, bbbb) in [(0u16, 0u16), (1, 1), (0xffff, 0xffff)] {
            let mut bytecode = vec![op, 0, 0, 0, 0, 0];
            bytecode[2..4].copy_from_slice(&le_u16(aaaa));
            bytecode[4..6].copy_from_slice(&le_u16(bbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.operands(), alloc::format!("v{}, v{}", aaaa, bbbb));
            assert_eq!(ins.length(), 6);
        }
    }
}
