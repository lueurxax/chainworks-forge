use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CapabilityToolId {
    IdeasCreate,
    IdeasList,
    RunsStart,
    RunsList,
    RunsGet,
    RunsCancel,
    ApprovalsList,
    ApprovalsResolve,
    StagesRetry,
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
}
