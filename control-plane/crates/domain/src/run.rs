use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{IdeaId, RunId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Ready,
    Running,
    WaitingApproval,
    Blocked,
    Completed,
    Failed,
    Cancelled,
    Cancelling,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunStatus::Pending => write!(f, "pending"),
            RunStatus::Ready => write!(f, "ready"),
            RunStatus::Running => write!(f, "running"),
            RunStatus::WaitingApproval => write!(f, "waiting_approval"),
            RunStatus::Blocked => write!(f, "blocked"),
            RunStatus::Completed => write!(f, "completed"),
            RunStatus::Failed => write!(f, "failed"),
            RunStatus::Cancelled => write!(f, "cancelled"),
            RunStatus::Cancelling => write!(f, "cancelling"),
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(RunStatus::Pending),
            "ready" => Ok(RunStatus::Ready),
            "running" => Ok(RunStatus::Running),
            "waiting_approval" => Ok(RunStatus::WaitingApproval),
            "blocked" => Ok(RunStatus::Blocked),
            "completed" => Ok(RunStatus::Completed),
            "failed" => Ok(RunStatus::Failed),
            "cancelled" => Ok(RunStatus::Cancelled),
            "cancelling" => Ok(RunStatus::Cancelling),
            other => Err(format!("Unknown RunStatus: {other}")),
        }
    }
}

impl RunStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub idea_id: IdeaId,
    pub status: RunStatus,
    pub workflow_id: String,
    pub workflow_title: String,
    pub workspace_root: String,
    pub artifact_root: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancellation_requested_at: Option<DateTime<Utc>>,
    pub cancellation_settled_at: Option<DateTime<Utc>>,
}
