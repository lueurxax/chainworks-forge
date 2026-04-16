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
    /// Keep the ACP transport-backed session alive after the first prompt so
    /// a later turn can reuse it via `session/prompt`.
    #[serde(default)]
    pub keep_session_alive: bool,
    /// Prefer routing this turn through an already-open session handle.
    #[serde(default)]
    pub reuse_existing_session: bool,
    /// Existing live-session generation handle to reuse. When present, the
    /// runtime manager routes the prompt to the already-open session instead
    /// of spawning a new `session/new` flow.
    #[serde(default)]
    pub session_generation_id: Option<String>,
    /// Provider-native session id from the transport layer. This is mainly
    /// useful for tracing and for proving the bridge between generation ids
    /// and the underlying ACP session handle.
    #[serde(default)]
    pub provider_session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub agent_execution_id: AgentExecutionId,
    pub status: AgentStatus,
    pub artifact_paths: Vec<String>,
    #[serde(default)]
    pub discovered_artifacts: Vec<DiscoveredArtifact>,
    pub cost_cents: Option<i64>,
    #[serde(default)]
    pub usage: Option<UsageSnapshot>,
    #[serde(default)]
    pub provider_session_id: Option<String>,
    #[serde(default)]
    pub reused_existing_session: bool,
    /// Live-session generation handle for reusable sessions. Populated when
    /// the manager keeps a session alive or routes a prompt through an
    /// existing live session.
    #[serde(default)]
    pub session_generation_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub cost_cents: Option<i64>,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub model_context_window: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredArtifact {
    pub name: String,
    pub content: Vec<u8>,
    #[serde(default)]
    pub source_path: Option<String>,
}
