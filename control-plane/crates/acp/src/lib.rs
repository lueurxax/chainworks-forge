pub mod adapters;
pub mod manager;
pub mod session;
pub mod transport;

pub use manager::AcpRuntimeManager;
pub use session::{AcpSession, AcpSessionHandle};

use domain::agent::AgentStatus;
use domain::ids::{AgentExecutionId, RunId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub run_id: RunId,
    pub stage_id: String,
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub workspace_root: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub agent_execution_id: AgentExecutionId,
    pub status: AgentStatus,
    pub artifact_paths: Vec<String>,
    pub cost_cents: Option<i64>,
}
