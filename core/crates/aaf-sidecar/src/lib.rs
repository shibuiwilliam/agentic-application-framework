//! Agent Sidecar.
//!
//! v0.1 ships the *logical* sidecar: the components downstream code
//! consumes (proxy, capability publisher, fast-path evaluator, local
//! guard, mapping, health). Concrete network drivers (HTTP / gRPC /
//! NATS) live in `aaf-transport` and are wired by the server binary.
//!
//! **Rule 13** is enforced by [`proxy::Proxy::handle`] which falls
//! through to `forward_direct` whenever `is_aaf_healthy()` returns
//! false.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod acl;
pub mod capability;
pub mod fast_path;
pub mod guard;
pub mod health;
pub mod mapping;
pub mod proxy;

pub use acl::{AclError, AclRegistry, EntityTranslator, FieldRenamingTranslator};
pub use capability::CapabilityPublisher;
pub use fast_path::LocalFastPath;
pub use guard::LocalGuard;
pub use health::SidecarHealth;
pub use mapping::FieldMapping;
pub use proxy::{Proxy, ProxyDecision};
