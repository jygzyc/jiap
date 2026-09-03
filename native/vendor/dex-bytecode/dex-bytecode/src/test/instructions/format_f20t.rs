//! F20t: op ØØØØ AAAA — 16-bit signed branch offset in 16-bit units (4 bytes). Opcodes: goto/16 (0x29).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_i16(v: i16) -> [u8; 2] {
    (v as u16).to_le_bytes()
}

#[test]
fn f20t_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F20t {
            continue;
        }
        for aaaa in [0i16, 1, -1, 0x7fff, -0x8000] {
            let mut bytecode = vec![op, 0, 0, 0];
            bytecode[2..4].copy_from_slice(&le_i16(aaaa));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.length(), 4);
        }
    }
}

#[test]
fn f20t_goto16_offsets() {
    let bytecode = [0x29u8, 0x00, 0x02, 0x00]; // goto/16 +2 (AAAA=2 in LE)
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "goto/16");
    assert!(
        ins.operands().contains("02") || ins.operands().contains("2"),
        "operands {:?}",
        ins.operands()
    );
}
