//! AAF online learning (Enhancement E1 Slice B).
//!
//! Four subscriber modules that consume [`aaf_trace::TraceSubscriber`]
//! notifications and write back adaptations through extension points
//! in the registry, router, planner, and approval workflow:
//!
//! - [`fast_path_miner`] — proposes new fast-path rules from
//!   recurring agent-assisted traffic patterns.
//! - [`capability_scorer`] — outcome-weighted reputation updates on
//!   capabilities.
//! - [`router_tuner`] — adjusts LLM routing weights per
//!   `(intent_type, risk_tier)`.
//! - [`escalation_tuner`] — adjusts approval-threshold hints within
//!   policy-pack bounds.
//!
//! ## Rules enforced by this crate
//!
//! | Rule | Where |
//! |---|---|
//! | 15 Feedback is a contract | Every subscriber reads `Observation.outcome_detail` |
//! | 16 Learning never touches the hot path | `TraceSubscriber::on_observation` is spawned by `tokio::spawn` in the recorder |
//! | 17 Every adaptation is reversible | Every change carries a `LearnedRuleRef` |
//! | 18 Policy governs learning | Learned fast-path rules require `Approved` state before going live |

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod capability_scorer;
pub mod escalation_tuner;
pub mod fast_path_miner;
pub mod router_tuner;

pub use capability_scorer::CapabilityScorer;
pub use escalation_tuner::EscalationTuner;
pub use fast_path_miner::FastPathMiner;
pub use router_tuner::RouterTuner;
