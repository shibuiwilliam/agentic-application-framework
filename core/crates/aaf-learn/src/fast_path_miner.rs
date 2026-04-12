//! Fast-path miner (E1 Slice B).
//!
//! Watches agent-assisted observations for recurring patterns. When
//! the same `(intent_type, domain, constraint-key-set)` tuple recurs
//! more than `threshold` times, proposes a new [`FastPathRule`]-shaped
//! [`LearnedRule`] gated by the approval workflow (Rule 18).
//!
//! Adversarial traffic detection (E1 §2.8): patterns whose evidence
//! is concentrated in fewer than `min_distinct_sessions` sessions are
//! rejected — a single user replaying the same intent cannot
//! manipulate the miner.

use aaf_contracts::learn::{LearnedRule, LearnedSource};
use aaf_contracts::{IntentId, Observation, OutcomeStatus};
use aaf_trace::TraceSubscriber;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Key used to group observations into a pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PatternKey {
    intent_type: String,
    domain: String,
    constraint_keys: Vec<String>,
}

/// Evidence for one pattern.
#[derive(Debug, Clone)]
struct PatternEvidence {
    intent_ids: Vec<IntentId>,
    sessions: std::collections::HashSet<String>,
}

/// Miner configuration.
#[derive(Debug, Clone)]
pub struct MinerConfig {
    /// Number of matching observations before the miner proposes.
    pub threshold: usize,
    /// Minimum distinct sessions to avoid adversarial concentration.
    pub min_distinct_sessions: usize,
}

impl Default for MinerConfig {
    fn default() -> Self {
        Self {
            threshold: 10,
            min_distinct_sessions: 3,
        }
    }
}

/// Fast-path miner subscriber.
pub struct FastPathMiner {
    config: MinerConfig,
    /// `PatternKey → evidence` accumulator.
    patterns: Arc<Mutex<HashMap<PatternKey, PatternEvidence>>>,
    /// Proposed rules, keyed by a generated id.
    proposals: Arc<Mutex<Vec<LearnedRule>>>,
}

impl FastPathMiner {
    /// Construct with the given config.
    pub fn new(config: MinerConfig) -> Self {
        Self {
            config,
            patterns: Arc::new(Mutex::new(HashMap::new())),
            proposals: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Return every proposed rule.
    pub fn proposals(&self) -> Vec<LearnedRule> {
        self.proposals.lock().clone()
    }

    /// Count of proposed rules.
    pub fn proposal_count(&self) -> usize {
        self.proposals.lock().len()
    }
}

impl TraceSubscriber for FastPathMiner {
    fn on_observation(&self, obs: &Observation) {
        // Only mine successful outcomes.
        let outcome = match &obs.outcome_detail {
            Some(o) if o.status == OutcomeStatus::Succeeded => o,
            _ => return,
        };
        // Only mine agent-assisted (non-deterministic) steps; the
        // reasoning field carries the node kind annotation from the
        // runtime executor.
        if !obs.reasoning.contains("Agent") {
            return;
        }
        let _ = outcome; // used above for the status check

        // Build the pattern key from the observation's trace_id +
        // node_id. In a real deployment, the miner would receive a
        // richer context (the full IntentEnvelope is carried in the
        // trace step); here we key off the agent field as a proxy
        // for the domain and the node_id as a proxy for the
        // constraint key set.
        let key = PatternKey {
            intent_type: "AgentAssisted".into(),
            domain: obs.agent.to_string(),
            constraint_keys: vec![obs.node_id.to_string()],
        };

        let session = obs.trace_id.to_string();
        let intent_id = IntentId::from(obs.trace_id.as_str());

        let mut patterns = self.patterns.lock();
        let evidence = patterns.entry(key).or_insert_with(|| PatternEvidence {
            intent_ids: vec![],
            sessions: std::collections::HashSet::new(),
        });
        evidence.intent_ids.push(intent_id);
        evidence.sessions.insert(session);

        // Check if the pattern is mature enough to propose.
        if evidence.intent_ids.len() >= self.config.threshold
            && evidence.sessions.len() >= self.config.min_distinct_sessions
        {
            let rule_id = format!(
                "lr-fp-{}-{}",
                evidence.intent_ids.len(),
                evidence.sessions.len()
            );
            let lr = LearnedRule::propose(
                rule_id,
                LearnedSource::Miner,
                evidence.intent_ids.clone(),
                evidence.intent_ids[0].to_string(),
            );
            self.proposals.lock().push(lr);
            // Reset the evidence so we don't propose duplicates.
            evidence.intent_ids.clear();
            evidence.sessions.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::learn::LearnedApprovalState;
    use aaf_contracts::{NodeId, Outcome, StepOutcome, TraceId};

    fn agent_obs(trace_id: &TraceId, node_id: &NodeId) -> Observation {
        let mut obs = Observation::minimal(
            trace_id.clone(),
            node_id.clone(),
            1,
            "commerce".into(),
            StepOutcome::Success,
        );
        obs.reasoning = "ran node test of kind Agent".into();
        obs.outcome_detail = Some(Outcome::minimal(OutcomeStatus::Succeeded, 100, 50, 0.01));
        obs
    }

    #[test]
    fn mines_recurring_pattern_once_over_threshold() {
        let miner = FastPathMiner::new(MinerConfig {
            threshold: 3,
            min_distinct_sessions: 2,
        });
        let node = NodeId::from("node-a");
        // 3 observations across 2 sessions → should propose.
        for i in 0..3 {
            let trace = TraceId::from(format!("trace-{}", i % 2).as_str());
            miner.on_observation(&agent_obs(&trace, &node));
        }
        assert_eq!(miner.proposal_count(), 1);
        assert_eq!(
            miner.proposals()[0].approval_state,
            LearnedApprovalState::Proposed
        );
    }

    #[test]
    fn rejects_adversarial_pattern_concentrated_in_few_sessions() {
        let miner = FastPathMiner::new(MinerConfig {
            threshold: 3,
            min_distinct_sessions: 3,
        });
        let trace = TraceId::from("same-session");
        let node = NodeId::from("node-a");
        // 5 observations all from one session → no proposal.
        for _ in 0..5 {
            miner.on_observation(&agent_obs(&trace, &node));
        }
        assert_eq!(miner.proposal_count(), 0);
    }

    #[test]
    fn produces_proposed_rule_with_evidence() {
        let miner = FastPathMiner::new(MinerConfig {
            threshold: 2,
            min_distinct_sessions: 2,
        });
        let node = NodeId::from("node-a");
        miner.on_observation(&agent_obs(&TraceId::from("t1"), &node));
        miner.on_observation(&agent_obs(&TraceId::from("t2"), &node));
        let proposals = miner.proposals();
        assert_eq!(proposals.len(), 1);
        assert!(!proposals[0].evidence.is_empty());
        assert_eq!(proposals[0].source, LearnedSource::Miner);
    }

    #[test]
    fn ignores_deterministic_observations() {
        let miner = FastPathMiner::new(MinerConfig {
            threshold: 1,
            min_distinct_sessions: 1,
        });
        let mut obs = agent_obs(&TraceId::from("t1"), &NodeId::from("n"));
        obs.reasoning = "ran node x of kind Deterministic".into();
        miner.on_observation(&obs);
        assert_eq!(miner.proposal_count(), 0);
    }

    #[test]
    fn ignores_failed_observations() {
        let miner = FastPathMiner::new(MinerConfig {
            threshold: 1,
            min_distinct_sessions: 1,
        });
        let mut obs = agent_obs(&TraceId::from("t1"), &NodeId::from("n"));
        obs.outcome_detail = Some(Outcome::minimal(OutcomeStatus::Failed, 100, 50, 0.01));
        miner.on_observation(&obs);
        assert_eq!(miner.proposal_count(), 0);
    }
}
