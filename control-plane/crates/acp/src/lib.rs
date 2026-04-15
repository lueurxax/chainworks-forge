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
    /// Provisioned worktree root path (Proposal 007). When set and
    /// `worktree_write_enabled` is true, the ACP session uses this as cwd.
    #[serde(default)]
    pub worktree_root: Option<String>,
    /// Whether the agent has write access to the worktree.
    #[serde(default)]
    pub worktree_write_enabled: bool,
    /// Worktree strategy from catalog (e.g. "dedicated", "shared_implementation_worktree").
    #[serde(default)]
    pub worktree_strategy: Option<String>,
    /// Canonical output paths declared by the compiled workflow task.
    /// These must be reported back even when the agent overwrites an existing
    /// file instead of creating a brand-new one.
    #[serde(default)]
    pub expected_output_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub agent_execution_id: AgentExecutionId,
    pub status: AgentStatus,
    pub artifact_paths: Vec<String>,
    pub cost_cents: Option<i64>,
}
