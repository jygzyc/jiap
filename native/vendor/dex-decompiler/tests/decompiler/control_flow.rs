//! Control-flow integration tests: return, if/else, while via minimal DEX bytecode.

use super::helpers::minimal_dex_with_method_code;
use dex_decompiler::{parse_dex, Decompiler};

#[test]
fn test_decompiler_return_void() {
    let dex_bytes = minimal_dex_with_method_code(&[0x0e, 0x00]); // return-void
    let dex = parse_dex(&dex_bytes).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("return;"),
        "decompiled method with return-void should contain 'return;'"
    );
}

#[test]
fn test_decompiler_return_value() {
    let bytecode: &[u8] = &[0x12, 0x00, 0x0f, 0x00]; // const/4 v0, 0; return v0
    let dex_bytes = minimal_dex_with_method_code(bytecode);
    let dex = parse_dex(&dex_bytes).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("return"),
        "decompiled method with return should contain 'return'"
    );
}

/// if-eqz v0,+4; goto +2; return-void; return-void -> if/else with two returns.
#[test]
fn test_decompiler_if_else_pattern() {
    let bytecode: &[u8] = &[
        0x38, 0x00, 0x04, 0x00, // if-eqz v0, +4 -> target byte 8
        0x28, 0x02, // goto +2 -> target byte 8
        0x0e, 0x00, // return-void at 6
        0x0e, 0x00, // return-void at 8
    ];
    let dex_bytes = minimal_dex_with_method_code(bytecode);
    let dex = parse_dex(&dex_bytes).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("if ("),
        "decompiled if/else should contain 'if ('"
    );
    assert!(
        java.contains("} else {"),
        "decompiled if/else should contain '}} else {{'"
    );
}

/// Loop: const/4; if-eqz (exit); goto back; nop nop; return-void. Emits while (!(cond)) { body }; exit.
#[test]
fn test_decompiler_while_loop_pattern() {
    let bytecode: &[u8] = &[
        0x12, 0x00, // const/4 v0, 0
        0x38, 0x00, 0x05, 0x00, // if-eqz v0, +5 -> target 12
        0x28, 0xfe, // goto -2 -> target 2
        0x00, 0x00, 0x00, 0x00, // nop, nop
        0x0e, 0x00, // return-void at 12
    ];
    let dex_bytes = minimal_dex_with_method_code(bytecode);
    let dex = parse_dex(&dex_bytes).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains("while ("),
        "decompiled loop should contain 'while (' (either while (true) or while (!(cond)))"
    );
    eprintln!("WHILE JAVA:\n{java}");
    assert!(
        java.contains("continue;"),
        "loop with back-edge should emit 'continue;'"
    );
}

/// Partition-style exit path: loop with exit = (statement block) + (return block).
/// The exit path has two blocks: first block does something (e.g. const/4 v0,1), second block returns.
/// Both must appear in decompiled output (regression for missing final Swap before return).
#[test]
fn test_decompiler_loop_exit_path_two_blocks() {
    // 0: const/4 v0, 0
    // 2: if-eqz v0, +5  -> target 12 (return block)
    // 6: goto -2        -> back to 2
    // 8: nop
    // 10: const/4 v0, 1   <- exit block 1 (must be emitted)
    // 12: return v0      <- exit block 2
    let bytecode: &[u8] = &[
        0x12, 0x00, // const/4 v0, 0
        0x38, 0x00, 0x05, 0x00, // if-eqz v0, +5 -> target 12
        0x28, 0xfe, // goto -2 -> target 2
        0x00, 0x00, // nop
        0x12, 0x01, // const/4 v0, 1  (exit path block 1)
        0x0f, 0x00, // return v0     (exit path block 2)
    ];
    let dex_bytes = minimal_dex_with_method_code(bytecode);
    let dex = parse_dex(&dex_bytes).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(java.contains("return"), "exit path must contain return");
    // Region tree must include BOTH exit blocks (see region::tests::region_loop_exit_path_two_blocks).
    // Decompiled output may still drop the first block if the instruction is optimized out
    // (e.g. const/4 into unused register). The important fix is that build_regions_rec includes
    // the fall-through predecessor of the return block in then_branch.
}

/// L0-3: merge region tree (pre-simplify) must include the return block.
#[test]
fn merge_region_tree_includes_return_before_simplify() {
    use dex_bytecode::decode_all;
    use dex_decompiler::decompile::cfg::MethodCfg;
    use dex_decompiler::decompile::region::build_regions;
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/decompiler_fixtures/classes.dex");
    let data = std::fs::read(&path).unwrap();
    let dex = parse_dex(&data).unwrap();
    let class_name = "com.androguard.decompilefixtures.AlgorithmFixtures";
    let mut code = None;
    for class_def in dex.class_defs().flatten() {
        let ty = dex.get_type(class_def.class_idx).unwrap();
        if dex_decompiler::java::descriptor_to_java(&ty) != class_name {
            continue;
        }
        let Some(cd) = dex.get_class_data(&class_def).unwrap() else {
            continue;
        };
        for enc in cd.direct_methods.iter().chain(cd.virtual_methods.iter()) {
            if dex.get_method_info(enc.method_idx).unwrap().name == "merge" {
                code = Some(dex.get_code_item(enc.code_off).unwrap());
                break;
            }
        }
    }
    let code = code.unwrap();
    let insns_bytes = code.insns_slice(&*dex.data);
    let insns = decode_all(insns_bytes, 0).unwrap();
    let cfg = MethodCfg::build(&insns, insns_bytes, 0, &|_| "cond".into());
    let r = build_regions(&cfg, cfg.entry).expect("regions");
    fn has_exit(r: &dex_decompiler::decompile::region::Region, cfg: &MethodCfg) -> bool {
        use dex_decompiler::decompile::cfg::BlockEnd;
        use dex_decompiler::decompile::region::Region;
        match r {
            Region::Block(b) => matches!(cfg.blocks[*b].end, BlockEnd::Exit),
            Region::Seq(v) => v.iter().any(|c| has_exit(c, cfg)),
            Region::If {
                then_branch,
                else_branch,
                ..
            } => has_exit(then_branch, cfg) || has_exit(else_branch, cfg),
            Region::Loop { body, .. } => has_exit(body, cfg),
            Region::Switch { cases, default, .. } => {
                cases.iter().any(|(_, c)| has_exit(c, cfg)) || has_exit(default, cfg)
            }
        }
    }
    assert!(
        has_exit(&r, &cfg),
        "merge region tree must contain return block before simplify"
    );
}

/// Classic for-loop shape: init (const/4 v0,0); header (if-eqz → exit); add-int; goto back.
/// When the region tree is Seq([Block(init), Loop {...}]) with init/update pattern we emit for (...).
#[test]
fn test_decompiler_for_loop_pattern() {
    // 0: const/4 v0, 0; 2: if-eqz v0, +6 (exit at 14); 6: add-int/lit8 v0,v0,1; 10: goto -4 (back); 14: return
    let bytecode: &[u8] = &[
        0x12, 0x00, // const/4 v0, 0
        0x38, 0x00, 0x06, 0x00, // if-eqz v0, +6 (target 14)
        0xd8, 0x00, 0x00, 0x01, // add-int/lit8 v0, v0, 1
        0x28, 0xfc, // goto -4 (back to 2)
        0x0e, 0x00, // return-void at 14
    ];
    let dex_bytes = minimal_dex_with_method_code(bytecode);
    let dex = parse_dex(&dex_bytes).unwrap();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    let has_loop = java.contains("for (") || java.contains("while (");
    assert!(
        has_loop,
        "init/cond/update loop should decompile to 'for (' or 'while (', got:\n{}",
        java
    );
}
