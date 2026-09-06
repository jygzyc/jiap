//! Native DECX core — a pure-Rust replacement for the JVM-based Kotlin
//! `decx-core`, built on the vendored dexdec engine (dex/apk) plus a JVM
//! class-file decoder for standard-java archives and `android.jar`.
//!
//! Layers mirror the Kotlin side:
//! - [`project`] ≈ JADX decompiler state: loaded dexes/apk + class index + lazy
//!   decompilation with a byte-bounded LRU source cache (≈ `DecompileGuard`)
//! - [`api`]     ≈ `DecxApi`/`RouteHandler`: one dispatcher from endpoint name to result
//! - [`error`]   ≈ `DecxError`: structured error codes shared with the HTTP layer
//! - [`java_archive`] / [`arsc`] / [`axml`] / [`manifest`] = format decoders

pub mod api;
pub mod arsc;
pub mod axml;
pub mod envelope;
pub mod error;
pub mod java_archive;
pub mod manifest;
pub mod names;
pub mod project;

pub use error::{DecxError, Result};
pub use java_archive::JavaArchive;
pub use project::{ClassEntry, Project, ProjectKind, SourceCache};
