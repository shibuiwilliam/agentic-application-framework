//! AAF Evaluation Harness (Enhancement E1 Slice A).
//!
//! `aaf-eval` is the offline / CI half of the Feedback Spine. It
//! exposes four things:
//!
//! - a [`Judge`] trait with a deterministic mock implementation so
//!   tests can score outputs without spending real tokens;
//! - a [`GoldenSuite`] loader + runner — curated `intent →
//!   expected-outcome` cases that every merge runs;
//! - a [`Replayer`] that compares two runs of the same trace and
//!   surfaces divergence;
//! - a [`RegressionReport`] builder that turns baseline-vs-candidate
//!   deltas into a structured, CI-friendly artifact.
//!
//! Slice A is intentionally offline-only. Slice B brings in
//! `aaf-learn`, which subscribes to trace events and adapts the
//! registry, router, and fast-path miner based on outcomes.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod golden;
pub mod judge;
pub mod regression;
pub mod replay;
pub mod report;

pub use golden::{GoldenCase, GoldenSuite};
pub use judge::{DeterministicJudge, Judge, JudgeVerdict};
pub use regression::{RegressionDelta, RegressionReport};
pub use replay::{Divergence, Replayer};
pub use report::ReportWriter;
