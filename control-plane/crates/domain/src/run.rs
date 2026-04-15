use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{IdeaId, RunId};

/// Frozen delivery configuration for repo-backed runs (Proposal 007).
/// Matches Swift `DeliveryConfiguration`. Persisted as JSON on the Run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliveryConfiguration {
    /// Repository identity (canonicalized).
    pub repo_identifier: String,
    /// Absolute path to the repository root.
    pub repo_root: String,
    /// Base branch for worktree creation (e.g. "main").
    pub base_branch: String,
    /// Base path for worktree directories.
    pub worktree_base_path: String,
    /// Target branch for the run's worktree.
    pub target_branch: String,
    /// Release target identifier.
    #[serde(default)]
    pub release_target_id: Option<String>,
    /// Release mode: "sandbox" or "staging".
    #[serde(default)]
    pub release_mode: Option<String>,
}

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
    // ── Workflow state machine fields ────────────────────────────────────
    /// Current state in the workflow state machine (e.g. "state_4_proposal_reviewed").
    pub current_state: Option<String>,
    /// Path to the workflow YAML file (stored so the orchestrator can re-compile).
    pub workflow_yaml_path: Option<String>,
    /// Path to the agent catalog YAML file.
    pub agent_catalog_yaml_path: Option<String>,
    // ── Worktree fields (Proposal 007) ──────────────────────────────────
    /// Provisioned worktree root path. Set after `WorktreeProvisioner::provision()`.
    pub worktree_root: Option<String>,
    /// Base branch the worktree was created from (e.g. "main").
    pub base_branch: Option<String>,
    /// Base revision (commit hash) at the time of worktree provisioning.
    pub base_revision: Option<String>,
    /// Target branch created for the worktree (e.g. "cw/auth-flow/a1b2c3d4").
    pub target_branch: Option<String>,
    /// Frozen delivery configuration JSON (Proposal 007).
    /// Set at run start for repo-backed runs. Consumed by WorktreeProvisioner,
    /// RepoSafetyGuard, release agents, and evidence export.
    pub delivery_configuration_json: Option<String>,
}
