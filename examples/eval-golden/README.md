# eval-golden

Demonstrates AAF's **offline evaluation harness** (`aaf-eval`) — the
CI half of the Feedback Spine (Enhancement E1 Slice A). Loads a
curated golden suite of `(intent, expected_output)` cases, scores
each case with a deterministic Jaccard-based judge, builds regression
reports comparing baseline vs. candidate runs, and detects trace-level
divergences (cost drift, latency drift, step type changes) between two
execution runs of the same intent.

## What it exercises

- Golden suite YAML loading and validation
- `DeterministicJudge` scoring (Jaccard word overlap)
- `SuiteResult` aggregation (pass/fail per case, mean score)
- Per-case `min_score` threshold override
- `RegressionReport` building (improvements + regressions)
- `Replayer` divergence detection (cost drift, latency drift, step
  type change, status change, missing steps)
- `ReportWriter` JSON serialisation for CI consumption

## Files

- `aaf.yaml` — project config with four e-commerce capabilities and
  eval settings (judge type, replay tolerances)
- `golden-suite.yaml` — six golden cases covering order placement,
  stock query, order cancellation, sales reporting, and edge cases

## Run it

```bash
# Run the full evaluation test suite
cargo test -p aaf-integration-tests --test eval_golden_e2e
```

12 tests exercise: YAML parsing, echo provider scoring, perfect
provider pass, partial match threshold enforcement, regression
detection (improvements + regressions), no-regression baseline,
multi-step trace divergence, identical trace silence, report JSON
round-trip, status change detection, empty suite rejection,
deterministic judge reproducibility.
