//! Control-flow graph utilities: dominators (Lengauer-Tarjan), reverse post-order.
//! Mirrors androguard.decompiler.graph for test parity with test_decompiler_dominator.py and test_decompiler_rpo.py.

use std::collections::{HashMap, HashSet};

/// Directed graph with named nodes (string ids). Used for dominator and RPO tests.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// Entry node id.
    pub entry: String,
    /// All node ids.
    pub nodes: Vec<String>,
    /// Out-edges: from node -> list of successors.
    pub edges: HashMap<String, Vec<String>>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            entry: String::new(),
            nodes: Vec::new(),
            edges: HashMap::new(),
        }
    }

    pub fn with_entry(entry: &str) -> Self {
        Self {
            entry: entry.to_string(),
            nodes: vec![entry.to_string()],
            edges: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, n: &str) {
        if !self.nodes.contains(&n.to_string()) {
            self.nodes.push(n.to_string());
        }
    }

    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.add_node(from);
        self.add_node(to);
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(to.to_string());
    }

    pub fn successors(&self, n: &str) -> &[String] {
        static EMPTY: Vec<String> = Vec::new();
        self.edges.get(n).map(|v| v.as_slice()).unwrap_or(&EMPTY)
    }

    /// Immediate dominators: for each node, its unique immediate dominator (entry has None).
    /// Uses Lengauer-Tarjan algorithm. Matches androguard graph.immediate_dominators().
    pub fn immediate_dominators(&self) -> HashMap<String, Option<String>> {
        let nodes = &self.nodes;
        if nodes.is_empty() || !nodes.contains(&self.entry) {
            return HashMap::new();
        }
        let rpo = self.compute_rpo();
        let rpo_set: HashSet<_> = rpo.iter().cloned().collect();
        let mut preds: HashMap<String, Vec<String>> = HashMap::new();
        for n in &self.nodes {
            for s in self.successors(n) {
                if rpo_set.contains(s) {
                    preds.entry(s.clone()).or_default().push(n.clone());
                }
            }
        }

        let mut idom: HashMap<String, Option<String>> = HashMap::new();
        idom.insert(self.entry.clone(), None);
        for n in rpo.iter().skip(1) {
            let pred_list: Vec<String> = preds.get(n).cloned().unwrap_or_default();
            let initial = pred_list.first().cloned();
            idom.insert(n.clone(), initial);
        }

        // Iterative dominator: idom(n) = intersect of idom(p) for all predecessors p.
        let max_iter = rpo.len() + 1;
        for _ in 0..max_iter {
            let mut changed = false;
            for n in rpo.iter().skip(1) {
                let pred_list: Vec<String> = preds.get(n).cloned().unwrap_or_default();
                if pred_list.is_empty() {
                    continue;
                }
                let mut new_idom = pred_list[0].clone();
                for p in pred_list.iter().skip(1) {
                    new_idom = intersect(&rpo, &idom, new_idom, p.clone());
                }
                let prev = idom.insert(n.clone(), Some(new_idom.clone()));
                if prev != Some(Some(new_idom.clone())) {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        idom
    }

    /// Reverse post-order from entry (DFS post-order, then reversed).
    /// Matches androguard graph.compute_rpo() semantics (node.num = RPO number).
    pub fn compute_rpo(&self) -> Vec<String> {
        let mut post = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = vec![(self.entry.clone(), 0)];
        let mut index: HashMap<String, usize> = HashMap::new();
        for (i, n) in self.successors(&self.entry).iter().enumerate() {
            index.insert(n.clone(), i);
        }
        while let Some((n, child_idx)) = stack.pop() {
            let succs = self.successors(&n);
            if child_idx == 0 && !visited.insert(n.clone()) {
                continue;
            }
            if child_idx >= succs.len() {
                post.push(n);
                continue;
            }
            let next_child = succs[child_idx].clone();
            stack.push((n, child_idx + 1));
            if !visited.contains(&next_child) {
                stack.push((next_child, 0));
            }
        }
        if !visited.contains(&self.entry) {
            post.push(self.entry.clone());
        }
        post.reverse();
        post
    }

    /// RPO as map: node -> rpo number (1-based like Python).
    pub fn rpo_numbers(&self) -> HashMap<String, usize> {
        let rpo = self.compute_rpo();
        rpo.into_iter()
            .enumerate()
            .map(|(i, n)| (n, i + 1))
            .collect()
    }
}

fn intersect(
    rpo: &[String],
    idom: &HashMap<String, Option<String>>,
    mut b1: String,
    mut b2: String,
) -> String {
    let rpo_idx: HashMap<&str, usize> = rpo
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let max_steps = rpo.len() + 2;
    for _ in 0..max_steps {
        if b1 == b2 {
            return b1;
        }
        let i1 = rpo_idx.get(b1.as_str()).copied().unwrap_or(0);
        let i2 = rpo_idx.get(b2.as_str()).copied().unwrap_or(0);
        if i1 < i2 {
            b2 = idom.get(&b2).cloned().flatten().unwrap_or(b2);
        } else {
            b1 = idom.get(&b1).cloned().flatten().unwrap_or(b1);
        }
    }
    b1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_from_edges(entry: &str, edges: &[(&str, &[&str])]) -> Graph {
        let mut g = Graph::new();
        g.entry = entry.to_string();
        g.add_node(entry);
        for (from, to_list) in edges {
            for to in *to_list {
                g.add_edge(from, to);
            }
        }
        g
    }

    // --- test_decompiler_dominator.py port ---

    #[test]
    fn test_tarjan_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["a", "b", "c"][..]),
            ("a", &["d"]),
            ("b", &["a", "d", "e"]),
            ("c", &["f", "g"]),
            ("d", &["l"]),
            ("e", &["h"]),
            ("f", &["i"]),
            ("g", &["i", "j"]),
            ("h", &["e", "k"]),
            ("i", &["k"]),
            ("j", &["i"]),
            ("k", &["i", "r"]),
            ("l", &["h"]),
        ];
        let g = graph_from_edges("r", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("r", None),
            ("a", Some("r")),
            ("b", Some("r")),
            ("c", Some("r")),
            ("d", Some("r")),
            ("e", Some("r")),
            ("f", Some("c")),
            ("g", Some("c")),
            ("h", Some("r")),
            ("i", Some("r")),
            ("j", Some("g")),
            ("k", Some("r")),
            ("l", Some("d")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    #[test]
    fn test_first_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["w1", "x1", "z5"]),
            ("w1", &["w2"]),
            ("w2", &["w3"]),
            ("w3", &["w4"]),
            ("w4", &["w5"]),
            ("x1", &["x2"]),
            ("x2", &["x3"]),
            ("x3", &["x4"]),
            ("x4", &["x5"]),
            ("x5", &["y1"]),
            ("y1", &["w1", "w2", "w3", "w4", "w5", "y2"]),
            ("y2", &["w1", "w2", "w3", "w4", "w5", "y3"]),
            ("y3", &["w1", "w2", "w3", "w4", "w5", "y4"]),
            ("y4", &["w1", "w2", "w3", "w4", "w5", "y5"]),
            ("y5", &["w1", "w2", "w3", "w4", "w5", "z1"]),
            ("z1", &["z2"]),
            ("z2", &["z1", "z3"]),
            ("z3", &["z2", "z4"]),
            ("z4", &["z3", "z5"]),
            ("z5", &["z4"]),
        ];
        let g = graph_from_edges("r", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("r", None),
            ("w1", Some("r")),
            ("w2", Some("r")),
            ("w3", Some("r")),
            ("w4", Some("r")),
            ("w5", Some("r")),
            ("x1", Some("r")),
            ("x2", Some("x1")),
            ("x3", Some("x2")),
            ("x4", Some("x3")),
            ("x5", Some("x4")),
            ("y1", Some("x5")),
            ("y2", Some("y1")),
            ("y3", Some("y2")),
            ("y4", Some("y3")),
            ("y5", Some("y4")),
            ("z1", Some("r")),
            ("z2", Some("r")),
            ("z3", Some("r")),
            ("z4", Some("r")),
            ("z5", Some("r")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    #[test]
    fn test_second_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["y1", "x12"]),
            ("y1", &["y2", "x11"]),
            ("y2", &["x21"]),
            ("x11", &["x12", "x22"]),
            ("x12", &["x11"]),
            ("x21", &["x22"]),
            ("x22", &["x21"]),
        ];
        let g = graph_from_edges("r", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("r", None),
            ("y1", Some("r")),
            ("y2", Some("y1")),
            ("x11", Some("r")),
            ("x12", Some("r")),
            ("x21", Some("r")),
            ("x22", Some("r")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    #[test]
    fn test_third_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["w", "y1"]),
            ("w", &["x1", "x2"]),
            ("y1", &["y2"]),
            ("y2", &["x2"]),
            ("x2", &["x1"]),
        ];
        let g = graph_from_edges("r", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("r", None),
            ("w", Some("r")),
            ("x1", Some("r")),
            ("y1", Some("r")),
            ("y2", Some("y1")),
            ("x2", Some("r")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    #[test]
    fn test_fourth_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["x1", "y1", "y2"]),
            ("x1", &["x2"]),
            ("x2", &["y1", "y2"]),
        ];
        let g = graph_from_edges("r", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("r", None),
            ("x1", Some("r")),
            ("x2", Some("x1")),
            ("y1", Some("r")),
            ("y2", Some("r")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    #[test]
    fn test_fifth_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["a", "i"]),
            ("a", &["b", "c"]),
            ("b", &["c", "e", "g"]),
            ("c", &["d"]),
            ("d", &["i"]),
            ("e", &["c", "f"]),
            ("f", &["i"]),
            ("g", &["h"]),
            ("h", &["d", "f", "i"]),
        ];
        let g = graph_from_edges("r", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("r", None),
            ("a", Some("r")),
            ("b", Some("a")),
            ("c", Some("a")),
            ("d", Some("a")),
            ("e", Some("b")),
            ("f", Some("b")),
            ("g", Some("b")),
            ("h", Some("g")),
            ("i", Some("r")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    #[test]
    fn test_linear_vit_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["w", "y"]),
            ("w", &["x1"]),
            ("y", &["x7"]),
            ("x1", &["x2"]),
            ("x2", &["x1", "x3"]),
            ("x3", &["x2", "x4"]),
            ("x4", &["x3", "x5"]),
            ("x5", &["x4", "x6"]),
            ("x6", &["x5", "x7"]),
            ("x7", &["x6"]),
        ];
        let g = graph_from_edges("r", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("r", None),
            ("w", Some("r")),
            ("y", Some("r")),
            ("x1", Some("r")),
            ("x2", Some("r")),
            ("x3", Some("r")),
            ("x4", Some("r")),
            ("x5", Some("r")),
            ("x6", Some("r")),
            ("x7", Some("r")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    #[test]
    fn test_cross_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["a", "d"]),
            ("a", &["b"]),
            ("b", &["c"]),
            ("c", &["a", "d", "g"]),
            ("d", &["e"]),
            ("e", &["f"]),
            ("f", &["a", "d", "g"]),
        ];
        let g = graph_from_edges("r", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("r", None),
            ("a", Some("r")),
            ("b", Some("a")),
            ("c", Some("b")),
            ("d", Some("r")),
            ("e", Some("d")),
            ("f", Some("e")),
            ("g", Some("r")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    #[test]
    fn test_tverify_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("n1", &["n2", "n8"]),
            ("n2", &["n3"]),
            ("n3", &["n4", "n8", "n9"]),
            ("n4", &["n3", "n5", "n6", "n7"]),
            ("n5", &["n4"]),
            ("n6", &["n5"]),
            ("n7", &["n6"]),
            ("n8", &["n9", "n12"]),
            ("n9", &["n10", "n11", "n12"]),
            ("n10", &["n11"]),
            ("n11", &["n7"]),
            ("n12", &["n10"]),
        ];
        let g = graph_from_edges("n1", edges);
        let idom = g.immediate_dominators();
        let expected: HashMap<String, Option<String>> = [
            ("n1", None),
            ("n2", Some("n1")),
            ("n3", Some("n1")),
            ("n4", Some("n1")),
            ("n5", Some("n1")),
            ("n6", Some("n1")),
            ("n7", Some("n1")),
            ("n8", Some("n1")),
            ("n9", Some("n1")),
            ("n10", Some("n1")),
            ("n11", Some("n1")),
            ("n12", Some("n1")),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
        for (k, v) in &expected {
            assert_eq!(idom.get(k), Some(v), "idom({})", k);
        }
    }

    // --- test_decompiler_rpo.py port ---

    fn verify_rpo(g: &Graph, expected: &[(&str, usize)]) {
        let rpo_num = g.rpo_numbers();
        for (node, num) in expected {
            assert_eq!(rpo_num.get(*node), Some(num), "rpo({})", node);
        }
    }

    #[test]
    fn test_rpo_tarjan_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["a", "b", "c"]),
            ("a", &["d"]),
            ("b", &["a", "d", "e"]),
            ("c", &["f", "g"]),
            ("d", &["l"]),
            ("e", &["h"]),
            ("f", &["i"]),
            ("g", &["i", "j"]),
            ("h", &["e", "k"]),
            ("i", &["k"]),
            ("j", &["i"]),
            ("k", &["i", "r"]),
            ("l", &["h"]),
        ];
        let g = graph_from_edges("r", edges);
        let expected = [
            ("r", 1),
            ("a", 7),
            ("b", 6),
            ("c", 2),
            ("d", 8),
            ("e", 13),
            ("f", 5),
            ("g", 3),
            ("h", 10),
            ("i", 12),
            ("j", 4),
            ("k", 11),
            ("l", 9),
        ];
        verify_rpo(&g, &expected);
    }

    #[test]
    fn test_rpo_first_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["w1", "x1", "z5"]),
            ("w1", &["w2"]),
            ("w2", &["w3"]),
            ("w3", &["w4"]),
            ("w4", &["w5"]),
            ("x1", &["x2"]),
            ("x2", &["x3"]),
            ("x3", &["x4"]),
            ("x4", &["x5"]),
            ("x5", &["y1"]),
            ("y1", &["w1", "w2", "w3", "w4", "w5", "y2"]),
            ("y2", &["w1", "w2", "w3", "w4", "w5", "y3"]),
            ("y3", &["w1", "w2", "w3", "w4", "w5", "y4"]),
            ("y4", &["w1", "w2", "w3", "w4", "w5", "y5"]),
            ("y5", &["w1", "w2", "w3", "w4", "w5", "z1"]),
            ("z1", &["z2"]),
            ("z2", &["z1", "z3"]),
            ("z3", &["z2", "z4"]),
            ("z4", &["z3", "z5"]),
            ("z5", &["z4"]),
        ];
        let g = graph_from_edges("r", edges);
        let expected = [
            ("r", 1),
            ("x1", 2),
            ("x2", 3),
            ("x3", 4),
            ("x4", 5),
            ("x5", 6),
            ("w1", 17),
            ("w2", 18),
            ("w3", 19),
            ("w4", 20),
            ("w5", 21),
            ("y1", 7),
            ("y2", 8),
            ("y3", 9),
            ("y4", 10),
            ("y5", 11),
            ("z1", 12),
            ("z2", 13),
            ("z3", 14),
            ("z4", 15),
            ("z5", 16),
        ];
        verify_rpo(&g, &expected);
    }

    #[test]
    fn test_rpo_second_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["y1", "x12"]),
            ("x11", &["x12", "x22"]),
            ("x12", &["x11"]),
            ("x21", &["x22"]),
            ("x22", &["x21"]),
            ("y1", &["y2", "x11"]),
            ("y2", &["x21"]),
        ];
        let g = graph_from_edges("r", edges);
        let expected = [
            ("r", 1),
            ("x11", 3),
            ("x12", 4),
            ("x21", 6),
            ("x22", 7),
            ("y1", 2),
            ("y2", 5),
        ];
        verify_rpo(&g, &expected);
    }

    #[test]
    fn test_rpo_third_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["w", "y1"]),
            ("w", &["x1", "x2"]),
            ("x2", &["x1"]),
            ("y1", &["y2"]),
            ("y2", &["x2"]),
        ];
        let g = graph_from_edges("r", edges);
        let expected = [
            ("r", 1),
            ("w", 4),
            ("x1", 6),
            ("x2", 5),
            ("y1", 2),
            ("y2", 3),
        ];
        verify_rpo(&g, &expected);
    }

    #[test]
    fn test_rpo_fourth_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["x1", "y1", "y2"]),
            ("x1", &["x2"]),
            ("x2", &["y1", "y2"]),
        ];
        let g = graph_from_edges("r", edges);
        let expected = [("r", 1), ("x1", 2), ("x2", 3), ("y1", 5), ("y2", 4)];
        verify_rpo(&g, &expected);
    }

    #[test]
    fn test_rpo_fifth_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["a", "i"]),
            ("a", &["b", "c"]),
            ("b", &["c", "e", "g"]),
            ("c", &["d"]),
            ("d", &["i"]),
            ("e", &["c", "f"]),
            ("f", &["i"]),
            ("g", &["h"]),
            ("h", &["d", "f", "i"]),
        ];
        let g = graph_from_edges("r", edges);
        let expected = [
            ("r", 1),
            ("a", 2),
            ("b", 3),
            ("c", 8),
            ("d", 9),
            ("e", 6),
            ("f", 7),
            ("g", 4),
            ("h", 5),
            ("i", 10),
        ];
        verify_rpo(&g, &expected);
    }

    #[test]
    fn test_rpo_linear_vit_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["w", "y"]),
            ("w", &["x1"]),
            ("y", &["x7"]),
            ("x1", &["x2"]),
            ("x2", &["x1", "x3"]),
            ("x3", &["x2", "x4"]),
            ("x4", &["x3", "x5"]),
            ("x5", &["x4", "x6"]),
            ("x6", &["x5", "x7"]),
            ("x7", &["x6"]),
        ];
        let g = graph_from_edges("r", edges);
        let expected = [
            ("r", 1),
            ("w", 3),
            ("x1", 4),
            ("x2", 5),
            ("x3", 6),
            ("x4", 7),
            ("x5", 8),
            ("x6", 9),
            ("x7", 10),
            ("y", 2),
        ];
        verify_rpo(&g, &expected);
    }

    #[test]
    fn test_rpo_cross_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("r", &["a", "d"]),
            ("a", &["b"]),
            ("b", &["c"]),
            ("c", &["a", "d", "g"]),
            ("d", &["e"]),
            ("e", &["f"]),
            ("f", &["a", "d", "g"]),
        ];
        let g = graph_from_edges("r", edges);
        let expected = [
            ("r", 1),
            ("a", 2),
            ("b", 3),
            ("c", 4),
            ("d", 5),
            ("e", 6),
            ("f", 7),
            ("g", 8),
        ];
        verify_rpo(&g, &expected);
    }

    #[test]
    fn test_rpo_tverify_graph() {
        let edges: &[(&str, &[&str])] = &[
            ("n1", &["n2", "n8"]),
            ("n2", &["n3"]),
            ("n3", &["n4", "n8", "n9"]),
            ("n4", &["n3", "n5", "n6", "n7"]),
            ("n5", &["n4"]),
            ("n6", &["n5"]),
            ("n7", &["n6"]),
            ("n8", &["n9", "n12"]),
            ("n9", &["n10", "n11", "n12"]),
            ("n10", &["n11"]),
            ("n11", &["n7"]),
            ("n12", &["n10"]),
        ];
        let g = graph_from_edges("n1", edges);
        let expected = [
            ("n1", 1),
            ("n2", 2),
            ("n3", 3),
            ("n4", 9),
            ("n5", 12),
            ("n6", 11),
            ("n7", 10),
            ("n8", 4),
            ("n9", 5),
            ("n10", 7),
            ("n11", 8),
            ("n12", 6),
        ];
        verify_rpo(&g, &expected);
    }
}
