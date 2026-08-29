use serde::{Deserialize, Serialize};

use crate::discovery::LegacyBroadDiscoveryPolicy;
use crate::ids::{AgentExecutionId, ApprovalId, IdeaId, RunId, StageExecutionId};
use crate::mediation::MediationConfirmationDecision;
use crate::risk_lineage::RiskAcceptanceLineage;

// ── P029: Canonical PrincipalClass definition (owned by domain) ────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalClass {
    Operator,
    Agent,
    Observer,
    /// P080: read-only operator — diagnostics and diagnose_only only.
    /// Grants p080:diagnostics and p080.reconcile diagnose_only; excludes
    /// all mutating repair/clear actions.
    ReadOnlyOperator,
}

impl std::fmt::Display for PrincipalClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrincipalClass::Operator => write!(f, "operator"),
            PrincipalClass::Agent => write!(f, "agent"),
            PrincipalClass::Observer => write!(f, "observer"),
            PrincipalClass::ReadOnlyOperator => write!(f, "read_only_operator"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    CreateIdea(CreateIdeaCmd),
    StartRun(StartRunCmd),
    ApproveStage(ApproveStageCmd),
    RejectStage(RejectStageCmd),
    RetryStage(RetryStageCmd),
    ConsumeProviderQuotaHold(ConsumeProviderQuotaHoldCmd),
    ResolveWorkflowConflictTransition(ResolveWorkflowConflictTransitionCmd),
    ExtendWorkflowLoopBudget(ExtendWorkflowLoopBudgetCmd),
    OverrideLegacyDiscoveryPolicy(OverrideLegacyDiscoveryPolicyCmd),
    MainSyncRequest(MainSyncRequestCmd),
    MainSyncRetry(MainSyncRetryCmd),
    MainSyncSetRunOverride(MainSyncSetRunOverrideCmd),
    MainSyncRepairState(MainSyncRepairStateCmd),
    MainSyncRecordRecoveryDecision(MainSyncRecordRecoveryDecisionCmd),
    KnowledgeCapsuleIgnore(KnowledgeCapsuleIgnoreCmd),
    RetrofitCatalogSnapshot(RetrofitCatalogSnapshotCmd),
    CancelRun(CancelRunCmd),
    /// P083: Retry a run — re-queues a new AdvanceRun work item for the run.
    /// CallerRequestId is required; command_idempotency_contract_v1 guards duplicate issuance.
    RetryRun(RetryRunCmd),
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
    /// P083: Initiate graceful shutdown of a provider session.
    /// Writes a planned shutdown signal and records idempotency lease.
    /// CallerRequestId UUIDv4 supplied by operator; command_idempotency_contract_v1 guards
    /// against duplicate issuance.
    ShutdownProviderSession(ShutdownProviderSessionCmd),
    /// P083: Execute rollback from enforce to permissive or disabled.
    /// Writes to p083_rollback_audit and updates enforcement mode state.
    /// CallerRequestId UUIDv4 supplied by operator.
    P083RollbackExecution(P083RollbackExecutionCmd),
    /// P083: Set enforcement mode (disabled/permissive/enforce).
    /// Writes to p083_enforcement_mode_transition_journal and updates enforcement mode state.
    /// CallerRequestId UUIDv4 supplied by operator.
    P083SetEnforcementMode(P083SetEnforcementModeCmd),
    /// P083: Force-reconcile a side effect to reconciled status with operator decision.
    /// CallerRequestId UUIDv4 supplied by operator; command_idempotency_contract_v1 with TTL=300s.
    ForceReconcileSideEffect(ForceReconcileSideEffectCmd),
    /// P083: Operator confirms process is absent for identity-ambiguous hold.
    /// Moves process_fate to absent_verified and transitions held intent back to requested
    /// so settlement can resume. Requires CallerRequestId and operator confirmation.
    MarkProviderSessionProcessAbsent(MarkProviderSessionProcessAbsentCmd),
    /// P058: operator-only recovery for an escalation paused specifically because
    /// its wall-clock deadline elapsed. Opens a new bounded deadline window and
    /// schedules the ledger's current tier without rewriting prior history.
    ResumeEscalationDeadline(ResumeEscalationDeadlineCmd),
    /// P058: operator-only one-shot recovery for an escalation whose frozen chain
    /// reached its terminal human pause. Opens a linked bounded window for an
    /// explicit frozen backend-profile tier without resetting attempt history.
    ResumeEscalationChain(ResumeEscalationChainCmd),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateIdeaCmd {
    pub title: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_key: Option<String>,
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
    /// P084: Optional rollout-contract run-start policy request.
    /// The command handler validates and stamps this with caller identity plus
    /// command journal id before freezing it into Run.delivery_preflight_json.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollout_contract_preflight_policy_json: Option<String>,
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
    /// P083: CallerRequestId (lowercase UUIDv4) for command_idempotency_contract_v1.
    /// Optional for backward compatibility; idempotency is applied when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsumeProviderQuotaHoldCmd {
    pub run_id: RunId,
    pub stage_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResumeEscalationDeadlineCmd {
    pub run_id: RunId,
    pub escalation_ledger_id: String,
    pub reason: String,
    /// UUIDv7 supplied by the caller. The engine stores it on the deadline window
    /// as a second replay fence in addition to MCP command idempotency.
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResumeEscalationChainCmd {
    pub run_id: RunId,
    pub escalation_ledger_id: String,
    pub target_tier_id: String,
    pub reason: String,
    /// Optional one-shot operator instruction for the recovery-created invocation scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_instruction: Option<String>,
    /// UUIDv7 supplied by the caller. The linked window provides the durable
    /// replay fence for the complete stage/authority/work-item transaction.
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveWorkflowConflictTransitionCmd {
    pub run_id: RunId,
    pub conflict_id: String,
    pub selected_transition_id: String,
    pub resolution_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_instruction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_budget_extension: Option<WorkflowLoopBudgetExtensionCmd>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowLoopBudgetExtensionCmd {
    pub counter: String,
    pub additional_cycles: u32,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_conflict_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtendWorkflowLoopBudgetCmd {
    pub run_id: RunId,
    pub extension: WorkflowLoopBudgetExtensionCmd,
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
pub enum CatalogSnapshotRetrofitScope {
    EscalationPolicyOnly,
}

impl Default for CatalogSnapshotRetrofitScope {
    fn default() -> Self {
        Self::EscalationPolicyOnly
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrofitCatalogSnapshotCmd {
    pub run_id: RunId,
    pub expected_catalog_snapshot_hash: String,
    pub reason: String,
    #[serde(default)]
    pub scope: CatalogSnapshotRetrofitScope,
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
    /// P083: CallerRequestId (lowercase UUIDv4) for command_idempotency_contract_v1.
    /// Optional for backward compatibility; idempotency is applied when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// P083: Retry a run — re-queues a new AdvanceRun work item for a run that has failed or stalled.
/// CallerRequestId is required for command_idempotency_contract_v1 (TTL 120s).
/// Principal is bound from CallerContext, not this struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryRunCmd {
    pub run_id: RunId,
    /// P083: CallerRequestId (lowercase UUIDv4). Required for idempotency lease.
    pub request_id: String,
}

/// P083: Force-reconcile a side effect to reconciled status.
/// command_idempotency_contract_v1 with TTL=300s. Principal bound from CallerContext.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForceReconcileSideEffectCmd {
    /// UUID of the side effect to force-reconcile.
    pub effect_id: String,
    /// P083: CallerRequestId (lowercase UUIDv4). Required for idempotency lease.
    pub request_id: String,
    /// Operator-supplied decision JSON (side_effect_decision_v1).
    pub decision_json: String,
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
    /// P081 Phase 5: client-supplied idempotency key (UUIDv7 per attempt).
    /// When present, the command handler performs deduplication against
    /// approval_mutation_idempotency and returns the original result on retry.
    #[serde(default)]
    pub idempotency_key: Option<String>,
    /// P083: CallerRequestId (lowercase UUIDv4) for command_idempotency_contract_v1.
    /// When present, uses P083 idempotency path (TTL=300s) in preference to
    /// idempotency_key. P081 idempotency_key is accepted for backward compatibility
    /// when request_id is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
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
    /// Optional managed gate executor timeout in milliseconds. The engine applies
    /// a bounded default when omitted and clamps unsafe values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Raw JSON receipt from the gate executor (max 256KiB enforced at MCP boundary).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_json: Option<String>,
    /// P077: typed accepted risk lineage supplied through the governed command path.
    /// Free-form known_risks text never satisfies release entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted_risks: Vec<RiskAcceptanceLineage>,
}

// ── P083: Lifecycle commands ─────────────────────────────────────────────

/// P083: Graceful shutdown of a provider session.
/// provider_session_id must exist in provider_sessions.
/// cancellation_epoch is service-assigned; caller MUST NOT supply it.
/// Principal is bound from CallerContext, not this struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShutdownProviderSessionCmd {
    pub provider_session_id: String,
    /// CallerRequestId (UUIDv4) for command_idempotency_contract_v1.
    pub request_id: String,
    pub reason: String,
}

/// P083: Roll back enforcement mode to permissive or disabled.
/// target_enforcement_mode must be "permissive" or "disabled".
/// Principal is bound from CallerContext, not this struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct P083RollbackExecutionCmd {
    /// CallerRequestId (UUIDv4) for command_idempotency_contract_v1.
    pub request_id: String,
    /// "permissive" or "disabled"
    pub target_enforcement_mode: String,
    pub reason: String,
}

/// P083: Set enforcement mode (disabled/permissive/enforce).
/// target_mode must be one of the three allowed values.
/// Principal is bound from CallerContext, not this struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct P083SetEnforcementModeCmd {
    /// CallerRequestId (UUIDv4) for command_idempotency_contract_v1.
    pub request_id: String,
    /// "disabled", "permissive", or "enforce"
    pub target_mode: String,
    pub reason: String,
}

/// P083: Operator confirms a provider process is absent for an identity-ambiguous hold.
/// Atomically moves process_fate to absent_verified and transitions held intent back to
/// requested so shutdown settlement can resume. Requires CallerRequestId and operator
/// confirmation per manual_process_identity_check_ui_v1.available_actions.mark_process_absent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkProviderSessionProcessAbsentCmd {
    /// The provider session that holds the identity_ambiguous fate.
    pub provider_session_id: String,
    /// The cancellation_epoch of the held intent to clear.
    pub cancellation_epoch: i64,
    /// CallerRequestId (UUIDv4) for command_idempotency_contract_v1. TTL=120s.
    pub request_id: String,
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
    /// P081 Phase 3: Request-scoped caller class derived at auth resolution time.
    /// Stored as a snake_case string matching the boundary matrix caller_class enum.
    /// `None` for pre-P081 command journal rows; deserialization uses `#[serde(default)]`
    /// so existing rows remain readable.
    #[serde(default)]
    pub caller_class: Option<String>,
    /// SEC-P081-M002: Derived token_id for audit correlation (base32 sha256 diagnostic id).
    /// Not the raw bearer token. Never persisted to command_journal or wire; carried in
    /// memory so audit_log entries can include it for incident correlation.
    #[serde(skip)]
    pub token_id: Option<String>,
    /// P081 Phase 3: MCP idempotency key for state-changing calls. Persisted in
    /// command_journal so replay readback can bind the journal row to the idempotency record.
    /// `None` for GraphQL paths and pre-P081 MCP calls; `#[serde(skip)]` because the key
    /// is derived at dispatch time and must not round-trip through the command payload.
    #[serde(skip)]
    pub mcp_idempotency_key: Option<String>,
    /// P081 Phase 5: canonical request hash computed at the MCP transport boundary.
    /// This is carried only in memory so the command transaction can claim the
    /// idempotency key atomically with the command journal and domain writes.
    #[serde(skip)]
    pub mcp_idempotency_request_hash: Option<String>,
    /// P081 Phase 3: Boundary matrix row_id that allowed this command. Persisted in
    /// command_journal for audit linkage. `None` in legacy_compat mode or when the policy
    /// returns LegacyPassthrough.
    #[serde(skip)]
    pub boundary_row_id: Option<String>,
}

impl CallerContext {
    pub fn mcp(principal_id: &str, principal_class: &PrincipalClass, tool_name: &str) -> Self {
        CallerContext {
            surface: CallerSurface::Mcp,
            principal_id: principal_id.to_string(),
            principal_class: principal_class.clone(),
            caller_tool: tool_name.to_string(),
            request_id: None,
            caller_class: None,
            token_id: None,
            mcp_idempotency_key: None,
            mcp_idempotency_request_hash: None,
            boundary_row_id: None,
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
            caller_class: None,
            token_id: None,
            mcp_idempotency_key: None,
            mcp_idempotency_request_hash: None,
            boundary_row_id: None,
        }
    }

    /// Attach a P042 §9.3 correlation id — typically the value of the
    /// inbound `X-Request-ID` header (or a freshly minted one if the
    /// client didn't send one). Returns `self` for builder-style chaining.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// P081 Phase 3: Attach a derived caller_class string. Returns `self`
    /// for builder-style chaining.
    pub fn with_caller_class(mut self, caller_class: impl Into<String>) -> Self {
        self.caller_class = Some(caller_class.into());
        self
    }

    /// SEC-P081-M002: Attach a derived token_id for audit correlation. Not the raw token.
    /// Returns `self` for builder-style chaining.
    pub fn with_token_id(mut self, token_id: impl Into<String>) -> Self {
        self.token_id = Some(token_id.into());
        self
    }

    /// P081 Phase 3: Attach the MCP idempotency key so command_journal persists the linkage.
    pub fn with_mcp_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.mcp_idempotency_key = Some(key.into());
        self
    }

    /// P081 Phase 5: Attach the canonical MCP request hash for transactional idempotency.
    pub fn with_mcp_idempotency_request_hash(mut self, hash: impl Into<String>) -> Self {
        self.mcp_idempotency_request_hash = Some(hash.into());
        self
    }

    /// P081 Phase 3: Attach the boundary matrix row_id that allowed this command.
    pub fn with_boundary_row_id(mut self, row_id: impl Into<String>) -> Self {
        self.boundary_row_id = Some(row_id.into());
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
            caller_class: None,
            token_id: None,
            mcp_idempotency_key: None,
            mcp_idempotency_request_hash: None,
            boundary_row_id: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunStewardAnalysisCmd {
    pub reason: String,
    pub artifact_base: Option<String>,
}

// ── P083 HARDEN-001/002: Shared typed denial vocabulary ─────────────────────

/// P083 lifecycle denial codes shared between GraphQL error extensions and MCP
/// `denial_code` response fields. Per implementation_hardening_requirements_v1
/// P083-HARDEN-001 and P083-HARDEN-002: this enum is the canonical denial
/// vocabulary referenced by both API surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum P083LifecycleDenialCode {
    /// Required caller request id is absent.
    MissingCallerRequestId,
    /// Request-id format does not match lowercase UUIDv4 pattern.
    MalformedRequestId,
    /// Caller principal class is not permitted for this lifecycle command.
    PrincipalClassNotAllowed,
    /// Lifecycle state or target mode is invalid for the requested command.
    LifecycleStateInvalid,
    /// Request body failed the published schema contract.
    SchemaInvalid,
    /// Request body included properties outside the published schema contract.
    AdditionalPropertiesRejected,
    /// Rollback target field is absent.
    RollbackTargetRequired,
    /// Rollback target field is invalid.
    RollbackTargetInvalid,
    /// Same request_id, different command or intent_hash.
    RequestIntentMismatch,
    /// Same request_id and intent_hash with a still-pending lease (retry_after_seconds applies).
    IdempotencyInFlight,
    /// Committed outcome replayed for same or aliased request.
    IdempotencyReplayed,
    /// Expired pending lease reacquired by a concurrent caller; current caller lost the race.
    IdempotencyExpiredReacquired,
    /// Caller principal class is not operator.
    OperatorRequired,
    /// P083 operator class required specifically for P083 lifecycle commands.
    P083OperatorRequired,
    /// Provider session not found in authoritative table.
    ProviderSessionNotFound,
    /// Run not found or in terminal state.
    RunNotFound,
    /// Stage execution not found or not retryable.
    StageNotRetryable,
    /// Approval not found or not actionable.
    ApprovalNotActionable,
    /// Side effect not found or not force-reconcilable.
    SideEffectNotReconcilable,
    /// Enforcement mode transition is not permitted from the current mode.
    EnforcementModeTransitionDenied,
    /// Process identity ambiguous — manual_process_identity_check required.
    IdentityAmbiguous,
    /// Committed idempotency row exists but outcome_json is absent or unparseable.
    IdempotencyReplayCorrupt,
    /// Terminal failure for this command/intent; retry not allowed by per-command policy.
    IdempotencyTerminalFailure,
    /// Internal error — details logged server-side; not surfaced to caller.
    Internal,
}

impl P083LifecycleDenialCode {
    /// The bounded string value used in MCP `denial_code` fields and GraphQL
    /// error extension `code` values. All values are lowercase_snake_case.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingCallerRequestId => "missing_caller_request_id",
            Self::MalformedRequestId => "malformed_request_id",
            Self::PrincipalClassNotAllowed => "principal_class_not_allowed",
            Self::LifecycleStateInvalid => "lifecycle_state_invalid",
            Self::SchemaInvalid => "schema_invalid",
            Self::AdditionalPropertiesRejected => "additional_properties_rejected",
            Self::RollbackTargetRequired => "rollback_target_required",
            Self::RollbackTargetInvalid => "rollback_target_invalid",
            Self::RequestIntentMismatch => "request_intent_mismatch",
            Self::IdempotencyInFlight => "idempotency_in_flight",
            Self::IdempotencyReplayed => "idempotency_replayed",
            Self::IdempotencyExpiredReacquired => "idempotency_expired_reacquired",
            Self::OperatorRequired => "operator_required",
            Self::P083OperatorRequired => "p083_operator_required",
            Self::ProviderSessionNotFound => "provider_session_not_found",
            Self::RunNotFound => "run_not_found",
            Self::StageNotRetryable => "stage_not_retryable",
            Self::ApprovalNotActionable => "approval_not_actionable",
            Self::SideEffectNotReconcilable => "side_effect_not_reconcilable",
            Self::EnforcementModeTransitionDenied => "enforcement_mode_transition_denied",
            Self::IdentityAmbiguous => "identity_ambiguous",
            Self::IdempotencyReplayCorrupt => "idempotency_replay_corrupt",
            Self::IdempotencyTerminalFailure => "idempotency_terminal_failure",
            Self::Internal => "internal",
        }
    }

    /// All denial code strings for use in bounded metric label validation and
    /// GraphQL schema documentation. Mirrors `metric_labels_contract_v1`.
    pub const ALL: &'static [&'static str] = &[
        "missing_caller_request_id",
        "malformed_request_id",
        "principal_class_not_allowed",
        "lifecycle_state_invalid",
        "schema_invalid",
        "additional_properties_rejected",
        "rollback_target_required",
        "rollback_target_invalid",
        "request_intent_mismatch",
        "idempotency_in_flight",
        "idempotency_replayed",
        "idempotency_expired_reacquired",
        "operator_required",
        "p083_operator_required",
        "provider_session_not_found",
        "run_not_found",
        "stage_not_retryable",
        "approval_not_actionable",
        "side_effect_not_reconcilable",
        "enforcement_mode_transition_denied",
        "identity_ambiguous",
        "idempotency_replay_corrupt",
        "idempotency_terminal_failure",
        "internal",
    ];
}

impl std::fmt::Display for P083LifecycleDenialCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── P083 HARDEN-011: Per-command recommended minimum TTL ────────────────────

/// Per-command recommended minimum TTL seconds per
/// implementation_hardening_requirements_v1 P083-HARDEN-011.
/// Rollout lint warns when configured TTL is below recommendation.
pub struct P083CommandTtlPolicy {
    pub command: &'static str,
    /// Mandatory TTL from command_idempotency_contract_v1.
    pub ttl_seconds: u64,
    /// Minimum TTL recommended for safe operation.
    pub recommended_min_ttl_seconds: u64,
}

/// Centralized per-command TTL policy table per P083-HARDEN-011.
/// All entries from command_idempotency_contract_v1.ttl_seconds.
pub const P083_COMMAND_TTL_POLICIES: &[P083CommandTtlPolicy] = &[
    P083CommandTtlPolicy {
        command: "runs.cancel",
        ttl_seconds: 120,
        recommended_min_ttl_seconds: 60,
    },
    P083CommandTtlPolicy {
        command: "runs.retry",
        ttl_seconds: 120,
        recommended_min_ttl_seconds: 60,
    },
    P083CommandTtlPolicy {
        command: "stages.retry",
        ttl_seconds: 120,
        recommended_min_ttl_seconds: 60,
    },
    P083CommandTtlPolicy {
        command: "approvals.resolve",
        ttl_seconds: 300,
        recommended_min_ttl_seconds: 120,
    },
    P083CommandTtlPolicy {
        command: "side_effects.force_reconcile",
        ttl_seconds: 300,
        recommended_min_ttl_seconds: 120,
    },
    P083CommandTtlPolicy {
        command: "provider_session.shutdown",
        ttl_seconds: 120,
        recommended_min_ttl_seconds: 90,
    },
    P083CommandTtlPolicy {
        command: "provider_session.mark_process_absent",
        ttl_seconds: 120,
        recommended_min_ttl_seconds: 90,
    },
    P083CommandTtlPolicy {
        command: "p083.rollback_execution",
        ttl_seconds: 120,
        recommended_min_ttl_seconds: 60,
    },
    P083CommandTtlPolicy {
        command: "p083.set_enforcement_mode",
        ttl_seconds: 120,
        recommended_min_ttl_seconds: 60,
    },
];

/// Return the TTL policy for a command. Returns None for unknown commands.
pub fn p083_ttl_policy_for(command: &str) -> Option<&'static P083CommandTtlPolicy> {
    P083_COMMAND_TTL_POLICIES
        .iter()
        .find(|p| p.command == command)
}

/// Check whether `ttl_seconds` is below the recommended minimum for `command`.
/// Returns Some(recommended) when below recommendation, None when acceptable.
pub fn p083_ttl_below_recommendation(command: &str, ttl_seconds: u64) -> Option<u64> {
    p083_ttl_policy_for(command).and_then(|p| {
        if ttl_seconds < p.recommended_min_ttl_seconds {
            Some(p.recommended_min_ttl_seconds)
        } else {
            None
        }
    })
}

// ── P083 HARDEN-007: Centralized per-command failed-terminal retry policy ───

/// Whether a new same-intent request may acquire a lease after the prior
/// attempt reached a failed-terminal state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailedTerminalRetryAllowed {
    /// New same-intent request may always acquire a new lease after failure.
    Always,
    /// New same-intent request may acquire a new lease only after a cooldown period.
    AfterCooldownSeconds(u64),
    /// New same-intent request for the same intent is denied after terminal failure.
    Never,
}

/// Per-command failed-terminal retry policy per implementation_hardening_requirements_v1
/// P083-HARDEN-007.
pub struct P083FailedTerminalRetryPolicy {
    pub command: &'static str,
    /// Whether a new lease may be acquired after a terminal failure.
    pub retry_allowed: FailedTerminalRetryAllowed,
    /// Rationale for the retry policy.
    pub rationale: &'static str,
}

/// Centralized per-command failed-terminal retry policy table per P083-HARDEN-007.
/// All entries from command_idempotency_contract_v1 recovery_rules.
pub const P083_FAILED_TERMINAL_RETRY_POLICIES: &[P083FailedTerminalRetryPolicy] = &[
    P083FailedTerminalRetryPolicy {
        command: "runs.cancel",
        retry_allowed: FailedTerminalRetryAllowed::Always,
        rationale: "Cancel is idempotent in effect; a new attempt with the same intent is safe.",
    },
    P083FailedTerminalRetryPolicy {
        command: "runs.retry",
        retry_allowed: FailedTerminalRetryAllowed::Always,
        rationale: "Re-queue is safe to retry; backend guards against double-queuing via run status.",
    },
    P083FailedTerminalRetryPolicy {
        command: "stages.retry",
        retry_allowed: FailedTerminalRetryAllowed::Always,
        rationale: "Stage retry is guarded by stage status; new same-intent request may proceed.",
    },
    P083FailedTerminalRetryPolicy {
        command: "approvals.resolve",
        retry_allowed: FailedTerminalRetryAllowed::Never,
        rationale: "Approval resolution is a one-way state transition; failure requires a human decision, not an automatic retry.",
    },
    P083FailedTerminalRetryPolicy {
        command: "side_effects.force_reconcile",
        retry_allowed: FailedTerminalRetryAllowed::Always,
        rationale: "Force-reconcile failure leaves the side effect unresolved; a new attempt is safe.",
    },
    P083FailedTerminalRetryPolicy {
        command: "provider_session.shutdown",
        retry_allowed: FailedTerminalRetryAllowed::AfterCooldownSeconds(30),
        rationale: "Shutdown failure may reflect a transient identity state; 30s cooldown reduces PID reuse risk.",
    },
    P083FailedTerminalRetryPolicy {
        command: "provider_session.mark_process_absent",
        retry_allowed: FailedTerminalRetryAllowed::AfterCooldownSeconds(30),
        rationale: "Manual process-absent recovery clears an identity hold; cooldown reduces stale identity and PID reuse risk after a failed terminal attempt.",
    },
    P083FailedTerminalRetryPolicy {
        command: "p083.rollback_execution",
        retry_allowed: FailedTerminalRetryAllowed::Always,
        rationale: "Rollback failure is safe to retry with a new request after the prior terminal failure.",
    },
    P083FailedTerminalRetryPolicy {
        command: "p083.set_enforcement_mode",
        retry_allowed: FailedTerminalRetryAllowed::Always,
        rationale: "Mode-set failure is safe to retry; backend enforces CAS on the mode state table.",
    },
];

/// Return the failed-terminal retry policy for a command. Returns None for unknown commands.
pub fn p083_failed_terminal_retry_policy_for(
    command: &str,
) -> Option<&'static P083FailedTerminalRetryPolicy> {
    P083_FAILED_TERMINAL_RETRY_POLICIES
        .iter()
        .find(|p| p.command == command)
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
            rollout_contract_preflight_policy_json: None,
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

    #[test]
    fn p083_lifecycle_denial_code_all_round_trip_as_str() {
        let codes = [
            P083LifecycleDenialCode::MissingCallerRequestId,
            P083LifecycleDenialCode::MalformedRequestId,
            P083LifecycleDenialCode::PrincipalClassNotAllowed,
            P083LifecycleDenialCode::LifecycleStateInvalid,
            P083LifecycleDenialCode::SchemaInvalid,
            P083LifecycleDenialCode::AdditionalPropertiesRejected,
            P083LifecycleDenialCode::RollbackTargetRequired,
            P083LifecycleDenialCode::RollbackTargetInvalid,
            P083LifecycleDenialCode::RequestIntentMismatch,
            P083LifecycleDenialCode::IdempotencyInFlight,
            P083LifecycleDenialCode::IdempotencyReplayed,
            P083LifecycleDenialCode::IdempotencyExpiredReacquired,
            P083LifecycleDenialCode::OperatorRequired,
            P083LifecycleDenialCode::P083OperatorRequired,
            P083LifecycleDenialCode::ProviderSessionNotFound,
            P083LifecycleDenialCode::RunNotFound,
            P083LifecycleDenialCode::StageNotRetryable,
            P083LifecycleDenialCode::ApprovalNotActionable,
            P083LifecycleDenialCode::SideEffectNotReconcilable,
            P083LifecycleDenialCode::EnforcementModeTransitionDenied,
            P083LifecycleDenialCode::IdentityAmbiguous,
            P083LifecycleDenialCode::IdempotencyReplayCorrupt,
            P083LifecycleDenialCode::IdempotencyTerminalFailure,
            P083LifecycleDenialCode::Internal,
        ];
        assert_eq!(codes.len(), P083LifecycleDenialCode::ALL.len());
        for code in &codes {
            assert!(
                P083LifecycleDenialCode::ALL.contains(&code.as_str()),
                "denial code {} not in ALL",
                code.as_str()
            );
        }
    }

    #[test]
    fn p083_ttl_policy_covers_all_nine_commands() {
        let required = [
            "runs.cancel",
            "runs.retry",
            "stages.retry",
            "approvals.resolve",
            "side_effects.force_reconcile",
            "provider_session.shutdown",
            "provider_session.mark_process_absent",
            "p083.rollback_execution",
            "p083.set_enforcement_mode",
        ];
        for cmd in &required {
            let policy = p083_ttl_policy_for(cmd);
            assert!(policy.is_some(), "no TTL policy for command {cmd}");
            let p = policy.unwrap();
            assert!(
                p.ttl_seconds >= p.recommended_min_ttl_seconds,
                "command {cmd}: ttl_seconds {} < recommended_min_ttl_seconds {}",
                p.ttl_seconds,
                p.recommended_min_ttl_seconds
            );
        }
    }

    #[test]
    fn p083_ttl_below_recommendation_returns_none_when_acceptable() {
        assert_eq!(p083_ttl_below_recommendation("runs.cancel", 120), None);
        assert_eq!(p083_ttl_below_recommendation("runs.cancel", 60), None);
    }

    #[test]
    fn p083_ttl_below_recommendation_returns_recommendation_when_too_low() {
        let result = p083_ttl_below_recommendation("runs.cancel", 10);
        assert_eq!(result, Some(60));
    }

    #[test]
    fn p083_failed_terminal_retry_policy_covers_all_nine_commands() {
        let required = [
            "runs.cancel",
            "runs.retry",
            "stages.retry",
            "approvals.resolve",
            "side_effects.force_reconcile",
            "provider_session.shutdown",
            "provider_session.mark_process_absent",
            "p083.rollback_execution",
            "p083.set_enforcement_mode",
        ];
        for cmd in &required {
            let policy = p083_failed_terminal_retry_policy_for(cmd);
            assert!(
                policy.is_some(),
                "no failed-terminal retry policy for command {cmd}"
            );
        }
    }

    #[test]
    fn p083_approvals_resolve_retry_not_allowed_after_terminal_failure() {
        let policy = p083_failed_terminal_retry_policy_for("approvals.resolve").unwrap();
        assert_eq!(policy.retry_allowed, FailedTerminalRetryAllowed::Never);
    }

    #[test]
    fn p083_provider_session_shutdown_has_cooldown_policy() {
        let policy = p083_failed_terminal_retry_policy_for("provider_session.shutdown").unwrap();
        assert!(matches!(
            policy.retry_allowed,
            FailedTerminalRetryAllowed::AfterCooldownSeconds(_)
        ));
    }

    #[test]
    fn p083_provider_session_mark_process_absent_has_cooldown_policy() {
        let ttl = p083_ttl_policy_for("provider_session.mark_process_absent").unwrap();
        assert_eq!(ttl.ttl_seconds, 120);
        let retry =
            p083_failed_terminal_retry_policy_for("provider_session.mark_process_absent").unwrap();
        assert!(matches!(
            retry.retry_allowed,
            FailedTerminalRetryAllowed::AfterCooldownSeconds(30)
        ));
    }
}
