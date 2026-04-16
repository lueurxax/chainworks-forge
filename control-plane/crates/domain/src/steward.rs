use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StewardAnalysisStatus {
    Running,
    Completed,
    Inconclusive,
    Failed,
    Superseded,
}

impl std::fmt::Display for StewardAnalysisStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StewardAnalysisStatus::Running => write!(f, "running"),
            StewardAnalysisStatus::Completed => write!(f, "completed"),
            StewardAnalysisStatus::Inconclusive => write!(f, "inconclusive"),
            StewardAnalysisStatus::Failed => write!(f, "failed"),
            StewardAnalysisStatus::Superseded => write!(f, "superseded"),
        }
    }
}

impl std::str::FromStr for StewardAnalysisStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "inconclusive" => Ok(Self::Inconclusive),
            "failed" => Ok(Self::Failed),
            "superseded" => Ok(Self::Superseded),
            other => Err(format!("Unknown StewardAnalysisStatus: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CohortQuality {
    Strong,
    Acceptable,
    Weak,
}

impl std::fmt::Display for CohortQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CohortQuality::Strong => write!(f, "strong"),
            CohortQuality::Acceptable => write!(f, "acceptable"),
            CohortQuality::Weak => write!(f, "weak"),
        }
    }
}

impl std::str::FromStr for CohortQuality {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "strong" => Ok(Self::Strong),
            "acceptable" => Ok(Self::Acceptable),
            "weak" => Ok(Self::Weak),
            other => Err(format!("Unknown CohortQuality: {other}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardAnalysis {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub run_count: i64,
    pub cohort_keys_json: String,
    pub cohort_quality: CohortQuality,
    pub status: StewardAnalysisStatus,
    pub degradation_count: i64,
    pub improvement_count: i64,
    pub workflow_snapshot_artifact_hash: String,
    pub agent_catalog_snapshot_hash: String,
    pub steward_config_snapshot_hash: String,
    pub metrics_snapshot_artifact_id: Option<String>,
    pub baseline_snapshot_artifact_id: Option<String>,
    pub agent_catalog_snapshot_artifact_id: Option<String>,
    pub workflow_snapshot_artifact_id: Option<String>,
    pub config_change_log_artifact_id: Option<String>,
    pub health_report_artifact_id: Option<String>,
    pub degradation_alert_artifact_id: Option<String>,
    pub agent_tuning_artifact_id: Option<String>,
    pub workflow_tuning_artifact_id: Option<String>,
    pub experiment_plan_artifact_id: Option<String>,
    pub audit_report_artifact_id: Option<String>,
    pub trigger_reason: String,
    pub error_summary: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardAnalysisRunLink {
    pub id: String,
    pub analysis_id: String,
    pub run_id: String,
    pub role: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StewardRecommendation {
    pub id: String,
    pub analysis_id: String,
    pub created_at: DateTime<Utc>,
    pub category: String,
    pub summary: String,
    pub target_metric: String,
    pub confidence_level: String,
    pub status: String,
    pub source_artifact_name: Option<String>,
    pub decision_comment: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
}
