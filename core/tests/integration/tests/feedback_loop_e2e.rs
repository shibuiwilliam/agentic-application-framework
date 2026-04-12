//! End-to-end test for examples/feedback-loop.
//!
//! Exercises AAF's **trust lifecycle** and **learning feedback loop** —
//! two major subsystems not covered by the other examples:
//!
//! **Trust & Autonomy (aaf-trust):**
//!
//! 1.  Score history tracks events and computes override rate.
//! 2.  Autonomy policy maps scores to 5 levels (L1–L5).
//! 3.  Promotion after 1,000 clean executions.
//! 4.  Demotion when override rate exceeds ceiling.
//! 5.  DropToFloor on any policy violation.
//! 6.  Delegation chain: effective trust = min(delegator, delegatee).
//!
//! **Learning Subscribers (aaf-learn):**
//!
//! 7.  FastPathMiner proposes rules after threshold observations across
//!     distinct sessions.
//! 8.  Miner rejects patterns with too few distinct sessions.
//! 9.  Proposed rules start in `Proposed` state (Rule 18).
//! 10. CapabilityScorer nudges reputation toward 1.0 on success.
//! 11. CapabilityScorer nudges reputation toward 0.0 on failure.
//! 12. EscalationTuner tracks escalation and false-escalation rates.
//! 13. RouterTuner accumulates per-bucket success rate and cost.
//! 14. Subscribers integrate with the Recorder during graph execution.
//!
//! Run this test with:
//!
//!     cargo test -p aaf-integration-tests --test feedback_loop_e2e

use aaf_contracts::learn::LearnedApprovalState;
use aaf_contracts::{
    AutonomyLevel, BudgetContract, IntentEnvelope, IntentId, IntentType, NodeId, Observation,
    Outcome, OutcomeStatus, Requester, RiskTier, SideEffect, StepOutcome, TraceId,
};
use aaf_learn::capability_scorer::ScorerConfig;
use aaf_learn::fast_path_miner::MinerConfig;
use aaf_learn::{CapabilityScorer, EscalationTuner, FastPathMiner, RouterTuner};
use aaf_policy::PolicyEngine;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node};
use aaf_trace::{Recorder, TraceRecorder, TraceSubscriber};
use aaf_trust::{
    effective_trust, AutonomyPolicy, PromotionDecision, PromotionRules, ScoreEvent, ScoreHistory,
};
use chrono::Utc;
use std::sync::Arc;

// ── Helpers ─────────────────────────────────────────────────────────

fn sample_intent(domain: &str, scopes: Vec<&str>) -> IntentEnvelope {
    IntentEnvelope {
        intent_id: IntentId::new(),
        intent_type: IntentType::AnalyticalIntent,
        requester: Requester {
            user_id: "user-tanaka".into(),
            role: "analyst".into(),
            scopes: scopes.into_iter().map(String::from).collect(),
            tenant: None,
        },
        goal: format!("run {domain} report"),
        domain: domain.into(),
        constraints: Default::default(),
        budget: BudgetContract {
            max_tokens: 5_000,
            max_cost_usd: 1.0,
            max_latency_ms: 30_000,
        },
        deadline: None,
        risk_tier: RiskTier::Read,
        approval_policy: "none".into(),
        output_contract: None,
        trace_id: TraceId::new(),
        depth: 0,
        created_at: Utc::now(),
        entities_in_context: vec![],
    }
}

fn agent_observation(trace_id: &str, node_id: &str, agent: &str, succeeded: bool) -> Observation {
    let outcome_status = if succeeded {
        OutcomeStatus::Succeeded
    } else {
        OutcomeStatus::Failed
    };
    let mut obs = Observation::minimal(
        TraceId::from(trace_id),
        NodeId::from(node_id),
        1,
        agent.into(),
        StepOutcome::Success,
    );
    obs.reasoning = format!("ran node {node_id} of kind Agent");
    obs.outcome_detail = Some(Outcome::minimal(outcome_status, 200, 100, 0.02));
    obs
}

// ════════════════════════════════════════════════════════════════════
// TRUST & AUTONOMY TESTS
// ════════════════════════════════════════════════════════════════════

/// 1. Score history tracks success, override, and violation events
///    and computes the correct override rate.
#[test]
fn score_history_tracks_events_and_override_rate() {
    let mut history = ScoreHistory::default();
    assert_eq!(history.total, 0);
    assert_eq!(history.override_rate(), 0.0);

    // 98 successes + 2 overrides = 2% override rate
    for _ in 0..98 {
        history.observe(ScoreEvent::Success);
    }
    for _ in 0..2 {
        history.observe(ScoreEvent::HumanOverride);
    }

    assert_eq!(history.total, 100);
    assert_eq!(history.success, 98);
    assert_eq!(history.human_override, 2);
    assert!((history.override_rate() - 0.02).abs() < 1e-9);
    assert!(history.last_updated.is_some());
}

/// 2. Autonomy policy maps numeric scores to the correct 5 levels.
#[test]
fn autonomy_policy_maps_scores_to_levels() {
    let policy = AutonomyPolicy::default();

    // Default thresholds: L2=0.55, L3=0.70, L4=0.85, L5=0.95
    assert_eq!(policy.level_for(0.00), AutonomyLevel::Level1);
    assert_eq!(policy.level_for(0.54), AutonomyLevel::Level1);
    assert_eq!(policy.level_for(0.55), AutonomyLevel::Level2);
    assert_eq!(policy.level_for(0.69), AutonomyLevel::Level2);
    assert_eq!(policy.level_for(0.70), AutonomyLevel::Level3);
    assert_eq!(policy.level_for(0.84), AutonomyLevel::Level3);
    assert_eq!(policy.level_for(0.85), AutonomyLevel::Level4);
    assert_eq!(policy.level_for(0.94), AutonomyLevel::Level4);
    assert_eq!(policy.level_for(0.95), AutonomyLevel::Level5);
    assert_eq!(policy.level_for(1.00), AutonomyLevel::Level5);
}

/// 2b. The two agents from aaf.yaml resolve to the expected levels.
#[test]
fn example_agents_resolve_to_expected_levels() {
    let policy = AutonomyPolicy::default();

    // agent-senior: score 0.75 → L3 (threshold 0.70)
    assert_eq!(policy.level_for(0.75), AutonomyLevel::Level3);

    // agent-junior: score 0.60 → L2 (threshold 0.55)
    assert_eq!(policy.level_for(0.60), AutonomyLevel::Level2);
}

/// 3. Promotion: after 1,000 clean executions with near-zero override
///    rate, the agent is promoted.
#[test]
fn promotion_after_many_clean_executions() {
    let rules = PromotionRules::default();
    let mut history = ScoreHistory::default();

    // 1,000 successes, 0 overrides → override_rate = 0%
    for _ in 0..1_000 {
        history.observe(ScoreEvent::Success);
    }

    let decision = rules.evaluate(AutonomyLevel::Level3, &history);
    assert_eq!(
        decision,
        PromotionDecision::Promote,
        "1000 successes, 0% override → promote"
    );
}

/// 3b. Hold when not enough executions for promotion.
#[test]
fn hold_when_insufficient_executions() {
    let rules = PromotionRules::default();
    let mut history = ScoreHistory::default();

    // Only 500 successes — below the 1,000 threshold
    for _ in 0..500 {
        history.observe(ScoreEvent::Success);
    }

    let decision = rules.evaluate(AutonomyLevel::Level3, &history);
    assert_eq!(
        decision,
        PromotionDecision::Hold,
        "500 executions < 1000 threshold → hold"
    );
}

/// 3c. Hold at Level5 — cannot promote further.
#[test]
fn hold_at_max_level() {
    let rules = PromotionRules::default();
    let mut history = ScoreHistory::default();
    for _ in 0..2_000 {
        history.observe(ScoreEvent::Success);
    }

    let decision = rules.evaluate(AutonomyLevel::Level5, &history);
    assert_eq!(decision, PromotionDecision::Hold, "already at L5 → hold");
}

/// 4. Demotion: high override rate (>5%) triggers demotion.
#[test]
fn demotion_when_override_rate_exceeds_ceiling() {
    let rules = PromotionRules::default();
    let mut history = ScoreHistory::default();

    // 90 successes + 10 overrides = 10% override rate (ceiling is 5%)
    for _ in 0..90 {
        history.observe(ScoreEvent::Success);
    }
    for _ in 0..10 {
        history.observe(ScoreEvent::HumanOverride);
    }

    let decision = rules.evaluate(AutonomyLevel::Level3, &history);
    assert_eq!(
        decision,
        PromotionDecision::Demote,
        "10% override rate > 5% ceiling → demote"
    );
}

/// 5. DropToFloor: any policy violation drops the agent to L1.
#[test]
fn policy_violation_drops_to_floor() {
    let rules = PromotionRules::default();
    let mut history = ScoreHistory::default();

    // 999 successes then 1 policy violation
    for _ in 0..999 {
        history.observe(ScoreEvent::Success);
    }
    history.observe(ScoreEvent::PolicyViolation);

    let decision = rules.evaluate(AutonomyLevel::Level4, &history);
    assert_eq!(
        decision,
        PromotionDecision::DropToFloor,
        "any policy violation → drop to L1 regardless of history"
    );
}

/// 6. Delegation chain: effective trust = min(delegator, delegatee).
#[test]
fn delegation_chain_uses_min_trust() {
    // L5 delegates to L2 → effective is L2
    assert_eq!(
        effective_trust(AutonomyLevel::Level5, AutonomyLevel::Level2),
        AutonomyLevel::Level2,
    );

    // L2 delegates to L4 → effective is L2
    assert_eq!(
        effective_trust(AutonomyLevel::Level2, AutonomyLevel::Level4),
        AutonomyLevel::Level2,
    );

    // L3 delegates to L3 → effective is L3
    assert_eq!(
        effective_trust(AutonomyLevel::Level3, AutonomyLevel::Level3),
        AutonomyLevel::Level3,
    );

    // Chained delegation: L5 → L3 → L4
    // First hop: min(L5, L3) = L3
    // Second hop: min(L3, L4) = L3
    let hop1 = effective_trust(AutonomyLevel::Level5, AutonomyLevel::Level3);
    let hop2 = effective_trust(hop1, AutonomyLevel::Level4);
    assert_eq!(
        hop2,
        AutonomyLevel::Level3,
        "chain preserves the weakest link"
    );
}

/// 6b. The `require` function rejects insufficient trust levels.
#[test]
fn require_rejects_insufficient_trust() {
    use aaf_trust::delegation::require;

    // Effective L2 does not meet required L4
    let err = require(AutonomyLevel::Level4, AutonomyLevel::Level2).unwrap_err();
    assert!(
        format!("{err}").contains("required"),
        "error should describe the gap"
    );

    // Effective L3 meets required L2
    require(AutonomyLevel::Level2, AutonomyLevel::Level3).unwrap();

    // Equal levels pass
    require(AutonomyLevel::Level3, AutonomyLevel::Level3).unwrap();
}

// ════════════════════════════════════════════════════════════════════
// LEARNING SUBSCRIBER TESTS
// ════════════════════════════════════════════════════════════════════

/// 7. FastPathMiner proposes a rule after threshold observations
///    across enough distinct sessions.
#[test]
fn miner_proposes_after_threshold_and_distinct_sessions() {
    let miner = FastPathMiner::new(MinerConfig {
        threshold: 3,
        min_distinct_sessions: 2,
    });

    // Feed 3 agent observations from 3 distinct traces (sessions)
    for i in 0..3 {
        let obs = agent_observation(&format!("trace-{i}"), "cap-sales-report", "sales", true);
        miner.on_observation(&obs);
    }

    assert_eq!(
        miner.proposal_count(),
        1,
        "3 observations × 3 sessions ≥ threshold(3) + min_sessions(2)"
    );

    let proposals = miner.proposals();
    assert!(
        !proposals[0].evidence.is_empty(),
        "proposal carries evidence"
    );
    assert!(!proposals[0].scope.is_empty(), "scope is populated");
}

/// 8. Miner rejects patterns with too few distinct sessions
///    (adversarial pattern protection).
#[test]
fn miner_rejects_insufficient_sessions() {
    let miner = FastPathMiner::new(MinerConfig {
        threshold: 3,
        min_distinct_sessions: 3,
    });

    // 5 observations but all from the same trace (1 session)
    for _ in 0..5 {
        let obs = agent_observation("same-trace", "cap-sales-report", "sales", true);
        miner.on_observation(&obs);
    }

    assert_eq!(
        miner.proposal_count(),
        0,
        "1 session < min_distinct_sessions(3) → no proposal"
    );
}

/// 9. Proposed rules start in `Proposed` state and can be approved
///    to go live (Rule 18: policy governs learning).
#[test]
fn learned_rules_require_approval_before_live() {
    let miner = FastPathMiner::new(MinerConfig {
        threshold: 2,
        min_distinct_sessions: 2,
    });

    for i in 0..2 {
        let obs = agent_observation(&format!("t-{i}"), "cap-order-process", "orders", true);
        miner.on_observation(&obs);
    }

    let proposals = miner.proposals();
    assert_eq!(proposals.len(), 1);

    // Rule 18: proposed, not auto-approved
    assert_eq!(
        proposals[0].approval_state,
        LearnedApprovalState::Proposed,
        "learned rules must start in Proposed state"
    );
    assert!(!proposals[0].is_live(), "not live before approval");

    // Approve the rule
    let mut rule = proposals[0].clone();
    rule.approve();
    assert!(rule.is_live(), "live after approval");
    assert_eq!(rule.approval_state, LearnedApprovalState::Approved);
}

/// 10. CapabilityScorer nudges reputation toward 1.0 on success.
#[test]
fn scorer_increases_on_success() {
    let scorer = CapabilityScorer::new(ScorerConfig::default());

    // Feed 5 successful observations
    for i in 0..5 {
        let obs = agent_observation(&format!("t-{i}"), "cap-sales-report", "sales-agent", true);
        scorer.on_observation(&obs);
    }

    let score = scorer.score_for("sales-agent");
    assert!(
        score > 0.5,
        "score should exceed 0.5 after 5 successes, got {score}"
    );
}

/// 11. CapabilityScorer nudges reputation toward 0.0 on failure.
#[test]
fn scorer_decreases_on_failure() {
    let scorer = CapabilityScorer::new(ScorerConfig::default());

    // Feed 5 failed observations
    for i in 0..5 {
        let obs = agent_observation(&format!("t-{i}"), "cap-order-process", "order-agent", false);
        scorer.on_observation(&obs);
    }

    let score = scorer.score_for("order-agent");
    assert!(
        score < 0.5,
        "score should fall below 0.5 after 5 failures, got {score}"
    );
}

/// 11b. Mixed success/failure produces an intermediate score.
#[test]
fn scorer_mixed_results_intermediate_score() {
    let scorer = CapabilityScorer::new(ScorerConfig::default());

    // 3 successes then 3 failures
    for i in 0..3 {
        let obs = agent_observation(&format!("s-{i}"), "cap-mixed", "mixed-agent", true);
        scorer.on_observation(&obs);
    }
    for i in 0..3 {
        let obs = agent_observation(&format!("f-{i}"), "cap-mixed", "mixed-agent", false);
        scorer.on_observation(&obs);
    }

    let score = scorer.score_for("mixed-agent");
    // After 3 successes nudging up and 3 failures nudging down,
    // score should be near 0.5 (the starting point).
    assert!(
        (0.3..=0.7).contains(&score),
        "mixed results should yield ~0.5, got {score}"
    );
}

/// 12. EscalationTuner tracks escalation and false-escalation rates.
#[test]
fn escalation_tuner_tracks_rates() {
    let tuner = EscalationTuner::new();

    // Feed 4 non-escalated observations
    for i in 0..4 {
        let obs = agent_observation(&format!("t-{i}"), "cap-sales", "sales", true);
        tuner.on_observation(&obs);
    }

    let stats = tuner.stats();
    assert_eq!(stats.total, 4);
    assert_eq!(stats.escalated, 0);
    assert_eq!(stats.escalation_rate(), 0.0);
    assert_eq!(stats.false_escalation_rate(), 0.0);
}

/// 13. RouterTuner accumulates per-bucket success rate and average cost.
#[test]
fn router_tuner_accumulates_bucket_stats() {
    let tuner = RouterTuner::new();

    // Feed 3 observations: 2 successes + 1 failure
    for i in 0..2 {
        let obs = agent_observation(&format!("t-{i}"), "cap-sales", "sales", true);
        tuner.on_observation(&obs);
    }
    let obs = agent_observation("t-2", "cap-sales", "sales", false);
    tuner.on_observation(&obs);

    let all_stats = tuner.stats();
    let total_count: u64 = all_stats.values().map(|b| b.count).sum();
    assert_eq!(total_count, 3, "3 observations tracked");

    // All observations have cost_usd=0.02
    let total_cost: f64 = all_stats.values().map(|b| b.total_cost).sum();
    assert!((total_cost - 0.06).abs() < 1e-9, "total cost = 3 × 0.02");
}

/// 14. Full integration: subscribers attached to Recorder receive
///     observations during graph execution and accumulate state.
#[tokio::test]
async fn subscribers_integrate_with_recorder_during_execution() {
    // Set up all four learning subscribers
    let miner = Arc::new(FastPathMiner::new(MinerConfig {
        threshold: 3,
        min_distinct_sessions: 2,
    }));
    let scorer = Arc::new(CapabilityScorer::new(ScorerConfig::default()));
    let escalation = Arc::new(EscalationTuner::new());
    let router = Arc::new(RouterTuner::new());

    let recorder: Arc<dyn TraceRecorder> = Arc::new(
        Recorder::in_memory()
            .with_subscriber(miner.clone())
            .with_subscriber(scorer.clone())
            .with_subscriber(escalation.clone())
            .with_subscriber(router.clone()),
    );

    let policy = Arc::new(PolicyEngine::with_default_rules());

    // Run 4 intents through the executor, each with distinct trace ids
    for i in 0..4 {
        let mut intent = sample_intent("sales", vec!["sales:read"]);
        intent.trace_id = TraceId::from(format!("fb-trace-{i}").as_str());

        let node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
            NodeId::from("cap-sales-report"),
            SideEffect::Read,
            Arc::new(move |_, _| Ok(serde_json::json!({"rows": 47}))),
        ));
        let graph = GraphBuilder::new().add_node(node).build().unwrap();

        let exec = GraphExecutor::new(policy.clone(), recorder.clone(), intent.budget);
        let outcome = exec.run(&graph, &intent).await.unwrap();
        assert!(matches!(outcome, ExecutionOutcome::Completed { .. }));
    }

    // Yield for subscriber background tasks
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;

    // Scorer should have accumulated scores from the "runtime" agent
    let scores = scorer.all_scores();
    if let Some(s) = scores.get("runtime") {
        assert!(*s >= 0.5, "4 successes → score ≥ 0.5, got {s}");
    }

    // Escalation tuner should have counted 4 non-escalated observations
    let esc = escalation.stats();
    assert_eq!(esc.total, 4, "4 observations");
    assert_eq!(esc.escalated, 0, "no escalations");

    // Router tuner should have accumulated cost stats
    let router_stats = router.stats();
    let total: u64 = router_stats.values().map(|b| b.count).sum();
    assert_eq!(total, 4, "4 observations in router tuner");

    // Miner should NOT propose from deterministic observations
    assert_eq!(
        miner.proposal_count(),
        0,
        "miner only mines agent-kind observations, not deterministic"
    );
}

/// 15. Full trust lifecycle story: an agent starts at L2, accumulates
///     successes to reach promotion threshold, gets promoted to L3, then
///     a policy violation drops it to L1.
#[test]
fn full_trust_lifecycle_promotion_then_violation_drop() {
    let policy = AutonomyPolicy::default();
    let rules = PromotionRules::default();
    let mut history = ScoreHistory::default();

    // Agent starts at L2 (score 0.60)
    let starting_level = AutonomyLevel::Level2;
    assert_eq!(policy.level_for(0.60), starting_level);

    // Phase 1: accumulate 1,000 successes — override rate stays at 0%
    for _ in 0..1_000 {
        history.observe(ScoreEvent::Success);
    }

    // Evaluate promotion from the agent's starting level (L2).
    // The promotion engine sees: 1000 executions, 0% override → Promote.
    let decision = rules.evaluate(starting_level, &history);
    assert_eq!(
        decision,
        PromotionDecision::Promote,
        "1000 successes, 0% override → promote from L2"
    );

    // The agent is now at L3.
    let promoted_level = AutonomyLevel::Level3;

    // Phase 2: one policy violation — drops to floor regardless
    history.observe(ScoreEvent::PolicyViolation);

    let decision = rules.evaluate(promoted_level, &history);
    assert_eq!(
        decision,
        PromotionDecision::DropToFloor,
        "policy violation → drop to L1 regardless of history"
    );
}

/// 16. YAML config loads and parses successfully.
#[test]
fn aaf_yaml_loads_successfully() {
    let candidates = [
        "examples/feedback-loop/aaf.yaml",
        "../../examples/feedback-loop/aaf.yaml",
        "../../../examples/feedback-loop/aaf.yaml",
    ];
    let yaml = candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("aaf.yaml should exist under examples/feedback-loop/");

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&yaml).expect("aaf.yaml should be valid YAML");

    let agents = doc
        .get("agents")
        .expect("should have 'agents' key")
        .as_sequence()
        .expect("agents should be a sequence");
    assert_eq!(agents.len(), 2, "two agents defined");

    let senior = &agents[0];
    assert_eq!(
        senior.get("id").and_then(|v| v.as_str()).unwrap(),
        "agent-senior"
    );
    let senior_score = senior
        .get("initial_score")
        .and_then(|v| v.as_f64())
        .unwrap();
    assert!((senior_score - 0.75).abs() < 1e-9);

    let learning = doc.get("learning").expect("should have 'learning' key");
    let threshold = learning
        .get("fast_path_miner")
        .and_then(|m| m.get("threshold"))
        .and_then(|v| v.as_u64())
        .unwrap();
    assert_eq!(threshold, 3);
}
