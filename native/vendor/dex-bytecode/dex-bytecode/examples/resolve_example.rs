//! Example: decode Dalvik bytecode with a pluggable reference resolver.
//!
//! Run with: `cargo run -p dex-bytecode --example resolve_example`

use dex_bytecode::{decode_all_with_resolver, RefKind, ResolveRef};

/// A simple resolver that returns fake symbols for indices (e.g. from another tool's DEX parser).
struct ExampleResolver;

impl ResolveRef for ExampleResolver {
    fn resolve(&self, kind: RefKind, index: u32) -> Option<String> {
        match kind {
            RefKind::String => Some(format!("\"string#{}\"", index)),
            RefKind::Type => Some(format!("Ltype/Type{};", index)),
            RefKind::Field => Some(format!("LClass;->field{} I", index)),
            RefKind::Method => Some(format!("LClass;->method{}(I)V", index)),
            RefKind::MethodProto => Some(format!("(I)V #proto{}", index)),
            _ => None,
        }
    }
}

fn main() {
    // Minimal bytecode: const-string v0, string@5; return-void
    let bytecode: &[u8] = &[0x1a, 0x00, 0x05, 0x00, 0x0e, 0x00];

    println!("Without resolver:");
    let instructions = dex_bytecode::decode_all(bytecode, 0).unwrap();
    for ins in &instructions {
        println!(
            "  {:08x}  {} {}",
            ins.offset,
            ins.mnemonic(),
            ins.operands()
        );
    }

    println!("\nWith resolver:");
    let resolver = ExampleResolver;
    let instructions = decode_all_with_resolver(bytecode, 0, &resolver).unwrap();
    for ins in &instructions {
        println!(
            "  {:08x}  {} {}",
            ins.offset,
            ins.mnemonic(),
            ins.operands()
        );
    }
}
