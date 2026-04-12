//! Deterministic node — pure function over the prior step outputs.
//!
//! Rule 5 prohibits LLM use here. The implementation enforces this by
//! design: the executor closure receives only the static inputs and
//! returns a `serde_json::Value`. There is no provider injected.

use super::{Node, NodeKind, NodeOutput};
use crate::error::RuntimeError;
use aaf_contracts::{IntentEnvelope, NodeId, SideEffect};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Synchronous closure type the deterministic node runs.
pub type DeterministicFn = Arc<
    dyn Fn(&IntentEnvelope, &HashMap<NodeId, NodeOutput>) -> Result<serde_json::Value, RuntimeError>
        + Send
        + Sync,
>;

/// Deterministic node — Rule 5 sacred path.
pub struct DeterministicNode {
    id: NodeId,
    side_effect: SideEffect,
    func: DeterministicFn,
}

impl DeterministicNode {
    /// Construct.
    pub fn new(id: NodeId, side_effect: SideEffect, func: DeterministicFn) -> Self {
        Self {
            id,
            side_effect,
            func,
        }
    }
}

#[async_trait]
impl Node for DeterministicNode {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Deterministic
    }
    fn side_effect(&self) -> SideEffect {
        self.side_effect
    }

    async fn run(
        &self,
        intent: &IntentEnvelope,
        prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        let started = std::time::Instant::now();
        let data = (self.func)(intent, prior)?;
        Ok(NodeOutput {
            data,
            tokens: 0,
            cost_usd: 0.0,
            duration_ms: started.elapsed().as_millis() as u64,
            model: None,
        })
    }
}
