//! F22x: op AA BBBB — vAA and vBBBB (16-bit register) (4 bytes).
//! Opcodes: move/from16, move-wide/from16, move-object/from16 (0x02, 0x05, 0x08).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn f22x_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F22x {
            continue;
        }
        for (aa, bbbb) in [(0u8, 0u16), (1, 1), (255, 0xffff)] {
            let mut bytecode = vec![op, aa, 0, 0];
            bytecode[2..4].copy_from_slice(&le_u16(bbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.operands(), alloc::format!("v{}, v{}", aa, bbbb));
            assert_eq!(ins.length(), 4);
        }
    }
}
