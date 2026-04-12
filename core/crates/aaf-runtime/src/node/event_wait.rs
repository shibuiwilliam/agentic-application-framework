//! External event wait node.
//!
//! v0.1 polls a [`tokio::sync::watch`] channel for the event payload.
//! A configurable timeout prevents unbounded waits that would violate
//! the intent's budget constraints (Rule 8).

use super::{Node, NodeKind, NodeOutput};
use crate::error::RuntimeError;
use aaf_contracts::{IntentEnvelope, NodeId, SideEffect};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::watch;

/// Default timeout when the intent has no `max_latency_ms` and no
/// explicit timeout was configured: 30 seconds.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Event-wait node.
pub struct EventWaitNode {
    id: NodeId,
    receiver: watch::Receiver<Option<serde_json::Value>>,
    /// Explicit timeout override. When `None`, the node falls back to
    /// the intent's `budget.max_latency_ms`.
    timeout: Option<Duration>,
}

impl EventWaitNode {
    /// Construct from a watch receiver. Send `Some(v)` on the matching
    /// sender to release the wait.
    pub fn new(id: NodeId, receiver: watch::Receiver<Option<serde_json::Value>>) -> Self {
        Self {
            id,
            receiver,
            timeout: None,
        }
    }

    /// Set an explicit timeout. Returns `self` for chaining.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Resolve the effective timeout: explicit override > intent
    /// budget > default.
    fn effective_timeout(&self, intent: &IntentEnvelope) -> Duration {
        if let Some(t) = self.timeout {
            return t;
        }
        let ms = intent.budget.max_latency_ms;
        if ms > 0 {
            Duration::from_millis(ms)
        } else {
            DEFAULT_TIMEOUT
        }
    }
}

#[async_trait]
impl Node for EventWaitNode {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::EventWait
    }
    fn side_effect(&self) -> SideEffect {
        SideEffect::None
    }

    async fn run(
        &self,
        intent: &IntentEnvelope,
        _prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        let timeout = self.effective_timeout(intent);
        let mut rx = self.receiver.clone();

        let wait_fut = async {
            loop {
                if let Some(v) = rx.borrow().clone() {
                    return Ok(NodeOutput {
                        data: v,
                        ..Default::default()
                    });
                }
                if rx.changed().await.is_err() {
                    return Err(RuntimeError::Node("event channel closed".into()));
                }
            }
        };

        tokio::time::timeout(timeout, wait_fut)
            .await
            .unwrap_or_else(|_| {
                Err(RuntimeError::StepTimeout {
                    step_id: self.id.to_string(),
                    timeout_ms: timeout.as_millis() as u64,
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{BudgetContract, IntentId, IntentType, Requester, RiskTier, TraceId};
    use chrono::Utc;

    fn intent_with_latency(ms: u64) -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::TransactionalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes: vec![],
                tenant: None,
            },
            goal: "wait".into(),
            domain: "test".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 100,
                max_cost_usd: 1.0,
                max_latency_ms: ms,
            },
            deadline: None,
            risk_tier: RiskTier::Read,
            approval_policy: "auto".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    #[tokio::test]
    async fn receives_event_before_timeout() {
        let (tx, rx) = watch::channel(None);
        let node =
            EventWaitNode::new(NodeId::from("wait"), rx).with_timeout(Duration::from_secs(5));
        tx.send(Some(serde_json::json!({"event": "arrived"})))
            .unwrap();
        let out = node
            .run(&intent_with_latency(5000), &HashMap::new())
            .await
            .unwrap();
        assert_eq!(out.data["event"], "arrived");
    }

    #[tokio::test]
    async fn times_out_when_no_event() {
        let (_tx, rx) = watch::channel(None);
        let node =
            EventWaitNode::new(NodeId::from("wait"), rx).with_timeout(Duration::from_millis(50));
        let err = node
            .run(&intent_with_latency(50), &HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, RuntimeError::StepTimeout { .. }));
    }

    #[tokio::test]
    async fn uses_intent_budget_when_no_explicit_timeout() {
        let (_tx, rx) = watch::channel(None);
        let node = EventWaitNode::new(NodeId::from("wait"), rx);
        let err = node
            .run(&intent_with_latency(50), &HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, RuntimeError::StepTimeout { .. }));
    }

    #[tokio::test]
    async fn channel_close_returns_error() {
        let (tx, rx) = watch::channel(None);
        let node =
            EventWaitNode::new(NodeId::from("wait"), rx).with_timeout(Duration::from_secs(5));
        drop(tx);
        let err = node
            .run(&intent_with_latency(5000), &HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, RuntimeError::Node(_)));
    }
}
