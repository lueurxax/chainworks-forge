pub mod auggie;
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod junie;

use anyhow::Result;
use async_trait::async_trait;
use tracing::warn;

use crate::session::AcpSessionHandle;
use crate::{ExecutionRequest, ExecutionResult};

/// Common interface for all ACP provider adapters.
#[async_trait]
pub trait AcpAdapter: Send + Sync {
    /// Returns the canonical provider name for this adapter.
    fn provider_name(&self) -> &str;

    /// Open a live transport-backed ACP session.
    async fn open_session(&self, req: &ExecutionRequest) -> Result<AcpSessionHandle>;

    /// Execute an agent session and return the result.
    async fn execute(&self, req: ExecutionRequest) -> Result<ExecutionResult> {
        let session = self.open_session(&req).await?;
        let mut result = match session.prompt(&req).await {
            Ok(result) => result,
            Err(prompt_error) => {
                if let Err(close_error) = session.close().await {
                    warn!(
                        provider = %req.provider,
                        run_id = %req.run_id,
                        stage_id = %req.stage_id,
                        "ACP session close after prompt error failed: {close_error}"
                    );
                }
                return Err(prompt_error);
            }
        };
        result.close_diagnostic = session.close().await?;
        result.session_generation_id = None;
        Ok(result)
    }
}
