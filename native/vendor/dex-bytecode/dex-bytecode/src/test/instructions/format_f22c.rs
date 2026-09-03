//! F22c: B|A|op CCCC — vA, vB and 16-bit index (type/field) (4 bytes).
//! Opcodes: instance-of, new-array, iget*, iput* (0x20, 0x23, 0x52..0x5f).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn f22c_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F22c {
            continue;
        }
        for (a, b, cccc) in [(0u8, 0u8, 0u16), (1, 2, 1), (15, 15, 0xffff)] {
            let byte1 = (b << 4) | a;
            let mut bytecode = vec![op, byte1, 0, 0];
            bytecode[2..4].copy_from_slice(&le_u16(cccc));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins
                .operands()
                .starts_with(&alloc::format!("v{}, v{}, ", a, b)));
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f22c_iget_field_ref() {
    let bytecode = [0x52u8, 0x10, 0x05, 0x00]; // iget v0, v1, field@5
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "iget");
    assert_eq!(ins.operands(), "v0, v1, field@5");
}
