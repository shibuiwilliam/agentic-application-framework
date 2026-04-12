//! State projections — Rule 19 (Projections Default-Deny).
//!
//! A [`StateProjection`] is a read-only view of an entity that the
//! application is willing to show to an agent. Only the fields
//! explicitly listed in `selected_fields` are visible — everything
//! else is denied with [`ProjectionError::FieldNotSelected`].

use crate::error::SurfaceError;
use aaf_contracts::{EntityRefLite, ProjectionId, TenantId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Dedicated projection error enum — separate from [`SurfaceError`] so
/// the read-path stays tightly typed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProjectionError {
    /// The field is not in `selected_fields`.
    #[error("field `{0}` not in the projection's explicit allow-list")]
    FieldNotSelected(String),

    /// The caller's tenant does not match the projection's tenant.
    #[error("cross-tenant projection access denied")]
    WrongTenant,
}

impl From<ProjectionError> for SurfaceError {
    fn from(e: ProjectionError) -> Self {
        match e {
            ProjectionError::FieldNotSelected(f) => SurfaceError::ProjectionDenied {
                projection: "<unknown>".into(),
                field: f,
            },
            ProjectionError::WrongTenant => SurfaceError::ProjectionDenied {
                projection: "<unknown>".into(),
                field: "<tenant>".into(),
            },
        }
    }
}

/// Read-only projection of an entity visible to an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateProjection {
    /// Stable id.
    pub projection_id: ProjectionId,
    /// Root entity this projection exposes.
    pub root_entity: EntityRefLite,
    /// Tenant scope.
    pub tenant: TenantId,
    /// Fields the caller may read. Anything else is denied (Rule 19).
    pub selected_fields: Vec<String>,
    /// Maximum staleness of the underlying data in milliseconds.
    pub freshness_ms: u64,
}

impl StateProjection {
    /// Construct.
    pub fn new(
        root_entity: EntityRefLite,
        tenant: TenantId,
        selected_fields: Vec<String>,
        freshness_ms: u64,
    ) -> Self {
        Self {
            projection_id: ProjectionId::new(),
            root_entity,
            tenant,
            selected_fields,
            freshness_ms,
        }
    }

    /// Whether a specific field is visible through this projection.
    /// Rule 19 default-deny.
    pub fn allows_field(&self, field: &str) -> bool {
        self.selected_fields.iter().any(|f| f == field)
    }

    /// Read a named field from a payload, enforcing the allow-list.
    pub fn read_field<'a>(
        &self,
        field: &str,
        payload: &'a serde_json::Value,
    ) -> Result<&'a serde_json::Value, ProjectionError> {
        if !self.allows_field(field) {
            return Err(ProjectionError::FieldNotSelected(field.to_string()));
        }
        Ok(payload.get(field).unwrap_or(&serde_json::Value::Null))
    }

    /// Enforce that the calling tenant matches the projection's tenant.
    pub fn check_tenant(&self, caller: &TenantId) -> Result<(), ProjectionError> {
        if &self.tenant != caller {
            return Err(ProjectionError::WrongTenant);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj() -> StateProjection {
        StateProjection::new(
            EntityRefLite::new("commerce.Order"),
            TenantId::from("t-a"),
            vec!["id".into(), "status".into()],
            60_000,
        )
    }

    #[test]
    fn listed_fields_are_visible() {
        let p = proj();
        assert!(p.allows_field("id"));
        assert!(p.allows_field("status"));
    }

    #[test]
    fn unlisted_fields_default_deny() {
        let p = proj();
        assert!(!p.allows_field("total"));
        let payload = serde_json::json!({"id": "ord-1", "total": 42});
        let err = p.read_field("total", &payload).unwrap_err();
        assert!(matches!(err, ProjectionError::FieldNotSelected(_)));
    }

    #[test]
    fn cross_tenant_access_is_rejected() {
        let p = proj();
        let other = TenantId::from("t-b");
        assert_eq!(p.check_tenant(&other), Err(ProjectionError::WrongTenant));
    }
}
