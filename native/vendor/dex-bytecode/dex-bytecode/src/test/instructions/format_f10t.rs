//! F10t: op AA — 8-bit signed branch offset in 16-bit units (2 bytes). Opcodes: goto (0x28).

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

#[test]
fn f10t_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F10t {
            continue;
        }
        // Test zero, positive, negative offset; operands end with ..h (hex 16-bit units)
        for aa in [0i8, 2, -1, 127] {
            let bytecode = [op, aa as u8];
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert!(
                ins.operands().ends_with('h'),
                "operands {:?}",
                ins.operands()
            );
            assert_eq!(ins.length(), 2);
        }
    }
}

#[test]
fn f10t_goto_all_offsets() {
    let tests: &[u8] = &[0x00, 0x02, 0x7f, 0x80, 0xff];
    for aa in tests.iter().copied() {
        let bytecode = [0x28u8, aa];
        let ins = decode_one(&bytecode[..], 0).unwrap();
        assert_eq!(ins.mnemonic(), "goto");
        assert!(
            ins.operands().ends_with('h'),
            "aa=0x{:02x} operands {:?}",
            aa,
            ins.operands()
        );
    }
}
