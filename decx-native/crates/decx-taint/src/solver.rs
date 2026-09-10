//! Inter-procedural fixpoint driver: iterate method analyses until
//! summaries / static-field taint / findings stabilize (Mariana-Trench
//! style, bounded rounds — default 8).

use crate::body::{extract_all, MethodBody};
use crate::dispatch::DispatchIndex;
use crate::intra::{analyze, Finding, Summary, Token};
use crate::model::Rules;
use decx_core::Project;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct ScanOptions {
    /// fixpoint round cap (also bounds the longest reportable chain)
    pub max_rounds: usize,
    /// stop reporting after this many distinct findings
    pub max_findings: usize,
    /// skip method bodies larger than this many code units
    pub max_insns: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            max_rounds: 8,
            max_findings: 50,
            max_insns: 20_000,
        }
    }
}

pub struct ScanReport {
    pub findings: Vec<Finding>,
    pub rounds_run: usize,
    pub methods_analyzed: usize,
    pub methods_total: usize,
    pub static_fields_tracked: usize,
    pub truncated: bool,
}

pub fn scan(project: &Project, rules: &Rules, opts: &ScanOptions) -> ScanReport {
    let bodies = extract_all(project, |sig| rules.owner_excluded(sig), opts.max_insns);
    let methods_total: usize = project
        .dexes
        .iter()
        .map(|d| {
            d.class_defs
                .iter()
                .map(|def| d.class_data(def).direct_methods.len() + d.class_data(def).virtual_methods.len())
                .sum::<usize>()
        })
        .sum();
    // owner-stripped method key -> all impls; powers virtual-dispatch fan-out
    let dispatch = DispatchIndex::build(project);

    let mut summaries: BTreeMap<String, Summary> = BTreeMap::new();
    let mut statics: BTreeMap<String, BTreeSet<Token>> = BTreeMap::new();
    let mut findings: BTreeSet<Finding> = BTreeSet::new();
    let mut truncated = false;
    let mut rounds_run = 0;

    for round in 1..=opts.max_rounds.max(1) {
        rounds_run = round;
        let mut changed = false;
        for body in &bodies {
            if findings.len() >= opts.max_findings {
                truncated = true;
                break;
            }
            let out = analyze(body, rules, &summaries, &statics, &dispatch);
            let sig = body.sig.clone();
            if summaries.get(&sig) != Some(&out.summary) {
                changed = true;
                summaries.insert(sig, out.summary);
            }
            if !out.findings.is_empty() {
                let before = findings.len();
                findings.extend(out.findings);
                if findings.len() != before {
                    changed = true;
                }
            }
            for (field, toks) in out.static_publishes {
                let entry = statics.entry(field).or_default();
                let before = entry.len();
                entry.extend(toks);
                if entry.len() != before {
                    changed = true;
                }
            }
        }
        if !changed || truncated {
            break;
        }
    }

    let mut out: Vec<Finding> = findings.into_iter().collect();
    out.sort();
    out.truncate(opts.max_findings);
    ScanReport {
        findings: out,
        rounds_run,
        methods_analyzed: bodies.len(),
        methods_total,
        static_fields_tracked: statics.len(),
        truncated,
    }
}

/// Bodies accessor for tests: run extraction with the same caps the solver uses.
pub fn bodies_for_test(project: &Project, rules: &Rules, opts: &ScanOptions) -> Vec<MethodBody> {
    extract_all(project, |sig| rules.owner_excluded(sig), opts.max_insns)
}
