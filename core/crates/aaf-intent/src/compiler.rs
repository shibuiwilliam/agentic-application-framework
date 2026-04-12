//! Compiler — orchestrates classification, extraction, enrichment,
//! refinement and caching into a single `compile` call.

use crate::cache::IntentCache;
use crate::classifier::{Classifier, RuleClassifier};
use crate::enricher::Enricher;
use crate::error::IntentError;
use crate::extractor::{Extractor, RuleExtractor};
use crate::refinement::{ClarificationQuestion, Refiner};
use aaf_contracts::{
    BudgetContract, IntentEnvelope, IntentId, IntentType, Requester, RiskTier, TraceId,
};
use chrono::Utc;
use std::sync::Arc;

/// Outcome of a compile.
#[derive(Debug)]
pub enum CompileOutcome {
    /// Successful compile (envelope is validated).
    Compiled(IntentEnvelope),
    /// Needs clarification before progressing.
    NeedsRefinement(Vec<ClarificationQuestion>),
}

/// Intent compiler.
pub struct IntentCompiler {
    classifier: Arc<dyn Classifier>,
    extractor: Arc<dyn Extractor>,
    cache: IntentCache,
}

impl std::fmt::Debug for IntentCompiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntentCompiler")
            .field("cache_size", &self.cache.len())
            .finish_non_exhaustive()
    }
}

impl Default for IntentCompiler {
    fn default() -> Self {
        Self::new(Arc::new(RuleClassifier), Arc::new(RuleExtractor))
    }
}

impl IntentCompiler {
    /// Construct with explicit components.
    pub fn new(classifier: Arc<dyn Classifier>, extractor: Arc<dyn Extractor>) -> Self {
        Self {
            classifier,
            extractor,
            cache: IntentCache::new(),
        }
    }

    /// Run the full pipeline.
    pub fn compile(
        &self,
        raw_input: &str,
        requester: Requester,
        domain: impl Into<String>,
        budget: BudgetContract,
    ) -> Result<CompileOutcome, IntentError> {
        if let Some(cached) = self.cache.get(raw_input) {
            return Ok(CompileOutcome::Compiled(cached));
        }

        let intent_type = self
            .classifier
            .classify(raw_input)
            .ok_or(IntentError::ClassificationFailed)?;
        let constraints = self.extractor.extract(raw_input);

        let risk_tier = match intent_type {
            IntentType::TransactionalIntent => RiskTier::Write,
            IntentType::AnalyticalIntent => RiskTier::Read,
            IntentType::PlanningIntent => RiskTier::Advisory,
            IntentType::DelegationIntent => RiskTier::Delegation,
            IntentType::GovernanceIntent => RiskTier::Governance,
        };

        let mut envelope = IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type,
            requester,
            goal: raw_input.trim().to_string(),
            domain: domain.into(),
            constraints,
            budget,
            deadline: None,
            risk_tier,
            approval_policy: "human".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        };

        Enricher::enrich(&mut envelope);

        let questions = Refiner::questions_for(&envelope);
        if !questions.is_empty()
            && questions
                .iter()
                .any(|q| matches!(q.field, "goal" | "domain"))
        {
            return Ok(CompileOutcome::NeedsRefinement(questions));
        }

        envelope.validate()?;
        self.cache.put(raw_input, envelope.clone());
        Ok(CompileOutcome::Compiled(envelope))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requester() -> Requester {
        Requester {
            user_id: "tanaka".into(),
            role: "analyst".into(),
            scopes: vec!["sales:read".into()],
            tenant: None,
        }
    }

    fn budget() -> BudgetContract {
        BudgetContract {
            max_tokens: 5000,
            max_cost_usd: 1.0,
            max_latency_ms: 30_000,
        }
    }

    #[test]
    fn end_to_end_japanese_analytical() {
        let c = IntentCompiler::default();
        let outcome = c
            .compile("先月の売上を地域別に見たい", requester(), "sales", budget())
            .expect("compile");
        match outcome {
            CompileOutcome::Compiled(env) => {
                assert_eq!(env.intent_type, IntentType::AnalyticalIntent);
                assert_eq!(env.risk_tier, RiskTier::Read);
                assert_eq!(env.approval_policy, "none");
                assert_eq!(
                    env.constraints.get("dimension"),
                    Some(&serde_json::json!("region"))
                );
            }
            CompileOutcome::NeedsRefinement(_) => panic!("expected compiled envelope"),
        }
    }

    #[test]
    fn second_compile_hits_cache() {
        let c = IntentCompiler::default();
        let _ = c
            .compile("先月の売上を地域別に見たい", requester(), "sales", budget())
            .unwrap();
        assert_eq!(c.cache.len(), 1);
        let _ = c
            .compile("先月の売上を地域別に見たい", requester(), "sales", budget())
            .unwrap();
        // still 1 because cache hit, no re-insert
        assert_eq!(c.cache.len(), 1);
    }

    #[test]
    fn unknown_input_fails_classification() {
        let c = IntentCompiler::default();
        let err = c
            .compile("zzzzzzz", requester(), "sales", budget())
            .unwrap_err();
        assert!(matches!(err, IntentError::ClassificationFailed));
    }
}
