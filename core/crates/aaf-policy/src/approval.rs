//! Human approval workflow.

use aaf_contracts::{IntentId, PolicyViolation};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// State of an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    /// Awaiting reviewer.
    Pending,
    /// Approved.
    Approved,
    /// Rejected.
    Rejected,
    /// Expired.
    Expired,
}

/// One pending approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Stable id (uses the intent id as the basis).
    pub request_id: String,
    /// Intent the request belongs to.
    pub intent_id: IntentId,
    /// Reason text shown to the reviewer.
    pub reason: String,
    /// Violations that triggered the request.
    pub violations: Vec<PolicyViolation>,
    /// Current state.
    pub state: ApprovalState,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Resolved at, if terminal.
    pub resolved_at: Option<DateTime<Utc>>,
}

/// In-memory approval workflow.
#[derive(Default)]
pub struct ApprovalWorkflow {
    inner: Arc<RwLock<HashMap<String, ApprovalRequest>>>,
}

impl std::fmt::Debug for ApprovalWorkflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.inner.read().len();
        f.debug_struct("ApprovalWorkflow")
            .field("pending_count", &count)
            .finish()
    }
}

impl ApprovalWorkflow {
    /// Create a new workflow.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a new pending approval. Returns the request id.
    pub fn open(
        &self,
        intent_id: IntentId,
        reason: impl Into<String>,
        violations: Vec<PolicyViolation>,
    ) -> String {
        let request_id = format!("appr-{}", intent_id.as_str());
        let req = ApprovalRequest {
            request_id: request_id.clone(),
            intent_id,
            reason: reason.into(),
            violations,
            state: ApprovalState::Pending,
            created_at: Utc::now(),
            resolved_at: None,
        };
        self.inner.write().insert(request_id.clone(), req);
        request_id
    }

    /// Resolve a request. Returns the new state, or `None` if unknown.
    pub fn resolve(&self, request_id: &str, approved: bool) -> Option<ApprovalState> {
        let mut guard = self.inner.write();
        let req = guard.get_mut(request_id)?;
        req.state = if approved {
            ApprovalState::Approved
        } else {
            ApprovalState::Rejected
        };
        req.resolved_at = Some(Utc::now());
        Some(req.state)
    }

    /// Inspect a request.
    pub fn get(&self, request_id: &str) -> Option<ApprovalRequest> {
        self.inner.read().get(request_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_then_approve_round_trip() {
        let wf = ApprovalWorkflow::new();
        let id = wf.open(IntentId::new(), "needs review", vec![]);
        let state = wf.resolve(&id, true).unwrap();
        assert_eq!(state, ApprovalState::Approved);
        let req = wf.get(&id).unwrap();
        assert!(req.resolved_at.is_some());
    }
}
