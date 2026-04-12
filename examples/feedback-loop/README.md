# feedback-loop

Demonstrates AAF's **trust lifecycle** and **learning feedback loop** --
two major subsystems that enable agents to earn autonomy over time and
the platform to continuously optimize routing and fast-path coverage.

This is the "human feedback loop" example: trust is earned through
consistent execution, learning proposals require human approval before
they go live, and a single policy violation drops an agent to the
floor.

## What this example covers

### Trust & Autonomy (aaf-trust)

| Feature | Where it's exercised |
|---|---|
| **Score history** | Events (Success, HumanOverride, PolicyViolation, AccuracyRegression) are tracked with deltas and override rate |
| **5-level autonomy** | Numeric scores map to L1 (all approval) through L5 (fully autonomous) via configurable thresholds |
| **Promotion** | 1,000 clean executions with <1% override rate triggers promotion to the next level |
| **Demotion** | Override rate exceeding 5% ceiling triggers demotion |
| **DropToFloor** | Any policy violation drops the agent straight to L1 regardless of history |
| **Delegation chain** | Effective trust = min(delegator, delegatee) -- prevents trust escalation through delegation |

### Learning Subscribers (aaf-learn)

| Feature | Where it's exercised |
|---|---|
| **FastPathMiner** | Watches agent-assisted observations for recurring patterns; proposes fast-path rules after threshold + distinct-session checks |
| **Adversarial rejection** | Patterns concentrated in too few sessions are rejected (prevents replay attacks) |
| **Rule 18 enforcement** | Proposed rules start in `Proposed` state -- never auto-approved |
| **CapabilityScorer** | Nudges per-agent reputation toward 1.0 on success, toward 0.0 on failure |
| **EscalationTuner** | Tracks escalation rate and false-escalation rate (escalated-then-succeeded) |
| **RouterTuner** | Accumulates per-bucket success rate and average cost statistics |
| **Recorder integration** | All four subscribers attach to the trace Recorder and receive observations during graph execution |

## Files

```
examples/feedback-loop/
├── README.md       <- this file
└── aaf.yaml        <- two agents at different trust levels + learning config
```

## Run the tests

```bash
cargo test -p aaf-integration-tests --test feedback_loop_e2e
```

Expected output:

```text
running 21 tests
test score_history_tracks_events_and_override_rate ... ok
test autonomy_policy_maps_scores_to_levels ... ok
test example_agents_resolve_to_expected_levels ... ok
test promotion_after_many_clean_executions ... ok
test hold_when_insufficient_executions ... ok
test hold_at_max_level ... ok
test demotion_when_override_rate_exceeds_ceiling ... ok
test policy_violation_drops_to_floor ... ok
test delegation_chain_uses_min_trust ... ok
test require_rejects_insufficient_trust ... ok
test miner_proposes_after_threshold_and_distinct_sessions ... ok
test miner_rejects_insufficient_sessions ... ok
test learned_rules_require_approval_before_live ... ok
test scorer_increases_on_success ... ok
test scorer_decreases_on_failure ... ok
test scorer_mixed_results_intermediate_score ... ok
test escalation_tuner_tracks_rates ... ok
test router_tuner_accumulates_bucket_stats ... ok
test subscribers_integrate_with_recorder_during_execution ... ok
test full_trust_lifecycle_promotion_then_violation_drop ... ok
test aaf_yaml_loads_successfully ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## What each test proves

### Trust & Autonomy (tests 1-6)

**1. `score_history_tracks_events_and_override_rate`**
98 successes + 2 overrides = 2% override rate. Verifies all counters
(total, success, human_override) and the override_rate() computation.

**2. `autonomy_policy_maps_scores_to_levels`**
Default thresholds (L2=0.55, L3=0.70, L4=0.85, L5=0.95) resolve
correctly at every boundary.

**2b. `example_agents_resolve_to_expected_levels`**
agent-senior (score 0.75) resolves to L3; agent-junior (score 0.60)
resolves to L2 -- matching the aaf.yaml configuration.

**3. `promotion_after_many_clean_executions`**
1,000 successes with 0% override rate triggers `Promote` from L3.

**3b. `hold_when_insufficient_executions`**
500 executions (below the 1,000 threshold) produces `Hold`.

**3c. `hold_at_max_level`**
An agent already at L5 cannot be promoted further.

**4. `demotion_when_override_rate_exceeds_ceiling`**
90 successes + 10 overrides = 10% override rate (exceeds the 5%
ceiling) triggers `Demote`.

**5. `policy_violation_drops_to_floor`**
Even after 999 successes, a single policy violation triggers
`DropToFloor` -- the agent goes straight to L1.

**6. `delegation_chain_uses_min_trust`**
L5 delegates to L2 -> effective L2. Chained delegation (L5 -> L3 -> L4)
preserves the weakest link at L3.

**6b. `require_rejects_insufficient_trust`**
The `require()` guard rejects effective L2 when L4 is required, and
passes when the effective level meets or exceeds the requirement.

### Learning Subscribers (tests 7-16)

**7. `miner_proposes_after_threshold_and_distinct_sessions`**
3 agent observations across 3 distinct traces (sessions) trigger a
rule proposal when threshold=3 and min_distinct_sessions=2.

**8. `miner_rejects_insufficient_sessions`**
5 observations from a single session do not trigger a proposal when
min_distinct_sessions=3 (adversarial concentration protection).

**9. `learned_rules_require_approval_before_live`**
Proposed rules start in `Proposed` state. Only after calling
`.approve()` does `is_live()` return true. This enforces Rule 18:
policy governs all learning adaptations.

**10. `scorer_increases_on_success`**
5 successful observations push the capability score above 0.5.

**11. `scorer_decreases_on_failure`**
5 failed observations push the capability score below 0.5.

**11b. `scorer_mixed_results_intermediate_score`**
3 successes then 3 failures produce an intermediate score near 0.5.

**12. `escalation_tuner_tracks_rates`**
4 non-escalated observations produce escalation_rate=0.0.

**13. `router_tuner_accumulates_bucket_stats`**
3 observations (cost 0.02 each) accumulate to count=3, total_cost=0.06.

**14. `subscribers_integrate_with_recorder_during_execution`**
All four subscribers (miner, scorer, escalation, router) are attached
to a Recorder and receive observations during actual graph execution.
Verifies the scorer accumulates, escalation counts, and router tracks
-- while the miner correctly ignores deterministic observations.

**15. `full_trust_lifecycle_promotion_then_violation_drop`**
Complete lifecycle: agent starts at L2, accumulates 1,000 successes,
gets promoted to L3, then a single policy violation drops it to L1.

**16. `aaf_yaml_loads_successfully`**
Loads `examples/feedback-loop/aaf.yaml` and verifies both agents and
the learning subscriber configuration parse correctly.

## Architecture rules exercised

| Rule | How |
|---|---|
| **Rule 3** (Autonomy Ladder) | 5 levels with configurable thresholds, earned through execution |
| **Rule 15** (Feedback is a Contract) | Subscribers read `Observation.outcome_detail` from the trace |
| **Rule 16** (Learning off hot path) | Subscribers run as Recorder callbacks, not blocking the executor |
| **Rule 17** (Adaptation is reversible) | LearnedRule carries evidence + approval state for traceability |
| **Rule 18** (Policy governs learning) | Proposed rules require explicit approval before going live |

## See also

- `PROJECT.md` -- trust scoring and autonomy levels
- `PROJECT.md` -- promotion/demotion criteria (overrides, violations)
- `CLAUDE.md` -- Enhancement E1 (Feedback Spine) design
- `examples/hello-agent/` -- simplest AAF pipeline (read-only)
- `examples/order-saga/` -- multi-step saga with compensation
- `examples/resilient-query/` -- guards, degradation, budget
- `examples/signed-agent/` -- identity + provenance CLI walkthrough
