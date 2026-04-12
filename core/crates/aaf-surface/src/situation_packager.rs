//! Situation packager.
//!
//! Turns a [`crate::event::Situation`] into a trimmed payload that
//! fits inside the context budget from Rule 10 (~7,500 tokens).
//!
//! v0.1 packages two things:
//! - the `current_entities` (forwarded into `IntentEnvelope.entities_in_context`)
//! - the visible field list from the screen context, budget-capped
//!
//! Nothing else is forwarded. This is deliberate: the more you put in
//! the situation package, the bigger the context budget cost.

use crate::error::SurfaceError;
use crate::event::Situation;
use aaf_contracts::EntityRefLite;

/// Default token budget for the situation payload. Should be a small
/// fraction of the 7,500 total context budget so the LLM still has
/// room for the system prompt, memory retrieval, step context, and
/// tool results.
pub const DEFAULT_SITUATION_BUDGET_TOKENS: usize = 1_000;

/// Pluggable packager. In v0.1 the implementation is trivial — it is
/// a struct so Slice B can swap in a smarter policy behind the same
/// surface.
#[derive(Debug, Clone, Copy)]
pub struct SituationPackager {
    /// Token budget for the packaged situation.
    pub budget_tokens: usize,
}

impl Default for SituationPackager {
    fn default() -> Self {
        Self {
            budget_tokens: DEFAULT_SITUATION_BUDGET_TOKENS,
        }
    }
}

impl SituationPackager {
    /// Approximate tokens ≈ chars / 4 (matches `aaf-memory::context`).
    pub fn approx_tokens(text: &str) -> usize {
        (text.chars().count() + 3) / 4
    }

    /// Extract the `current_entities` in budget order.
    pub fn package_entities(&self, situation: &Situation) -> Vec<EntityRefLite> {
        situation.current_entities.clone()
    }

    /// Render the visible-field list into a one-line string that fits
    /// the budget. Errors with [`SurfaceError::ContextBudgetExceeded`]
    /// if the caller insists on including too much.
    pub fn package_screen_fields(&self, situation: &Situation) -> Result<String, SurfaceError> {
        let fields: Vec<&str> = situation
            .current_screen
            .as_ref()
            .map(|s| s.visible_fields.iter().map(String::as_str).collect())
            .unwrap_or_default();
        let joined = fields.join(",");
        let used = Self::approx_tokens(&joined);
        if used > self.budget_tokens {
            return Err(SurfaceError::ContextBudgetExceeded {
                used,
                limit: self.budget_tokens,
            });
        }
        Ok(joined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{ScreenContext, SessionContext, SurfaceConstraints};
    use aaf_contracts::{EntityRefLite, TenantId, UserId};

    fn sit(fields: Vec<String>) -> Situation {
        Situation {
            current_entities: vec![EntityRefLite::new("commerce.Order")],
            current_screen: Some(ScreenContext {
                route: "/orders/1".into(),
                component: "OrderDetail".into(),
                visible_fields: fields,
            }),
            session: SessionContext {
                user_id: UserId::new(),
                role: "analyst".into(),
                scopes: vec![],
                locale: "en".into(),
                tenant_id: TenantId::from("t-a"),
            },
            constraints: SurfaceConstraints::default(),
        }
    }

    #[test]
    fn packages_entity_refs_unchanged() {
        let p = SituationPackager::default();
        let v = p.package_entities(&sit(vec!["id".into()]));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].entity_id, "commerce.Order");
    }

    #[test]
    fn small_field_list_fits() {
        let p = SituationPackager::default();
        let s = p
            .package_screen_fields(&sit(vec!["id".into(), "status".into()]))
            .unwrap();
        assert_eq!(s, "id,status");
    }

    #[test]
    fn oversized_field_list_errors() {
        let p = SituationPackager { budget_tokens: 1 }; // effectively zero
        let s = sit((0..50).map(|i| format!("field_{i}")).collect());
        let err = p.package_screen_fields(&s).unwrap_err();
        assert!(matches!(err, SurfaceError::ContextBudgetExceeded { .. }));
    }
}
