//! Plan cache with bounded capacity.
//!
//! Uses a simple clock-hand eviction strategy: once the cache exceeds
//! its capacity, the oldest entry (by insertion order) is evicted.
//! This is simpler than a full LRU but sufficient for the common case
//! where recent plans are reused and old plans drift out of relevance.

use crate::plan::ExecutionPlan;
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

/// Default maximum number of cached plans.
const DEFAULT_CAPACITY: usize = 4096;

/// Compute the cache key for an intent.
pub fn key_for(goal: &str, domain: &str, intent_type: &str) -> String {
    let normalized = format!("{}|{}|{}", intent_type, domain, goal.trim().to_lowercase());
    let mut h = Sha256::new();
    h.update(normalized.as_bytes());
    hex::encode(h.finalize())
}

/// Process-local plan cache with bounded capacity. Once the cache
/// exceeds `capacity`, the oldest entry is evicted.
pub struct PlanCache {
    inner: Arc<RwLock<CacheInner>>,
}

struct CacheInner {
    map: HashMap<String, ExecutionPlan>,
    order: VecDeque<String>,
    capacity: usize,
}

impl Default for PlanCache {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }
}

impl PlanCache {
    /// New empty cache with the default capacity (4096).
    pub fn new() -> Self {
        Self::default()
    }

    /// New empty cache with a specific capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(CacheInner {
                map: HashMap::new(),
                order: VecDeque::new(),
                capacity,
            })),
        }
    }

    /// Look up a plan.
    pub fn get(&self, key: &str) -> Option<ExecutionPlan> {
        self.inner.read().map.get(key).cloned()
    }

    /// Insert a plan, evicting the oldest entry if at capacity.
    pub fn put(&self, key: String, plan: ExecutionPlan) {
        let mut guard = self.inner.write();
        let is_update = guard.map.contains_key(&key);
        if is_update {
            // Update in place — no order change needed.
            if let Some(slot) = guard.map.get_mut(&key) {
                *slot = plan;
            }
            return;
        }
        // Evict oldest entries until we are under capacity.
        while guard.map.len() >= guard.capacity {
            if let Some(oldest) = guard.order.pop_front() {
                guard.map.remove(&oldest);
            } else {
                break;
            }
        }
        guard.order.push_back(key.clone());
        guard.map.insert(key, plan);
    }

    /// Number of cached plans.
    pub fn len(&self) -> usize {
        self.inner.read().map.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{PlannedStep, PlannedStepKind};
    use aaf_contracts::{CapabilityId, IntentId, NodeId};

    fn dummy_plan(name: &str) -> ExecutionPlan {
        ExecutionPlan {
            intent_id: IntentId::new(),
            steps: vec![PlannedStep {
                step_id: 1,
                kind: PlannedStepKind::Deterministic,
                capability: CapabilityId::from(name),
                input_mapping: String::new(),
                output_id: NodeId::new(),
            }],
        }
    }

    #[test]
    fn evicts_oldest_when_at_capacity() {
        let cache = PlanCache::with_capacity(2);
        cache.put("a".into(), dummy_plan("a"));
        cache.put("b".into(), dummy_plan("b"));
        assert_eq!(cache.len(), 2);

        // Insert a third — should evict "a".
        cache.put("c".into(), dummy_plan("c"));
        assert_eq!(cache.len(), 2);
        assert!(cache.get("a").is_none(), "oldest should be evicted");
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn update_in_place_does_not_evict() {
        let cache = PlanCache::with_capacity(2);
        cache.put("a".into(), dummy_plan("a1"));
        cache.put("b".into(), dummy_plan("b1"));
        // Update "a" — should not evict anything.
        cache.put("a".into(), dummy_plan("a2"));
        assert_eq!(cache.len(), 2);
        let plan = cache.get("a").unwrap();
        assert_eq!(plan.steps[0].capability.as_str(), "a2");
    }

    #[test]
    fn key_for_normalizes_goal() {
        let k1 = key_for("Show Sales", "commerce", "Analytical");
        let k2 = key_for("  show sales  ", "commerce", "Analytical");
        assert_eq!(k1, k2);
    }
}
