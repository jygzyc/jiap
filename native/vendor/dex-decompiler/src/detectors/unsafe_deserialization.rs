//! Unsafe deserialization / memory-corruption parcelables.

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::detectors::types::{invoke_scan, source_sink_scan, VulnFinding};

const SER_SOURCES: &[&str] = &[
    "getSerializableExtra",
    "getParcelableExtra",
    "readSerializable",
    "readParcelable",
];
const SER_SINKS: &[&str] = &[
    "ObjectInputStream.readObject",
    "readObject",
    "ObjectInputStream.<init>",
];

const UNSAFE_INVOKE: &[&str] = &[
    "ObjectInputStream.readObject",
    "readObject",
    "MemoryCorruptionParcelable",
    "MemoryCorruptionSerializable",
    "DeleteFilesSerializable",
];

pub fn scan_unsafe_deserialization(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<VulnFinding> {
    let mut out = source_sink_scan(
        owned,
        class_name,
        method_name,
        "unsafe_deserialization",
        SER_SOURCES,
        SER_SINKS,
    );
    // Flag classes / methods that construct known-dangerous serializable types.
    if class_name.contains("MemoryCorruption")
        || class_name.contains("DeleteFilesSerializable")
        || method_name.contains("createFromParcel")
            && (class_name.contains("MemoryCorruption") || class_name.contains("DeleteFiles"))
    {
        out.extend(invoke_scan(
            owned,
            class_name,
            method_name,
            "unsafe_deserialization",
            &[
                "writeToParcel",
                "createFromParcel",
                "readObject",
                "writeObject",
            ],
        ));
    }
    out.extend(invoke_scan(
        owned,
        class_name,
        method_name,
        "unsafe_deserialization",
        UNSAFE_INVOKE,
    ));
    out
}
