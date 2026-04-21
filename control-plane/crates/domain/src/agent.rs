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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentFailureKind {
    HostInterruption,
}

impl std::fmt::Display for AgentFailureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentFailureKind::HostInterruption => write!(f, "host_interruption"),
        }
    }
}

impl std::str::FromStr for AgentFailureKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "host_interruption" => Ok(AgentFailureKind::HostInterruption),
            other => Err(format!("Unknown AgentFailureKind: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorActionHint {
    RecoveringFromSystemSleep,
    ResumingAfterNetworkChange,
}

impl std::fmt::Display for OperatorActionHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperatorActionHint::RecoveringFromSystemSleep => {
                write!(f, "recovering_from_system_sleep")
            }
            OperatorActionHint::ResumingAfterNetworkChange => {
                write!(f, "resuming_after_network_change")
            }
        }
    }
}

impl std::str::FromStr for OperatorActionHint {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "recovering_from_system_sleep" => Ok(OperatorActionHint::RecoveringFromSystemSleep),
            "resuming_after_network_change" => Ok(OperatorActionHint::ResumingAfterNetworkChange),
            other => Err(format!("Unknown OperatorActionHint: {other}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentExecution {
    pub id: AgentExecutionId,
    pub stage_execution_id: StageExecutionId,
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
    pub mcp_session_startup_latency_ms: Option<i64>,
}
