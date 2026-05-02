use serde::{Deserialize, Serialize};

use crate::discovery::LegacyBroadDiscoveryPolicy;
use crate::ids::{AgentExecutionId, ApprovalId, IdeaId, RunId, StageExecutionId};
use crate::mediation::MediationConfirmationDecision;

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
    ResolveWorkflowConflictTransition(ResolveWorkflowConflictTransitionCmd),
    OverrideLegacyDiscoveryPolicy(OverrideLegacyDiscoveryPolicyCmd),
    MainSyncRequest(MainSyncRequestCmd),
    MainSyncRetry(MainSyncRetryCmd),
    MainSyncSetRunOverride(MainSyncSetRunOverrideCmd),
    MainSyncRepairState(MainSyncRepairStateCmd),
    MainSyncRecordRecoveryDecision(MainSyncRecordRecoveryDecisionCmd),
    KnowledgeCapsuleIgnore(KnowledgeCapsuleIgnoreCmd),
    CancelRun(CancelRunCmd),
    ResetSession(ResetSessionCmd),
    RunStewardAnalysis(RunStewardAnalysisCmd),
    OverrideArtifactContract(OverrideArtifactContractCmd),
    /// P017 Phase B: Resolve a lead mediation confirmation via the
    /// engine-owned settlement boundary.
    ResolveLeadMediationConfirmation(ResolveLeadMediationConfirmationCmd),
    /// P072: Converged stage-approval command keyed by approval_id.
    /// Both GraphQL (approveApproval / rejectApproval) and MCP
    /// (approvals.resolve with subject_kind == stage_approval) route
    /// through this command. CallerContext carries identity; the command
    /// carries only domain data and server-resolved provenance.
    ResolveApproval(ResolveApprovalCmd),
    /// P077: Phase 0 gate-settlement command (journaled-only; no orchestrator side effects).
    /// MCP settle_proposal_gate tool routes through this command.
    /// CallerContext.principal_id overrides the command's principal field at the engine
    /// boundary (BLK-008: bind from authenticated context, not caller-supplied payload).
    SettleProposalGate(SettleProposalGateCmd),
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
    /// P060: Frozen review routing options JSON (ReviewRoutingOptions).
    /// When present, controls how proposal reviewers are selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_routing_json: Option<String>,
    /// P077: Closeout readiness mode frozen at run admission from workflow snapshot metadata.
    /// Values: "advisory" | "enforcement". NULL → advisory (legacy fallback).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closeout_readiness_mode: Option<String>,
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
    /// P065: Optional one-shot operator instruction for the retry-created invocation scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_instruction: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveWorkflowConflictTransitionCmd {
    pub run_id: RunId,
    pub conflict_id: String,
    pub selected_transition_id: String,
    pub resolution_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_instruction: Option<String>,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainSyncMode {
    Off,
    DryRun,
    ManualOnly,
    Automatic,
}

impl Default for MainSyncMode {
    fn default() -> Self {
        Self::Off
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeCapsulesMode {
    Off,
    EmitOnly,
    AttachAndInject,
}

impl Default for KnowledgeCapsulesMode {
    fn default() -> Self {
        Self::Off
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainSyncTriggerReason {
    BeforeInitialImplementation,
    BeforeRetry,
    BeforeReview,
    OperatorRequest,
    BeforeFinalApproval,
    StartupRepair,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MainSyncRequestCmd {
    pub run_id: RunId,
    pub trigger_reason: MainSyncTriggerReason,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by_stage_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by_work_item_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MainSyncRetryCmd {
    pub run_id: RunId,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_attempt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MainSyncSetRunOverrideCmd {
    pub run_id: RunId,
    pub mode: MainSyncMode,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MainSyncRepairStateCmd {
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_note: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainSyncRecoveryDecision {
    RetrySync,
    MarkRecovered,
    Escalate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MainSyncRecordRecoveryDecisionCmd {
    pub run_id: RunId,
    pub decision: MainSyncRecoveryDecision,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeCapsuleIgnoreCmd {
    pub run_id: RunId,
    pub capsule_id: String,
    pub reason: String,
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

/// P017 Phase B: Command to resolve a lead mediation confirmation.
/// Distinct from ApproveStage/RejectStage per the frozen contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveLeadMediationConfirmationCmd {
    pub run_id: RunId,
    pub mediation_record_id: String,
    pub confirmation_subject_id: String,
    pub decision: MediationConfirmationDecision,
    pub comment: Option<String>,
    pub conflict_fingerprint: String,
    pub idempotency_key: String,
}

// ── P072: Converged approval resolution command ─────────────────────────

/// Decision for `ResolveApprovalCmd`. Maps directly to the two GraphQL
/// mutations: `approveApproval` → `Approved`, `rejectApproval` → `Rejected`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalResolutionDecision {
    Approved,
    Rejected,
}

impl std::fmt::Display for ApprovalResolutionDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalResolutionDecision::Approved => write!(f, "approved"),
            ApprovalResolutionDecision::Rejected => write!(f, "rejected"),
        }
    }
}

impl std::str::FromStr for ApprovalResolutionDecision {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "approved" | "granted" => Ok(ApprovalResolutionDecision::Approved),
            "rejected" => Ok(ApprovalResolutionDecision::Rejected),
            other => Err(format!("Unknown ApprovalResolutionDecision: {other}")),
        }
    }
}

/// P072: Converged stage-approval command. `approval_id` is the canonical
/// northbound identity; `run_id` and `stage_id` are server-resolved
/// provenance retained for command_journal auditability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveApprovalCmd {
    pub approval_id: ApprovalId,
    pub decision: ApprovalResolutionDecision,
    pub rationale: Option<String>,
    /// Server-resolved from approval_id — not supplied by caller.
    pub run_id: RunId,
    /// Server-resolved from approval_id — not supplied by caller.
    pub stage_id: String,
}

// ── P077: Gate settlement command ──────────────────────────────────────

/// Action for `SettleProposalGateCmd`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalGateSettlementAction {
    /// Compatibility alias for the managed gate executor path.
    RecordSettlement,
    /// Run the managed gate executor and activate its result.
    Execute,
    /// Import a typed `proposal_gate_receipt.v1` emitted by the managed executor.
    ImportReceipt,
    /// Operator-authorized waiver with full lineage.
    Waive,
}

/// P077 Phase 0: settle a proposal gate result.
/// The `principal` field is server-overridden from CallerContext at the engine
/// boundary — do NOT trust the caller-supplied value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettleProposalGateCmd {
    pub run_id: RunId,
    pub proposal_id: String,
    pub stage_id: String,
    pub action: ProposalGateSettlementAction,
    /// Server-overridden from CallerContext.principal_id at engine boundary (BLK-008).
    pub principal: String,
    pub capability: String,
    pub journal_id: String,
    pub authority: String,
    pub reason: String,
    pub source_artifacts: Vec<String>,
    pub workflow_digest: String,
    pub worktree_head: String,
    pub dirty_or_changed_file_digest: String,
    pub source_generation_ids: Vec<String>,
    pub current_fingerprint: String,
    /// Raw JSON receipt from the gate executor (max 256KiB enforced at MCP boundary).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_json: Option<String>,
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
            review_routing_json: None,
            closeout_readiness_mode: None,
        };

        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(
            json["delivery_configuration_json"],
            r#"{"repo_identifier":"repo-1","repo_root":"/repo"}"#
        );
    }

    #[test]
    fn proposal_064_main_sync_command_round_trips_through_serde() {
        let cmd = Command::MainSyncRequest(MainSyncRequestCmd {
            run_id: RunId::new(),
            trigger_reason: MainSyncTriggerReason::BeforeReview,
            idempotency_key: "run:123:before-review".into(),
            requested_by_stage_id: Some("state_8_review_started".into()),
            requested_by_work_item_id: Some("work-item-1".into()),
        });

        let json = serde_json::to_value(&cmd).unwrap();
        assert_eq!(
            json["MainSyncRequest"]["trigger_reason"],
            serde_json::Value::String("before_review".into())
        );
        assert_eq!(
            json["MainSyncRequest"]["idempotency_key"],
            serde_json::Value::String("run:123:before-review".into())
        );

        let parsed: Command = serde_json::from_value(json).unwrap();
        match parsed {
            Command::MainSyncRequest(parsed) => {
                assert_eq!(parsed.trigger_reason, MainSyncTriggerReason::BeforeReview);
                assert_eq!(
                    parsed.requested_by_stage_id.as_deref(),
                    Some("state_8_review_started")
                );
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn proposal_064_mode_enums_use_snake_case_contract_values() {
        assert_eq!(
            serde_json::to_value(MainSyncMode::ManualOnly).unwrap(),
            serde_json::Value::String("manual_only".into())
        );
        assert_eq!(
            serde_json::to_value(KnowledgeCapsulesMode::AttachAndInject).unwrap(),
            serde_json::Value::String("attach_and_inject".into())
        );
        assert_eq!(MainSyncMode::default(), MainSyncMode::Off);
        assert_eq!(KnowledgeCapsulesMode::default(), KnowledgeCapsulesMode::Off);
    }
}
