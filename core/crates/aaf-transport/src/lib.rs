//! Transport abstraction.
//!
//! Defines the [`Transport`] trait that the runtime / sidecar / server
//! all consume. Concrete drivers (gRPC, HTTP, NATS, WebSocket,
//! CloudEvents) are intentionally deferred — this crate ships only the
//! trait + an in-memory loopback driver used in tests.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Transport-level errors.
#[derive(Debug, Error)]
pub enum TransportError {
    /// Underlying transport failed.
    #[error("transport failure: {0}")]
    Failure(String),

    /// Remote returned an error payload.
    #[error("remote error: {0}")]
    Remote(String),
}

/// One framed envelope passed across the transport boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    /// Logical channel name.
    pub channel: String,
    /// Opaque body.
    pub body: serde_json::Value,
}

/// Pluggable transport.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a one-way envelope.
    async fn send(&self, env: Envelope) -> Result<(), TransportError>;

    /// Send and await a reply.
    async fn request(&self, env: Envelope) -> Result<Envelope, TransportError>;
}

/// Loopback transport that records sent envelopes — used in tests.
#[derive(Default)]
pub struct LoopbackTransport {
    sent: Arc<Mutex<Vec<Envelope>>>,
}

impl LoopbackTransport {
    /// Construct.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inspect everything that has been `send`/`request`-ed.
    pub fn sent(&self) -> Vec<Envelope> {
        self.sent.lock().clone()
    }
}

#[async_trait]
impl Transport for LoopbackTransport {
    async fn send(&self, env: Envelope) -> Result<(), TransportError> {
        self.sent.lock().push(env);
        Ok(())
    }

    async fn request(&self, env: Envelope) -> Result<Envelope, TransportError> {
        self.sent.lock().push(env.clone());
        Ok(Envelope {
            channel: env.channel,
            body: serde_json::json!({"echo": env.body}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loopback_records_sends() {
        let t = LoopbackTransport::new();
        t.send(Envelope {
            channel: "x".into(),
            body: serde_json::json!(1),
        })
        .await
        .unwrap();
        assert_eq!(t.sent().len(), 1);
    }

    #[tokio::test]
    async fn loopback_request_echoes_body() {
        let t = LoopbackTransport::new();
        let r = t
            .request(Envelope {
                channel: "x".into(),
                body: serde_json::json!({"k": "v"}),
            })
            .await
            .unwrap();
        assert_eq!(r.body, serde_json::json!({"echo": {"k": "v"}}));
    }
}
