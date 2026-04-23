use serde::{Deserialize, Serialize};

use crate::discovery::LegacyBroadDiscoveryPolicy;
use crate::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};

// ── P029: Canonical PrincipalClass definition (owned by domain) ────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalClass {
    Operator,
    Agent,
    Observer,
}

impl std::fmt::Display for PrincipalClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrincipalClass::Operator => write!(f, "operator"),
            PrincipalClass::Agent => write!(f, "agent"),
            PrincipalClass::Observer => write!(f, "observer"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    StartRun(StartRunCmd),
    ApproveStage(ApproveStageCmd),
    RejectStage(RejectStageCmd),
    RetryStage(RetryStageCmd),
    OverrideLegacyDiscoveryPolicy(OverrideLegacyDiscoveryPolicyCmd),
    CancelRun(CancelRunCmd),
    ResetSession(ResetSessionCmd),
    RunStewardAnalysis(RunStewardAnalysisCmd),
    OverrideArtifactContract(OverrideArtifactContractCmd),
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
    #[serde(default)]
    pub consume_quota_budget_now: bool,
    /// Optional narrow retry target. When set, the command schedules only the
    /// matching InvokeAgent task instead of rerunning the full stage fanout.
    #[serde(default)]
    pub agent_execution_id: Option<AgentExecutionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_discovery_override_policy: Option<LegacyBroadDiscoveryPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_discovery_override_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OverrideLegacyDiscoveryPolicyCmd {
    pub run_id: RunId,
    pub stage_id: String,
    pub target_stage_execution_id: StageExecutionId,
    pub target_attempt_number: i64,
    pub legacy_discovery_override_policy: LegacyBroadDiscoveryPolicy,
    pub legacy_discovery_override_reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelRunCmd {
    pub run_id: RunId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OverrideArtifactContractCmd {
    pub run_id: RunId,
    pub contract_id: String,
    pub override_type: String,
    pub from_status: String,
    pub to_status: String,
    pub reason: String,
    pub source_artifacts: Vec<String>,
    pub expires_at_stage: String,
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
    pub principal_class: PrincipalClass,
    pub caller_tool: String,
    /// Inbound HTTP `X-Request-ID` (P042 §9.3). Populated by the axum
    /// middleware on GraphQL/MCP HTTP paths; left `None` for MCP stdio
    /// and for call sites that bypass the middleware (tests). The
    /// daemon persists it in `command_journal.request_id` so an
    /// operator can join HTTP access logs, daemon logs, and the audit
    /// trail on one id.
    #[serde(default)]
    pub request_id: Option<String>,
}

impl CallerContext {
    pub fn mcp(principal_id: &str, principal_class: &PrincipalClass, tool_name: &str) -> Self {
        CallerContext {
            surface: CallerSurface::Mcp,
            principal_id: principal_id.to_string(),
            principal_class: principal_class.clone(),
            caller_tool: tool_name.to_string(),
            request_id: None,
        }
    }

    pub fn graphql(
        principal_id: &str,
        principal_class: &PrincipalClass,
        mutation_name: &str,
    ) -> Self {
        CallerContext {
            surface: CallerSurface::Graphql,
            principal_id: principal_id.to_string(),
            principal_class: principal_class.clone(),
            caller_tool: mutation_name.to_string(),
            request_id: None,
        }
    }

    /// Attach a P042 §9.3 correlation id — typically the value of the
    /// inbound `X-Request-ID` header (or a freshly minted one if the
    /// client didn't send one). Returns `self` for builder-style chaining.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Test/fixture stand-in. Tags rows as caller_surface='mcp' with a
    /// synthetic operator principal. Plain pub fn (not cfg(test)) because
    /// integration tests in engine/tests/, graphql-server/tests/, and
    /// daemon/tests/ are separate crates.
    pub fn test_fixture() -> Self {
        CallerContext {
            surface: CallerSurface::Mcp,
            principal_id: "test-operator".to_string(),
            principal_class: PrincipalClass::Operator,
            caller_tool: "test".to_string(),
            request_id: None,
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
