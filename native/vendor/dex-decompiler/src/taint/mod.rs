//! Mariana-Trench–inspired global taint solver for Dalvik/DEX.
//!
//! Features (MT-aligned):
//! - JSON **models**: sources, sinks, propagations (TITO), sanitizers
//! - **Rules**: source-kind → sink-kind issue matching
//! - **Call graph** + interprocedural fixpoint over method summaries
//! - **Traces** (source → root → sink) and JSON issue reports
//!
//! Built on the existing intraprocedural [`crate::decompile::value_flow`] engine.

mod call_graph;
mod config;
mod defaults;
mod index;
mod issue;
mod models;
mod mt_compat;
mod report;
mod solver;

pub use call_graph::{CallEdge, CallGraph};
pub use config::TaintConfig;
pub use defaults::default_config;
pub use index::{MethodId, MethodIndex, MethodRef};
pub use issue::{Issue, TraceFrame};
pub use models::{
    AccessPath, Port, PropagationModel, Rule, SanitizerModel, SinkModel, SourceModel,
};
pub use mt_compat::{
    convert_models_json, convert_rules_json, load_mt_case_config, method_patterns, parse_mt_port,
};
pub use report::{write_issues_json, IssueReport};
pub use solver::{solve_dex, solve_dexes, SolveOptions, SolveResult};
