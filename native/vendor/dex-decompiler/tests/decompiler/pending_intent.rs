//! Integration tests for PendingIntent vulnerability scan.
//!
//! Uses minimal DEX with method bytecode to ensure scan_pending_intents works
//! end-to-end with the full decompiler pipeline.

use dex_decompiler::{parse_dex, scan_pending_intents, Decompiler, ValueFlowAnalysisOwned};

fn get_first_encoded_method(
    dex_bytes: &[u8],
) -> Option<(dex_parser::EncodedMethod, String, String)> {
    let dex = parse_dex(dex_bytes).ok()?;
    let class_def = dex.class_defs().next()?.ok()?;
    let class_type = dex.get_type(class_def.class_idx).ok()?;
    let class_name = dex_decompiler::java::descriptor_to_java(&class_type);
    let class_data = dex.get_class_data(&class_def).ok()??;
    let encoded = class_data.direct_methods.first().cloned()?;
    let method_info = dex.get_method_info(encoded.method_idx).ok()?;
    let method_name = method_info.name.to_string();
    Some((encoded, class_name, method_name))
}

/// Bytecode: const/4 v0,0; move v1,v0; return v1 (no PendingIntent).
fn bytecode_no_pending_intent() -> Vec<u8> {
    vec![
        0x12, 0x00, // const/4 v0, 0
        0x04, 0x10, // move v1, v0
        0x0f, 0x10, // return v1
    ]
}

#[test]
fn scan_returns_empty_when_method_has_no_pending_intent() {
    let dex_bytes = super::helpers::minimal_dex_with_method_code(&bytecode_no_pending_intent());
    let (encoded, class_name, method_name) =
        get_first_encoded_method(&dex_bytes).expect("one direct method");
    let dex = parse_dex(&dex_bytes).expect("parse");
    let decompiler = Decompiler::new(&dex);
    let owned: ValueFlowAnalysisOwned = decompiler
        .value_flow_analysis(&encoded)
        .expect("value_flow_analysis");

    let findings = scan_pending_intents(&owned, &class_name, &method_name);
    assert!(
        findings.is_empty(),
        "method with no PendingIntent should yield no findings, got {:?}",
        findings
    );
}
