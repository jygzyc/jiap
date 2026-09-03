//! F3rc: op AA BBBB CCCC — register range vCCCC..v(CCCC+AA-1) and 16-bit ref (6 bytes).
//! Opcodes: filled-new-array/range, invoke-*/range, invoke-custom/range.

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn f3rc_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F3rc {
            continue;
        }
        for (aa, bbbb, cccc) in [(1u8, 0u16, 0u16), (2, 1, 2), (255, 0xffff, 0xffff)] {
            let mut bytecode = vec![op, aa, 0, 0, 0, 0];
            bytecode[2..4].copy_from_slice(&le_u16(bbbb));
            bytecode[4..6].copy_from_slice(&le_u16(cccc));
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(ins.operands().contains("v"));
            assert_eq!(ins.length(), 6);
        }
    }
}

#[test]
fn f3rc_invoke_range_single_reg() {
    // invoke-virtual/range {v0}, method@0
    let bytecode = [0x74u8, 0x01, 0x00, 0x00, 0x00, 0x00];
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "invoke-virtual/range");
    assert_eq!(ins.operands(), "v0, method@0");
}
