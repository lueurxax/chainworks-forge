use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{ArtifactId, RunId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactFormat {
    Json,
    Markdown,
    Diff,
    Report,
}

impl std::fmt::Display for ArtifactFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactFormat::Json => write!(f, "json"),
            ArtifactFormat::Markdown => write!(f, "markdown"),
            ArtifactFormat::Diff => write!(f, "diff"),
            ArtifactFormat::Report => write!(f, "report"),
        }
    }
}

impl std::str::FromStr for ArtifactFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(ArtifactFormat::Json),
            "markdown" => Ok(ArtifactFormat::Markdown),
            "diff" => Ok(ArtifactFormat::Diff),
            "report" => Ok(ArtifactFormat::Report),
            other => Err(format!("Unknown ArtifactFormat: {other}")),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub run_id: RunId,
    pub stage_id: String,
    pub agent_id: String,
    pub name: String,
    pub contract_id: String,
    pub format: ArtifactFormat,
    pub file_path: String,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub provider: String,
    pub model: Option<String>,
    pub created_at: DateTime<Utc>,
    pub is_pinned: bool,
    pub report_kind: Option<String>,
    pub report_version: Option<i64>,
}
