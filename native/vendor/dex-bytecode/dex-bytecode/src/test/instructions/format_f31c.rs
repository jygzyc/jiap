//! F31c: op AA BBBBBBBB — vAA and 32-bit index (6 bytes). Opcodes: const-string/jumbo (0x1b).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

#[test]
fn f31c_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F31c {
            continue;
        }
        for (aa, bbbbbbbb) in [(0u8, 0u32), (1, 1), (255, 0xffff_ffff)] {
            let mut bytecode = vec![op, aa, 0, 0, 0, 0];
            bytecode[2..6].copy_from_slice(&le_u32(bbbbbbbb));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins.operands().starts_with(&alloc::format!("v{}, ", aa)));
            assert_eq!(ins.length(), 6);
        }
    }
}

#[test]
fn f31c_const_string_jumbo() {
    // BBBBBBBB = 65536 in LE -> bytes [0x00, 0x00, 0x01, 0x00]
    let bytecode = [0x1bu8, 0x00, 0x00, 0x00, 0x01, 0x00]; // const-string/jumbo v0, string@65536
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const-string/jumbo");
    assert_eq!(ins.operands(), "v0, string@65536");
}
