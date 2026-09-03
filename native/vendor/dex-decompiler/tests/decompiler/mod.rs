//! Decompiler integration tests.
//!
//! - `equivalence`: decompilation output (minimal DEX, parse failures, optional fixtures).
//! - `control_flow`: return, if/else, while via minimal DEX with hand-crafted bytecode.
//! - `source_fidelity`: manifest-driven loop comparing DEX/APK output to Java sources.
//! - `fixture_harness` / `fixture_manifest`: shared decompile + per-method expectations.
//! - `try_catch_fixtures`: strong try/catch/finally/TWR tests (DEX metadata + Java shape).
//! - `jadx_parity`: jadx integration-test style regression (conditions/loops/switch/try/generics/inner).
//! - `dataflow`: expected A/R, def_to_loc, du, ud, group_variables (same as androguard).

mod apk_decompiler_fixtures;
mod control_flow;
mod dataflow;
mod decompiler_fixtures;
mod equivalence;
mod filters;
mod fixture_harness;
mod fixture_manifest;
mod helpers;
mod imports;
mod jadx_parity;
mod pending_intent;
mod privacy_vuln_demo;
mod renames;
mod source_fidelity;
mod taint_solver;
mod try_catch_assertions;
mod try_catch_fixtures;
mod value_flow;
