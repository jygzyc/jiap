//! F35c: A|G|op BBBB C|D|E|F — 0–5 registers and 16-bit ref (6 bytes).
//! Opcodes: filled-new-array, invoke-virtual, invoke-super, invoke-direct, invoke-static,
//! invoke-interface, invoke-custom (0x24, 0x6e..0x72, 0xfc).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn f35c_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F35c {
            continue;
        }
        // A=0 (no regs), A=1 (one reg), A=5 (five regs)
        for (_a, w1_high) in [(0u8, 0u8), (1, 0x10u8), (5, 0x50u8)] {
            let w1 = (w1_high as u16) << 8 | (op as u16);
            let w2 = 0u16; // C=D=E=F=0
            let mut bytecode = vec![0u8; 6];
            bytecode[0..2].copy_from_slice(&w1.to_le_bytes());
            bytecode[2..4].copy_from_slice(&le_u16(0));
            bytecode[4..6].copy_from_slice(&w2.to_le_bytes());
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.length(), 6);
        }
    }
}

#[test]
fn f35c_invoke_virtual_zero_and_five_regs() {
    // invoke-virtual {} (no args)
    let bytecode = [0x6eu8, 0x00, 0x00, 0x00, 0x00, 0x00];
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "invoke-virtual");
    assert_eq!(ins.operands(), "method@0");

    // invoke-virtual {v0, v1, v2, v3, v4}, method@1
    let bytecode = [0x6eu8, 0x50, 0x01, 0x00, 0x43, 0x21]; // A=5, G=0, B=1, C=1,D=2,E=3,F=4
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "invoke-virtual");
    assert!(ins.operands().contains("v1"));
    assert!(ins.operands().contains("method@1"));
}
