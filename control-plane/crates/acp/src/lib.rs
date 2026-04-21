pub mod adapters;
pub mod manager;
pub mod session;
pub mod transport;
pub mod xcode_broker;
pub mod xcode_target;

pub use manager::{AcpRuntimeManager, BrokeredXcodeLeaseAttachment, XcodeBrokerLeaseAttacher};
pub use session::{AcpSession, AcpSessionHandle};
pub use xcode_broker::{
    BrokerMcpPolicy, BrokerMcpPolicyDecision, XcodeBrokerHealthSnapshot, XcodeBrokerHealthState,
    XcodeBrokerHttpRouteState, XcodeMcpBackend, XcodeMcpBackendRequestContext, XcodeMcpBridgePool,
    XcodeMcpBridgePoolConfig, XcodeMcpLeaseState, XcodeMcpProcessBackend,
    XcodeMcpProcessBackendConfig,
};
pub use xcode_target::{
    probe_local_xcode_host, target_resolver_failure_class, HostProbeContext,
    LocalXcodeHostProbeConfig, XcodeProcessCandidate, XcodeTargetResolver,
    XcodeTargetSelectionConfidence, XcodeTargetSelectionInput, XcodeTargetSnapshot,
};

use std::collections::BTreeMap;

use anyhow::Result;
use domain::agent::AgentStatus;
use domain::ids::{AgentExecutionId, RunId};
use domain::xcode_runtime::XcodeRuntimeObservationUpdate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionRequest {
    /// Engine-persisted execution id, when this request is owned by a durable
    /// agent_executions row. ACP uses this only for runtime observation sinks.
    #[serde(default)]
    pub agent_execution_id: Option<AgentExecutionId>,
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
    /// Executable MCP server payloads resolved from Chainworks extension ids
    /// into provider-local runtime ids before ACP startup.
    #[serde(default)]
    pub mcp_servers: Vec<AcpMcpServerPayload>,
}

impl ExecutionRequest {
    pub fn brokered_xcode_intents(&self) -> Vec<&BrokeredXcodeMcpIntent> {
        self.mcp_servers
            .iter()
            .filter_map(|server| match &server.transport {
                ResolvedMcpServerTransport::XcodeBrokerIntent { intent }
                    if intent.provider_http_required =>
                {
                    Some(intent)
                }
                _ => None,
            })
            .collect()
    }
}

#[async_trait::async_trait]
pub trait XcodeRuntimeObservationSink: Send + Sync {
    async fn append_xcode_runtime_observation(
        &self,
        agent_execution_id: AgentExecutionId,
        update: XcodeRuntimeObservationUpdate,
    ) -> Result<()>;
}

pub struct NoopXcodeRuntimeObservationSink;

#[async_trait::async_trait]
impl XcodeRuntimeObservationSink for NoopXcodeRuntimeObservationSink {
    async fn append_xcode_runtime_observation(
        &self,
        _agent_execution_id: AgentExecutionId,
        _update: XcodeRuntimeObservationUpdate,
    ) -> Result<()> {
        Ok(())
    }
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
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    XcodeBrokerIntent {
        intent: BrokeredXcodeMcpIntent,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrokeredXcodeMcpIntent {
    pub extension_id: String,
    pub runtime_id: String,
    pub server_id: String,
    #[serde(default)]
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub xcode_pid_selector: Option<String>,
    #[serde(default)]
    pub runtime_profile_id: Option<String>,
    #[serde(default)]
    pub permission_profile_id: Option<String>,
    #[serde(default)]
    pub resolved_tool_allowlist_hash: Option<String>,
    pub provider_http_required: bool,
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
}
