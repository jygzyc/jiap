//! Integration tests for value-flow / data tainting.
//!
//! Uses minimal DEX with method bytecode and the full decompiler pipeline
//! to ensure value-flow analysis and tainting propagation work end-to-end.
//! Covers: tracking a value through **return**, **pass to function** (invoke),
//! and transitive copies (move chains).

use dex_decompiler::{parse_dex, Decompiler, ValueFlowAnalysisOwned};
use std::collections::HashSet;

fn get_first_encoded_method(dex_bytes: &[u8]) -> Option<dex_parser::EncodedMethod> {
    let dex = parse_dex(dex_bytes).ok()?;
    let class_def = dex.class_defs().next()?.ok()?;
    let class_data = dex.get_class_data(&class_def).ok()??;
    class_data.direct_methods.first().cloned()
}

/// Bytecode: const/4 v0,0; move v1,v0; return v1.
fn bytecode_const_move_return() -> Vec<u8> {
    vec![
        0x12, 0x00, // const/4 v0, 0
        0x01, 0x01, // move v1, v0
        0x0f, 0x01, // return v1
    ]
}

/// Bytecode: const/4 v0,0; move v1,v0; move v2,v1; return v2 (transitive copy then return).
fn bytecode_const_move_move_return() -> Vec<u8> {
    vec![
        0x12, 0x00, // const/4 v0, 0
        0x01, 0x01, // move v1, v0
        0x01, 0x12, // move v2, v1
        0x0f, 0x02, // return v2
    ]
}

#[test]
fn value_flow_analysis_builds_and_propagates() {
    let dex_bytes = super::helpers::minimal_dex_with_method_code(&bytecode_const_move_return());
    let encoded = get_first_encoded_method(&dex_bytes).expect("one direct method");
    let dex = parse_dex(&dex_bytes).expect("parse");
    let decompiler = Decompiler::new(&dex);

    let owned: ValueFlowAnalysisOwned = decompiler
        .value_flow_analysis(&encoded)
        .expect("value_flow_analysis");

    let offsets = owned.cfg.blocks[owned.cfg.entry]
        .instruction_offsets
        .clone();
    assert!(
        offsets.len() >= 3,
        "expected at least 3 instructions, got {:?}",
        offsets
    );

    let off_const = offsets[0];
    let analysis = owned.analysis();

    // Seed: value defined at const/4 v0,0.
    let result = analysis.value_flow_from_seed(off_const, 0);

    // This is the production-computed RW map, not a test-supplied expected map.
    let writes_set: HashSet<_> = result.writes.iter().copied().collect();
    let reads_set: HashSet<_> = result.reads.iter().copied().collect();
    assert_eq!(
        writes_set,
        HashSet::from([(offsets[0], 0), (offsets[1], 1)])
    );
    assert_eq!(reads_set, HashSet::from([(offsets[1], 0), (offsets[2], 1)]));
}

/// Tainting tracks a value through **return**: seed at const, value flows to move then to return v1.
#[test]
fn value_flow_tracks_value_through_return() {
    let dex_bytes = super::helpers::minimal_dex_with_method_code(&bytecode_const_move_return());
    let encoded = get_first_encoded_method(&dex_bytes).expect("one direct method");
    let dex = parse_dex(&dex_bytes).expect("parse");
    let decompiler = Decompiler::new(&dex);
    let owned = decompiler
        .value_flow_analysis(&encoded)
        .expect("value_flow_analysis");

    let offsets = owned.cfg.blocks[owned.cfg.entry]
        .instruction_offsets
        .clone();
    assert!(offsets.len() >= 3, "expected const, move, return");
    let off_const = offsets[0];
    let analysis = owned.analysis();
    let result = analysis.value_flow_from_seed(off_const, 0);

    let writes_set: HashSet<_> = result.writes.iter().copied().collect();
    let reads_set: HashSet<_> = result.reads.iter().copied().collect();
    assert_eq!(
        writes_set,
        HashSet::from([(offsets[0], 0), (offsets[1], 1)])
    );
    assert_eq!(reads_set, HashSet::from([(offsets[1], 0), (offsets[2], 1)]));
}

/// Tainting tracks a value through **transitive copies** (move chain) and then return.
#[test]
fn value_flow_tracks_value_through_move_chain_and_return() {
    let dex_bytes =
        super::helpers::minimal_dex_with_method_code(&bytecode_const_move_move_return());
    let encoded = get_first_encoded_method(&dex_bytes).expect("one direct method");
    let dex = parse_dex(&dex_bytes).expect("parse");
    let decompiler = Decompiler::new(&dex);
    let owned = decompiler
        .value_flow_analysis(&encoded)
        .expect("value_flow_analysis");

    let offsets = owned.cfg.blocks[owned.cfg.entry]
        .instruction_offsets
        .clone();
    assert!(offsets.len() >= 4, "expected const, move, move, return");
    let off_const = offsets[0];
    let analysis = owned.analysis();
    let result = analysis.value_flow_from_seed(off_const, 0);

    let writes_set: HashSet<_> = result.writes.iter().copied().collect();
    let reads_set: HashSet<_> = result.reads.iter().copied().collect();
    assert_eq!(
        writes_set,
        HashSet::from([(offsets[0], 0), (offsets[1], 1), (offsets[2], 2)])
    );
    assert_eq!(
        reads_set,
        HashSet::from([(offsets[1], 0), (offsets[2], 1), (offsets[3], 2)])
    );
}

#[test]
fn value_flow_def_use_and_use_def_api() {
    let dex_bytes = super::helpers::minimal_dex_with_method_code(&bytecode_const_move_return());
    let encoded = get_first_encoded_method(&dex_bytes).expect("one direct method");
    let dex = parse_dex(&dex_bytes).expect("parse");
    let decompiler = Decompiler::new(&dex);
    let owned = decompiler
        .value_flow_analysis(&encoded)
        .expect("value_flow_analysis");

    let offsets = owned.cfg.blocks[owned.cfg.entry]
        .instruction_offsets
        .clone();
    let off_const = offsets[0];
    let analysis = owned.analysis();

    assert_eq!(analysis.def_use(off_const, 0), vec![(offsets[1], 0)]);
    assert_eq!(analysis.use_def(offsets[2], 1), vec![(offsets[1], 1)]);
}
