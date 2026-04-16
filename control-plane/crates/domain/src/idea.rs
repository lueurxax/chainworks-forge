use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::IdeaId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeaStatus {
    Draft,
    Active,
    Archived,
}

impl std::fmt::Display for IdeaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdeaStatus::Draft => write!(f, "draft"),
            IdeaStatus::Active => write!(f, "active"),
            IdeaStatus::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for IdeaStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(IdeaStatus::Draft),
            "active" => Ok(IdeaStatus::Active),
            "archived" => Ok(IdeaStatus::Archived),
            other => Err(format!("Unknown IdeaStatus: {other}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Idea {
    pub id: IdeaId,
    pub title: String,
    pub body: String,
    pub workspace_root_path: Option<String>,
    /// Optional stable project cohort key. New P049 runs freeze this onto Run;
    /// missing values fall back to `untagged` at run creation.
    pub project_key: Option<String>,
    pub status: IdeaStatus,
    pub created_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}
