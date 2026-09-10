//! Virtual dispatch index: maps the owner-stripped method key
//! (`->name(args)ret`) of every method in the project to the full set of
//! signatures sharing it (base declaration + every override / interface
//! implementation). The taint solver fans summaries and rule matches out
//! over this set at `invoke-virtual` / `invoke-interface` / `invoke-super`
//! call sites — the conservative, type-analysis-free approximation that
//! keeps base-class call sites from missing subclass sinks.

use decx_core::Project;
use std::collections::{BTreeMap, BTreeSet};

pub struct DispatchIndex {
    /// `->name(args)ret` -> all full signatures with that key
    impls: BTreeMap<String, Vec<String>>,
}

/// owner-stripped key of a full method signature
fn key_of(sig: &str) -> &str {
    match sig.find("->") {
        Some(i) => &sig[i..],
        None => sig,
    }
}

impl DispatchIndex {
    pub fn empty() -> DispatchIndex {
        DispatchIndex { impls: BTreeMap::new() }
    }

    /// register one method signature (idempotent); lets tests assemble
    /// hierarchies without a full `Project`
    pub fn insert(&mut self, full_sig: &str) {
        self.impls
            .entry(key_of(full_sig).to_string())
            .or_default()
            .push(full_sig.to_string());
        if let Some(v) = self.impls.get_mut(key_of(full_sig)) {
            v.sort();
            v.dedup();
        }
    }

    /// Build over every method (direct + virtual) of every class def in
    /// every dex. Excluded-owner methods are included too — they simply
    /// never acquire summaries, so they are harmless in fan-out.
    pub fn build(project: &Project) -> DispatchIndex {
        let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for dex in &project.dexes {
            for def in &dex.class_defs {
                let cd = dex.class_data(def);
                for section in [&cd.direct_methods, &cd.virtual_methods] {
                    for m in section {
                        let full = dex.method_full(m.method_idx);
                        map.entry(key_of(&full).to_string())
                            .or_default()
                            .insert(full);
                    }
                }
            }
        }
        DispatchIndex {
            impls: map.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect(),
        }
    }

    /// All signatures that a call to `callee` may dispatch to: the callee
    /// itself plus every same-key override. Falls back to just the callee
    /// when the key is unknown (synthetic / filtered methods).
    pub fn fanout(&self, callee: &str) -> Vec<String> {
        match self.impls.get(key_of(callee)) {
            Some(v) => v.clone(),
            None => vec![callee.to_string()],
        }
    }

    /// number of distinct method keys indexed (diagnostics / tests)
    pub fn len_keys(&self) -> usize {
        self.impls.len()
    }
}

/// does this invoke kind dispatch virtually (target may be an override)?
pub fn is_dispatch_kind(kind: &str) -> bool {
    matches!(kind, "virtual" | "interface" | "super" | "polymorphic")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_of_strips_owner() {
        assert_eq!(key_of("La/B;->m(I)V"), "->m(I)V");
        assert_eq!(key_of("no-arrow"), "no-arrow");
    }

    #[test]
    fn empty_fanout_returns_callee() {
        let d = DispatchIndex::empty();
        assert_eq!(d.fanout("La/B;->m(I)V"), vec!["La/B;->m(I)V".to_string()]);
    }

    #[test]
    fn dispatch_kinds() {
        assert!(is_dispatch_kind("virtual"));
        assert!(is_dispatch_kind("interface"));
        assert!(is_dispatch_kind("super"));
        assert!(is_dispatch_kind("polymorphic"));
        assert!(!is_dispatch_kind("static"));
        assert!(!is_dispatch_kind("direct"));
        assert!(!is_dispatch_kind("custom"));
    }
}
