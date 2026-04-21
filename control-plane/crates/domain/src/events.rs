use serde::{Deserialize, Serialize};

use crate::approval::ApprovalDecision;
use crate::ids::{ApprovalId, ArtifactId, IdeaId, RunId, StageExecutionId};
use crate::run::RunStatus;
use crate::stage::StageStatus;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DomainEvent {
    RunStarted {
        run_id: RunId,
        idea_id: IdeaId,
    },
    RunStatusChanged {
        run_id: RunId,
        status: RunStatus,
    },
    StageStatusChanged {
        run_id: RunId,
        stage_execution_id: StageExecutionId,
        status: StageStatus,
    },
    ApprovalRequested {
        run_id: RunId,
        approval_id: ApprovalId,
        stage_id: String,
    },
    ApprovalResolved {
        approval_id: ApprovalId,
        decision: ApprovalDecision,
    },
    ArtifactCreated {
        run_id: RunId,
        artifact_id: ArtifactId,
    },
    /// Runtime/session lifecycle event: emitted when an ACP runtime session
    /// starts, completes, or fails. Used by the runtime health subscription.
    RuntimeStatusChanged {
        run_id: RunId,
        stage_id: String,
        agent_id: String,
        provider: String,
        /// "session_started" | "session_completed" | "session_failed"
        event_kind: String,
    },
    SchedulerBackpressureChanged {
        run_id: Option<String>,
        stage_execution_id: Option<String>,
        provider_family: Option<String>,
        top_reason: String,
        queued_count: i64,
        oldest_queued_age_ms: i64,
        global_queue_depth: i64,
        state: String,
        updated_at: String,
        is_stale: bool,
    },
}
