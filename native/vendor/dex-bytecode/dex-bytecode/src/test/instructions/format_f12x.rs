//! F12x: B|A|op — two 4-bit registers vA, vB (2 bytes). Many opcodes: move, move-wide,
//! move-object, array-length, neg-*, not-*, int-to-*, add-*/2addr, etc.

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

#[test]
fn f12x_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F12x {
            continue;
        }
        // A and B in 0..16. First 16-bit unit: op in low byte, (B<<4)|A in high byte (LE: [op, (B<<4)|A])
        for a in 0u8..=15u8 {
            for b in 0u8..=15u8 {
                let byte1 = (b << 4) | a;
                let bytecode = [op, byte1];
                let ins = decode_one(&bytecode[..], 0).unwrap();
                assert_eq!(ins.opcode(), op);
                assert_eq!(ins.mnemonic(), entry.mnemonic);
                assert_eq!(ins.operands(), alloc::format!("v{}, v{}", a, b));
                assert_eq!(ins.length(), 2);
            }
        }
    }
}

#[test]
fn f12x_move_min_max() {
    // move v0, v0 and move v15, v15
    let ins = decode_one(&[0x01u8, 0x00], 0).unwrap();
    assert_eq!(ins.mnemonic(), "move");
    assert_eq!(ins.operands(), "v0, v0");

    let ins2 = decode_one(&[0x01u8, 0xff], 0).unwrap();
    assert_eq!(ins2.mnemonic(), "move");
    assert_eq!(ins2.operands(), "v15, v15");
}
