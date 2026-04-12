//! AAF Application-Native Surface (Enhancement E3 Slice A).
//!
//! `aaf-surface` lets any application integrate AAF natively:
//!
//! - **Events become intents.** A click, a page-view, a webhook, a
//!   state change — any application signal is wrapped in an
//!   [`event::AppEvent`] and handed to an
//!   [`ingest::EventToIntentAdapter`], which produces an
//!   [`aaf_contracts::IntentEnvelope`].
//! - **Agents propose, apps decide.** Agents never mutate application
//!   state. They emit [`proposal::ActionProposal`]s that the app
//!   renders inline, and the user accepts / rejects / transforms.
//!   **Rule 20** is enforced at construction time: a proposal whose
//!   `mutations[]` is non-empty must carry a `compensation_ref`.
//! - **State flows both ways, authority does not.** Agents can read
//!   [`projection::StateProjection`]s of entity data, but every
//!   projection is **default-deny** (**Rule 19**) — only explicitly
//!   listed fields are visible.
//!
//! Slice A delivers the contracts + in-memory lifecycle. Slice B wires
//! `EventGateway` into the sidecar/wrapper and hooks proposal outcomes
//! into `aaf-trace`. Slice C ships the SDK decorators and the
//! `examples/app-native/` reference application.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod error;
pub mod event;
pub mod ingest;
pub mod lifecycle;
pub mod projection;
pub mod proposal;
pub mod router;
pub mod situation_packager;

pub use error::SurfaceError;
pub use event::{
    AppEvent, EventSource, ScreenContext, SessionContext, Situation, SurfaceConstraints,
};
pub use ingest::{EventToIntentAdapter, RuleBasedAdapter};
pub use lifecycle::ProposalLifecycle;
pub use projection::{ProjectionError, StateProjection};
pub use proposal::{
    ActionProposal, ApprovalState as ProposalApprovalState, CompensationRef, StateMutationProposal,
    UiHints, UiKind,
};
pub use router::{EventCategory, EventClassifier, EventRouter, RouteOutcome, RuleBasedClassifier};
pub use situation_packager::SituationPackager;
