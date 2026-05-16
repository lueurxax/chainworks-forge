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
    RunsCancel,
    ApprovalsList,
    ApprovalsResolve,
    StagesRetry,
    WorkflowConflictsResolve,
    LegacyDiscoveryOverrideCreate,
    ReportsGet,
    ArtifactsOverrideContract,
    StewardRunAnalysis,
    StewardListAnalyses,
    StewardGetAnalysis,
    StorageHealth,
    RuntimeHealth,
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
}
