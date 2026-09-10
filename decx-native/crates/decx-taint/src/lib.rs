//! decx-taint: inter-procedural taint tracking for Android DEX code
//! (Mariana-Trench style: per-method summaries over a bounded fixpoint).
//!
//! Pipeline: `model` (rule set) → `body` (DEX → taint-oriented IR) →
//! `intra` (per-method abstract interpretation producing summaries +
//! local findings) → `solver` (global fixpoint over summaries, static
//! fields, findings).

pub mod body;
pub mod dispatch;
pub mod intra;
pub mod model;
pub mod solver;

pub use body::MethodBody;
pub use dispatch::DispatchIndex;
pub use intra::{Finding, Origin, Summary, Token};
pub use model::{PropMode, Rules};
pub use solver::{scan, ScanOptions, ScanReport};

use decx_core::Project;
use decx_json::Json;

/// scan with the given rule set; returns findings as `decx_json` items:
/// each item = { source, sink, route:[{method,pc,role}...], severity }
pub fn scan_to_json(project: &Project, opts: &ScanOptions, rules: &Rules) -> Json {
    let rep = scan(project, rules, opts);

    let mut items: Vec<Json> = Vec::new();
    for f in &rep.findings {
        items.push(finding_to_json(rules, f));
    }

    let meta = Json::obj(vec![
        ("rounds_run", Json::int(rep.rounds_run as i64)),
        ("methods_analyzed", Json::int(rep.methods_analyzed as i64)),
        ("methods_total", Json::int(rep.methods_total as i64)),
        ("static_fields_tracked", Json::int(rep.static_fields_tracked as i64)),
        ("truncated", Json::Bool(rep.truncated)),
    ]);
    Json::obj(vec![("items", Json::Arr(items)), ("meta", meta)])
}

pub fn finding_to_json(rules: &Rules, f: &Finding) -> Json {
    // route = [source site] + hops (call edges) + [sink site]
    let mut route: Vec<Json> = Vec::with_capacity(f.hops.len() + 2);
    route.push(Json::obj(vec![
        ("method", Json::str(&f.src_method)),
        ("pc", Json::int(f.src_pc as i64)),
        ("role", Json::str("source")),
    ]));
    for (m, pc) in &f.hops {
        route.push(Json::obj(vec![
            ("method", Json::str(m)),
            ("pc", Json::int(*pc as i64)),
            ("role", Json::str("call")),
        ]));
    }
    route.push(Json::obj(vec![
        ("method", Json::str(&f.sink_method)),
        ("pc", Json::int(f.sink_pc as i64)),
        ("role", Json::str("sink")),
    ]));

    Json::obj(vec![
        (
            "source",
            Json::obj(vec![
                ("method", Json::str(&f.src_method)),
                ("pc", Json::int(f.src_pc as i64)),
                ("api", Json::str(&f.src_api)),
            ]),
        ),
        (
            "sink",
            Json::obj(vec![
                ("method", Json::str(&f.sink_method)),
                ("pc", Json::int(f.sink_pc as i64)),
                ("api", Json::str(&f.sink_api)),
            ]),
        ),
        ("route", Json::Arr(route)),
        (
            "severity",
            Json::str(model::severity_for(&rules.sources[f.src_rule].label, &rules.sinks[f.sink_rule].label)),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rules_parse_nonempty() {
        let r = Rules::default_rules();
        assert!(!r.sources.is_empty());
        assert!(!r.sinks.is_empty());
        assert!(!r.propagators.is_empty());
        assert!(!r.sanitizers.is_empty());
        assert!(r.sources.iter().any(|e| e.kind == "field"));
    }
}
