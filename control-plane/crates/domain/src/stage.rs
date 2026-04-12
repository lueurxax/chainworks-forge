use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{RunId, StageExecutionId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Pending,
    Ready,
    Running,
    WaitingApproval,
    Blocked,
    Completed,
    Failed,
    Skipped,
}

impl std::fmt::Display for StageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageStatus::Pending => write!(f, "pending"),
            StageStatus::Ready => write!(f, "ready"),
            StageStatus::Running => write!(f, "running"),
            StageStatus::WaitingApproval => write!(f, "waiting_approval"),
            StageStatus::Blocked => write!(f, "blocked"),
            StageStatus::Completed => write!(f, "completed"),
            StageStatus::Failed => write!(f, "failed"),
            StageStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for StageStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(StageStatus::Pending),
            "ready" => Ok(StageStatus::Ready),
            "running" => Ok(StageStatus::Running),
            "waiting_approval" => Ok(StageStatus::WaitingApproval),
            "blocked" => Ok(StageStatus::Blocked),
            "completed" => Ok(StageStatus::Completed),
            "failed" => Ok(StageStatus::Failed),
            "skipped" => Ok(StageStatus::Skipped),
            other => Err(format!("Unknown StageStatus: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageSettlementKind {
    Completed,
    Skipped,
    Failed,
}

impl std::fmt::Display for StageSettlementKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageSettlementKind::Completed => write!(f, "completed"),
            StageSettlementKind::Skipped => write!(f, "skipped"),
            StageSettlementKind::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for StageSettlementKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "completed" => Ok(StageSettlementKind::Completed),
            "skipped" => Ok(StageSettlementKind::Skipped),
            "failed" => Ok(StageSettlementKind::Failed),
            other => Err(format!("Unknown StageSettlementKind: {other}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageExecution {
    pub id: StageExecutionId,
    pub run_id: RunId,
    pub stage_id: String,
    pub label: String,
    pub status: StageStatus,
    pub iteration: i64,
    pub attempt_number: i64,
    pub settlement_kind: Option<StageSettlementKind>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    // ── Workflow-driven fields (populated when run is backed by YAML) ────
    /// Agent ID from the workflow state's `owner` field.
    pub owner_agent: Option<String>,
    /// Resolved ACP provider name ("claude", "codex", "gemini", etc.).
    pub provider: Option<String>,
    /// Resolved model from the backend profile.
    pub model: Option<String>,
    /// State type from the workflow: "start", "end", "manual_gate", or None.
    pub stage_type: Option<String>,
}
