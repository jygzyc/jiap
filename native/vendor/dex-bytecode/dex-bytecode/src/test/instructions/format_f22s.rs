//! F22s: B|A|op CCCC — vA, vB and 16-bit signed constant (4 bytes).
//! Opcodes: add-int/lit16, rsub-int, mul-int/lit16, ..., xor-int/lit16 (0xd0..0xd7).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i16(v: i16) -> [u8; 2] {
    (v as u16).to_le_bytes()
}

#[test]
fn f22s_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F22s {
            continue;
        }
        for (a, b, cccc) in [(0u8, 0u8, 0i16), (1, 2, 1), (15, 15, -0x8000i16)] {
            let byte1 = (b << 4) | a;
            let mut bytecode = vec![op, byte1, 0, 0];
            bytecode[2..4].copy_from_slice(&le_i16(cccc));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.operands(), alloc::format!("v{}, v{}, {}", a, b, cccc));
            assert_eq!(ins.length(), 4);
        }
    }
}
