//! Semantic-hash intent cache.
//!
//! v0.1 normalises the input string (lowercase, collapse whitespace) and
//! hashes it with SHA-256. A real semantic cache will hash an embedding;
//! the API is the same.

use aaf_contracts::IntentEnvelope;
use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

/// Compute the semantic hash of an input.
pub fn semantic_hash(input: &str) -> String {
    let normalised: String = input
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                ' '
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut h = Sha256::new();
    h.update(normalised.as_bytes());
    hex::encode(h.finalize())
}

/// Process-local cache.
#[derive(Default)]
pub struct IntentCache {
    inner: Arc<RwLock<HashMap<String, IntentEnvelope>>>,
}

impl IntentCache {
    /// New empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up by raw input.
    pub fn get(&self, raw_input: &str) -> Option<IntentEnvelope> {
        self.inner.read().get(&semantic_hash(raw_input)).cloned()
    }

    /// Insert.
    pub fn put(&self, raw_input: &str, envelope: IntentEnvelope) {
        self.inner
            .write()
            .insert(semantic_hash(raw_input), envelope);
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_and_case_normalised() {
        assert_eq!(
            semantic_hash("  Show LAST   month "),
            semantic_hash("show last month")
        );
    }
}
