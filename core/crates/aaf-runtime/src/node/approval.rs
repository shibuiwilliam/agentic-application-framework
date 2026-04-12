//! Human approval gate node.
//!
//! Two operating modes:
//!
//! - **Auto-approve** — for tests; the gate trivially passes.
//! - **Workflow-backed** — the gate consults an
//!   [`aaf_policy::ApprovalWorkflow`] for an approval request keyed by
//!   `intent_id`. If the request is `Approved`, the gate passes; if
//!   `Rejected`, the gate fails the node; if `Pending` or missing, the
//!   gate opens a fresh request and returns `RuntimeError::Node` so the
//!   executor surfaces a pause to the caller.

use super::{Node, NodeKind, NodeOutput};
use crate::error::RuntimeError;
use aaf_contracts::{IntentEnvelope, NodeId, SideEffect};
use aaf_policy::approval::{ApprovalState, ApprovalWorkflow};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Operating mode of the [`ApprovalNode`].
#[derive(Debug, Clone)]
pub enum ApprovalMode {
    /// Always pass — for tests only.
    AutoApprove,
    /// Consult a real [`ApprovalWorkflow`].
    Workflow(Arc<ApprovalWorkflow>),
}

/// Approval gate node.
pub struct ApprovalNode {
    id: NodeId,
    mode: ApprovalMode,
    /// Optional reason text shown to the reviewer if a new request is
    /// opened.
    pub reason: String,
}

impl ApprovalNode {
    /// Construct an auto-approve gate (tests).
    pub fn auto(id: NodeId) -> Self {
        Self {
            id,
            mode: ApprovalMode::AutoApprove,
            reason: "auto-approve".into(),
        }
    }

    /// Construct a workflow-backed gate.
    pub fn with_workflow(
        id: NodeId,
        workflow: Arc<ApprovalWorkflow>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            id,
            mode: ApprovalMode::Workflow(workflow),
            reason: reason.into(),
        }
    }

    /// Backward-compatible constructor — `auto_approve = true` keeps
    /// the iteration-1 call sites working.
    pub fn new(id: NodeId, auto_approve: bool) -> Self {
        if auto_approve {
            Self::auto(id)
        } else {
            Self {
                id,
                mode: ApprovalMode::Workflow(Arc::new(ApprovalWorkflow::new())),
                reason: "approval required".into(),
            }
        }
    }
}

#[async_trait]
impl Node for ApprovalNode {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Approval
    }
    fn side_effect(&self) -> SideEffect {
        SideEffect::None
    }

    async fn run(
        &self,
        intent: &IntentEnvelope,
        _prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        match &self.mode {
            ApprovalMode::AutoApprove => Ok(NodeOutput {
                data: serde_json::json!({"approved": true, "via": "auto"}),
                ..Default::default()
            }),
            ApprovalMode::Workflow(wf) => {
                let request_id = format!("appr-{}", intent.intent_id.as_str());
                if let Some(req) = wf.get(&request_id) {
                    return match req.state {
                        ApprovalState::Approved => Ok(NodeOutput {
                            data: serde_json::json!({"approved": true, "via": "workflow"}),
                            ..Default::default()
                        }),
                        ApprovalState::Rejected => Err(RuntimeError::Node(format!(
                            "approval rejected for intent {}",
                            intent.intent_id
                        ))),
                        ApprovalState::Pending => Err(RuntimeError::Node(format!(
                            "approval pending for intent {}",
                            intent.intent_id
                        ))),
                        ApprovalState::Expired => Err(RuntimeError::Node(format!(
                            "approval expired for intent {}",
                            intent.intent_id
                        ))),
                    };
                }
                wf.open(intent.intent_id.clone(), self.reason.clone(), vec![]);
                Err(RuntimeError::Node(format!(
                    "approval opened for intent {}, awaiting reviewer",
                    intent.intent_id
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{BudgetContract, IntentId, IntentType, Requester, RiskTier, TraceId};
    use chrono::Utc;

    fn intent() -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::TransactionalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes: vec![],
                tenant: None,
            },
            goal: "g".into(),
            domain: "d".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 10,
                max_cost_usd: 0.1,
                max_latency_ms: 100,
            },
            deadline: None,
            risk_tier: RiskTier::Write,
            approval_policy: "human".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    #[tokio::test]
    async fn auto_mode_passes() {
        let n = ApprovalNode::auto(NodeId::from("gate"));
        let r = n.run(&intent(), &HashMap::new()).await.unwrap();
        assert_eq!(r.data["approved"], true);
    }

    #[tokio::test]
    async fn workflow_pending_then_approved() {
        let wf = Arc::new(ApprovalWorkflow::new());
        let n = ApprovalNode::with_workflow(NodeId::from("gate"), wf.clone(), "needs review");
        let i = intent();

        // First call opens a fresh request and returns pending.
        let err = n.run(&i, &HashMap::new()).await.unwrap_err();
        assert!(matches!(err, RuntimeError::Node(_)));

        // Reviewer approves.
        let request_id = format!("appr-{}", i.intent_id.as_str());
        wf.resolve(&request_id, true);

        // Second call sees the approval and passes.
        let r = n.run(&i, &HashMap::new()).await.unwrap();
        assert_eq!(r.data["approved"], true);
    }

    #[tokio::test]
    async fn workflow_rejected_fails_node() {
        let wf = Arc::new(ApprovalWorkflow::new());
        let n = ApprovalNode::with_workflow(NodeId::from("gate"), wf.clone(), "x");
        let i = intent();
        let _ = n.run(&i, &HashMap::new()).await;
        wf.resolve(&format!("appr-{}", i.intent_id.as_str()), false);
        let err = n.run(&i, &HashMap::new()).await.unwrap_err();
        match err {
            RuntimeError::Node(msg) => assert!(msg.contains("rejected")),
            _ => panic!(),
        }
    }
}
