//! Native DECX core — replaces the JADX-based Kotlin `decx-core` with a pure-Rust
//! engine (vendored from androguard `dex-parser` / `dex-bytecode` / `dex-decompiler`).
//!
//! Layers mirror the Kotlin side:
//! - [`project`] ≈ JADX decompiler state: loaded dexes/apk + class index + lazy
//!   decompilation with a byte-bounded LRU source cache (≈ `DecompileGuard`)
//! - [`api`]     ≈ `DecxApi`/`RouteHandler`: one dispatcher from endpoint name to result
//! - [`error`]   ≈ `DecxError`: structured error codes shared with the HTTP layer

pub mod api;
pub mod error;
pub mod names;
pub mod project;

pub use error::{DecxError, Result};
pub use project::{ClassEntry, FieldEntry, MethodEntry, Project, SourceCache};
