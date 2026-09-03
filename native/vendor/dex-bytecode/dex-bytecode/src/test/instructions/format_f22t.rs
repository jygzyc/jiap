//! F22t: B|A|op CCCC — vA, vB and 16-bit signed branch offset (4 bytes).
//! Opcodes: if-eq, if-ne, if-lt, if-ge, if-gt, if-le (0x32..0x37).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i16(v: i16) -> [u8; 2] {
    (v as u16).to_le_bytes()
}

#[test]
fn f22t_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F22t {
            continue;
        }
        for (a, b, cccc) in [(0u8, 0u8, 0i16), (1, 2, 2), (15, 15, -1)] {
            let byte1 = (b << 4) | a;
            let mut bytecode = vec![op, byte1, 0, 0];
            bytecode[2..4].copy_from_slice(&le_i16(cccc));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins
                .operands()
                .starts_with(&alloc::format!("v{}, v{}, ", a, b)));
            assert!(ins.operands().contains('h'));
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f22t_if_eq() {
    let bytecode = [0x32u8, 0x21, 0x01, 0x00]; // if-eq v1, v2, +1
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "if-eq");
    assert!(ins.operands().starts_with("v1, v2, "));
}
