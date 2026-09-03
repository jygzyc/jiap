//! F21h: op AA BBBB — vAA and 16-bit constant with special shift (4 bytes).
//! Opcodes: const/high16 (0x15), const-wide/high16 (0x19).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i16(v: i16) -> [u8; 2] {
    (v as u16).to_le_bytes()
}

#[test]
fn f21h_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F21h {
            continue;
        }
        for (aa, bbbb) in [(0u8, 0i16), (1, 1), (255, -1), (0, 0x7fff), (0, -0x8000)] {
            let mut bytecode = vec![op, aa, 0, 0];
            bytecode[2..4].copy_from_slice(&le_i16(bbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins.operands().starts_with(&alloc::format!("v{}, ", aa)));
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f21h_const_high16_values() {
    // const/high16 v0, 0x12340000 (BBBB=0x1234 << 16)
    let bytecode = [0x15u8, 0x00, 0x34, 0x12];
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const/high16");
    assert!(ins.operands().contains("v0"));
}
