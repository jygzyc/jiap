//! F31i: op AA BBBBBBBB — vAA and 32-bit immediate (6 bytes).
//! Opcodes: const (0x14), const-wide/32 (0x17).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i32(v: i32) -> [u8; 4] {
    (v as u32).to_le_bytes()
}

#[test]
fn f31i_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F31i {
            continue;
        }
        for (aa, bbbbbbbb) in [(0u8, 0i32), (1, 1), (255, -1), (0, 0x7fff_ffff)] {
            let mut bytecode = vec![op, aa, 0, 0, 0, 0];
            bytecode[2..6].copy_from_slice(&le_i32(bbbbbbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.operands(), alloc::format!("v{}, {}", aa, bbbbbbbb));
            assert_eq!(ins.length(), 6);
        }
    }
}
