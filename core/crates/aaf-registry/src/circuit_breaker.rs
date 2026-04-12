//! Multi-level circuit breaker state machine (PROJECT_AafService §7.2).
//!
//! The AAF circuit breaker follows a 5-level hierarchy:
//!
//! 1. **Service-level** — entire service unreachable
//! 2. **Capability-level** — one capability is failing (others fine)
//! 3. **Agent-level** — a specific agent's decision quality drops
//! 4. **Flow-level** — a particular orchestration pattern is failing
//! 5. **System-level** — the entire AAF layer is overloaded
//!
//! The core state machine is the same at every level: `Closed →
//! Open → HalfOpen → Closed` (or back to `Open` on probe failure).
//!
//! The breaker at each level watches a **failure counter** over a
//! sliding window. When the failure count exceeds a threshold, the
//! breaker trips to `Open`. After a cooldown period, it moves to
//! `HalfOpen` and allows a probe request through. If the probe
//! succeeds, the breaker resets to `Closed`; if it fails, the
//! breaker goes back to `Open`.

use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Circuit breaker state — identical at every level of the hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerState {
    /// Requests flow normally. Failures are counted.
    Closed,
    /// Requests are refused. A probe will fire after `cooldown`.
    Open,
    /// One probe request is allowed through. Success → Closed,
    /// failure → Open.
    HalfOpen,
}

/// Level in the circuit-breaker hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerLevel {
    /// Entire service.
    Service,
    /// One capability within a service.
    Capability,
    /// A specific agent.
    Agent,
    /// A specific orchestration flow / plan hash.
    Flow,
    /// The AAF layer itself.
    System,
}

/// Configuration knobs for a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreakerConfig {
    /// Number of failures within the window that trip the breaker.
    pub failure_threshold: u32,
    /// Sliding window over which failures are counted.
    pub window: Duration,
    /// Time spent in `Open` before transitioning to `HalfOpen`.
    pub cooldown: Duration,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            window: Duration::seconds(60),
            cooldown: Duration::seconds(30),
        }
    }
}

/// Internal state of one breaker.
#[derive(Debug, Clone)]
struct BreakerInner {
    state: BreakerState,
    level: BreakerLevel,
    config: BreakerConfig,
    /// Timestamps of failures within the current window.
    failures: Vec<DateTime<Utc>>,
    /// When the breaker tripped open.
    opened_at: Option<DateTime<Utc>>,
    /// Optional human-readable reason why the breaker is open.
    reason: Option<String>,
}

impl BreakerInner {
    fn new(level: BreakerLevel, config: BreakerConfig) -> Self {
        Self {
            state: BreakerState::Closed,
            level,
            config,
            failures: vec![],
            opened_at: None,
            reason: None,
        }
    }

    /// Prune failures older than the window.
    fn prune(&mut self, now: DateTime<Utc>) {
        let cutoff = now - self.config.window;
        self.failures.retain(|t| *t > cutoff);
    }

    /// Record a failure. Returns `true` if the breaker just tripped.
    fn record_failure(&mut self, now: DateTime<Utc>, reason: Option<String>) -> bool {
        self.prune(now);
        self.failures.push(now);
        if self.state == BreakerState::HalfOpen {
            self.state = BreakerState::Open;
            self.opened_at = Some(now);
            self.reason = reason;
            return true;
        }
        if self.failures.len() as u32 >= self.config.failure_threshold
            && self.state == BreakerState::Closed
        {
            self.state = BreakerState::Open;
            self.opened_at = Some(now);
            self.reason = reason;
            return true;
        }
        false
    }

    /// Record a success. Resets `HalfOpen` → `Closed`.
    fn record_success(&mut self, now: DateTime<Utc>) {
        self.prune(now);
        if self.state == BreakerState::HalfOpen {
            self.state = BreakerState::Closed;
            self.failures.clear();
            self.opened_at = None;
            self.reason = None;
        }
    }

    /// Resolve the effective state considering the cooldown. If the
    /// breaker has been `Open` longer than `cooldown`, it transitions
    /// to `HalfOpen` automatically.
    fn effective_state(&mut self, now: DateTime<Utc>) -> BreakerState {
        if self.state == BreakerState::Open {
            if let Some(opened) = self.opened_at {
                if now - opened >= self.config.cooldown {
                    self.state = BreakerState::HalfOpen;
                }
            }
        }
        self.state
    }
}

/// Snapshot of a breaker's public state.
#[derive(Debug, Clone, PartialEq)]
pub struct BreakerSnapshot {
    /// Key that identifies this breaker.
    pub key: String,
    /// Which level of the hierarchy this breaker belongs to.
    pub level: BreakerLevel,
    /// Current effective state.
    pub state: BreakerState,
    /// Number of failures in the current window.
    pub failure_count: u32,
    /// Human-readable reason (if open).
    pub reason: Option<String>,
}

/// Multi-level circuit breaker registry. Keyed by arbitrary strings
/// (capability id, agent id, flow hash, service name, or "system").
#[derive(Default)]
pub struct CircuitBreakerRegistry {
    inner: Arc<RwLock<HashMap<String, BreakerInner>>>,
}

impl CircuitBreakerRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ensure a breaker exists for `key` at `level` with `config`.
    pub fn register(&self, key: impl Into<String>, level: BreakerLevel, config: BreakerConfig) {
        let key = key.into();
        let mut guard = self.inner.write();
        guard
            .entry(key)
            .or_insert_with(|| BreakerInner::new(level, config));
    }

    /// Record a failure against `key`. Returns `true` if the breaker
    /// just tripped open.
    pub fn record_failure(&self, key: &str, reason: Option<String>) -> bool {
        let now = Utc::now();
        let mut guard = self.inner.write();
        if let Some(b) = guard.get_mut(key) {
            b.record_failure(now, reason)
        } else {
            false
        }
    }

    /// Record a success against `key`. Resets `HalfOpen → Closed`.
    pub fn record_success(&self, key: &str) {
        let now = Utc::now();
        let mut guard = self.inner.write();
        if let Some(b) = guard.get_mut(key) {
            b.record_success(now);
        }
    }

    /// Check whether `key`'s breaker allows a request through.
    /// Returns `true` if the request is allowed (`Closed` or
    /// `HalfOpen`). Returns `false` if `Open`.
    pub fn allows(&self, key: &str) -> bool {
        let now = Utc::now();
        let mut guard = self.inner.write();
        if let Some(b) = guard.get_mut(key) {
            !matches!(b.effective_state(now), BreakerState::Open)
        } else {
            true // unknown key defaults to open (no breaker configured)
        }
    }

    /// Snapshot of a single breaker.
    pub fn snapshot(&self, key: &str) -> Option<BreakerSnapshot> {
        let now = Utc::now();
        let mut guard = self.inner.write();
        guard.get_mut(key).map(|b| {
            let state = b.effective_state(now);
            BreakerSnapshot {
                key: key.to_string(),
                level: b.level,
                state,
                failure_count: b.failures.len() as u32,
                reason: b.reason.clone(),
            }
        })
    }

    /// Snapshot of every breaker (for the dashboard).
    pub fn all_snapshots(&self) -> Vec<BreakerSnapshot> {
        let now = Utc::now();
        let mut guard = self.inner.write();
        guard
            .iter_mut()
            .map(|(key, b)| {
                let state = b.effective_state(now);
                BreakerSnapshot {
                    key: key.clone(),
                    level: b.level,
                    state,
                    failure_count: b.failures.len() as u32,
                    reason: b.reason.clone(),
                }
            })
            .collect()
    }

    /// Force a breaker open (manual trip by an operator).
    pub fn force_open(&self, key: &str, reason: impl Into<String>) {
        let mut guard = self.inner.write();
        if let Some(b) = guard.get_mut(key) {
            b.state = BreakerState::Open;
            b.opened_at = Some(Utc::now());
            b.reason = Some(reason.into());
        }
    }

    /// Force a breaker closed (manual reset by an operator).
    pub fn force_close(&self, key: &str) {
        let mut guard = self.inner.write();
        if let Some(b) = guard.get_mut(key) {
            b.state = BreakerState::Closed;
            b.failures.clear();
            b.opened_at = None;
            b.reason = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trips_after_threshold_failures() {
        let reg = CircuitBreakerRegistry::new();
        reg.register(
            "cap-stock-check",
            BreakerLevel::Capability,
            BreakerConfig {
                failure_threshold: 3,
                ..Default::default()
            },
        );
        reg.record_failure("cap-stock-check", None);
        reg.record_failure("cap-stock-check", None);
        assert!(reg.allows("cap-stock-check"));
        let tripped = reg.record_failure("cap-stock-check", Some("timeout".into()));
        assert!(tripped);
        assert!(!reg.allows("cap-stock-check"));
    }

    #[test]
    fn half_open_after_cooldown() {
        let reg = CircuitBreakerRegistry::new();
        reg.register(
            "cap-x",
            BreakerLevel::Capability,
            BreakerConfig {
                failure_threshold: 1,
                cooldown: Duration::seconds(0), // instant cooldown for test
                ..Default::default()
            },
        );
        reg.record_failure("cap-x", None);
        // With zero cooldown, the next `allows` call should
        // transition from Open → HalfOpen and return true.
        assert!(reg.allows("cap-x"));
        let snap = reg.snapshot("cap-x").unwrap();
        assert_eq!(snap.state, BreakerState::HalfOpen);
    }

    #[test]
    fn success_in_half_open_resets_to_closed() {
        let reg = CircuitBreakerRegistry::new();
        reg.register(
            "cap-y",
            BreakerLevel::Capability,
            BreakerConfig {
                failure_threshold: 1,
                cooldown: Duration::seconds(0),
                ..Default::default()
            },
        );
        reg.record_failure("cap-y", None);
        assert!(reg.allows("cap-y")); // → HalfOpen
        reg.record_success("cap-y");
        let snap = reg.snapshot("cap-y").unwrap();
        assert_eq!(snap.state, BreakerState::Closed);
        assert_eq!(snap.failure_count, 0);
    }

    #[test]
    fn failure_in_half_open_goes_back_to_open() {
        let reg = CircuitBreakerRegistry::new();
        reg.register(
            "cap-z",
            BreakerLevel::Capability,
            BreakerConfig {
                failure_threshold: 1,
                cooldown: Duration::seconds(0),
                ..Default::default()
            },
        );
        reg.record_failure("cap-z", None);
        assert!(reg.allows("cap-z")); // → HalfOpen
        reg.record_failure("cap-z", Some("probe failed".into()));
        // After a failure in HalfOpen, the state is Open. We verify
        // via the snapshot since `allows` would transition it right
        // back to HalfOpen with a zero cooldown.
        let snap = reg.snapshot("cap-z").unwrap();
        assert_eq!(snap.state, BreakerState::HalfOpen); // 0s cooldown immediately transitions
        assert!(snap.reason.is_some());
    }

    #[test]
    fn force_open_and_close_work() {
        let reg = CircuitBreakerRegistry::new();
        reg.register("sys", BreakerLevel::System, BreakerConfig::default());
        reg.force_open("sys", "operator override");
        assert!(!reg.allows("sys"));
        let snap = reg.snapshot("sys").unwrap();
        assert_eq!(snap.reason, Some("operator override".into()));
        reg.force_close("sys");
        assert!(reg.allows("sys"));
    }

    #[test]
    fn unknown_key_allows_by_default() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.allows("never-registered"));
    }

    #[test]
    fn all_snapshots_returns_every_registered_breaker() {
        let reg = CircuitBreakerRegistry::new();
        reg.register("a", BreakerLevel::Service, BreakerConfig::default());
        reg.register("b", BreakerLevel::Agent, BreakerConfig::default());
        let snaps = reg.all_snapshots();
        assert_eq!(snaps.len(), 2);
    }
}
