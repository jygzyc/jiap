//! F11x: op AA — single register vAA (2 bytes). Opcodes: move-result, move-result-wide,
//! move-result-object, move-exception, return, return-wide, return-object, monitor-enter,
//! monitor-exit, throw.

use crate::opcodes::Format;
use crate::{decode_one, get_opcode_entry};

#[test]
fn f11x_every_opcode_decodes() {
    for op in 0u8..=255u8 {
        let entry = get_opcode_entry(op);
        if entry.format != Format::F11x {
            continue;
        }
        // v0, v1, ..., v255
        for aa in [0u8, 1, 15, 255] {
            let bytecode = [op, aa];
            let ins = decode_one(&bytecode[..], 0).unwrap();
            assert_eq!(ins.opcode(), op);
            assert_eq!(ins.mnemonic(), entry.mnemonic);
            assert_eq!(ins.operands(), alloc::format!("v{}", aa));
            assert_eq!(ins.length(), 2);
        }
    }
}

#[test]
fn f11x_move_result_return_throw() {
    let tests: &[(u8, &str)] = &[(0x0a, "move-result"), (0x0f, "return"), (0x27, "throw")];
    for (op, mnemonic) in tests.iter().copied() {
        let ins = decode_one(&[op, 0u8], 0).unwrap();
        assert_eq!(ins.mnemonic(), mnemonic);
        assert_eq!(ins.operands(), "v0");
    }
}
