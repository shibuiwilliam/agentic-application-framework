//! Anti-Corruption Layer (PROJECT_AafService §5.3).
//!
//! An ACL sits between each service and the AAF layer, translating
//! between AAF's unified semantic model and each service's internal
//! model. This prevents AAF's entity vocabulary from leaking into
//! service code, and vice versa.
//!
//! ```text
//! [AAF Semantic Model]
//!   "Customer" = { name, segment, lifetime_value, risk_score }
//!       │
//!       ▼
//! [Anti-Corruption Layer]
//!   Translate: AAF "Customer" ↔ CRM "Account" + Billing "Customer"
//!       │
//!       ▼
//! [Service Internal Models]
//!   CRM: Account { account_name, industry, size }
//!   Billing: Customer { customer_id, plan, payment_method }
//! ```
//!
//! The ACL is a **trait** (`EntityTranslator`) so each service can
//! provide its own translation without imposing a common base type.
//! The sidecar holds a registry of translators keyed by entity id.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors raised by the anti-corruption layer.
#[derive(Debug, Error)]
pub enum AclError {
    /// No translator registered for the requested entity id.
    #[error("no ACL translator for entity `{0}`")]
    NoTranslator(String),

    /// The translator failed to convert.
    #[error("translation failed: {0}")]
    TranslationFailed(String),
}

/// Direction of translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// AAF semantic model → service internal model.
    ToService,
    /// Service internal model → AAF semantic model.
    ToAaf,
}

/// Pluggable translator for a single entity kind.
///
/// Implementors provide two conversions:
/// - `to_service`: AAF `serde_json::Value` → service-native
///   `serde_json::Value`
/// - `to_aaf`: service-native `serde_json::Value` → AAF
///   `serde_json::Value`
///
/// Both are fallible and async so translators can call lookups,
/// enrichment APIs, or mapping tables.
#[async_trait]
pub trait EntityTranslator: Send + Sync {
    /// The ontology entity id this translator handles
    /// (e.g. `"commerce.Customer"`).
    fn entity_id(&self) -> &str;

    /// AAF semantic model → service internal model.
    async fn to_service(
        &self,
        aaf_value: &serde_json::Value,
    ) -> Result<serde_json::Value, AclError>;

    /// Service internal model → AAF semantic model.
    async fn to_aaf(
        &self,
        service_value: &serde_json::Value,
    ) -> Result<serde_json::Value, AclError>;
}

/// Registry of entity translators keyed by entity id.
#[derive(Default)]
pub struct AclRegistry {
    translators: HashMap<String, Arc<dyn EntityTranslator>>,
}

impl AclRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a translator for a specific entity id.
    pub fn register(&mut self, translator: Arc<dyn EntityTranslator>) {
        self.translators
            .insert(translator.entity_id().to_string(), translator);
    }

    /// Translate a value from AAF model to service model.
    pub async fn to_service(
        &self,
        entity_id: &str,
        aaf_value: &serde_json::Value,
    ) -> Result<serde_json::Value, AclError> {
        let t = self
            .translators
            .get(entity_id)
            .ok_or_else(|| AclError::NoTranslator(entity_id.to_string()))?;
        t.to_service(aaf_value).await
    }

    /// Translate a value from service model to AAF model.
    pub async fn to_aaf(
        &self,
        entity_id: &str,
        service_value: &serde_json::Value,
    ) -> Result<serde_json::Value, AclError> {
        let t = self
            .translators
            .get(entity_id)
            .ok_or_else(|| AclError::NoTranslator(entity_id.to_string()))?;
        t.to_aaf(service_value).await
    }

    /// Check whether a translator exists for the given entity.
    pub fn has(&self, entity_id: &str) -> bool {
        self.translators.contains_key(entity_id)
    }

    /// Number of registered translators.
    pub fn len(&self) -> usize {
        self.translators.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.translators.is_empty()
    }
}

/// A simple field-renaming translator that maps AAF field names to
/// service field names and vice versa. Suitable for services whose
/// internal model is structurally identical to the AAF model but uses
/// different naming conventions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldRenamingTranslator {
    entity: String,
    /// Map of `aaf_field → service_field`.
    aaf_to_service: HashMap<String, String>,
    /// Map of `service_field → aaf_field` (reverse).
    service_to_aaf: HashMap<String, String>,
}

impl FieldRenamingTranslator {
    /// Construct from a list of `(aaf_name, service_name)` pairs.
    pub fn new(
        entity: impl Into<String>,
        mappings: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        let mut aaf_to_service = HashMap::new();
        let mut service_to_aaf = HashMap::new();
        for (a, s) in mappings {
            service_to_aaf.insert(s.clone(), a.clone());
            aaf_to_service.insert(a, s);
        }
        Self {
            entity: entity.into(),
            aaf_to_service,
            service_to_aaf,
        }
    }

    fn rename(value: &serde_json::Value, mapping: &HashMap<String, String>) -> serde_json::Value {
        match value {
            serde_json::Value::Object(obj) => {
                let mut out = serde_json::Map::new();
                for (k, v) in obj {
                    let new_key = mapping.get(k).cloned().unwrap_or_else(|| k.clone());
                    out.insert(new_key, v.clone());
                }
                serde_json::Value::Object(out)
            }
            other => other.clone(),
        }
    }
}

#[async_trait]
impl EntityTranslator for FieldRenamingTranslator {
    fn entity_id(&self) -> &str {
        &self.entity
    }

    async fn to_service(
        &self,
        aaf_value: &serde_json::Value,
    ) -> Result<serde_json::Value, AclError> {
        Ok(Self::rename(aaf_value, &self.aaf_to_service))
    }

    async fn to_aaf(
        &self,
        service_value: &serde_json::Value,
    ) -> Result<serde_json::Value, AclError> {
        Ok(Self::rename(service_value, &self.service_to_aaf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn customer_translator() -> FieldRenamingTranslator {
        FieldRenamingTranslator::new(
            "commerce.Customer",
            vec![
                ("name".into(), "account_name".into()),
                ("segment".into(), "industry".into()),
                ("lifetime_value".into(), "total_revenue".into()),
            ],
        )
    }

    #[tokio::test]
    async fn to_service_renames_fields() {
        let t = customer_translator();
        let aaf = serde_json::json!({"name": "Acme", "segment": "tech", "lifetime_value": 100_000});
        let svc = t.to_service(&aaf).await.unwrap();
        assert_eq!(svc["account_name"], "Acme");
        assert_eq!(svc["industry"], "tech");
        assert_eq!(svc["total_revenue"], 100_000);
    }

    #[tokio::test]
    async fn to_aaf_renames_fields_in_reverse() {
        let t = customer_translator();
        let svc =
            serde_json::json!({"account_name": "Acme", "industry": "tech", "total_revenue": 50000});
        let aaf = t.to_aaf(&svc).await.unwrap();
        assert_eq!(aaf["name"], "Acme");
        assert_eq!(aaf["segment"], "tech");
        assert_eq!(aaf["lifetime_value"], 50000);
    }

    #[tokio::test]
    async fn unknown_fields_pass_through_unchanged() {
        let t = customer_translator();
        let aaf = serde_json::json!({"name": "Acme", "extra_field": true});
        let svc = t.to_service(&aaf).await.unwrap();
        assert_eq!(svc["account_name"], "Acme");
        assert_eq!(svc["extra_field"], true); // passes through
    }

    #[tokio::test]
    async fn registry_routes_to_correct_translator() {
        let mut reg = AclRegistry::new();
        reg.register(Arc::new(customer_translator()));
        assert!(reg.has("commerce.Customer"));
        assert!(!reg.has("commerce.Order"));

        let aaf = serde_json::json!({"name": "Acme"});
        let svc = reg.to_service("commerce.Customer", &aaf).await.unwrap();
        assert_eq!(svc["account_name"], "Acme");
    }

    #[tokio::test]
    async fn registry_errors_on_unknown_entity() {
        let reg = AclRegistry::new();
        let err = reg
            .to_service("unknown.Entity", &serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(matches!(err, AclError::NoTranslator(_)));
    }

    #[tokio::test]
    async fn round_trip_preserves_data() {
        let t = customer_translator();
        let original = serde_json::json!({"name": "Acme", "segment": "tech", "lifetime_value": 42});
        let svc = t.to_service(&original).await.unwrap();
        let back = t.to_aaf(&svc).await.unwrap();
        assert_eq!(back, original);
    }
}
