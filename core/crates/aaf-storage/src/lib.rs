//! AAF storage traits and in-memory backends.
//!
//! Rule 11 (Storage Behind Traits): no domain crate may import a database
//! driver. Every persistent surface in AAF is defined here as an
//! `async_trait` and consumed by the runtime / registry / memory crates
//! through `Arc<dyn Trait>`. Production deployments swap the in-memory
//! backends for PostgreSQL / Redis / S3 / ClickHouse / pgvector
//! implementations.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod artifact;
pub mod checkpoint;
pub mod error;
pub mod memory;
pub mod registry;
pub mod trace;

pub use artifact::{ArtifactStore, InMemoryArtifactStore};
pub use checkpoint::{Checkpoint, CheckpointStore, InMemoryCheckpointStore};
pub use error::StorageError;
pub use memory::{
    InMemoryLongTermStore, InMemoryThreadStore, InMemoryWorkingStore, LongTermMemoryStore,
    LongTermRecord, ThreadId, ThreadMemoryStore, WorkingMemoryStore,
};
pub use registry::{InMemoryRegistryStore, RegistryStore};
pub use trace::{InMemoryTraceStore, TraceStore};
