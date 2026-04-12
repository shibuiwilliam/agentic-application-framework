//! Intent type version migration.
//!
//! Lets the platform evolve `IntentType` v1 → v2 → v3 by registering
//! migration hooks. Hooks operate on the constraint map so contract
//! shape stays stable.

use aaf_contracts::IntentEnvelope;
use std::collections::BTreeMap;

/// Hook function pointer.
pub type MigrationFn = fn(&mut IntentEnvelope);

/// Registry of migration hooks keyed by `(from_version, to_version)`.
#[derive(Default)]
pub struct IntentMigrator {
    hooks: BTreeMap<(u32, u32), MigrationFn>,
}

impl IntentMigrator {
    /// New empty migrator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a migration hook.
    pub fn register(&mut self, from: u32, to: u32, hook: MigrationFn) {
        self.hooks.insert((from, to), hook);
    }

    /// Apply hooks needed to walk `from` → `to`.
    pub fn migrate(&self, env: &mut IntentEnvelope, from: u32, to: u32) {
        let mut cursor = from;
        while cursor < to {
            let next = cursor + 1;
            if let Some(h) = self.hooks.get(&(cursor, next)) {
                h(env);
            }
            cursor = next;
        }
    }
}
