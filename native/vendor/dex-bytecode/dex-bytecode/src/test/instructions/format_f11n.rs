//! F11n: B|A|op — 4-bit register A, 4-bit signed literal B (2 bytes). Opcodes: const/4 (0x12).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

#[test]
fn f11n_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F11n {
            continue;
        }
        // A in 0..16, B is sign-extended 4-bit: -8..7
        for a in 0u8..=15u8 {
            for b_nibble in 0u8..=15u8 {
                let b_signed = (b_nibble as i8) << 4 >> 4; // sign-extend 4-bit
                let byte1 = (b_nibble << 4) | a;
                let bytecode = [op, byte1];
                let ins = decode_one(&bytecode[..], 0).unwrap();
                assert_eq!(ins.opcode(), op);
                assert_eq!(ins.mnemonic(), entry.mnemonic);
                let expected = alloc::format!("v{}, {}", a, b_signed);
                assert_eq!(ins.operands(), expected);
                assert_eq!(ins.length(), 2);
            }
        }
    }
}

#[test]
fn f11n_const4_min_max() {
    // v0, -8 (byte1: A=0, B=-8 -> nibble 8) and v15, 7 (byte1: A=15, B=7 -> (7<<4)|15 = 0x4f)
    let ins_min = decode_one(&[0x12u8, 0x80], 0).unwrap(); // A=0, B=-8
    assert_eq!(ins_min.mnemonic(), "const/4");
    assert_eq!(ins_min.operands(), "v0, -8");

    let ins_max = decode_one(&[0x12u8, 0x7f], 0).unwrap(); // A=15, B=7 -> (7<<4)|15 = 0x7f
    assert_eq!(ins_max.mnemonic(), "const/4");
    assert_eq!(ins_max.operands(), "v15, 7");
}
