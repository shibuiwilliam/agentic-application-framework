//! Strongly-typed identifiers for the AAF object model.
//!
//! All AAF identifiers are wrappers around `String` so they implement
//! `Display`, `Hash`, `Eq`, and (de)serialise as plain JSON strings while
//! still preventing accidental confusion at the type level.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident, $prefix:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Construct a new random identifier with the canonical prefix.
            pub fn new() -> Self {
                Self(format!(
                    "{}-{}",
                    $prefix,
                    &uuid::Uuid::new_v4().simple().to_string()[..12]
                ))
            }

            /// Wrap an existing string as this id type without checks.
            pub fn from_raw<S: Into<String>>(s: S) -> Self {
                Self(s.into())
            }

            /// Borrow the inner string.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }
    };
}

id_type!(
    IntentId,
    "int",
    "Identifier for an [`crate::intent::IntentEnvelope`]."
);
id_type!(TaskId, "task", "Identifier for a [`crate::task::Task`].");
id_type!(
    TraceId,
    "trace",
    "Identifier for a [`crate::trace::ExecutionTrace`]."
);
id_type!(
    ArtifactId,
    "art",
    "Identifier for a [`crate::artifact::Artifact`]."
);
id_type!(
    CapabilityId,
    "cap",
    "Identifier for a [`crate::capability::CapabilityContract`]."
);
id_type!(NodeId, "node", "Identifier for a runtime DAG node.");
id_type!(
    HandoffId,
    "handoff",
    "Identifier for a [`crate::handoff::Handoff`]."
);
id_type!(CheckpointId, "cp", "Identifier for a checkpoint.");
id_type!(AgentId, "agent", "Identifier for an agent process / role.");
id_type!(
    TenantId,
    "tenant",
    "Tenant identifier for multi-tenant isolation."
);
id_type!(
    EventId,
    "evt",
    "Identifier for an `AppEvent` emitted by an application surface."
);
id_type!(
    ProposalId,
    "prop",
    "Identifier for an `ActionProposal` emitted by the runtime."
);
id_type!(
    ProjectionId,
    "proj",
    "Identifier for a `StateProjection` visible to a caller."
);
id_type!(
    SessionId,
    "sess",
    "Identifier for a user session that groups events/proposals."
);
id_type!(
    UserId,
    "usr",
    "Identifier for a distinct end-user (not an agent)."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_prefixed() {
        let a = IntentId::new();
        let b = IntentId::new();
        assert_ne!(a, b);
        assert!(a.as_str().starts_with("int-"));
    }

    #[test]
    fn ids_round_trip_through_json() {
        let id = TaskId::new();
        let json = serde_json::to_string(&id).expect("serialise");
        let back: TaskId = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(id, back);
    }
}
