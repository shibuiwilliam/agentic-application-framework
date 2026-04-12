//! Per-step / per-task timeout helpers.

use std::time::Duration;
use tokio::time::error::Elapsed;

/// Wrap a future with a hard deadline.
pub async fn with_timeout<F, T>(d: Duration, fut: F) -> Result<T, Elapsed>
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(d, fut).await
}
