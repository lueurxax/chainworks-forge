use serde::{Deserialize, Serialize};

use crate::approval::ApprovalDecision;
use crate::ids::{ApprovalId, ArtifactId, IdeaId, RunId, StageExecutionId};
use crate::lifecycle::DaemonStatus;
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
    /// Daemon lifecycle state changed (Proposal 042 §5.1). Emitted by the
    /// lifecycle reporter on every transition so the `daemonStatusChanged`
    /// GraphQL subscription can push the updated snapshot.
    DaemonStatusChanged {
        status: DaemonStatus,
    },
}
