//! Graph executor.

use crate::budget::{BudgetTracker, BudgetTrackerError};
use crate::compensation::CompensationChain;
use crate::error::RuntimeError;
use crate::graph::Graph;
use crate::node::NodeOutput;
use aaf_contracts::{
    BudgetContract, IntentEnvelope, NodeId, Observation, PolicyDecision, StepOutcome, TraceId,
    TraceStatus,
};
use aaf_identity::{RevocationKind, RevocationRegistry};
use aaf_policy::{PolicyContext, PolicyEngine, PolicyHook};
use aaf_trace::TraceRecorder;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;

/// Outcome of a [`GraphExecutor::run`] call.
///
/// Note: a *policy denial* is reported as `Err(RuntimeError::PolicyViolation)`
/// because the runtime cannot continue, while a *budget exhaustion* is
/// returned as `Ok(Partial)` because the executed steps are still
/// usable. Approval pauses are returned as `Ok(PendingApproval)` so the
/// caller can persist the trace and resume later.
#[derive(Debug, Clone)]
pub enum ExecutionOutcome {
    /// All nodes ran successfully.
    Completed {
        /// Per-node outputs keyed by node id.
        outputs: HashMap<NodeId, NodeOutput>,
        /// Number of steps executed.
        steps: u32,
    },
    /// Budget exhausted — what we managed to execute is included.
    Partial {
        /// Per-node outputs keyed by node id.
        outputs: HashMap<NodeId, NodeOutput>,
        /// Number of steps executed.
        steps: u32,
        /// Budget kind that was exhausted.
        reason: BudgetTrackerError,
    },
    /// Policy returned `RequireApproval` and the runtime is configured
    /// to pause rather than fail.
    PendingApproval {
        /// Per-node outputs accumulated up to the gate.
        outputs: HashMap<NodeId, NodeOutput>,
        /// Step at which the approval gate fired.
        at_step: u32,
        /// The full decision so the caller can render the violations.
        decision: PolicyDecision,
    },
    /// A node failed mid-graph and the executor rolled back every
    /// previously-successful write step via the compensators registered
    /// on the [`Graph`]. The original failure is wrapped inside.
    RolledBack {
        /// The failure that triggered rollback.
        failed_at: u32,
        /// Reason text from the originating failure.
        reason: String,
        /// Steps whose compensators ran successfully (in reverse order).
        compensated: Vec<NodeId>,
    },
}

/// Graph executor.
pub struct GraphExecutor {
    /// Policy engine that gates each hook.
    pub policy: Arc<PolicyEngine>,
    /// Trace recorder.
    pub recorder: Arc<dyn TraceRecorder>,
    /// Budget tracker.
    pub budget: BudgetTracker,
    /// Optional revocation registry consulted at `PrePlan` (Wave 2 X1
    /// Slice B). When `None`, the runtime behaves exactly as iteration
    /// 8 did, preserving backward compatibility.
    pub revocation: Option<Arc<dyn RevocationRegistry>>,
    /// Shadow mode (PROJECT_AafService §6.2). When `true`, the
    /// executor records every decision (trace + observations + policy
    /// hooks) but **does not execute** any node whose `side_effect`
    /// is `Write / Delete / Send / Payment`. Those nodes produce a
    /// synthetic `{"shadow": true, "would_have_run": "<node_id>"}`
    /// output instead.
    ///
    /// This enables phased adoption: deploy AAF in shadow mode first,
    /// compare its decisions against the existing system, and cut
    /// over once the agreement rate exceeds the threshold (§6.2:
    /// ≥ 95%).
    pub shadow: bool,
}

impl GraphExecutor {
    /// Construct from explicit components. Produces an executor with
    /// no revocation gate and shadow mode **off**.
    pub fn new(
        policy: Arc<PolicyEngine>,
        recorder: Arc<dyn TraceRecorder>,
        budget: BudgetContract,
    ) -> Self {
        Self {
            policy,
            recorder,
            budget: BudgetTracker::new(budget),
            revocation: None,
            shadow: false,
        }
    }

    /// Attach a revocation registry so the pre-plan hook can
    /// short-circuit revoked requesters. Returns `self` for chaining.
    pub fn with_revocation(mut self, reg: Arc<dyn RevocationRegistry>) -> Self {
        self.revocation = Some(reg);
        self
    }

    /// Enable shadow mode. In shadow mode the executor records
    /// every decision but does not actually execute write-class nodes
    /// — their output is a synthetic marker. Returns `self` for
    /// chaining.
    pub fn with_shadow(mut self) -> Self {
        self.shadow = true;
        self
    }

    /// Run the graph for `intent`. Records every step on the recorder
    /// and returns an [`ExecutionOutcome`] describing the result.
    ///
    /// The four policy hooks are honoured as follows:
    ///
    /// | Decision        | PrePlan / PreStep      | PostStep                |
    /// |-----------------|------------------------|--------------------------|
    /// | `Allow`         | proceed                | proceed                 |
    /// | `AllowWithWarnings` | proceed (record)   | proceed (record)        |
    /// | `RequireApproval` | return PendingApproval | return PendingApproval |
    /// | `Deny`          | Err(PolicyViolation)   | Err(PolicyViolation)    |
    #[allow(clippy::too_many_lines)]
    pub async fn run(
        &self,
        graph: &Graph,
        intent: &IntentEnvelope,
    ) -> Result<ExecutionOutcome, RuntimeError> {
        // ── Hook 0: Revocation check (Wave 2 X1 Slice B, Rule 22) ─────
        // Runs **before** the trace is opened so a revoked agent's
        // rejected attempt leaves no partial trace artefacts. The
        // requester's user_id is treated as a DID only when it begins
        // with the `did:aaf:` prefix; Wave 1 requesters without a
        // cryptographic identity are unaffected.
        if let Some(reg) = self.revocation.as_ref() {
            let candidate = intent.requester.user_id.as_str();
            if candidate.starts_with("did:aaf:")
                && reg.is_revoked(&RevocationKind::Did, candidate).await
            {
                return Err(RuntimeError::Revoked {
                    did: candidate.to_string(),
                    reason: "revoked in revocation registry".to_string(),
                });
            }
        }

        // ── Hook 1: PrePlan ───────────────────────────────────────────
        let pre_plan_decision = self.policy.evaluate(
            PolicyHook::PrePlan,
            &PolicyContext {
                intent,
                capability: None,
                requester: &intent.requester,
                payload: Some(&intent.goal),
                output: None,
                side_effect: None,
                remaining_budget: self.budget.remaining(),
                tenant: intent.requester.tenant.as_ref(),
                composed_writes: 0,
                ontology_class_lookup: None,
            },
        );

        self.recorder
            .open(intent.trace_id.clone(), intent.intent_id.clone())
            .await
            .map_err(|e| RuntimeError::Node(e.to_string()))?;

        match &pre_plan_decision {
            PolicyDecision::Deny(violations) => {
                self.close_trace(&intent.trace_id, TraceStatus::Failed)
                    .await?;
                return Err(RuntimeError::PolicyViolation(violations.clone()));
            }
            PolicyDecision::RequireApproval(_) => {
                self.close_trace(&intent.trace_id, TraceStatus::Partial)
                    .await?;
                return Ok(ExecutionOutcome::PendingApproval {
                    outputs: HashMap::new(),
                    at_step: 0,
                    decision: pre_plan_decision,
                });
            }
            _ => {}
        }

        let mut outputs: HashMap<NodeId, NodeOutput> = HashMap::new();
        let mut step: u32 = 0;
        let mut composed_writes: u32 = 0;
        // Compensation chain — populated when a write-class node
        // succeeds AND a compensator was registered for it. Drained on
        // any failure path.
        let mut chain = CompensationChain::new();
        // Track ids of compensators we have queued, in execution order,
        // so we can report which steps were rolled back.
        let mut compensator_targets: Vec<NodeId> = Vec::new();

        for node_id in &graph.order {
            step += 1;
            let node = graph
                .nodes
                .get(node_id)
                .ok_or_else(|| RuntimeError::Node(format!("missing node {node_id}")))?;

            // ── Hook 2: PreStep ────────────────────────────────────
            let pre_step = self.policy.evaluate(
                PolicyHook::PreStep,
                &PolicyContext {
                    intent,
                    capability: None,
                    requester: &intent.requester,
                    payload: Some(&intent.goal),
                    output: None,
                    side_effect: Some(node.side_effect()),
                    remaining_budget: self.budget.remaining(),
                    tenant: intent.requester.tenant.as_ref(),
                    composed_writes,
                    ontology_class_lookup: None,
                },
            );
            match &pre_step {
                PolicyDecision::Deny(violations) => {
                    self.close_trace(&intent.trace_id, TraceStatus::Failed)
                        .await?;
                    return Err(RuntimeError::PolicyViolation(violations.clone()));
                }
                PolicyDecision::RequireApproval(_) => {
                    self.close_trace(&intent.trace_id, TraceStatus::Partial)
                        .await?;
                    return Ok(ExecutionOutcome::PendingApproval {
                        outputs,
                        at_step: step,
                        decision: pre_step,
                    });
                }
                _ => {}
            }

            // ── Shadow mode guard (AafService §6.2) ────────────────
            // In shadow mode, write-class nodes produce a synthetic
            // marker instead of executing. Read-class nodes still
            // run so the shadow trace contains real data flows.
            let is_write_class = matches!(
                node.side_effect(),
                aaf_contracts::SideEffect::Write
                    | aaf_contracts::SideEffect::Delete
                    | aaf_contracts::SideEffect::Send
                    | aaf_contracts::SideEffect::Payment
            );

            // ── Run the node ───────────────────────────────────────
            let started = std::time::Instant::now();
            let output = if self.shadow && is_write_class {
                // Shadow: don't execute, produce a marker.
                NodeOutput {
                    data: serde_json::json!({
                        "shadow": true,
                        "would_have_run": node_id.to_string(),
                        "side_effect": format!("{:?}", node.side_effect()),
                    }),
                    ..Default::default()
                }
            } else {
                match node.run(intent, &outputs).await {
                    Ok(o) => o,
                    Err(e) => {
                        // Roll back successful write steps before
                        // surfacing the failure.
                        let compensated = self
                            .rollback(&mut chain, &compensator_targets, intent)
                            .await?;
                        self.close_trace(&intent.trace_id, TraceStatus::Failed)
                            .await?;
                        if !compensated.is_empty() {
                            return Ok(ExecutionOutcome::RolledBack {
                                failed_at: step,
                                reason: e.to_string(),
                                compensated,
                            });
                        }
                        return Err(e);
                    }
                }
            };
            let elapsed_ms = started.elapsed().as_millis() as u64;

            // ── Charge budget (Rule 8) ─────────────────────────────
            if let Err(e) = self
                .budget
                .charge(output.tokens, output.cost_usd, elapsed_ms)
            {
                self.close_trace(&intent.trace_id, TraceStatus::Partial)
                    .await?;
                return Ok(ExecutionOutcome::Partial {
                    outputs,
                    steps: step - 1,
                    reason: e,
                });
            }

            // ── Hook 3: PostStep ───────────────────────────────────
            let post_step = self.policy.evaluate(
                PolicyHook::PostStep,
                &PolicyContext {
                    intent,
                    capability: None,
                    requester: &intent.requester,
                    payload: None,
                    output: Some(&output.data.to_string()),
                    side_effect: Some(node.side_effect()),
                    remaining_budget: self.budget.remaining(),
                    tenant: intent.requester.tenant.as_ref(),
                    composed_writes,
                    ontology_class_lookup: None,
                },
            );
            match &post_step {
                PolicyDecision::Deny(violations) => {
                    self.close_trace(&intent.trace_id, TraceStatus::Failed)
                        .await?;
                    return Err(RuntimeError::PolicyViolation(violations.clone()));
                }
                PolicyDecision::RequireApproval(_) => {
                    // Persist the partial outputs *including* the step that
                    // produced the gated output, so a reviewer can see what
                    // would have been emitted.
                    outputs.insert(node_id.clone(), output);
                    self.close_trace(&intent.trace_id, TraceStatus::Partial)
                        .await?;
                    return Ok(ExecutionOutcome::PendingApproval {
                        outputs,
                        at_step: step,
                        decision: post_step,
                    });
                }
                _ => {}
            }

            // ── Record observation (Rule 12) + attach outcome (Rule 15)
            // Enhancement E1: the runtime attaches a *minimal* Outcome
            // at step-end. Richer outcomes (user feedback, downstream
            // error, semantic score) are layered in later by the saga
            // engine, app-native surface, and eval harness.
            let minimal_outcome = aaf_contracts::Outcome::minimal(
                aaf_contracts::OutcomeStatus::Succeeded,
                output.duration_ms,
                u32::try_from(output.tokens).unwrap_or(u32::MAX),
                output.cost_usd,
            );
            let obs = Observation {
                trace_id: intent.trace_id.clone(),
                node_id: node_id.clone(),
                step,
                agent: "runtime".into(),
                observed: vec![],
                reasoning: format!("ran node {node_id} of kind {:?}", node.kind()),
                decision: "continue".into(),
                confidence: 1.0,
                alternatives: vec![],
                outcome: StepOutcome::Success,
                recorded_at: Utc::now(),
                outcome_detail: Some(minimal_outcome),
            };
            let model = output.model.clone();
            let cost_usd = output.cost_usd;
            let tokens = output.tokens;
            let duration_ms = output.duration_ms;
            self.recorder
                .record_observation(
                    obs,
                    "node_run",
                    cost_usd,
                    duration_ms,
                    tokens / 2,
                    tokens - tokens / 2,
                    model,
                )
                .await
                .map_err(|e| RuntimeError::Node(e.to_string()))?;

            if matches!(
                node.side_effect(),
                aaf_contracts::SideEffect::Write
                    | aaf_contracts::SideEffect::Delete
                    | aaf_contracts::SideEffect::Send
                    | aaf_contracts::SideEffect::Payment
            ) {
                composed_writes += 1;
                // Queue this step's compensator (if any) so a later
                // failure can roll it back.
                if let Some(comp) = graph.compensators.get(node_id).cloned() {
                    chain.push(node_id.clone(), comp);
                    compensator_targets.push(node_id.clone());
                }
            }

            outputs.insert(node_id.clone(), output);
        }

        // ── Hook 4: PreArtifact ───────────────────────────────────────
        let pre_artifact = self.policy.evaluate(
            PolicyHook::PreArtifact,
            &PolicyContext {
                intent,
                capability: None,
                requester: &intent.requester,
                payload: None,
                output: None,
                side_effect: None,
                remaining_budget: self.budget.remaining(),
                tenant: intent.requester.tenant.as_ref(),
                composed_writes,
                ontology_class_lookup: None,
            },
        );
        match &pre_artifact {
            PolicyDecision::Deny(violations) => {
                self.close_trace(&intent.trace_id, TraceStatus::Failed)
                    .await?;
                return Err(RuntimeError::PolicyViolation(violations.clone()));
            }
            PolicyDecision::RequireApproval(_) => {
                self.close_trace(&intent.trace_id, TraceStatus::Partial)
                    .await?;
                return Ok(ExecutionOutcome::PendingApproval {
                    outputs,
                    at_step: step,
                    decision: pre_artifact,
                });
            }
            _ => {}
        }

        self.close_trace(&intent.trace_id, TraceStatus::Completed)
            .await?;
        Ok(ExecutionOutcome::Completed {
            outputs,
            steps: step,
        })
    }

    /// Helper: close the trace and convert recorder errors into a
    /// `RuntimeError`.
    async fn close_trace(
        &self,
        trace_id: &TraceId,
        status: TraceStatus,
    ) -> Result<(), RuntimeError> {
        self.recorder
            .close(trace_id, status)
            .await
            .map_err(|e| RuntimeError::Node(e.to_string()))
    }

    /// Drain the chain, returning the list of node ids that were
    /// successfully compensated (in execution order, not pop order).
    async fn rollback(
        &self,
        chain: &mut CompensationChain,
        targets: &[NodeId],
        intent: &IntentEnvelope,
    ) -> Result<Vec<NodeId>, RuntimeError> {
        if chain.is_empty() {
            return Ok(vec![]);
        }
        chain.rollback(intent).await?;
        Ok(targets.to_vec())
    }

    /// Convenience: trace id of the most recent run.
    pub async fn fetch_trace(
        &self,
        id: &TraceId,
    ) -> Result<aaf_contracts::ExecutionTrace, RuntimeError> {
        self.recorder
            .get(id)
            .await
            .map_err(|e| RuntimeError::Node(e.to_string()))
    }
}
