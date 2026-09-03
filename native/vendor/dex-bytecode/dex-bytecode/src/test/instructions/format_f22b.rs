//! F22b: op AA BB CC — vAA, vBB, 8-bit signed CC (4 bytes).
//! Opcodes: add-int/lit8, rsub-int/lit8, mul-int/lit8, ..., ushr-int/lit8 (0xd8..0xe2).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

#[test]
fn f22b_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F22b {
            continue;
        }
        for (aa, bb, cc) in [(0u8, 0u8, 0i8), (1, 2, 3), (255, 255, -1i8)] {
            let bytecode = [op, aa, bb, cc as u8];
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.operands(), alloc::format!("v{}, v{}, {}", aa, bb, cc));
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f22b_add_int_lit8() {
    let ins = decode_one(&[0xd8u8, 0x00, 0x01, 0xff], 0).unwrap(); // add-int/lit8 v0, v1, -1
    assert_eq!(ins.mnemonic(), "add-int/lit8");
    assert_eq!(ins.operands(), "v0, v1, -1");
}
