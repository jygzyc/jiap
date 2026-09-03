//! SQLCipher / Room / SQLite open with possible hardcoded passphrase.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, VulnFinding};

const OPEN_APIS: &[&str] = &[
    "SQLiteDatabase.openOrCreateDatabase",
    "openOrCreateDatabase",
    "net.sqlcipher",
    "SupportFactory",
    "SQLiteOpenHelper.getWritableDatabase",
    "Room.databaseBuilder",
    "Room.inMemoryDatabaseBuilder",
];

fn passphrase_hint(owned: &ValueFlowAnalysisOwned) -> bool {
    owned.insn_at.values().any(|s| {
        let l = s.to_lowercase();
        (l.contains("const-string") || l.contains("\""))
            && (l.contains("password")
                || l.contains("passphrase")
                || l.contains("sqlcipher")
                || l.contains("secret")
                || l.contains("pragma key"))
    })
}

pub fn scan_sqlcipher_passphrase(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = invoke_scan(
        owned,
        class_name,
        method_name,
        "sqlcipher_hardcoded_passphrase",
        OPEN_APIS,
    );
    if passphrase_hint(owned) {
        if out.is_empty() {
            out.extend(invoke_scan(
                owned,
                class_name,
                method_name,
                "sqlcipher_hardcoded_passphrase",
                &[
                    "openOrCreateDatabase",
                    "getWritableDatabase",
                    "SupportFactory",
                ],
            ));
        }
    } else {
        // Without passphrase/string hints, drop Room.databaseBuilder noise.
        out.retain(|f| {
            f.sink_desc.contains("sqlcipher")
                || f.sink_desc.contains("SupportFactory")
                || f.sink_desc.contains("openOrCreateDatabase")
        });
    }
    out
}
