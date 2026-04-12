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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartRunCmd {
    pub idea_id: IdeaId,
    pub workflow_id: String,
    pub workflow_title: String,
    pub workspace_root: String,
    pub artifact_root: String,
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
