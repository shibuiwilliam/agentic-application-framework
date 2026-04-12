//! Cost attribution.
//!
//! PROJECT.md §6.1 specifies that AAF must be able to attribute the
//! cost of a single trace across departments / cost centres. v0.1
//! ships a deterministic in-process attribution: callers register
//! attribution rules (`tag → department`) and apply them to a trace.
//!
//! Real deployments will load these rules from PostgreSQL or a config
//! file; the engine surface stays the same.

use aaf_contracts::ExecutionTrace;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One bucket of attributed cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostBucket {
    /// Department or cost-centre identifier.
    pub department: String,
    /// USD attributed to the bucket.
    pub cost_usd: f64,
    /// Human-readable reason.
    pub reason: String,
}

/// Cost attribution result for a single trace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostAttribution {
    /// Total cost across the trace.
    pub total_cost_usd: f64,
    /// Per-department breakdown.
    pub buckets: Vec<CostBucket>,
}

/// Attribution rule: a step model name (or `*` for all) maps to a
/// department.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributionRule {
    /// Glob: either `*` or an exact model id.
    pub model_glob: String,
    /// Destination department.
    pub department: String,
    /// Reason text recorded in the bucket.
    pub reason: String,
}

/// Attribution engine. Holds an ordered list of rules; the first match
/// per step wins.
#[derive(Debug, Clone, Default)]
pub struct CostAttributor {
    rules: Vec<AttributionRule>,
    /// Department charged when no rule matches.
    pub default_department: String,
}

impl CostAttributor {
    /// Construct.
    pub fn new(default_department: impl Into<String>) -> Self {
        Self {
            rules: vec![],
            default_department: default_department.into(),
        }
    }

    /// Append a rule.
    pub fn add_rule(&mut self, rule: AttributionRule) {
        self.rules.push(rule);
    }

    /// Compute attribution for a single trace.
    pub fn attribute(&self, trace: &ExecutionTrace) -> CostAttribution {
        let mut acc: BTreeMap<String, (f64, String)> = BTreeMap::new();
        for step in &trace.steps {
            let model = step.model.clone().unwrap_or_default();
            let (dept, reason) = self
                .rules
                .iter()
                .find(|r| r.model_glob == "*" || r.model_glob == model)
                .map_or_else(
                    || {
                        (
                            self.default_department.clone(),
                            format!("default for model `{model}`"),
                        )
                    },
                    |r| (r.department.clone(), r.reason.clone()),
                );
            let entry = acc.entry(dept).or_insert((0.0, reason));
            entry.0 += step.cost_usd;
        }
        let buckets = acc
            .into_iter()
            .map(|(dept, (cost, reason))| CostBucket {
                department: dept,
                cost_usd: cost,
                reason,
            })
            .collect();
        CostAttribution {
            total_cost_usd: trace.total_cost_usd,
            buckets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        IntentId, NodeId, Observation, StepOutcome, TraceId, TraceStatus, TraceStep,
    };

    fn step_with_cost(cost: f64, model: &str) -> TraceStep {
        let trace_id = TraceId::from("trace-x");
        TraceStep {
            step: 1,
            node_id: NodeId::from("a"),
            step_type: "node_run".into(),
            model: Some(model.into()),
            tokens_in: 0,
            tokens_out: 0,
            cost_usd: cost,
            duration_ms: 1,
            observation: Observation::minimal(
                trace_id,
                NodeId::from("a"),
                1,
                "agent".into(),
                StepOutcome::Success,
            ),
        }
    }

    fn trace_with(steps: Vec<TraceStep>) -> ExecutionTrace {
        let mut t = ExecutionTrace::open(TraceId::from("trace-x"), IntentId::from("int-x"));
        for s in steps {
            t.record(s);
        }
        t.close(TraceStatus::Completed);
        t
    }

    #[test]
    fn rules_partition_cost_by_department() {
        let mut a = CostAttributor::new("infra");
        a.add_rule(AttributionRule {
            model_glob: "claude-prod".into(),
            department: "marketing".into(),
            reason: "marketing analyst tasks".into(),
        });
        let trace = trace_with(vec![
            step_with_cost(0.40, "claude-prod"),
            step_with_cost(0.10, "small-llm"),
        ]);
        let attr = a.attribute(&trace);
        let by_dept: BTreeMap<_, _> = attr
            .buckets
            .iter()
            .map(|b| (b.department.clone(), b.cost_usd))
            .collect();
        assert!((by_dept["marketing"] - 0.40).abs() < 1e-9);
        assert!((by_dept["infra"] - 0.10).abs() < 1e-9);
    }
}
