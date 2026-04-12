//! Compensation chain — drives Saga rollback.

use crate::error::RuntimeError;
use crate::node::Node;
use aaf_contracts::{IntentEnvelope, NodeId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::node::NodeOutput;

/// Compensation chain. Holds a stack of `(step_node_id, compensator)`
/// pairs and runs them in reverse on failure.
#[derive(Default)]
pub struct CompensationChain {
    stack: Vec<(NodeId, Arc<dyn Node>)>,
}

impl CompensationChain {
    /// New empty chain.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a compensator.
    pub fn push(&mut self, for_step: NodeId, compensator: Arc<dyn Node>) {
        self.stack.push((for_step, compensator));
    }

    /// Number of compensators.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Whether the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Run all compensators in reverse. Each compensator receives the
    /// accumulated outputs of previously-run compensators so that it
    /// can reference identifiers produced during rollback (e.g. a
    /// "release stock" compensator that needs the reservation id from
    /// the original step's output).
    ///
    /// Returns all compensator outputs on success, or the first
    /// failure.
    pub async fn rollback(
        &mut self,
        intent: &IntentEnvelope,
    ) -> Result<Vec<NodeOutput>, RuntimeError> {
        let mut prior: HashMap<NodeId, NodeOutput> = HashMap::new();
        let mut out = vec![];
        while let Some((step_id, comp)) = self.stack.pop() {
            match comp.run(intent, &prior).await {
                Ok(o) => {
                    prior.insert(step_id, o.clone());
                    out.push(o);
                }
                Err(e) => {
                    return Err(RuntimeError::CompensationFailed {
                        step_id: step_id.to_string(),
                        reason: e.to_string(),
                    })
                }
            }
        }
        Ok(out)
    }
}
