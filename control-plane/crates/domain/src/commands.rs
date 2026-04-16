use serde::{Deserialize, Serialize};

use crate::ids::{IdeaId, RunId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    StartRun(StartRunCmd),
    ApproveStage(ApproveStageCmd),
    RejectStage(RejectStageCmd),
    RetryStage(RetryStageCmd),
    CancelRun(CancelRunCmd),
    ResetSession(ResetSessionCmd),
    RunStewardAnalysis(RunStewardAnalysisCmd),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartRunCmd {
    pub idea_id: IdeaId,
    pub workflow_id: String,
    pub workflow_title: String,
    pub workspace_root: String,
    pub artifact_root: String,
    /// Frozen delivery configuration JSON for repo-backed runs.
    pub delivery_configuration_json: Option<String>,
    /// Required by active run-start ingress when deterministic Steward snapshot truth is enabled.
    /// Path to the workflow YAML file (enables state-machine-driven execution).
    pub workflow_yaml_path: String,
    /// Required by active run-start ingress when deterministic Steward snapshot truth is enabled.
    /// Path to the agent catalog YAML file.
    pub agent_catalog_yaml_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApproveStageCmd {
    pub run_id: RunId,
    pub stage_id: String,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RejectStageCmd {
    pub run_id: RunId,
    pub stage_id: String,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryStageCmd {
    pub run_id: RunId,
    pub stage_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelRunCmd {
    pub run_id: RunId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResetSessionCmd {
    pub run_id: RunId,
    pub stage_id: String,
}

// ── P029: Caller identity for audit journaling ──────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerSurface {
    Mcp,
    Graphql,
}

impl std::fmt::Display for CallerSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallerSurface::Mcp => write!(f, "mcp"),
            CallerSurface::Graphql => write!(f, "graphql"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CallerContext {
    pub surface: CallerSurface,
    pub principal_id: String,
    pub principal_class: auth::PrincipalClass,
    pub caller_tool: String,
}

impl CallerContext {
    pub fn mcp(
        principal_id: &str,
        principal_class: &auth::PrincipalClass,
        tool_name: &str,
    ) -> Self {
        CallerContext {
            surface: CallerSurface::Mcp,
            principal_id: principal_id.to_string(),
            principal_class: principal_class.clone(),
            caller_tool: tool_name.to_string(),
        }
    }

    pub fn graphql(
        principal_id: &str,
        principal_class: &auth::PrincipalClass,
        mutation_name: &str,
    ) -> Self {
        CallerContext {
            surface: CallerSurface::Graphql,
            principal_id: principal_id.to_string(),
            principal_class: principal_class.clone(),
            caller_tool: mutation_name.to_string(),
        }
    }

    /// Test/fixture stand-in. Tags rows as caller_surface='mcp' with a
    /// synthetic operator principal. Plain pub fn (not cfg(test)) because
    /// integration tests in engine/tests/, graphql-server/tests/, and
    /// daemon/tests/ are separate crates.
    pub fn test_fixture() -> Self {
        CallerContext {
            surface: CallerSurface::Mcp,
            principal_id: "test-operator".to_string(),
            principal_class: auth::PrincipalClass::Operator,
            caller_tool: "test".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunStewardAnalysisCmd {
    pub reason: String,
    pub artifact_base: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_run_cmd_serializes_delivery_configuration_json() {
        let cmd = StartRunCmd {
            idea_id: IdeaId::new(),
            workflow_id: "wf-1".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/workspace".into(),
            artifact_root: "/tmp/artifacts".into(),
            workflow_yaml_path: "examples/workflows/workflow.yaml".into(),
            agent_catalog_yaml_path: "examples/agents/agents.yaml".into(),
            delivery_configuration_json: Some(
                r#"{"repo_identifier":"repo-1","repo_root":"/repo"}"#.into(),
            ),
        };

        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(
            json["delivery_configuration_json"],
            r#"{"repo_identifier":"repo-1","repo_root":"/repo"}"#
        );
    }
}
