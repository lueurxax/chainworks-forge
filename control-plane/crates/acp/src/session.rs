use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::oneshot;

use crate::{ExecutionRequest, ExecutionResult};

/// Trait representing a live ACP session for a single agent execution.
#[async_trait]
pub trait AcpSession: Send + Sync {
    /// Run the session to completion, returning the execution result.
    async fn run(&self, req: ExecutionRequest) -> Result<ExecutionResult>;

    /// Request cancellation of the session.
    async fn cancel(&self) -> Result<()>;
}

/// Handle to an in-progress ACP session.
/// Holds a cancellation sender and can be used to await the result.
pub struct AcpSessionHandle {
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl AcpSessionHandle {
    pub fn new(cancel_tx: oneshot::Sender<()>) -> Self {
        Self {
            cancel_tx: Some(cancel_tx),
        }
    }

    /// Signal the session to cancel.
    pub fn request_cancel(&mut self) -> Result<()> {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        Ok(())
    }
}
