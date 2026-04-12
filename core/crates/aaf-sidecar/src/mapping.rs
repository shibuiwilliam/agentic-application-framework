//! Intent field ↔ API field mapping.

use serde::{Deserialize, Serialize};

/// One mapping rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMapping {
    /// Intent constraint key.
    pub intent_field: String,
    /// API field name.
    pub api_field: String,
    /// Default value if the intent constraint is missing.
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

impl FieldMapping {
    /// Apply the mapping to a constraints map and write into `out`.
    pub fn apply(
        &self,
        constraints: &std::collections::BTreeMap<String, serde_json::Value>,
        out: &mut serde_json::Map<String, serde_json::Value>,
    ) {
        if let Some(v) = constraints.get(&self.intent_field) {
            out.insert(self.api_field.clone(), v.clone());
        } else if let Some(v) = &self.default {
            out.insert(self.api_field.clone(), v.clone());
        }
    }
}
