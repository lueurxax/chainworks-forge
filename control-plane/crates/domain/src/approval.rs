use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{ApprovalId, RunId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Pending,
    Requested,
    Granted,
    Rejected,
    Expired,
}

impl std::fmt::Display for ApprovalDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalDecision::Pending => write!(f, "pending"),
            ApprovalDecision::Requested => write!(f, "requested"),
            ApprovalDecision::Granted => write!(f, "granted"),
            ApprovalDecision::Rejected => write!(f, "rejected"),
            ApprovalDecision::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for ApprovalDecision {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ApprovalDecision::Pending),
            "requested" => Ok(ApprovalDecision::Requested),
            "granted" => Ok(ApprovalDecision::Granted),
            "rejected" => Ok(ApprovalDecision::Rejected),
            "expired" => Ok(ApprovalDecision::Expired),
            other => Err(format!("Unknown ApprovalDecision: {other}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Approval {
    pub id: ApprovalId,
    pub run_id: RunId,
    pub stage_id: String,
    pub decision: ApprovalDecision,
    pub requested_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    pub comment: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}
