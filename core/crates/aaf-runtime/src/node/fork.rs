//! Fork node — runs a set of child nodes concurrently and joins their
//! outputs into a single map keyed by child id.
//!
//! Note: full parallel scheduling lives in [`crate::scheduler`]; the
//! fork node is a *static* convenience that bundles a known set of
//! children. The runtime's `parallel_groups` field on
//! [`crate::graph::Graph`] is the more general mechanism.

use super::{Node, NodeKind, NodeOutput};
use crate::error::RuntimeError;
use aaf_contracts::{IntentEnvelope, NodeId, SideEffect};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Fork / join node.
pub struct ForkNode {
    id: NodeId,
    /// Child nodes that run concurrently.
    pub children: Vec<Arc<dyn Node>>,
}

impl ForkNode {
    /// Construct.
    pub fn new(id: NodeId, children: Vec<Arc<dyn Node>>) -> Self {
        Self { id, children }
    }
}

#[async_trait]
impl Node for ForkNode {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Fork
    }
    fn side_effect(&self) -> SideEffect {
        SideEffect::None
    }

    async fn run(
        &self,
        intent: &IntentEnvelope,
        prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        let started = std::time::Instant::now();
        let mut handles = vec![];
        for child in &self.children {
            let child = child.clone();
            let child_id = child.id().clone();
            let intent = intent.clone();
            let prior = prior.clone();
            handles.push((
                child_id,
                tokio::spawn(async move { child.run(&intent, &prior).await }),
            ));
        }
        let mut combined = serde_json::Map::new();
        let mut tokens = 0u64;
        let mut cost = 0.0_f64;
        for (idx, (child_id, h)) in handles.into_iter().enumerate() {
            let res = h.await.map_err(|e| {
                RuntimeError::Node(format!(
                    "fork child {child_id} (index {idx}) join failed: {e}"
                ))
            })??;
            tokens = tokens.saturating_add(res.tokens);
            cost += res.cost_usd;
            combined.insert(format!("child_{idx}"), res.data);
        }
        Ok(NodeOutput {
            data: serde_json::Value::Object(combined),
            tokens,
            cost_usd: cost,
            duration_ms: started.elapsed().as_millis() as u64,
            model: None,
        })
    }
}
