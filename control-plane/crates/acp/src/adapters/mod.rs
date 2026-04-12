pub mod auggie;
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod junie;

use anyhow::Result;
use async_trait::async_trait;

use crate::{ExecutionRequest, ExecutionResult};

/// Common interface for all ACP provider adapters.
#[async_trait]
pub trait AcpAdapter: Send + Sync {
    /// Returns the canonical provider name for this adapter.
    fn provider_name(&self) -> &str;

    /// Execute an agent session and return the result.
    async fn execute(&self, req: ExecutionRequest) -> Result<ExecutionResult>;
}
