//! Tests mirroring androguard tests/test_dex.py (InstructionTest and related).

mod instructions;

use crate::decoder::Decoder;
use crate::opcodes::{format_length, get_opcode_entry};
use alloc::vec;
use alloc::vec::Vec;
use core::iter;

/// Decode hex string (no spaces) into bytes.
fn hex(s: &str) -> Vec<u8> {
    let s = s.replace(' ', "");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn decoder_with_ip() {
    let decoder = Decoder::with_ip(b"", 0x1234_5678_9ABC_DEF1, 0).unwrap();
    assert_eq!(decoder.ip(), 0x1234_5678_9ABC_DEF1);
}

#[test]
fn decode_nop() {
    let data = [0x00u8, 0x00];
    let ins = crate::decode_one(&data[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "nop");
    assert_eq!(ins.operands(), "");
    assert_eq!(ins.length(), 2);
}

#[test]
fn decode_move() {
    let data = [0x01u8, 0x21];
    let ins = crate::decode_one(&data[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "move");
    assert_eq!(ins.operands(), "v1, v2");
    assert_eq!(ins.length(), 2);
}

/// testInstructions: each opcode 0..256 has correct format length; unused raise error; rest decode.
#[test]
fn test_instructions() {
    for op_value in 0u8..=255 {
        let entry = get_opcode_entry(op_value);
        let expected_len = format_length(entry.format) as usize;

        if entry.mnemonic == "unused" {
            let bytecode = [op_value, 0];
            let r = crate::decode_one(&bytecode[..], 0);
            assert!(r.is_err(), "opcode 0x{:02x} (unused) should fail", op_value);
            continue;
        }

        let buf_len = expected_len.max(2);
        let mut bytecode = vec![op_value];
        bytecode.extend(iter::repeat(0).take(buf_len - 1));

        let ins = crate::decode_one(&bytecode[..], 0)
            .unwrap_or_else(|e| panic!("opcode 0x{:02x} {}: {}", op_value, entry.mnemonic, e));
        assert_eq!(ins.opcode(), op_value, "opcode 0x{:02x}", op_value);
        assert_eq!(ins.mnemonic(), entry.mnemonic, "opcode 0x{:02x}", op_value);
        assert_eq!(
            ins.length() as usize,
            expected_len,
            "opcode 0x{:02x} {}",
            op_value,
            entry.mnemonic
        );
    }
}

/// testNOP: nop parses correctly (same as decode_nop).
#[test]
fn test_nop() {
    let ins = crate::decode_one(&[0x00u8, 0x00], 0).unwrap();
    assert_eq!(ins.mnemonic(), "nop");
}

/// testLinearSweep: nop, nop, nop, return-void; total 8 bytes.
#[test]
fn test_linear_sweep() {
    let bytecode: &[u8] = &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let instructions = ["nop", "nop", "nop", "return-void"];
    let mut total_len: usize = 0;

    let decoded = crate::decode_all(bytecode, 0).unwrap();
    assert_eq!(decoded.len(), 4);
    for (ins, expected_name) in decoded.iter().zip(instructions.iter()) {
        assert_eq!(ins.length(), 2);
        assert_eq!(ins.mnemonic(), *expected_name);
        total_len += ins.length() as usize;
    }
    assert_eq!(total_len, bytecode.len());
}

/// testLinearSweepStrings: const-string, sget-object, invoke-virtual, ... return-void.
#[test]
fn test_linear_sweep_strings() {
    let bytecode = hex(
        "1A000F001A0100001A0214001A0311001A0415001A0413001A0508001A061200\
         1A0716001A081000620900006E2002000900620000006E200200100062000000\
         6E2002002000620000006E2002003000620000006E2002003000620000006E20\
         02004000620000006E2002005000620000006E2002006000620000006E200200\
         7000620000006E20020080000E00",
    );
    let expected: &[&str] = &[
        "const-string",
        "const-string",
        "const-string",
        "const-string",
        "const-string",
        "const-string",
        "const-string",
        "const-string",
        "const-string",
        "const-string",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "sget-object",
        "invoke-virtual",
        "return-void",
    ];
    let decoded = crate::decode_all(&bytecode, 0).unwrap();
    let mut total_len: usize = 0;
    for (ins, name) in decoded.iter().zip(expected.iter()) {
        assert_eq!(ins.mnemonic(), *name);
        total_len += ins.length() as usize;
    }
    assert_eq!(decoded.len(), expected.len());
    assert_eq!(total_len, bytecode.len());
}

/// testLinearSweepSwitch: packed-switch, const/16, ..., packed-switch-payload.
#[test]
fn test_linear_sweep_switch() {
    let bytecode = hex(
        "2B02140000001300110038030400130063000F001300170028F913002A0028F6\
         1300480028F3000000010300010000000A0000000D00000010000000",
    );
    let expected: &[&str] = &[
        "packed-switch",
        "const/16",
        "if-eqz",
        "const/16",
        "return",
        "const/16",
        "goto",
        "const/16",
        "goto",
        "const/16",
        "goto",
        "nop",
        "packed-switch-payload",
    ];
    let decoded = crate::decode_all(&bytecode, 0).unwrap();
    let mut total_len: usize = 0;
    for (ins, name) in decoded.iter().zip(expected.iter()) {
        assert_eq!(ins.mnemonic(), *name);
        total_len += ins.length() as usize;
    }
    assert_eq!(decoded.len(), expected.len());
    assert_eq!(total_len, bytecode.len());
}

/// test_linear_sweep_arrays: fill-array-data and fill-array-data-payload.
#[test]
fn test_linear_sweep_arrays() {
    let bytecode = hex(
        "12412310030026002D0000005B30000012702300050026002B0000005B300300\
         1250230004002600350000005B300100231007002600380000005B3002001220\
         2300060012011A020D004D02000112111A0211004D0200015B3004000E000000\
         0003010004000000141E28320003040007000000010000000200000003000000\
         0400000005000000E70300000A899D0000030200050000006100620078007A00\
         63000000000302000400000005000A000F001400",
    );
    let expected: &[&str] = &[
        "const/4",
        "new-array",
        "fill-array-data",
        "iput-object",
        "const/4",
        "new-array",
        "fill-array-data",
        "iput-object",
        "const/4",
        "new-array",
        "fill-array-data",
        "iput-object",
        "new-array",
        "fill-array-data",
        "iput-object",
        "const/4",
        "new-array",
        "const/4",
        "const-string",
        "aput-object",
        "const/4",
        "const-string",
        "aput-object",
        "iput-object",
        "return-void",
        "nop",
        "fill-array-data-payload",
        "fill-array-data-payload",
        "fill-array-data-payload",
        "nop",
        "fill-array-data-payload",
    ];
    let decoded = crate::decode_all(&bytecode, 0).unwrap();
    for (ins, name) in decoded.iter().zip(expected.iter()) {
        assert_eq!(ins.mnemonic(), *name, "at offset {}", ins.offset);
    }
    assert_eq!(decoded.len(), expected.len());
    let total_len: usize = decoded.iter().map(|i| i.length() as usize).sum();
    assert_eq!(total_len, bytecode.len());
}

/// testWrongInstructions: invalid opcodes raise error.
#[test]
fn test_wrong_instructions() {
    let r = crate::decode_all(&[0xffu8, 0xab], 0);
    assert!(r.is_err());

    let r = crate::decode_all(&[0x00u8, 0x00, 0xff, 0xab], 0);
    assert!(
        r.is_err(),
        "second instruction 0xff 0xab is incomplete (needs 4 bytes for const-method-type)"
    );
    let r2 = crate::decode_one(&[0xffu8, 0xab], 0);
    assert!(r2.is_err());
}

/// testIncompleteInstruction: truncated bytecode raises error.
#[test]
fn test_incomplete_instruction() {
    let bytecode = [0x18u8, 0x01, 0xff, 0xff];
    let r = crate::decode_all(&bytecode[..], 0);
    assert!(r.is_err());
}

/// Valid 51l parses.
#[test]
fn test_instruction_51l_valid() {
    let bytecode = [0x18u8, 0x01, 0x23, 0x23, 0x00, 0xff, 0x99, 0x11, 0x22, 0x22];
    let ins = crate::decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const-wide");
    assert_eq!(ins.opcode(), 0x18);
    assert_eq!(ins.length(), 10);
}

/// testInstruction21h: const/high16 and const-wide/high16 literals and output.
#[test]
fn test_instruction_21h() {
    let ins = crate::decode_one(&[0x15u8, 0x00, 0x42, 0x11], 0).unwrap();
    assert_eq!(ins.opcode(), 0x15);
    assert_eq!(ins.mnemonic(), "const/high16");
    assert_eq!(ins.operands(), "v0, 289538048");
    assert_eq!(ins.length(), 4);

    let ins = crate::decode_one(&[0x19u8, 0x00, 0x42, 0x11], 0).unwrap();
    assert_eq!(ins.opcode(), 0x19);
    assert_eq!(ins.mnemonic(), "const-wide/high16");
    assert_eq!(ins.operands(), "v0, 1243556447107678208");
    assert_eq!(ins.length(), 4);

    let ins = crate::decode_one(&[0x19u8, 0x00, 0xbe, 0xff], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const-wide/high16");
    assert_eq!(ins.operands(), "v0, -18577348462903296");
}

/// testInstruction51l: const-wide cases.
#[test]
fn test_instruction_51l() {
    let bytecode = [0x18u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let ins = crate::decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const-wide");
    assert_eq!(ins.operands(), "v0, 0");

    let bytecode = [0x18u8, 0x00, 0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x70];
    let ins = crate::decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const-wide");
    assert_eq!(ins.operands(), "v0, 8085107642740388882");

    let bytecode = [0x18u8, 0x00, 0xee, 0xcb, 0xa9, 0x87, 0x6f, 0xed, 0xcb, 0x8f];
    let ins = crate::decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "const-wide");
    assert_eq!(ins.operands(), "v0, -8085107642740388882");
}

/// testInstruction11n: const/4 (register and literal from second byte).
#[test]
fn test_instruction_11n() {
    let tests: &[(u8, u8, i8)] = &[
        (0x00, 0, 0),
        (0x13, 3, 1),
        (0x10, 0, 1),
        (0x11, 1, 1),
        (0x71, 1, 7),
        (0xF0, 0, -1),
        (0x61, 1, 6),
        (0xE6, 6, -2),
        (0x86, 6, -8),
    ];
    for (args, reg, lit) in tests.iter().copied() {
        let ins = crate::decode_one(&[0x12u8, args], 0).unwrap();
        assert_eq!(ins.mnemonic(), "const/4");
        let expected = alloc::format!("v{}, {}", reg, lit);
        assert_eq!(ins.operands(), expected);
    }
}

/// testInstruction21s: const/16 and const-wide/16.
#[test]
fn test_instruction_21s() {
    let tests: &[(&[u8], u8, i16)] = &[
        (&[0x01, 0x0e, 0x00], 1, 0x0e),
        (&[0x02, 0x10, 0x00], 2, 0x10),
        (&[0x00, 0x02, 0x20], 0, 0x2002),
        (&[0x00, 0x00, 0x80], 0, -0x8000_i16),
        (&[0x00, 0xff, 0x7f], 0, 0x7fff),
        (&[0x00, 0x80, 0xff], 0, -0x80_i16),
    ];
    for (args, reg, lit) in tests.iter().copied() {
        let mut bc = vec![0x13u8];
        bc.extend_from_slice(args);
        let ins = crate::decode_one(&bc, 0).unwrap();
        assert_eq!(ins.mnemonic(), "const/16");
        assert_eq!(ins.operands(), alloc::format!("v{}, {}", reg, lit));

        let mut bc_wide = vec![0x16u8];
        bc_wide.extend_from_slice(args);
        let ins = crate::decode_one(&bc_wide, 0).unwrap();
        assert_eq!(ins.mnemonic(), "const-wide/16");
        assert_eq!(ins.operands(), alloc::format!("v{}, {}", reg, lit));
    }
}

/// testInstruction31i: const and const-wide/32.
#[test]
fn test_instruction_31i() {
    let tests: &[(&[u8], u8, i32)] = &[
        (&[0x00, 0xff, 0xff, 0xff, 0x1f], 0, 0x1fff_ffff),
        (&[0x00, 0x62, 0x00, 0x07, 0x7f], 0, 0x7f07_0062),
        (&[0x00, 0x74, 0x00, 0x07, 0x7f], 0, 0x7f07_0074),
    ];
    for (args, reg, lit) in tests.iter().copied() {
        let mut bc = vec![0x14u8];
        bc.extend_from_slice(args);
        let ins = crate::decode_one(&bc, 0).unwrap();
        assert_eq!(ins.mnemonic(), "const");
        assert_eq!(ins.operands(), alloc::format!("v{}, {}", reg, lit));

        let mut bc_wide = vec![0x17u8];
        bc_wide.extend_from_slice(args);
        let ins = crate::decode_one(&bc_wide, 0).unwrap();
        assert_eq!(ins.mnemonic(), "const-wide/32");
        assert_eq!(ins.operands(), alloc::format!("v{}, {}", reg, lit));
    }
}

/// Decoder with size_16bit_units stops at boundary (like androguard get_instructions(cm, size, insn, idx)).
#[test]
fn test_decoder_with_size_units() {
    let bytecode: &[u8] = &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00];
    let mut decoder = Decoder::new(bytecode, 0, Some(4));
    let mut count = 0;
    while let Some(Ok(ins)) = decoder.next() {
        count += 1;
        assert_eq!(ins.length(), 2);
    }
    assert_eq!(count, 4);
}

// ============== Tests for Decoder & analysis features ==============

/// Mock resolver: returns "resolved_{kind}_{index}" for any ref; used to verify resolver is called.
struct MockResolver;

impl crate::ResolveRef for MockResolver {
    fn resolve(&self, kind: crate::RefKind, index: u32) -> Option<alloc::string::String> {
        let kind_str = match kind {
            crate::RefKind::None => "none",
            crate::RefKind::String => "string",
            crate::RefKind::Type => "type",
            crate::RefKind::Field => "field",
            crate::RefKind::Method => "method",
            crate::RefKind::MethodProto => "proto",
            crate::RefKind::CallSite => "callsite",
            crate::RefKind::Varies => "varies",
        };
        Some(alloc::format!("resolved_{}_{}", kind_str, index))
    }
}

/// test_resolve_ref: decode_one_with_resolver / decode_all_with_resolver use resolver for refs.
#[test]
fn test_resolve_ref() {
    // const-string v0, string@15 (21c: AA=0, BBBB=15)
    let bytecode = [0x1au8, 0x00, 0x0f, 0x00];
    let ins = crate::decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.operands(), "v0, string@15");

    let ins_resolved = crate::decode_one_with_resolver(&bytecode[..], 0, &MockResolver).unwrap();
    assert_eq!(ins_resolved.mnemonic(), "const-string");
    assert_eq!(ins_resolved.operands(), "v0, resolved_string_15");
}

#[test]
fn test_resolve_ref_decode_all() {
    // nop; const-string v0, string@0; return-void
    let bytecode = [0x00u8, 0x00, 0x1a, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let instructions = crate::decode_all_with_resolver(&bytecode[..], 0, &MockResolver).unwrap();
    assert_eq!(instructions.len(), 3);
    assert_eq!(instructions[0].mnemonic(), "nop");
    assert_eq!(instructions[1].operands(), "v0, resolved_string_0");
    assert_eq!(instructions[2].mnemonic(), "return-void");
}

/// test_resolve_fn_resolver: FnResolver adapter works.
#[test]
fn test_resolve_fn_resolver() {
    let resolver = crate::FnResolver(|kind, index| {
        if kind == crate::RefKind::Type && index == 2 {
            Some("Lfoo/Bar;".into())
        } else {
            None
        }
    });
    // const-class v0, type@2 (21c)
    let bytecode = [0x1cu8, 0x00, 0x02, 0x00];
    let ins = crate::decode_one_with_resolver(&bytecode[..], 0, &resolver).unwrap();
    assert_eq!(ins.operands(), "v0, Lfoo/Bar;");

    let ins_no =
        crate::decode_one_with_resolver(&bytecode[..], 0, &crate::FnResolver(|_, _| None)).unwrap();
    assert_eq!(ins_no.operands(), "v0, type@2");
}

/// test_branch_targets: branch_targets returns correct byte offset for goto (F10t).
#[test]
fn test_branch_targets_goto() {
    // goto +2 (in 16-bit units) -> target = 0 + 2*2 = 4
    let bytecode = [0x28u8, 0x02, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let targets = crate::branch_targets(&bytecode[..], 0);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], 4);
}

/// test_branch_targets_if: if-eqz has one target (branch offset).
#[test]
fn test_branch_targets_if() {
    // if-eqz v0, +2 (21t: AA=0, BBBB=2) -> target = 0 + 2*2 = 4
    let bytecode = [0x38u8, 0x00, 0x02, 0x00];
    let targets = crate::branch_targets(&bytecode[..], 0);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0], 4);
}

/// test_branch_targets_nop: nop has no branch targets.
#[test]
fn test_branch_targets_nop() {
    let bytecode = [0x00u8, 0x00];
    let targets = crate::branch_targets(&bytecode[..], 0);
    assert!(targets.is_empty());
}

/// test_collect_branch_targets: collect all targets from decoded list.
#[test]
fn test_collect_branch_targets() {
    // goto +3 (target 6); nop; nop; return-void at 6
    let bytecode = [0x28u8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let targets = crate::collect_branch_targets(&instructions, &bytecode[..], 0);
    assert!(targets.contains(&6));
    assert_eq!(targets.len(), 1);
}

/// test_basic_blocks: at least two blocks when there is a branch.
#[test]
fn test_basic_blocks() {
    // goto +2; nop; return-void
    let bytecode = [0x28u8, 0x02, 0x00, 0x00, 0x0e, 0x00];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let blocks = crate::basic_blocks(&instructions, &bytecode[..], 0);
    assert!(!blocks.is_empty());
    let with_successors: Vec<_> = blocks.iter().filter(|b| !b.successors.is_empty()).collect();
    assert!(
        !with_successors.is_empty(),
        "at least one block should have successor (goto target)"
    );
}

// ============== CFG and basic block tests ==============

/// test_cfg_single_block_no_branches: linear code (no branches) → exactly one block.
#[test]
fn test_cfg_single_block_no_branches() {
    // nop; nop; return-void
    let bytecode = [0x00u8, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let blocks = crate::basic_blocks(&instructions, &bytecode[..], 0);
    assert_eq!(blocks.len(), 1, "linear code must be one block");
    assert_eq!(blocks[0].start_offset, 0);
    assert_eq!(
        blocks[0].end_offset,
        u32::MAX,
        "single block extends to end"
    );
    assert!(blocks[0].successors.is_empty());
    assert!(blocks[0].fallthrough_to.is_none(), "no next block");
}

/// test_cfg_goto_three_blocks: goto +3 creates block boundary after branch and at target.
/// Bytecode: goto +03h (0-2); nop (2-4); nop (4-6); nop (6-8); return-void (8-10).
/// Block boundaries: 0 (start), 2 (after goto), 6 (target). So 3 blocks: [0,2), [2,6), [6,MAX).
#[test]
fn test_cfg_goto_three_blocks() {
    let bytecode = [0x28u8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let blocks = crate::basic_blocks(&instructions, &bytecode[..], 0);

    assert_eq!(
        blocks.len(),
        3,
        "goto creates 3 blocks: before branch, fall-through, target"
    );

    // Block 0: [0, 2) — goto only; successor 6, no fallthrough (unconditional)
    assert_eq!(blocks[0].start_offset, 0);
    assert_eq!(blocks[0].end_offset, 2);
    assert_eq!(blocks[0].successors.len(), 1);
    assert_eq!(blocks[0].successors[0], 6);
    assert!(
        blocks[0].fallthrough_to.is_none(),
        "goto has no fallthrough"
    );

    // Block 1: [2, 6) — two nops; fallthrough to block 2
    assert_eq!(blocks[1].start_offset, 2);
    assert_eq!(blocks[1].end_offset, 6);
    assert!(blocks[1].successors.is_empty());
    assert_eq!(blocks[1].fallthrough_to, Some(6));

    // Block 2: [6, MAX) — nop + return-void; no next block
    assert_eq!(blocks[2].start_offset, 6);
    assert_eq!(blocks[2].end_offset, u32::MAX);
    assert!(blocks[2].successors.is_empty());
    assert!(blocks[2].fallthrough_to.is_none());
}

/// test_cfg_if_eqz_blocks: if-eqz creates block boundary at target; fall-through stays in same block.
/// if-eqz v0, +2 (21t) -> target = 0 + 2*2 = 4. Block starts: 0, 4 (target + after-branch end).
/// So blocks: [0,4) (if-eqz) → 4; [4,MAX) (nop, return-void).
#[test]
fn test_cfg_if_eqz_blocks() {
    let bytecode = [0x38u8, 0x00, 0x02, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let blocks = crate::basic_blocks(&instructions, &bytecode[..], 0);

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].start_offset, 0);
    assert_eq!(blocks[0].end_offset, 4);
    assert_eq!(blocks[0].successors, vec![4u32]);
    assert_eq!(
        blocks[0].fallthrough_to,
        Some(4),
        "if-eqz falls through to next block"
    );
    assert_eq!(blocks[1].start_offset, 4);
    assert_eq!(blocks[1].end_offset, u32::MAX);
    assert!(blocks[1].successors.is_empty());
    assert!(blocks[1].fallthrough_to.is_none());
}

/// test_cfg_collect_branch_targets_multiple: multiple branches → all targets collected (set, no duplicates).
#[test]
fn test_cfg_collect_branch_targets_multiple() {
    // Two gotos: first targets 8, second (at 6) also targets 8. So unique set is {8}.
    // Layout: goto +4 (0-2), nop (2-4), nop (4-6), goto +1 (6-8), nop (8-10), return (10-12).
    let bytecode = [
        0x28u8, 0x04, // goto +4 -> 8
        0x00, 0x00, // nop
        0x00, 0x00, // nop
        0x28u8, 0x01, // goto +1 -> 8
        0x00, 0x00, // nop
        0x0e, 0x00, // return-void
    ];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let targets = crate::collect_branch_targets(&instructions, &bytecode[..], 0);
    assert!(targets.contains(&8), "at least one branch targets 8");
    assert!(!targets.is_empty());
}

/// test_cfg_basic_block_successors_deduplicated: block with multiple branches to same target has deduplicated, sorted successors.
#[test]
fn test_cfg_basic_block_successors_deduplicated() {
    // Block 0: if-eqz v0, +2 (target 4); if-nez v1, +2 (target 4). Both branch to 4 → one successor after dedupe.
    // Boundaries: 0, 4 (target), 6 (after second branch). So blocks: [0,4), [4,6), [6,MAX).
    let bytecode = [
        0x38u8, 0x00, 0x02, 0x00, // if-eqz v0, +2 -> 4
        0x39u8, 0x01, 0x02, 0x00, // if-nez v1, +2 -> 4
        0x00, 0x00, 0x0e, 0x00, // nop, return-void
    ];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let blocks = crate::basic_blocks(&instructions, &bytecode[..], 0);
    assert_eq!(blocks.len(), 3);
    assert_eq!(
        blocks[0].successors.len(),
        1,
        "successors deduplicated when two branches share target"
    );
    assert_eq!(blocks[0].successors[0], 4);
    assert_eq!(blocks[0].successors, vec![4u32], "sorted order");
}

/// test_cfg_collect_branch_targets_two_distinct: two branches to different targets → set size 2.
#[test]
fn test_cfg_collect_branch_targets_two_distinct() {
    // goto +3 (0-2) -> 6; nop (2-4); goto -1 (4-6) -> 2; nop (6-8); return (8-10).
    let bytecode = [
        0x28u8, 0x03, // goto +3 -> 6
        0x00, 0x00, // nop
        0x28u8, 0xff, // goto -1 -> 2
        0x00, 0x00, // nop
        0x0e, 0x00, // return-void
    ];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let targets = crate::collect_branch_targets(&instructions, &bytecode[..], 0);
    assert_eq!(targets.len(), 2);
    assert!(targets.contains(&2));
    assert!(targets.contains(&6));
}

/// test_cfg_base_offset: basic_blocks with base_offset returns block offsets in absolute (base) space.
/// data must be the full buffer so branch_targets(data, start) can read at start; code lives at data[base..].
#[test]
fn test_cfg_base_offset() {
    let code = [0x28u8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let base = 16usize;
    let mut data = vec![0u8; base];
    data.extend_from_slice(&code);
    let instructions = crate::decode_all(&data[base..], 0).unwrap();
    let blocks = crate::basic_blocks(&instructions, &data[..], base);

    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].start_offset, 16);
    assert_eq!(blocks[0].end_offset, 18);
    assert_eq!(blocks[0].successors, vec![22u32]); // 16 + 6
    assert_eq!(blocks[1].start_offset, 18);
    assert_eq!(blocks[1].end_offset, 22);
    assert_eq!(blocks[2].start_offset, 22);
}

/// test_cfg_packed_switch_expands_case_targets: explicit_successors expands packed-switch payload into case edges.
#[test]
fn test_cfg_packed_switch_expands_case_targets() {
    // packed-switch v0, +5 (payload at 10)
    // 0x00: packed-switch (6 bytes)
    // 0x06: nop
    // 0x08: return-void
    // 0x0a: packed-switch-payload (size=2, targets -> 0x06 and 0x08)
    let bytecode = hex(concat!(
        "2B0005000000", // packed-switch v0, +5 (31t: BBBBBBBB = 5) -> payload at 10
        "0000",         // nop (fallthrough)
        "0E00",         // return-void
        "0001",         // ident = 0x0100
        "0200",         // size = 2
        "00000000",     // first_key = 0
        "03000000",     // target[0] = +3 units => 6 bytes
        "04000000"      // target[1] = +4 units => 8 bytes
    ));

    let succ = crate::explicit_successors(&bytecode[..], 0);
    assert_eq!(succ, vec![6u32, 8u32]);

    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let blocks = crate::basic_blocks(&instructions, &bytecode[..], 0);
    assert_eq!(
        blocks.len(),
        3,
        "switch splits blocks at case targets and after switch"
    );
    assert_eq!(blocks[0].start_offset, 0);
    assert_eq!(blocks[0].end_offset, 6);
    assert_eq!(blocks[0].successors, vec![6u32, 8u32]);
}

/// test_cfg_sparse_switch_expands_case_targets: explicit_successors expands sparse-switch payload.
#[test]
fn test_cfg_sparse_switch_expands_case_targets() {
    // sparse-switch v0, +5 (payload at 10) with one key and one target -> 0x08
    let bytecode = hex(concat!(
        "2C0005000000", // sparse-switch v0, +5
        "0000",         // nop
        "0E00",         // return-void (target)
        "0002",         // ident = 0x0200
        "0100",         // size = 1
        "00000000",     // key[0] = 0
        "04000000"      // target[0] = +4 units => 8 bytes
    ));
    let succ = crate::explicit_successors(&bytecode[..], 0);
    assert_eq!(succ, vec![8u32]);
}

/// test_cfg_fill_array_data_is_not_branch: fill-array-data references a payload but has no control-flow successors.
#[test]
fn test_cfg_fill_array_data_is_not_branch() {
    // fill-array-data v0, +4 (payload at 10); return-void; payload
    let bytecode = hex(concat!(
        "260004000000", // fill-array-data v0, +4
        "0E00",         // return-void
        "0003",         // ident = 0x0300
        "0200",         // elem_width = 2
        "01000000",     // size = 1
        "0000"          // data (padded)
    ));
    let succ = crate::explicit_successors(&bytecode[..], 0);
    assert!(succ.is_empty());
}

/// test_is_unconditional_branch: goto/goto/16/goto/32 are unconditional; if-* are not.
#[test]
fn test_is_unconditional_branch() {
    assert!(crate::is_unconditional_branch(&[0x28u8, 0x00, 0x00], 0)); // goto
    assert!(crate::is_unconditional_branch(
        &[0x29u8, 0x00, 0x00, 0x00],
        0
    )); // goto/16
    assert!(crate::is_unconditional_branch(
        &[0x2au8, 0x00, 0x00, 0x00, 0x00, 0x00],
        0
    )); // goto/32
    assert!(!crate::is_unconditional_branch(
        &[0x38u8, 0x00, 0x01, 0x00],
        0
    )); // if-eqz
    assert!(!crate::is_unconditional_branch(
        &[0x00u8, 0x00, 0x00, 0x00],
        0
    )); // nop
}

/// test_cfg_edges_includes_fallthrough: cfg_edges returns both branch and fallthrough edges.
#[test]
fn test_cfg_edges_includes_fallthrough() {
    // goto +3 (0-2) -> 6; nop (2-4); nop (4-6); nop (6-8); return-void (8-10).
    // Blocks: [0,2) [2,6) [6,MAX). Edges: 0->6 (goto), 2->6 (fallthrough), 6->(none).
    let bytecode = [0x28u8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let edges = crate::cfg_edges(&instructions, &bytecode[..], 0);
    assert!(edges.contains(&(0, 6)), "goto edge 0 -> 6");
    assert!(edges.contains(&(2, 6)), "fallthrough edge 2 -> 6");
    assert_eq!(edges.len(), 2);
}

/// test_try_catch_entry_format: format_catch_line produces expected string.
#[test]
fn test_try_catch_entry_format() {
    let entry = crate::TryCatchEntry {
        start_offset: 0,
        end_offset: 8,
        handler_offset: 16,
        type_index: Some(1),
    };
    let line = crate::format_catch_line(&entry, Some("Ljava/lang/Exception;"));
    assert!(line.contains(".catch"));
    assert!(line.contains("Ljava/lang/Exception;"));
    assert!(line.contains("0x00000000"));
    assert!(line.contains("0x00000008"));
    assert!(line.contains(":L00000010"));

    let line_all = crate::format_catch_line(&entry, None);
    assert!(line_all.contains("all"));
}

/// test_exception_edges: exception_edges returns (block_start, handler) for blocks in try range.
#[test]
fn test_exception_edges() {
    let entries = [
        crate::TryCatchEntry {
            start_offset: 0,
            end_offset: 8,
            handler_offset: 16,
            type_index: Some(1),
        },
        crate::TryCatchEntry {
            start_offset: 8,
            end_offset: 20,
            handler_offset: 24,
            type_index: None,
        },
    ];
    let blocks = [
        crate::BasicBlock {
            start_offset: 0,
            end_offset: 4,
            successors: vec![],
            fallthrough_to: Some(4),
        },
        crate::BasicBlock {
            start_offset: 4,
            end_offset: 8,
            successors: vec![],
            fallthrough_to: Some(8),
        },
        crate::BasicBlock {
            start_offset: 8,
            end_offset: 16,
            successors: vec![],
            fallthrough_to: Some(16),
        },
        crate::BasicBlock {
            start_offset: 16,
            end_offset: 24,
            successors: vec![],
            fallthrough_to: Some(24),
        },
    ];
    let edges = crate::exception_edges(&entries[..], &blocks[..]);
    assert!(edges.contains(&(0, 16)));
    assert!(edges.contains(&(4, 16)));
    assert!(edges.contains(&(8, 24)));
    assert!(edges.contains(&(16, 24)));
    assert_eq!(edges.len(), 4);
}

/// test_patch_branch_target: patch_branch_target rewrites goto to new target.
#[test]
fn test_patch_branch_target() {
    let mut data = [0x28u8, 0x03, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00]; // goto +3 -> 6; nop; nop; return-void
    crate::patch_branch_target(&mut data[..], 0, 6).unwrap();
    assert_eq!(data[0..2], [0x28, 0x03]); // unchanged: already targets 6
    crate::patch_branch_target(&mut data[..], 0, 4).unwrap();
    assert_eq!(data[0..2], [0x28, 0x02]); // +2 units -> 4
}

/// test_encode_helpers: encode_nop, encode_return_void, encode_goto produce correct bytes.
#[test]
fn test_encode_helpers() {
    assert_eq!(crate::encode_nop(), [0x00, 0x00]);
    assert_eq!(crate::encode_return_void(), [0x0e, 0x00]);
    assert_eq!(crate::encode_goto(2), [0x28, 0x02]);
    assert_eq!(crate::encode_goto(-1), [0x28, 0xff]);
}

// ============== Disassembler precompute regression tests ==============
// Ensure the one-pass precompute (targets_per_ins + label_offsets) used in the CLI
// matches the legacy behavior (collect_branch_targets + branch_targets per instruction).

/// test_disasm_precompute_label_offsets_matches_collect_branch_targets: one-pass label set equals collect_branch_targets.
#[test]
fn test_disasm_precompute_label_offsets_matches_collect_branch_targets() {
    let bytecode = [
        0x28u8, 0x04, // goto +4 -> 8
        0x00, 0x00, // nop
        0x00, 0x00, // nop
        0x28u8, 0x01, // goto +1 -> 8
        0x00, 0x00, // nop
        0x0e, 0x00, // return-void
    ];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let slice = &bytecode[..];
    let base = 0usize;

    let mut label_offsets = alloc::collections::BTreeSet::new();
    for ins in &instructions {
        let targets = crate::branch_targets(slice, ins.offset as usize);
        for &t in &targets {
            label_offsets.insert(t);
        }
    }
    let expected = crate::collect_branch_targets(&instructions, slice, base);
    assert_eq!(
        label_offsets, expected,
        "precompute label set must match collect_branch_targets"
    );
}

/// test_disasm_precompute_targets_per_ins_matches_branch_targets: precomputed targets_per_ins matches per-instruction branch_targets.
#[test]
fn test_disasm_precompute_targets_per_ins_matches_branch_targets() {
    let bytecode = [
        0x38u8, 0x00, 0x01, 0x00, // if-eqz v0, +1 -> 4
        0x39u8, 0x01, 0x01, 0x00, // if-nez v1, +1 -> 4
        0x00, 0x00, 0x0e, 0x00, // nop, return-void
    ];
    let instructions = crate::decode_all(&bytecode[..], 0).unwrap();
    let slice = &bytecode[..];

    let mut targets_per_ins: Vec<Vec<u32>> = Vec::with_capacity(instructions.len());
    for ins in &instructions {
        let targets = crate::branch_targets(slice, ins.offset as usize);
        targets_per_ins.push(targets);
    }

    for (i, ins) in instructions.iter().enumerate() {
        let expected = crate::branch_targets(slice, ins.offset as usize);
        assert_eq!(
            &targets_per_ins[i], &expected,
            "targets_per_ins[{}] must match branch_targets at offset {}",
            i, ins.offset
        );
    }
}

/// test_decode_all_large_unchanged: decode_all with reserve produces same result (no regression from capacity hint).
#[test]
fn test_decode_all_large_unchanged() {
    let mut bytecode = Vec::new();
    for _ in 0..2000 {
        bytecode.extend_from_slice(&[0x00u8, 0x00]);
    }
    bytecode.extend_from_slice(&[0x0e, 0x00]);

    let a = crate::decode_all(&bytecode[..], 0).unwrap();
    let b = crate::decode_all(&bytecode[..], 0).unwrap();
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), 2001);
    for (i, (ins_a, ins_b)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(ins_a.offset, ins_b.offset, "instruction {} offset", i);
        assert_eq!(ins_a.length, ins_b.length, "instruction {} length", i);
        assert_eq!(ins_a.opcode(), ins_b.opcode(), "instruction {} opcode", i);
        assert_eq!(
            ins_a.mnemonic(),
            ins_b.mnemonic(),
            "instruction {} mnemonic",
            i
        );
    }
}
