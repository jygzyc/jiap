//! decx-cli-core — the DECX CLI engine room.
//!
//! This crate is the Rust rewrite of the TypeScript `decx-cli`. It is organized
//! around three architectural pieces (the design follows the opencli
//! <https://github.com/jackwener/opencli> adapter/registry model):
//!
//! - [`project`] — an independent project manager that tracks every analysis
//!   target (session), supervises its background server process, and reports
//!   live execution state through monitors, events, and `watch`.
//! - [`tools`] — a unified tool integration surface. Every command group
//!   (`project`, `code`, `android`, `self`, `tools`) is a [`tools::Tool`]
//!   registered in a [`tools::ToolRegistry`]; external CLI tools can plug in
//!   through `decx tools register` (opencli-style `external register`).
//! - [`engine`] — pluggable analysis backend spawn logic (`jvm`, `native`).
//!
//! Cross-cutting contracts: JSON on stdout, human notices on stderr,
//! `--format json|table`, and sysexits-style exit codes ([`error`]).

pub mod client;
pub mod config;
pub mod engine;
pub mod error;
pub mod fsx;
pub mod hash;
pub mod iface;
pub mod installer;
pub mod net;
pub mod output;
pub mod params;
pub mod ports;
pub mod session;
pub mod spawn;
pub mod tools;

pub use error::{DecxError, DecxResult};
