//! decx-kuna: the code-level kuna engine.
//!
//! Historically the CLI spawned an external `kuna` binary; kuna is now
//! imported at the code level: the analyzer lives in this crate and is served
//! through the DECX HTTP contract by `decx-kuna-server`, built with the
//! workspace (`cargo build --release` compiles the CLI and every engine
//! server together).

pub mod analyzer;
pub mod service;

pub use analyzer::Analyzer;
pub use service::KunaService;
