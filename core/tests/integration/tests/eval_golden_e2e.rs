//! End-to-end test for examples/eval-golden.
//!
//! Exercises the `aaf-eval` crate — the offline CI half of the
//! Feedback Spine (Enhancement E1 Slice A):
//!
//! 1. **Golden suite YAML parsing** — the golden-suite.yaml from
//!    examples/eval-golden/ loads and validates correctly.
//!
//! 2. **Suite run with deterministic judge** — a mock provider maps
//!    intents to outputs and the Jaccard-based judge scores each case
//!    against the expected answer.
//!
//! 3. **Perfect provider passes all cases** — a provider that returns
//!    the exact expected string achieves 100% pass rate.
//!
//! 4. **Partial match scores correctly** — a provider that returns
//!    related but imperfect output still clears the threshold for some
//!    cases but not others (e.g. the stricter `min_score: 0.7` case).
//!
//! 5. **Regression detection** — running two suite variants (baseline
//!    vs. candidate) and building a `RegressionReport` surfaces cases
//!    that improved and cases that regressed.
//!
//! 6. **No regression when all improve** — when every candidate score
//!    meets or exceeds the baseline, `has_regression()` returns false.
//!
//! 7. **Trace replay divergence detection** — two execution traces for
//!    the same intent with different costs, latencies, and step types
//!    are compared, and the `Replayer` surfaces every divergence.
//!
//! 8. **Identical traces produce no divergence** — the replayer is
//!    silent when both traces match.
//!
//! 9. **Report generation** — a `ReportDocument` combining suite
//!    results and an optional regression report is serialised to JSON.
//!
//! Run this test with:
//!
//!     cargo test -p aaf-integration-tests --test eval_golden_e2e

use aaf_contracts::{
    ExecutionTrace, IntentId, NodeId, Observation, StepOutcome, TraceId, TraceStatus, TraceStep,
};
use aaf_eval::report::ReportDocument;
use aaf_eval::{
    DeterministicJudge, Divergence, GoldenSuite, Judge, RegressionReport, Replayer, ReportWriter,
};

// ── Helpers ─────────────────────────────────────────────────────────

/// Load the golden suite YAML from the examples directory.
fn load_suite_yaml() -> String {
    let candidates = [
        "examples/eval-golden/golden-suite.yaml",
        "../../examples/eval-golden/golden-suite.yaml",
        "../../../examples/eval-golden/golden-suite.yaml",
    ];
    candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("golden-suite.yaml should exist under examples/eval-golden/")
}

/// Build a single-step execution trace with the given parameters.
fn make_trace(
    trace_id: TraceId,
    cost: f64,
    duration_ms: u64,
    step_type: &str,
    status: TraceStatus,
) -> ExecutionTrace {
    let intent_id = IntentId::new();
    let mut trace = ExecutionTrace::open(trace_id.clone(), intent_id);
    trace.record(TraceStep {
        step: 1,
        node_id: NodeId::from("eval-node"),
        step_type: step_type.into(),
        model: None,
        tokens_in: 100,
        tokens_out: 200,
        cost_usd: cost,
        duration_ms,
        observation: Observation::minimal(
            trace_id,
            NodeId::from("eval-node"),
            1,
            "eval-agent".into(),
            StepOutcome::Success,
        ),
    });
    trace.close(status);
    trace
}

/// Build a two-step execution trace for richer divergence testing.
#[allow(clippy::too_many_arguments)]
fn make_two_step_trace(
    trace_id: TraceId,
    step1_cost: f64,
    step1_ms: u64,
    step1_type: &str,
    step2_cost: f64,
    step2_ms: u64,
    step2_type: &str,
    status: TraceStatus,
) -> ExecutionTrace {
    let intent_id = IntentId::new();
    let mut trace = ExecutionTrace::open(trace_id.clone(), intent_id);
    trace.record(TraceStep {
        step: 1,
        node_id: NodeId::from("step-1"),
        step_type: step1_type.into(),
        model: None,
        tokens_in: 100,
        tokens_out: 200,
        cost_usd: step1_cost,
        duration_ms: step1_ms,
        observation: Observation::minimal(
            trace_id.clone(),
            NodeId::from("step-1"),
            1,
            "eval-agent".into(),
            StepOutcome::Success,
        ),
    });
    trace.record(TraceStep {
        step: 2,
        node_id: NodeId::from("step-2"),
        step_type: step2_type.into(),
        model: None,
        tokens_in: 50,
        tokens_out: 80,
        cost_usd: step2_cost,
        duration_ms: step2_ms,
        observation: Observation::minimal(
            trace_id,
            NodeId::from("step-2"),
            2,
            "eval-agent".into(),
            StepOutcome::Success,
        ),
    });
    trace.close(status);
    trace
}

// ── Tests ───────────────────────────────────────────────────────────

// 1. YAML parsing

#[test]
fn golden_suite_yaml_parses_successfully() {
    let yaml = load_suite_yaml();
    let suite = GoldenSuite::from_yaml(&yaml).expect("golden-suite.yaml should parse");
    assert_eq!(suite.name, "ecommerce-golden");
    assert!((suite.threshold - 0.5).abs() < f64::EPSILON);
    assert_eq!(suite.cases.len(), 6);
    assert_eq!(suite.cases[0].id, "place-order");
    assert_eq!(suite.cases[2].id, "cancel-order");
    assert_eq!(
        suite.cases[2].min_score,
        Some(0.7),
        "cancel-order case should have min_score override"
    );
}

// 2. Suite run with deterministic judge

#[tokio::test]
async fn suite_run_with_echo_provider_scores_all_cases() {
    let yaml = load_suite_yaml();
    let suite = GoldenSuite::from_yaml(&yaml).unwrap();
    let judge = DeterministicJudge::default();

    // Provider echoes back the intent with a prefix — partial overlap.
    let result = suite
        .run(|intent| format!("result: {intent}"), &judge)
        .await;

    assert_eq!(result.suite, "ecommerce-golden");
    assert_eq!(result.total, 6);
    // Every case has a verdict with a positive score (partial overlap).
    for case in &result.cases {
        assert!(
            case.verdict.score > 0.0,
            "case {} should have positive overlap with echo provider",
            case.case_id
        );
    }
}

// 3. Perfect provider passes all cases

#[tokio::test]
async fn perfect_provider_passes_all_cases() {
    let yaml = load_suite_yaml();
    let suite = GoldenSuite::from_yaml(&yaml).unwrap();
    let judge = DeterministicJudge::default();

    // Map each intent to its exact expected output.
    let expected: std::collections::HashMap<&str, &str> = [
        ("place an order for SKU-1", "order placed for SKU-1"),
        ("check stock for SKU-1", "stock level for SKU-1 is 42"),
        ("cancel order ord-123", "order ord-123 cancelled"),
        (
            "show last month sales by region",
            "last month sales report by region",
        ),
        ("check stock for SKU-999", "stock level for SKU-999 is 0"),
        (
            "place a bulk order for 100 units of SKU-5",
            "bulk order placed for 100 units of SKU-5",
        ),
    ]
    .into_iter()
    .collect();

    let result = suite
        .run(
            |intent| (*expected.get(intent).unwrap_or(&"unknown")).to_string(),
            &judge,
        )
        .await;

    assert!(
        result.all_passed(),
        "all cases should pass with perfect answers; failed: {:?}",
        result
            .cases
            .iter()
            .filter(|c| !c.passed)
            .map(|c| &c.case_id)
            .collect::<Vec<_>>()
    );
    assert!((result.mean_score - 1.0).abs() < 1e-9, "mean should be 1.0");
}

// 4. Partial match respects per-case min_score

#[tokio::test]
async fn partial_match_respects_per_case_threshold() {
    let yaml = load_suite_yaml();
    let suite = GoldenSuite::from_yaml(&yaml).unwrap();
    let judge = DeterministicJudge::default();

    // Return a loosely related answer that has some token overlap but
    // not enough to clear the stricter 0.7 threshold on cancel-order.
    let result = suite
        .run(
            |intent| {
                if intent.contains("cancel") {
                    // "order cancelled" overlaps with "order ord-123 cancelled"
                    // but misses "ord-123", lowering the Jaccard score.
                    "order cancelled".to_string()
                } else {
                    // For all other cases return the intent itself — moderate overlap.
                    intent.to_string()
                }
            },
            &judge,
        )
        .await;

    // Find the cancel-order case and verify it did NOT pass the stricter threshold.
    let cancel_case = result
        .cases
        .iter()
        .find(|c| c.case_id == "cancel-order")
        .expect("cancel-order case should exist");
    assert!(
        cancel_case.verdict.score < 0.7,
        "cancel-order Jaccard score ({}) should be below 0.7 for partial answer",
        cancel_case.verdict.score
    );
    assert!(
        !cancel_case.passed,
        "cancel-order should fail its min_score of 0.7"
    );
}

// 5. Regression detection

#[tokio::test]
async fn regression_report_detects_improvements_and_regressions() {
    let yaml = load_suite_yaml();
    let suite = GoldenSuite::from_yaml(&yaml).unwrap();
    let judge = DeterministicJudge::default();

    // Baseline: echo the intent back (moderate overlap).
    let baseline = suite.run(|intent| intent.to_string(), &judge).await;

    // Candidate: improved on some cases, worse on others.
    let candidate = suite
        .run(
            |intent| {
                if intent.contains("stock") {
                    // Exact match for stock queries → improvement.
                    if intent.contains("SKU-999") {
                        "stock level for SKU-999 is 0".to_string()
                    } else {
                        "stock level for SKU-1 is 42".to_string()
                    }
                } else if intent.contains("cancel") {
                    // Completely wrong answer → regression.
                    "unknown command".to_string()
                } else {
                    intent.to_string()
                }
            },
            &judge,
        )
        .await;

    let report = RegressionReport::build(&baseline, &candidate);

    assert!(
        report.improvements > 0,
        "stock cases should show as improvements"
    );
    assert!(
        report.has_regression(),
        "cancel-order should regress (disjoint output)"
    );
    assert!(
        !report.per_case.is_empty(),
        "per-case deltas should be populated"
    );
}

// 6. No regression when all improve

#[tokio::test]
async fn no_regression_when_candidate_matches_or_exceeds_baseline() {
    let yaml = load_suite_yaml();
    let suite = GoldenSuite::from_yaml(&yaml).unwrap();
    let judge = DeterministicJudge::default();

    // Baseline: random noise (low scores).
    let baseline = suite.run(|_| "noise".to_string(), &judge).await;

    // Candidate: echo intent (always >= baseline).
    let candidate = suite.run(|intent| intent.to_string(), &judge).await;

    let report = RegressionReport::build(&baseline, &candidate);
    assert!(
        !report.has_regression(),
        "candidate should not regress against a weak baseline"
    );
    assert!(report.mean_delta > 0.0, "mean should improve");
}

// 7. Trace replay divergence detection

#[test]
fn replayer_detects_cost_latency_and_type_divergences() {
    let replayer = Replayer {
        cost_tolerance_usd: 0.001,
        latency_tolerance_ms: 100,
    };

    let tid = TraceId::new();
    let baseline = make_two_step_trace(
        tid.clone(),
        0.01, // step 1 cost
        100,  // step 1 latency
        "deterministic",
        0.005, // step 2 cost
        50,    // step 2 latency
        "deterministic",
        TraceStatus::Completed,
    );

    let candidate = make_two_step_trace(
        tid,
        0.05, // step 1 cost drifted (0.04 > tolerance 0.001)
        300,  // step 1 latency drifted (200 > tolerance 100)
        "deterministic",
        0.005,       // step 2 cost same
        50,          // step 2 latency same
        "agent_run", // step 2 type changed
        TraceStatus::Completed,
    );

    let divergences = replayer.diverges(&baseline, &candidate);

    let has_cost = divergences
        .iter()
        .any(|d| matches!(d, Divergence::CostDrift { step: 1, .. }));
    let has_latency = divergences
        .iter()
        .any(|d| matches!(d, Divergence::LatencyDrift { step: 1, .. }));
    let has_type = divergences
        .iter()
        .any(|d| matches!(d, Divergence::StepTypeChanged { step: 2, .. }));

    assert!(has_cost, "step 1 cost drift should be detected");
    assert!(has_latency, "step 1 latency drift should be detected");
    assert!(has_type, "step 2 type change should be detected");
}

// 8. Identical traces produce no divergence

#[test]
fn identical_traces_produce_no_divergence() {
    let replayer = Replayer::default();
    let tid = TraceId::new();
    let a = make_trace(
        tid.clone(),
        0.01,
        100,
        "deterministic",
        TraceStatus::Completed,
    );
    let b = make_trace(tid, 0.01, 100, "deterministic", TraceStatus::Completed);

    assert!(
        replayer.diverges(&a, &b).is_empty(),
        "identical traces should produce zero divergences"
    );
}

// 9. Report generation

#[tokio::test]
async fn report_document_serialises_to_json() {
    let yaml = load_suite_yaml();
    let suite = GoldenSuite::from_yaml(&yaml).unwrap();
    let judge = DeterministicJudge::default();

    let baseline = suite.run(|_| "noise".to_string(), &judge).await;
    let candidate = suite.run(|intent| intent.to_string(), &judge).await;

    let regression = RegressionReport::build(&baseline, &candidate);
    let doc = ReportDocument {
        suite: candidate,
        regression: Some(regression),
    };

    let json = ReportWriter::to_json(&doc);
    assert!(json.contains("\"suite\""), "JSON should contain suite key");
    assert!(
        json.contains("\"regression\""),
        "JSON should contain regression key"
    );
    assert!(
        json.contains("\"mean_delta\""),
        "JSON should contain mean_delta"
    );
    assert!(
        json.contains("ecommerce-golden"),
        "JSON should contain suite name"
    );

    // Verify it round-trips through serde.
    let parsed: ReportDocument = serde_json::from_str(&json).expect("report JSON should be valid");
    assert_eq!(parsed.suite.total, 6);
    assert!(parsed.regression.is_some());
}

// 10. Status change divergence

#[test]
fn replayer_detects_status_change() {
    let replayer = Replayer::default();
    let tid = TraceId::new();
    let baseline = make_trace(
        tid.clone(),
        0.01,
        100,
        "deterministic",
        TraceStatus::Completed,
    );
    let candidate = make_trace(tid, 0.01, 100, "deterministic", TraceStatus::Failed);

    let divergences = replayer.diverges(&baseline, &candidate);
    assert!(
        divergences
            .iter()
            .any(|d| matches!(d, Divergence::StatusChanged { .. })),
        "terminal status change should be detected"
    );
}

// 11. Empty suite is rejected

#[test]
fn empty_suite_yaml_is_rejected() {
    let yaml = "name: empty\nthreshold: 0.5\ncases: []\n";
    let err = GoldenSuite::from_yaml(yaml);
    assert!(err.is_err(), "empty suite should be rejected");
}

// 12. Judge scores are deterministic

#[tokio::test]
async fn deterministic_judge_is_reproducible() {
    let judge = DeterministicJudge::default();
    let v1 = judge
        .judge("order placed for SKU-1", "order placed for SKU-1")
        .await;
    let v2 = judge
        .judge("order placed for SKU-1", "order placed for SKU-1")
        .await;

    assert!(
        (v1.score - v2.score).abs() < f64::EPSILON,
        "same inputs should produce identical scores"
    );
    assert!(
        (v1.score - 1.0).abs() < f64::EPSILON,
        "identical strings should score 1.0"
    );
    assert_eq!(v1.judge_model, "deterministic-jaccard");
}
