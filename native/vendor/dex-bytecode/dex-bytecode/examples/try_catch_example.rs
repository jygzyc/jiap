//! Example: try/catch entries (data provided by another tool from DEX encoded_catch_handler).
//!
//! Run with: `cargo run -p dex-bytecode --example try_catch_example`

use dex_bytecode::{format_catch_line, TryCatchEntry};

fn main() {
    // Simulate try/catch data that another tool (e.g. DEX parser) would fill from
    // encoded_catch_handler: protected 0..8, handler at 16, type Exception
    let entries = [
        TryCatchEntry {
            start_offset: 0,
            end_offset: 8,
            handler_offset: 16,
            type_index: Some(1),
        },
        TryCatchEntry {
            start_offset: 8,
            end_offset: 20,
            handler_offset: 24,
            type_index: None, // catch-all
        },
    ];

    println!("Catch lines (type names from another tool):\n");
    for entry in &entries {
        let type_name = entry.type_index.map(|_| "Ljava/lang/Exception;");
        let line = format_catch_line(entry, type_name);
        println!("  {}", line);
    }
    println!("\nCatch-all (type_name = None):");
    let line = format_catch_line(&entries[1], None);
    println!("  {}", line);
}
