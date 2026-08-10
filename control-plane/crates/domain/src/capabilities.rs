use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CapabilityToolId {
    IdeasCreate,
    IdeasList,
    RunsStart,
    RunsList,
    RunsGet,
    RunsMainSyncRequest,
    RunsMainSyncRetry,
    RunsMainSyncSetOverride,
    RunsMainSyncRepairState,
    RunsMainSyncRecordRecoveryDecision,
    RunsKnowledgeCapsuleIgnore,
    RunsRetrofitCatalogSnapshot,
    RunsCancel,
    ApprovalsList,
    ApprovalsResolve,
    StagesRetry,
    StagesConsumeProviderQuotaHold,
    WorkflowConflictsResolve,
    WorkflowLoopBudgetExtend,
    LegacyDiscoveryOverrideCreate,
    ReportsGet,
    ArtifactsOverrideContract,
    StewardRunAnalysis,
    StewardListAnalyses,
    StewardGetAnalysis,
    RuntimeHealth,
    OperatorAlertsList,
    StorageHealth,
    StorageWritePressure,
    StorageEvidenceSpoolSummary,
    StorageReconcileEvidenceOrphans,
    /// P077: settle a proposal gate result via runs.settle_proposal_gate.
    ProposalGateSettle,
    /// P078: read-only side-effect list projection.
    EffectsList,
    /// P078: inspect a single side-effect record.
    EffectsInspect,
    /// P078: perform read-only reconciliation readback for an unresolved effect.
    EffectsReconcile,
    /// P078: mark an effect as conflict after operator confirmation.
    EffectsMarkConflict,
    /// P078: mark an effect as unrecoverable after operator confirmation.
    EffectsMarkUnrecoverable,
    /// P078: clear an effect after manual operator verification.
    EffectsClearAfterManualVerification,
    /// P087: repair an orphaned maintenance slot.
    StorageMaintenanceRepairSlot,
    /// P087: clear projection invalidation backlog.
    StorageProjectionsClearBacklog,
    /// P087: clear projection poison flag.
    StorageProjectionsClearPoison,
    /// P086: read-only continuation history and current status for an agent execution.
    AgentsContinuationStatus,
    /// P086: read-only list of eligible continuation candidates for a run.
    AgentsContinuationCandidates,
    /// P086: issue a continuation command for an eligible code_writer AgentExecution.
    AgentsContinueWork,
    /// P076: read latest observe-only auto-retry ledger/catalog state.
    AutomationAutoRetryLatest,
    /// P080: read-only stale execution diagnostics projection.
    P080DiagnosticsGet,
    /// P080: request reconciliation or diagnose-only analysis for a stale execution.
    P080ReconcileRequest,
    /// P080: clear a permanent hold on a stale execution.
    P080ClearPermanentHold,
    /// P083: initiate graceful shutdown of a provider session.
    ProviderSessionShutdown,
    /// P083: execute rollback to permissive or disabled enforcement mode.
    P083RollbackExecution,
    /// P083: set the P083 enforcement mode (disabled/permissive/enforce).
    P083SetEnforcementMode,
    /// P083: re-queue an AdvanceRun work item for a stalled or failed run.
    RetryRun,
    /// P083: force-reconcile a side effect to reconciled status with operator decision.
    SideEffectsForceReconcile,
    /// P083: operator confirms provider process is absent for identity-ambiguous hold.
    ProviderSessionMarkProcessAbsent,
    /// P089: read-only advisory temporary artifact inventory preview (disabled-mode readback).
    TempArtifactsInventoryPreview,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ResourceTemplateId {
    RunEntity,
    IdeaEntity,
    ArtifactEntity,
    ReportEntity,
    StewardAnalysisEntity,
    ChainworksRuns,
    ChainworksIdeas,
    ChainworksApprovalsInbox,
    ChainworksRunStages,
    ChainworksRunArtifacts,
    ChainworksRunTempArtifactInventory,
}
