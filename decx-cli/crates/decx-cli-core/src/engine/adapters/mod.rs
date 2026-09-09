//! Built-in engine adapters — the manifest.
//!
//! The opencli model applied to decompiler backends: one self-contained file
//! per engine under `adapters/`, and this module is the manifest — add one
//! line to [`builtin`] to ship a new engine. Everything else (project
//! supervision, background monitoring, `project check`, `decx code` query
//! routing, exit codes) is runtime-owned; adapters never touch CLI plumbing.

pub mod jvm;
pub mod kuna;
pub mod native;

use std::sync::Arc;

use super::Engine;

/// The engine manifest: one line per adapter.
pub fn builtin() -> Vec<Arc<dyn Engine>> {
    vec![
        Arc::new(jvm::JvmEngine),
        Arc::new(native::NativeEngine),
        Arc::new(kuna::Kuna),
    ]
}
