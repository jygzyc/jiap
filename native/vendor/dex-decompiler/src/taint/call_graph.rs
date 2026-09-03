//! Call-graph construction from invoke sites.

use std::collections::HashMap;

use crate::decompile::value_flow::ValueFlowAnalysisOwned;
use crate::error::Result;

use super::index::{MethodId, MethodIndex};

#[derive(Clone, Debug)]
pub struct CallEdge {
    pub caller: MethodId,
    pub callee: MethodId,
    /// Invoke instruction offset (method-relative).
    pub invoke_offset: u32,
    /// Argument registers at the call site (order = Dalvik invoke regs).
    pub arg_regs: Vec<u32>,
    pub method_ref: String,
}

#[derive(Debug, Default)]
pub struct CallGraph {
    /// caller → outgoing edges
    pub outs: HashMap<MethodId, Vec<CallEdge>>,
    /// callee → incoming edges
    pub ins: HashMap<MethodId, Vec<CallEdge>>,
}

impl CallGraph {
    /// Build the call graph from a precomputed value-flow cache (no re-analysis).
    pub fn build_from_vf_cache(
        index: &MethodIndex,
        vf_cache: &HashMap<MethodId, ValueFlowAnalysisOwned>,
        include: impl Fn(MethodId) -> bool,
    ) -> Result<Self> {
        let mut cg = CallGraph::default();
        for mref in &index.methods {
            if !include(mref.id) {
                continue;
            }
            let Some(owned) = vf_cache.get(&mref.id) else {
                continue;
            };
            for (&invoke_offset, method_ref) in &owned.invoke_method_map {
                let callees = index.resolve_callees(method_ref);
                if callees.is_empty() {
                    continue;
                }
                let arg_regs = owned
                    .rw_map
                    .get(&invoke_offset)
                    .map(|(reads, _)| reads.clone())
                    .unwrap_or_default();
                for callee in callees {
                    if !include(callee) {
                        continue;
                    }
                    let edge = CallEdge {
                        caller: mref.id,
                        callee,
                        invoke_offset,
                        arg_regs: arg_regs.clone(),
                        method_ref: method_ref.clone(),
                    };
                    cg.outs.entry(mref.id).or_default().push(edge.clone());
                    cg.ins.entry(callee).or_default().push(edge);
                }
            }
            // Runnable/Executor/Handler shim: resolve the callback object's reaching
            // definition to either an invoke-custom implementation or an app class
            // implementing run(). This keeps the edge bounded to the concrete value
            // passed at the scheduling site.
            let analysis = owned.analysis();
            for (&dispatch_offset, dispatch_ref) in &owned.invoke_method_map {
                if !is_callback_dispatch(dispatch_ref) {
                    continue;
                }
                let dispatch_args = owned
                    .rw_map
                    .get(&dispatch_offset)
                    .map(|(reads, _)| reads.clone())
                    .unwrap_or_default();
                let Some(&callback_reg) = dispatch_args.last() else {
                    continue;
                };
                for (callee, captured_args, method_ref) in
                    callback_targets(index, owned, &analysis, dispatch_offset, callback_reg)
                {
                    if !include(callee)
                        || cg.outs.get(&mref.id).is_some_and(|edges| {
                            edges.iter().any(|edge| {
                                edge.invoke_offset == dispatch_offset && edge.callee == callee
                            })
                        })
                    {
                        continue;
                    }
                    let edge = CallEdge {
                        caller: mref.id,
                        callee,
                        invoke_offset: dispatch_offset,
                        arg_regs: captured_args,
                        method_ref,
                    };
                    cg.outs.entry(mref.id).or_default().push(edge.clone());
                    cg.ins.entry(callee).or_default().push(edge);
                }
            }
        }
        Ok(cg)
    }

    pub fn edge_count(&self) -> usize {
        self.outs.values().map(|v| v.len()).sum()
    }
}

fn is_callback_dispatch(method_ref: &str) -> bool {
    method_ref.contains("Executor.execute")
        || method_ref.contains("ExecutorService.submit")
        || method_ref.contains("Handler.post")
        || method_ref.contains("Handler.postDelayed")
}

fn callback_targets(
    index: &MethodIndex,
    owned: &ValueFlowAnalysisOwned,
    analysis: &crate::decompile::value_flow::ValueFlowAnalysis<'_>,
    dispatch_offset: u32,
    callback_reg: u32,
) -> Vec<(MethodId, Vec<u32>, String)> {
    let mut out = Vec::new();
    for (def_offset, _) in analysis.use_def(dispatch_offset, callback_reg) {
        let label = owned
            .insn_at
            .get(&def_offset)
            .map(String::as_str)
            .unwrap_or("");
        if label.starts_with("new-instance") {
            if let Some(class_name) = label.rsplit_once(',').map(|(_, class)| class.trim()) {
                let method_ref = format!("{class_name}.run");
                for callee in index.resolve_callees(&method_ref) {
                    out.push((callee, vec![callback_reg], method_ref.clone()));
                }
            }
            continue;
        }
        if !label.starts_with("move-result") {
            continue;
        }
        for (&custom_offset, method_ref) in &owned.invoke_method_map {
            if !owned
                .insn_at
                .get(&custom_offset)
                .is_some_and(|label| label.starts_with("invoke-custom"))
            {
                continue;
            }
            let next_result = super::solver::move_result_for_invoke(owned, custom_offset);
            if next_result != Some((def_offset, callback_reg)) {
                continue;
            }
            let captures = owned
                .rw_map
                .get(&custom_offset)
                .map(|(reads, _)| reads.clone())
                .unwrap_or_default();
            for callee in index.resolve_callees(method_ref) {
                out.push((callee, captures.clone(), method_ref.clone()));
            }
        }
    }
    out
}
