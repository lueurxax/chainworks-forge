use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentExecutionId, StageExecutionId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Completed => write!(f, "completed"),
            AgentStatus::Failed => write!(f, "failed"),
            AgentStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for AgentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "running" => Ok(AgentStatus::Running),
            "completed" => Ok(AgentStatus::Completed),
            "failed" => Ok(AgentStatus::Failed),
            "cancelled" => Ok(AgentStatus::Cancelled),
            other => Err(format!("Unknown AgentStatus: {other}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentExecution {
    pub id: AgentExecutionId,
    pub stage_execution_id: Option<StageExecutionId>,
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: AgentStatus,
    pub owner_execution_lineage_id: Option<String>,
    pub session_lineage_id: Option<String>,
    pub session_generation_id: Option<String>,
    pub rehydrated_from_checkpoint_artifact_id: Option<String>,
    pub invocation_owner_key: Option<String>,
    pub session_reuse_scope: Option<String>,
    pub session_family_id: Option<String>,
    pub session_reuse_disposition: Option<String>,
    pub session_reset_reason: Option<String>,
    pub backend_profile_id: Option<String>,
    pub requested_mcp_extensions_json: Option<String>,
    pub predicted_mcp_extensions_json: Option<String>,
    pub predicted_mcp_runtime_ids_json: Option<String>,
    pub actual_mcp_extensions_json: Option<String>,
    pub actual_mcp_runtime_ids_json: Option<String>,
    pub denied_mcp_extensions_json: Option<String>,
    pub mcp_blocking_issues_json: Option<String>,
    pub actual_mcp_observation_json: Option<String>,
    pub actual_xcode_runtime_observation_json: Option<String>,
    pub mcp_session_startup_latency_ms: Option<i64>,
    /// P017: Owner-aware execution identity. Defaults to "stage_execution".
    pub owner_kind: Option<String>,
    /// P017: Owner ID — stage_execution_id for stage-owned, mediation record ID for mediation-owned.
    pub owner_id: Option<String>,
    /// P017: Mediation record ID when owner_kind = lead_conflict_mediation.
    pub lead_mediation_record_id: Option<String>,
    /// P017: Origin stage execution ID for mediation-owned executions (compatibility context).
    pub origin_stage_execution_id: Option<String>,
    // ── P017 R4 / API-002: per-attempt cost & transcript attribution ──
    /// Total cost in cents reported by the provider for this execution
    /// attempt (sum of provider-reported `cost_cents`). Always populated
    /// for mediation-owned executions; stage-owned executions populate
    /// when the provider reports usage.
    #[serde(default)]
    pub total_cost_cents: Option<i64>,
    /// Provider input tokens for this attempt (from `UsageSnapshot`).
    #[serde(default)]
    pub input_tokens: Option<i64>,
    /// Provider output tokens for this attempt (from `UsageSnapshot`).
    #[serde(default)]
    pub output_tokens: Option<i64>,
    /// Provider cached input tokens for this attempt.
    #[serde(default)]
    pub cached_input_tokens: Option<i64>,
    /// FK into `artifacts.id` for the session-transcript artifact when the
    /// executor persisted one. Lets MCP/GraphQL `execution_attempts.transcript_ref`
    /// resolve directly without artifact-name heuristics.
    #[serde(default)]
    pub transcript_artifact_id: Option<String>,
    // ── P066: Toolchain cache mapping diagnostics ─────────────────────────────
    /// Bounded JSON document describing the toolchain cache mapping outcome for
    /// this execution attempt. NULL = pre-P066 legacy row (synthesize
    /// mapping_state=legacy_row_unavailable northbound).
    #[serde(default)]
    pub actual_toolchain_mapping_diagnostics_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentFailureKind {
    ProviderQuota,
    ProviderPermissionRequired,
    ProviderPermissionRejected,
    ProviderTimeout,
    ProviderInternalError,
    TransportEpipe,
    TransportProtocolError,
    TransportClosed,
    McpStartupTimeout,
    McpPermissionModalStall,
    XcodeHostEnvironmentError,
    HostInterruption,
    MissingRequiredOutputs,
    InvalidOutputContract,
    CancelledByOperator,
    SupersededByRetry,
    Unknown,
}

impl std::fmt::Display for AgentFailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AgentFailureKind::ProviderQuota => "provider_quota",
            AgentFailureKind::ProviderPermissionRequired => "provider_permission_required",
            AgentFailureKind::ProviderPermissionRejected => "provider_permission_rejected",
            AgentFailureKind::ProviderTimeout => "provider_timeout",
            AgentFailureKind::ProviderInternalError => "provider_internal_error",
            AgentFailureKind::TransportEpipe => "transport_epipe",
            AgentFailureKind::TransportProtocolError => "transport_protocol_error",
            AgentFailureKind::TransportClosed => "transport_closed",
            AgentFailureKind::McpStartupTimeout => "mcp_startup_timeout",
            AgentFailureKind::McpPermissionModalStall => "mcp_permission_modal_stall",
            AgentFailureKind::XcodeHostEnvironmentError => "xcode_host_environment_error",
            AgentFailureKind::HostInterruption => "host_interruption",
            AgentFailureKind::MissingRequiredOutputs => "missing_required_outputs",
            AgentFailureKind::InvalidOutputContract => "invalid_output_contract",
            AgentFailureKind::CancelledByOperator => "cancelled_by_operator",
            AgentFailureKind::SupersededByRetry => "superseded_by_retry",
            AgentFailureKind::Unknown => "unknown",
        })
    }
}

impl std::str::FromStr for AgentFailureKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "provider_quota" => AgentFailureKind::ProviderQuota,
            "provider_permission_required" => AgentFailureKind::ProviderPermissionRequired,
            "provider_permission_rejected" => AgentFailureKind::ProviderPermissionRejected,
            "provider_timeout" => AgentFailureKind::ProviderTimeout,
            "provider_internal_error" => AgentFailureKind::ProviderInternalError,
            "transport_epipe" => AgentFailureKind::TransportEpipe,
            "transport_protocol_error" => AgentFailureKind::TransportProtocolError,
            "transport_closed" => AgentFailureKind::TransportClosed,
            "mcp_startup_timeout" => AgentFailureKind::McpStartupTimeout,
            "mcp_permission_modal_stall" => AgentFailureKind::McpPermissionModalStall,
            "xcode_host_environment_error" => AgentFailureKind::XcodeHostEnvironmentError,
            "host_interruption" => AgentFailureKind::HostInterruption,
            "missing_required_outputs" => AgentFailureKind::MissingRequiredOutputs,
            "invalid_output_contract" => AgentFailureKind::InvalidOutputContract,
            "cancelled_by_operator" => AgentFailureKind::CancelledByOperator,
            "superseded_by_retry" => AgentFailureKind::SupersededByRetry,
            "unknown" => AgentFailureKind::Unknown,
            other => return Err(format!("Unknown AgentFailureKind: {other}")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentOutputSettlement {
    None,
    MissingRequiredOutputs,
    InvalidRequiredOutputs,
    ValidOutputsFromCompletedExecution,
    ValidOutputsFromFailedExecution,
    IgnoredLateOutputs,
}

impl Default for AgentOutputSettlement {
    fn default() -> Self {
        AgentOutputSettlement::None
    }
}

impl std::fmt::Display for AgentOutputSettlement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AgentOutputSettlement::None => "none",
            AgentOutputSettlement::MissingRequiredOutputs => "missing_required_outputs",
            AgentOutputSettlement::InvalidRequiredOutputs => "invalid_required_outputs",
            AgentOutputSettlement::ValidOutputsFromCompletedExecution => {
                "valid_outputs_from_completed_execution"
            }
            AgentOutputSettlement::ValidOutputsFromFailedExecution => {
                "valid_outputs_from_failed_execution"
            }
            AgentOutputSettlement::IgnoredLateOutputs => "ignored_late_outputs",
        })
    }
}

impl std::str::FromStr for AgentOutputSettlement {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "none" => AgentOutputSettlement::None,
            "missing_required_outputs" => AgentOutputSettlement::MissingRequiredOutputs,
            "invalid_required_outputs" => AgentOutputSettlement::InvalidRequiredOutputs,
            "valid_outputs_from_completed_execution" => {
                AgentOutputSettlement::ValidOutputsFromCompletedExecution
            }
            "valid_outputs_from_failed_execution" => {
                AgentOutputSettlement::ValidOutputsFromFailedExecution
            }
            "ignored_late_outputs" => AgentOutputSettlement::IgnoredLateOutputs,
            other => return Err(format!("Unknown AgentOutputSettlement: {other}")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorActionHint {
    Retry,
    WaitUntilRetryAfter,
    SwitchProvider,
    AuthorizeXcode,
    RecoveringFromSystemSleep,
    ResumingAfterNetworkChange,
    IgnoreStaleOutput,
    InspectLogs,
}

impl std::fmt::Display for OperatorActionHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            OperatorActionHint::Retry => "retry",
            OperatorActionHint::WaitUntilRetryAfter => "wait_until_retry_after",
            OperatorActionHint::SwitchProvider => "switch_provider",
            OperatorActionHint::AuthorizeXcode => "authorize_xcode",
            OperatorActionHint::RecoveringFromSystemSleep => "recovering_from_system_sleep",
            OperatorActionHint::ResumingAfterNetworkChange => "resuming_after_network_change",
            OperatorActionHint::IgnoreStaleOutput => "ignore_stale_output",
            OperatorActionHint::InspectLogs => "inspect_logs",
        })
    }
}

impl std::str::FromStr for OperatorActionHint {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "retry" => OperatorActionHint::Retry,
            "wait_until_retry_after" => OperatorActionHint::WaitUntilRetryAfter,
            "switch_provider" => OperatorActionHint::SwitchProvider,
            "authorize_xcode" => OperatorActionHint::AuthorizeXcode,
            "recovering_from_system_sleep" => OperatorActionHint::RecoveringFromSystemSleep,
            "resuming_after_network_change" => OperatorActionHint::ResumingAfterNetworkChange,
            "ignore_stale_output" => OperatorActionHint::IgnoreStaleOutput,
            "inspect_logs" => OperatorActionHint::InspectLogs,
            other => return Err(format!("Unknown OperatorActionHint: {other}")),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentExecutionRuntimeFacts {
    pub agent_execution_id: AgentExecutionId,
    pub failure_kind: Option<AgentFailureKind>,
    pub failure_kind_raw_debug: Option<String>,
    pub failure_kind_version: i64,
    pub failure_message_redacted: Option<String>,
    pub failure_message_redaction_version: i64,
    pub retry_after: Option<DateTime<Utc>>,
    pub operator_action_hint: Option<OperatorActionHint>,
    pub provider_exit_status: Option<i64>,
    pub transport_error_code: Option<String>,
    pub supervision_classification: Option<String>,
    pub output_settlement: AgentOutputSettlement,
    pub valid_required_outputs: bool,
    pub late_output_count: i64,
    pub ignored_late_output_count: i64,
    pub session_reuse_reason: Option<String>,
    pub quota_ledger_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentExecutionRuntimeFacts {
    pub fn defaults_for(agent_execution_id: AgentExecutionId, now: DateTime<Utc>) -> Self {
        Self {
            agent_execution_id,
            failure_kind: None,
            failure_kind_raw_debug: None,
            failure_kind_version: 1,
            failure_message_redacted: None,
            failure_message_redaction_version: 1,
            retry_after: None,
            operator_action_hint: None,
            provider_exit_status: None,
            transport_error_code: None,
            supervision_classification: None,
            output_settlement: AgentOutputSettlement::None,
            valid_required_outputs: false,
            late_output_count: 0,
            ignored_late_output_count: 0,
            session_reuse_reason: None,
            quota_ledger_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSourceClaimState {
    Active,
    SupersededPendingRetry,
    Superseded,
    Closed,
    LegacyUnowned,
}

impl std::fmt::Display for ArtifactSourceClaimState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ArtifactSourceClaimState::Active => "active",
            ArtifactSourceClaimState::SupersededPendingRetry => "superseded_pending_retry",
            ArtifactSourceClaimState::Superseded => "superseded",
            ArtifactSourceClaimState::Closed => "closed",
            ArtifactSourceClaimState::LegacyUnowned => "legacy_unowned",
        })
    }
}

impl std::str::FromStr for ArtifactSourceClaimState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "active" => ArtifactSourceClaimState::Active,
            "superseded_pending_retry" => ArtifactSourceClaimState::SupersededPendingRetry,
            "superseded" => ArtifactSourceClaimState::Superseded,
            "closed" => ArtifactSourceClaimState::Closed,
            "legacy_unowned" => ArtifactSourceClaimState::LegacyUnowned,
            other => return Err(format!("Unknown ArtifactSourceClaimState: {other}")),
        })
    }
}
