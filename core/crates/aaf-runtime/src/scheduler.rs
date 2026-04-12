//! Sequential / parallel scheduling helpers.
//!
//! v0.1's executor walks the topological order sequentially. The
//! scheduler module exposes the helpers used to identify nodes that
//! could safely run in parallel — for now this is informational.

use crate::graph::Graph;
use aaf_contracts::NodeId;
use std::collections::HashMap;

/// Levelise the topo order: nodes at the same level have no edges
/// between them and may run in parallel.
pub fn parallel_levels(graph: &Graph) -> Vec<Vec<NodeId>> {
    let mut level: HashMap<NodeId, usize> = HashMap::new();
    for n in &graph.order {
        let lvl = graph
            .edges
            .iter()
            .filter(|e| &e.to == n)
            .map(|e| level.get(&e.from).copied().unwrap_or(0) + 1)
            .max()
            .unwrap_or(0);
        level.insert(n.clone(), lvl);
    }
    let max_level = level.values().copied().max().unwrap_or(0);
    let mut levels: Vec<Vec<NodeId>> = vec![vec![]; max_level + 1];
    for (n, l) in level {
        levels[l].push(n);
    }
    for l in &mut levels {
        l.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    }
    levels
}
