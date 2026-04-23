pub mod adapters;
pub mod manager;
pub mod session;
pub mod transport;

pub use manager::AcpRuntimeManager;
pub use session::{AcpSession, AcpSessionHandle};

use std::collections::BTreeMap;

use domain::agent::AgentStatus;
use domain::discovery::{
    ExpectedOutputSpec, LegacyBroadDiscoveryPolicy, LegacyBroadDiscoverySnapshot,
    PrePromptExpectedOutputMetadata,
};
use domain::ids::{AgentExecutionId, RunId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub run_id: RunId,
    /// Durable stage execution UUID when this request originates from the
    /// orchestrator. Adapter-level tests and legacy serialized requests fall
    /// back to `stage_id`.
    #[serde(default)]
    pub stage_execution_id: Option<String>,
    pub stage_id: String,
    /// Stage execution attempt number. P053 pre-prompt discovery metadata uses
    /// this to keep retry baselines distinct from earlier attempts.
    #[serde(default = "default_attempt_number")]
    pub attempt_number: u32,
    /// Durable agent execution UUID when this request originates from the
    /// orchestrator. Adapter-level tests and legacy serialized requests fall
    /// back to `agent_id`.
    #[serde(default)]
    pub agent_execution_id: Option<String>,
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
    /// Typed P053 discovery specs. New runtime paths consume this when present;
    /// `expected_output_paths` remains a compatibility projection for older
    /// adapters and prompt rendering.
    #[serde(default)]
    pub expected_outputs: Vec<ExpectedOutputSpec>,
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
    /// Executable MCP server payloads resolved from Chainworks extension ids
    /// into provider-local runtime ids before ACP startup.
    #[serde(default)]
    pub mcp_servers: Vec<AcpMcpServerPayload>,
    /// P050: Per-run meta root for workspace isolation.
    /// Set as `CHAINWORKS_META_ROOT` env var on ACP subprocess so
    /// YAML artifact path templates resolve to the per-run directory.
    #[serde(default)]
    pub chainworks_meta_root: Option<String>,
    /// P053 compatibility escape hatch. Broad workspace/worktree diffing is
    /// disabled unless the frozen run plan or audited retry override enables it.
    #[serde(default)]
    pub legacy_broad_discovery_policy: LegacyBroadDiscoveryPolicy,
}

fn default_attempt_number() -> u32 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub agent_execution_id: AgentExecutionId,
    pub status: AgentStatus,
    pub artifact_paths: Vec<String>,
    #[serde(default)]
    pub discovered_artifacts: Vec<DiscoveredArtifact>,
    /// P053 typed metadata captured for declared expected outputs immediately
    /// before this prompt turn is sent to the provider.
    #[serde(default)]
    pub pre_prompt_expected_outputs: Vec<PrePromptExpectedOutputMetadata>,
    /// Sanitized text streamed by the ACP provider during this prompt turn.
    /// This is persisted by the engine as recovery evidence when present.
    #[serde(default)]
    pub transcript_text: Option<String>,
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
    /// Transport-owned observation of the MCP runtime truth accepted for this
    /// ACP session. When the provider does not return an explicit accepted
    /// server list, this records an explicit predicted-after-success fallback
    /// rather than silently treating predicted truth as observed truth.
    #[serde(default)]
    pub mcp_observation: Option<McpActualObservation>,
    #[serde(default)]
    pub actual_mcp_extensions: Vec<String>,
    #[serde(default)]
    pub actual_mcp_runtime_ids: Vec<String>,
    #[serde(default)]
    pub mcp_session_startup_latency_ms: Option<i64>,
    /// Nonblocking diagnostics observed while closing a one-shot ACP session
    /// after the provider already returned a prompt result.
    #[serde(default)]
    pub close_diagnostic: Option<AcpCloseDiagnostic>,
    #[serde(default)]
    pub acp_pre_initialize_local_latency_ms: Option<u64>,
    #[serde(default)]
    pub acp_initialize_latency_ms: Option<u64>,
    #[serde(default)]
    pub acp_session_new_latency_ms: Option<u64>,
    #[serde(default)]
    pub acp_prompt_duration_ms: Option<u64>,
    #[serde(default)]
    pub acp_pre_prompt_metadata_latency_ms: Option<u64>,
    #[serde(default)]
    pub acp_pre_prompt_metadata_timeout: bool,
    #[serde(default)]
    pub acp_pre_prompt_metadata_digest_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_broad_discovery_snapshot: Option<LegacyBroadDiscoverySnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpCloseDiagnostic {
    #[serde(default)]
    pub transport_error_code: Option<String>,
    #[serde(default)]
    pub provider_exit_status: Option<i64>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AcpMcpServerPayload {
    pub id: String,
    #[serde(rename = "extensionId")]
    pub extension_id: String,
    pub transport: ResolvedMcpServerTransport,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedMcpServerTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    Platform {
        provider: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpActualObservation {
    pub source: String,
    pub trust_level: String,
    pub actual_equals_predicted: bool,
    #[serde(default)]
    pub provider_session_id: Option<String>,
    #[serde(default)]
    pub actual_extensions: Vec<String>,
    #[serde(default)]
    pub actual_runtime_ids: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
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
    #[serde(default)]
    pub source_kind: DiscoveredArtifactSourceKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveredArtifactSourceKind {
    #[default]
    ProviderEnvelope,
    ChainworksOutput,
    ExactPath,
}
