//! F23x: op AA BB CC — vAA, vBB, vCC (4 bytes). Many opcodes: cmpl-float, cmpg-float,
//! aget*, aput*, add-int, sub-int, ..., rem-double.

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

#[test]
fn f23x_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F23x {
            continue;
        }
        for (aa, bb, cc) in [(0u8, 0u8, 0u8), (1, 2, 3), (255, 255, 255)] {
            let bytecode = [op, aa, bb, cc];
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.operands(), alloc::format!("v{}, v{}, v{}", aa, bb, cc));
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f23x_aget_add_int() {
    let ins = decode_one(&[0x44u8, 0x00, 0x01, 0x02], 0).unwrap();
    assert_eq!(ins.mnemonic(), "aget");
    assert_eq!(ins.operands(), "v0, v1, v2");

    let ins2 = decode_one(&[0x90u8, 0x01, 0x02, 0x03], 0).unwrap();
    assert_eq!(ins2.mnemonic(), "add-int");
    assert_eq!(ins2.operands(), "v1, v2, v3");
}
