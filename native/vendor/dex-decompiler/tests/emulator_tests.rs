//! Tests for the bytecode emulator.

use dex_decompiler::emulator::state::*;

fn make_ins(index: usize, offset: u32, mnemonic: &str, operands: &str) -> InstructionInfo {
    InstructionInfo {
        index,
        offset,
        mnemonic: mnemonic.to_string(),
        operands: operands.to_string(),
    }
}

#[test]
fn emulator_const_and_add() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, 0x5"),
        make_ins(1, 2, "const/4", "v1, 0x3"),
        make_ins(2, 4, "add-int", "v2, v0, v1"),
        make_ins(3, 6, "return", "v2"),
    ];
    let resolved = instructions.iter().map(|i| i.operands.clone()).collect();
    let mut emu = Emulator::new(instructions, resolved, 3, 0, true, vec![]);
    emu.step().unwrap(); // const/4 v0, 5
    assert_eq!(emu.registers[0], Value::Int(5));
    emu.step().unwrap(); // const/4 v1, 3
    assert_eq!(emu.registers[1], Value::Int(3));
    emu.step().unwrap(); // add-int v2, v0, v1
    assert_eq!(emu.registers[2], Value::Int(8));
    emu.step().unwrap(); // return v2
    assert!(emu.finished);
    assert_eq!(emu.return_value, Some(Value::Int(8)));
}

#[test]
fn emulator_branch_taken() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, 0x0"),
        make_ins(1, 2, "if-eqz", "v0, 0x2"), // branch +2 code units = +4 bytes → offset 6
        make_ins(2, 4, "const/4", "v1, 0x1"), // skipped
        make_ins(3, 6, "const/4", "v1, 0x2"), // target
        make_ins(4, 8, "return-void", ""),
    ];
    let resolved = instructions.iter().map(|i| i.operands.clone()).collect();
    let mut emu = Emulator::new(instructions, resolved, 2, 0, true, vec![]);
    emu.step().unwrap(); // v0 = 0
    emu.step().unwrap(); // if-eqz v0, +2 code units → taken (v0 == 0)
    assert_eq!(emu.pc, 3); // jumped to index 3 (offset 6)
    emu.step().unwrap(); // v1 = 2
    assert_eq!(emu.registers[1], Value::Int(2));
}

#[test]
fn emulator_branch_not_taken() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, 0x1"),
        make_ins(1, 2, "if-eqz", "v0, 0x2"), // branch +2 code units = +4 bytes
        make_ins(2, 4, "const/4", "v1, 0xa"), // NOT skipped
        make_ins(3, 6, "const/4", "v1, 0x14"),
        make_ins(4, 8, "return-void", ""),
    ];
    let resolved = instructions.iter().map(|i| i.operands.clone()).collect();
    let mut emu = Emulator::new(instructions, resolved, 2, 0, true, vec![]);
    emu.step().unwrap(); // v0 = 1
    emu.step().unwrap(); // if-eqz v0 → not taken
    assert_eq!(emu.pc, 2); // falls through
    emu.step().unwrap(); // v1 = 10
    assert_eq!(emu.registers[1], Value::Int(10));
}

#[test]
fn emulator_array_operations() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, 0x3"),      // size=3
        make_ins(1, 2, "new-array", "v1, v0, [I"), // v1 = new int[3]
        make_ins(2, 4, "const/4", "v2, 0x7"),      // value=7
        make_ins(3, 6, "const/4", "v3, 0x1"),      // index=1
        make_ins(4, 8, "aput", "v2, v1, v3"),      // v1[1] = 7
        make_ins(5, 10, "aget", "v4, v1, v3"),     // v4 = v1[1]
        make_ins(6, 12, "return", "v4"),
    ];
    let resolved = vec![
        "v0, 0x3".into(),
        "v1, v0, [I".into(),
        "v2, 0x7".into(),
        "v3, 0x1".into(),
        "v2, v1, v3".into(),
        "v4, v1, v3".into(),
        "v4".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 5, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert!(emu.finished);
    assert_eq!(emu.return_value, Some(Value::Int(7)));
    assert_eq!(emu.heap.len(), 1);
    if let HeapObjectKind::Array { ref values, .. } = emu.heap[0].kind {
        assert_eq!(values[1], Value::Int(7));
    } else {
        panic!("expected array on heap");
    }
}

#[test]
fn emulator_parameter_registers() {
    // Static method with registers_size=3, ins_size=2 → p0=v1, p1=v2
    let instructions = vec![
        make_ins(0, 0, "add-int", "v0, v1, v2"), // v0 = p0 + p1
        make_ins(1, 2, "return", "v0"),
    ];
    let resolved = instructions.iter().map(|i| i.operands.clone()).collect();
    let params = vec![Value::Int(10), Value::Int(20)];
    let mut emu = Emulator::new(instructions, resolved, 3, 2, true, params);
    assert_eq!(emu.registers[1], Value::Int(10)); // p0 = v1
    assert_eq!(emu.registers[2], Value::Int(20)); // p1 = v2
    emu.run_to_end().unwrap();
    assert_eq!(emu.return_value, Some(Value::Int(30)));
}

#[test]
fn emulator_division_by_zero() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, 0xa"),
        make_ins(1, 2, "const/4", "v1, 0x0"),
        make_ins(2, 4, "div-int", "v2, v0, v1"),
    ];
    let resolved = instructions.iter().map(|i| i.operands.clone()).collect();
    let mut emu = Emulator::new(instructions, resolved, 3, 0, true, vec![]);
    emu.step().unwrap();
    emu.step().unwrap();
    let result = emu.step();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("division by zero"));
}

#[test]
fn emulator_array_index_out_of_bounds() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, 0x2"), // size=2
        make_ins(1, 2, "new-array", "v1, v0, [I"),
        make_ins(2, 4, "const/4", "v2, 0x5"), // index=5 (out of bounds)
        make_ins(3, 6, "aget", "v3, v1, v2"),
    ];
    let resolved = vec![
        "v0, 0x2".into(),
        "v1, v0, [I".into(),
        "v2, 0x5".into(),
        "v3, v1, v2".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 4, 0, true, vec![]);
    emu.step().unwrap();
    emu.step().unwrap();
    emu.step().unwrap();
    let result = emu.step();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("array index out of bounds"));
}

#[test]
fn emulator_snapshot_and_history() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, 0x1"),
        make_ins(1, 2, "const/4", "v1, 0x2"),
        make_ins(2, 4, "return-void", ""),
    ];
    let resolved = instructions.iter().map(|i| i.operands.clone()).collect();
    let mut emu = Emulator::new(instructions, resolved, 2, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.history.len(), 3);
    assert_eq!(emu.history[0].step, 1);
    assert_eq!(emu.history[0].instruction.mnemonic, "const/4");
    let snap = emu.snapshot();
    assert!(snap.finished);
    assert_eq!(snap.step_count, 3);
}

#[test]
fn emulator_reset() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, 0x5"),
        make_ins(1, 2, "return", "v0"),
    ];
    let resolved = instructions.iter().map(|i| i.operands.clone()).collect();
    let mut emu = Emulator::new(instructions, resolved, 1, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert!(emu.finished);
    emu.reset(vec![]);
    assert!(!emu.finished);
    assert_eq!(emu.pc, 0);
    assert_eq!(emu.step_count, 0);
    assert!(emu.history.is_empty());
}

// ========== API Stub Tests ==========

/// StringBuilder: init → append → append → toString chain.
#[test]
fn stub_string_builder_chain() {
    let instructions = vec![
        make_ins(0, 0, "new-instance", "v0, java.lang.StringBuilder"),
        make_ins(1, 4, "invoke-direct", "v0, method@100"),
        make_ins(2, 10, "const-string", "v1, string@200"),
        make_ins(3, 14, "invoke-virtual", "v0, v1, method@101"),
        make_ins(4, 20, "move-result-object", "v0"),
        make_ins(5, 22, "const-string", "v2, string@201"),
        make_ins(6, 26, "invoke-virtual", "v0, v2, method@102"),
        make_ins(7, 32, "move-result-object", "v0"),
        make_ins(8, 34, "invoke-virtual", "v0, method@103"),
        make_ins(9, 40, "move-result-object", "v3"),
        make_ins(10, 42, "return-void", ""),
    ];
    let resolved = vec![
        "v0, java.lang.StringBuilder".into(),
        "v0, java.lang.StringBuilder.<init>()".into(),
        "v1, \"Hello\"".into(),
        "v0, v1, java.lang.StringBuilder.append(java.lang.String)".into(),
        "v0".into(),
        "v2, \" World\"".into(),
        "v0, v2, java.lang.StringBuilder.append(java.lang.String)".into(),
        "v0".into(),
        "v0, java.lang.StringBuilder.toString()".into(),
        "v3".into(),
        "".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 4, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.registers[3], Value::Str("Hello World".into()));
}

/// PrintStream.println captures to console_output.
#[test]
fn stub_println_captures_output() {
    let instructions = vec![
        make_ins(0, 0, "sget-object", "v0, field@100"),
        make_ins(1, 4, "const-string", "v1, string@200"),
        make_ins(2, 8, "invoke-virtual", "v0, v1, method@300"),
        make_ins(3, 14, "return-void", ""),
    ];
    let resolved = vec![
        "v0, java.lang.System.out".into(),
        "v1, \"test message\"".into(),
        "v0, v1, java.io.PrintStream.println(java.lang.String)".into(),
        "".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 2, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.console_output.len(), 1);
    assert!(emu.console_output[0].contains("test message"));
}

/// String.length stub returns correct length.
#[test]
fn stub_string_length() {
    let instructions = vec![
        make_ins(0, 0, "const-string", "v0, string@100"),
        make_ins(1, 4, "invoke-virtual", "v0, method@200"),
        make_ins(2, 10, "move-result", "v1"),
        make_ins(3, 12, "return", "v1"),
    ];
    let resolved = vec![
        "v0, \"hello\"".into(),
        "v0, java.lang.String.length()".into(),
        "v1".into(),
        "v1".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 2, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.return_value, Some(Value::Int(5)));
}

/// Math.max and Math.abs stubs.
#[test]
fn stub_math_operations() {
    let instructions = vec![
        make_ins(0, 0, "const/4", "v0, -0x5"),
        make_ins(1, 2, "const/4", "v1, 0x3"),
        make_ins(2, 4, "invoke-static", "v0, v1, method@100"),
        make_ins(3, 10, "move-result", "v2"),
        make_ins(4, 12, "invoke-static", "v0, method@101"),
        make_ins(5, 18, "move-result", "v3"),
        make_ins(6, 20, "return-void", ""),
    ];
    let resolved = vec![
        "v0, -0x5".into(),
        "v1, 0x3".into(),
        "v0, v1, java.lang.Math.max(int, int)".into(),
        "v2".into(),
        "v0, java.lang.Math.abs(int)".into(),
        "v3".into(),
        "".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 4, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.registers[2], Value::Int(3)); // max(-5, 3) = 3
    assert_eq!(emu.registers[3], Value::Int(5)); // abs(-5) = 5
}

/// Integer.parseInt stub.
#[test]
fn stub_integer_parseint() {
    let instructions = vec![
        make_ins(0, 0, "const-string", "v0, string@100"),
        make_ins(1, 4, "invoke-static", "v0, method@200"),
        make_ins(2, 10, "move-result", "v1"),
        make_ins(3, 12, "return", "v1"),
    ];
    let resolved = vec![
        "v0, \"42\"".into(),
        "v0, java.lang.Integer.parseInt(java.lang.String)".into(),
        "v1".into(),
        "v1".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 2, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.return_value, Some(Value::Int(42)));
}

/// String.equals stub returns correct boolean.
#[test]
fn stub_string_equals() {
    let instructions = vec![
        make_ins(0, 0, "const-string", "v0, string@100"),
        make_ins(1, 4, "const-string", "v1, string@101"),
        make_ins(2, 8, "invoke-virtual", "v0, v1, method@200"),
        make_ins(3, 14, "move-result", "v2"),
        make_ins(4, 16, "const-string", "v3, string@102"),
        make_ins(5, 20, "invoke-virtual", "v0, v3, method@201"),
        make_ins(6, 26, "move-result", "v4"),
        make_ins(7, 28, "return-void", ""),
    ];
    let resolved = vec![
        "v0, \"abc\"".into(),
        "v1, \"abc\"".into(),
        "v0, v1, java.lang.String.equals(java.lang.Object)".into(),
        "v2".into(),
        "v3, \"xyz\"".into(),
        "v0, v3, java.lang.String.equals(java.lang.Object)".into(),
        "v4".into(),
        "".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 5, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.registers[2], Value::Int(1)); // "abc" == "abc" → true
    assert_eq!(emu.registers[4], Value::Int(0)); // "abc" == "xyz" → false
}

/// Android Log.d stub records output and returns 0.
#[test]
fn stub_android_log() {
    let instructions = vec![
        make_ins(0, 0, "const-string", "v0, string@100"),
        make_ins(1, 4, "const-string", "v1, string@101"),
        make_ins(2, 8, "invoke-static", "v0, v1, method@200"),
        make_ins(3, 14, "move-result", "v2"),
        make_ins(4, 16, "return-void", ""),
    ];
    let resolved = vec![
        "v0, \"MyTag\"".into(),
        "v1, \"debug message\"".into(),
        "v0, v1, android.util.Log.d(java.lang.String, java.lang.String)".into(),
        "v2".into(),
        "".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 3, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.registers[2], Value::Int(0));
    assert_eq!(emu.console_output.len(), 1);
    assert!(emu.console_output[0].contains("MyTag"));
    assert!(emu.console_output[0].contains("debug message"));
}

/// String operations: contains, startsWith, trim, toUpperCase
#[test]
fn stub_string_operations() {
    let instructions = vec![
        make_ins(0, 0, "const-string", "v0, string@100"),
        // contains
        make_ins(1, 4, "const-string", "v1, string@101"),
        make_ins(2, 8, "invoke-virtual", "v0, v1, method@200"),
        make_ins(3, 14, "move-result", "v2"),
        // toUpperCase
        make_ins(4, 16, "invoke-virtual", "v0, method@201"),
        make_ins(5, 22, "move-result-object", "v3"),
        // trim (" hello " → "hello")
        make_ins(6, 24, "const-string", "v4, string@102"),
        make_ins(7, 28, "invoke-virtual", "v4, method@202"),
        make_ins(8, 34, "move-result-object", "v5"),
        make_ins(9, 36, "return-void", ""),
    ];
    let resolved = vec![
        "v0, \"hello world\"".into(),
        "v1, \"world\"".into(),
        "v0, v1, java.lang.String.contains(java.lang.CharSequence)".into(),
        "v2".into(),
        "v0, java.lang.String.toUpperCase()".into(),
        "v3".into(),
        "v4, \"  trimmed  \"".into(),
        "v4, java.lang.String.trim()".into(),
        "v5".into(),
        "".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 6, 0, true, vec![]);
    emu.run_to_end().unwrap();
    assert_eq!(emu.registers[2], Value::Int(1)); // "hello world".contains("world") → true
    assert_eq!(emu.registers[3], Value::Str("HELLO WORLD".into()));
    assert_eq!(emu.registers[5], Value::Str("trimmed".into()));
}

/// Unknown invokes set an Unknown result (not panic).
#[test]
fn stub_unknown_invoke_produces_unknown_result() {
    let instructions = vec![
        make_ins(0, 0, "invoke-virtual", "v0, method@999"),
        make_ins(1, 6, "move-result-object", "v1"),
        make_ins(2, 8, "return-void", ""),
    ];
    let resolved = vec![
        "v0, com.example.Unknown.foo()".into(),
        "v1".into(),
        "".into(),
    ];
    let mut emu = Emulator::new(instructions, resolved, 2, 0, true, vec![]);
    emu.run_to_end().unwrap();
    if let Value::Unknown(ref s) = emu.registers[1] {
        assert!(s.contains("com.example.Unknown.foo"));
    } else {
        panic!(
            "expected Unknown value for unrecognized method, got {:?}",
            emu.registers[1]
        );
    }
}

/// End-to-end: create emulator from actual DEX method if testdata exists.
#[test]
fn emulator_from_dex_method() {
    use std::path::Path;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/androguard_test_classes.dex");
    if !path.exists() {
        return;
    }
    let data = std::fs::read(&path).unwrap();
    let dex = dex_decompiler::parse_dex(&data).unwrap();
    let options = dex_decompiler::DecompilerOptions {
        only_package: Some("tests.androguard".to_string()),
        exclude: vec![],
        ..Default::default()
    };
    let dc = dex_decompiler::Decompiler::with_options(&dex, options);

    // Find testException2(int, int)
    for class_def_result in dex.class_defs() {
        let class_def = match class_def_result {
            Ok(c) => c,
            Err(_) => continue,
        };
        let class_data = match dex.get_class_data(&class_def) {
            Ok(Some(cd)) => cd,
            _ => continue,
        };
        for encoded in class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
        {
            let info = match dex.get_method_info(encoded.method_idx) {
                Ok(mi) => mi,
                Err(_) => continue,
            };
            if info.name != "testException2" {
                continue;
            }

            let params = vec![
                dex_decompiler::emulator::Value::Int(5),
                dex_decompiler::emulator::Value::Int(3),
            ];
            let mut emu = dc.build_emulator(encoded, params, vec![]).unwrap();
            assert!(!emu.instructions.is_empty());
            assert!(!emu.finished);

            // Step a few times — should not panic
            for _ in 0..5 {
                if emu.finished {
                    break;
                }
                let _ = emu.step();
            }
            assert!(emu.step_count > 0);
            return;
        }
    }
}
