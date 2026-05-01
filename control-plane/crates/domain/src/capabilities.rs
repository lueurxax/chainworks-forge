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
    /// P073: stability-budget://latest — the authoritative durable snapshot.
    StabilityBudgetLatest,
}
