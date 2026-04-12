//! Directed acyclic graph definition + topological validation.

use crate::node::Node;
use aaf_contracts::NodeId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;

/// Errors raised by [`Graph::validate`].
#[derive(Debug, Error, Clone)]
pub enum GraphValidationError {
    /// A cycle was detected.
    #[error("cycle detected at node {0}")]
    Cycle(String),

    /// A referenced node id is missing from the graph.
    #[error("missing node referenced in edges: {0}")]
    MissingNode(String),

    /// The graph contains zero nodes.
    #[error("empty graph")]
    Empty,
}

/// One DAG edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    /// Source node.
    pub from: NodeId,
    /// Destination node.
    pub to: NodeId,
}

/// A validated execution graph.
pub struct Graph {
    /// Nodes keyed by id.
    pub nodes: HashMap<NodeId, Arc<dyn Node>>,
    /// Forward edges.
    pub edges: Vec<Edge>,
    /// Topologically sorted node order produced by `validate`.
    pub order: Vec<NodeId>,
    /// Compensation map: `step_node_id → compensator`. Populated by
    /// [`GraphBuilder::add_compensator`] and consumed by the executor
    /// to roll back successful steps when a later step fails.
    pub compensators: HashMap<NodeId, Arc<dyn Node>>,
}

impl Graph {
    /// Validate the graph and return a topologically-sorted execution
    /// plan with no compensators.
    pub fn validate(
        nodes: HashMap<NodeId, Arc<dyn Node>>,
        edges: Vec<Edge>,
    ) -> Result<Self, GraphValidationError> {
        Self::validate_with_compensators(nodes, edges, HashMap::new())
    }

    /// Validate while preserving an externally-supplied compensator
    /// map. Each compensator key must match an existing node id.
    pub fn validate_with_compensators(
        nodes: HashMap<NodeId, Arc<dyn Node>>,
        edges: Vec<Edge>,
        compensators: HashMap<NodeId, Arc<dyn Node>>,
    ) -> Result<Self, GraphValidationError> {
        if nodes.is_empty() {
            return Err(GraphValidationError::Empty);
        }
        for e in &edges {
            if !nodes.contains_key(&e.from) {
                return Err(GraphValidationError::MissingNode(e.from.to_string()));
            }
            if !nodes.contains_key(&e.to) {
                return Err(GraphValidationError::MissingNode(e.to.to_string()));
            }
        }
        for k in compensators.keys() {
            if !nodes.contains_key(k) {
                return Err(GraphValidationError::MissingNode(format!(
                    "compensator-for-{k}"
                )));
            }
        }
        // Kahn's algorithm.
        let mut indeg: HashMap<NodeId, usize> = nodes.keys().map(|k| (k.clone(), 0)).collect();
        let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for e in &edges {
            *indeg.entry(e.to.clone()).or_insert(0) += 1;
            adj.entry(e.from.clone()).or_default().push(e.to.clone());
        }
        let mut queue: Vec<NodeId> = indeg
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(k, _)| k.clone())
            .collect();
        // Stable order: sort by id string for determinism.
        queue.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let mut order = vec![];
        let mut visited: HashSet<NodeId> = HashSet::new();
        while let Some(n) = queue.pop() {
            order.push(n.clone());
            visited.insert(n.clone());
            if let Some(children) = adj.get(&n) {
                for c in children {
                    if let Some(d) = indeg.get_mut(c) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push(c.clone());
                            queue.sort_by(|a, b| a.as_str().cmp(b.as_str()));
                        }
                    }
                }
            }
        }
        if visited.len() != nodes.len() {
            // At least one node was not visited → it's part of a cycle.
            // The find() is guaranteed to succeed because the sets differ.
            let bad = nodes
                .keys()
                .find(|k| !visited.contains(k))
                .cloned()
                .unwrap_or_else(|| NodeId::from("unknown"));
            return Err(GraphValidationError::Cycle(bad.to_string()));
        }
        Ok(Self {
            nodes,
            edges,
            order,
            compensators,
        })
    }
}

/// Builder API for graphs.
#[derive(Default)]
pub struct GraphBuilder {
    nodes: HashMap<NodeId, Arc<dyn Node>>,
    edges: Vec<Edge>,
    compensators: HashMap<NodeId, Arc<dyn Node>>,
}

impl GraphBuilder {
    /// New empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node.
    pub fn add_node(mut self, node: Arc<dyn Node>) -> Self {
        self.nodes.insert(node.id().clone(), node);
        self
    }

    /// Add an edge.
    pub fn add_edge(mut self, from: NodeId, to: NodeId) -> Self {
        self.edges.push(Edge { from, to });
        self
    }

    /// Register a compensator that the executor will run on rollback
    /// if the step `for_step` completed successfully but a later node
    /// failed.
    pub fn add_compensator(mut self, for_step: NodeId, compensator: Arc<dyn Node>) -> Self {
        self.compensators.insert(for_step, compensator);
        self
    }

    /// Build and validate the graph.
    pub fn build(self) -> Result<Graph, GraphValidationError> {
        Graph::validate_with_compensators(self.nodes, self.edges, self.compensators)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::DeterministicNode;
    use aaf_contracts::SideEffect;

    fn det(id: &str) -> Arc<dyn Node> {
        Arc::new(DeterministicNode::new(
            NodeId::from(id),
            SideEffect::None,
            std::sync::Arc::new(|_, _| Ok(serde_json::json!({}))),
        ))
    }

    #[test]
    fn topological_sort_orders_chain() {
        let g = GraphBuilder::new()
            .add_node(det("a"))
            .add_node(det("b"))
            .add_node(det("c"))
            .add_edge(NodeId::from("a"), NodeId::from("b"))
            .add_edge(NodeId::from("b"), NodeId::from("c"))
            .build()
            .unwrap();
        let pos = |n: &str| g.order.iter().position(|x| x.as_str() == n).unwrap();
        assert!(pos("a") < pos("b"));
        assert!(pos("b") < pos("c"));
    }

    #[test]
    fn cycle_is_detected() {
        let res = GraphBuilder::new()
            .add_node(det("a"))
            .add_node(det("b"))
            .add_edge(NodeId::from("a"), NodeId::from("b"))
            .add_edge(NodeId::from("b"), NodeId::from("a"))
            .build();
        match res {
            Err(GraphValidationError::Cycle(_)) => {}
            _ => panic!("expected cycle"),
        }
    }

    #[test]
    fn empty_graph_rejected() {
        match GraphBuilder::new().build() {
            Err(GraphValidationError::Empty) => {}
            _ => panic!("expected empty"),
        }
    }
}
