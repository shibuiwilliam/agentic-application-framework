//! Convenience prelude — `use aaf_policy::prelude::*;` brings the
//! most-used types into scope.

pub use crate::approval::{ApprovalRequest, ApprovalState, ApprovalWorkflow};
pub use crate::context::{EntityClass, OntologyClassificationLookup, PolicyContext};
pub use crate::engine::{PolicyEngine, PolicyHook};
pub use crate::guard::{ActionGuard, InputGuard, OutputGuard};
pub use crate::scope::{compute_effective_scopes, effective_scopes, scope_matches};
