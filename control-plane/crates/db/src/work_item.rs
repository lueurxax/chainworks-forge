use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use domain::ids::RunId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemKind {
    InvokeAgent,
    SettleStage,
    AdvanceRun,
    RebuildProjection,
    StartupRepair,
    TriggerNextStage,
    StewardAnalysis,
    /// P086: claim and execute a queued agent_work_continuations row.
    ProcessContinuation,
}

impl std::fmt::Display for WorkItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkItemKind::InvokeAgent => write!(f, "invoke_agent"),
            WorkItemKind::SettleStage => write!(f, "settle_stage"),
            WorkItemKind::AdvanceRun => write!(f, "advance_run"),
            WorkItemKind::RebuildProjection => write!(f, "rebuild_projection"),
            WorkItemKind::StartupRepair => write!(f, "startup_repair"),
            WorkItemKind::TriggerNextStage => write!(f, "trigger_next_stage"),
            WorkItemKind::StewardAnalysis => write!(f, "steward_analysis"),
            WorkItemKind::ProcessContinuation => write!(f, "process_continuation"),
        }
    }
}

impl std::str::FromStr for WorkItemKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "invoke_agent" => Ok(WorkItemKind::InvokeAgent),
            "settle_stage" => Ok(WorkItemKind::SettleStage),
            "advance_run" => Ok(WorkItemKind::AdvanceRun),
            "rebuild_projection" => Ok(WorkItemKind::RebuildProjection),
            "startup_repair" => Ok(WorkItemKind::StartupRepair),
            "trigger_next_stage" => Ok(WorkItemKind::TriggerNextStage),
            "steward_analysis" => Ok(WorkItemKind::StewardAnalysis),
            "process_continuation" => Ok(WorkItemKind::ProcessContinuation),
            other => Err(format!("Unknown WorkItemKind: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for WorkItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkItemStatus::Pending => write!(f, "pending"),
            WorkItemStatus::Running => write!(f, "running"),
            WorkItemStatus::Completed => write!(f, "completed"),
            WorkItemStatus::Failed => write!(f, "failed"),
            WorkItemStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for WorkItemStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(WorkItemStatus::Pending),
            "running" => Ok(WorkItemStatus::Running),
            "completed" => Ok(WorkItemStatus::Completed),
            "failed" => Ok(WorkItemStatus::Failed),
            "cancelled" => Ok(WorkItemStatus::Cancelled),
            other => Err(format!("Unknown WorkItemStatus: {other}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub kind: WorkItemKind,
    pub payload_json: String,
    pub status: WorkItemStatus,
    pub run_id: Option<RunId>,
    pub stage_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub scheduled_at: DateTime<Utc>,
    pub attempt_count: i64,
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::WorkItemKind;

    #[test]
    fn steward_analysis_work_item_kind_roundtrips() {
        let value = WorkItemKind::StewardAnalysis.to_string();
        assert_eq!(value, "steward_analysis");
        assert_eq!(
            value.parse::<WorkItemKind>().unwrap(),
            WorkItemKind::StewardAnalysis
        );
    }
}
