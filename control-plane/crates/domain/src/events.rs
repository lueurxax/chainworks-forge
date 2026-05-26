use serde::{Deserialize, Serialize};

use crate::approval::ApprovalDecision;
use crate::ids::{
    ApprovalId, ArtifactId, IdeaId, RoutingReceiptId, RunId, StageExecutionId, SystemExecutionId,
};
use crate::lifecycle::DaemonStatus;
use crate::mediation::MediationConfirmationDecision;
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
    /// Live, non-durable transcript/timeline event for an active ACP prompt.
    /// This is intentionally transient: subscribers render it while the agent
    /// is active and discard it when that agent execution completes.
    RuntimeTimelineEvent {
        run_id: RunId,
        stage_id: String,
        agent_id: String,
        provider: String,
        event_kind: String,
        title: String,
        detail: Option<String>,
        surface_label: String,
        session_generation_id: Option<String>,
    },
    /// Daemon lifecycle state changed (Proposal 042 §5.1). Emitted by the
    /// lifecycle reporter on every transition so the `daemonStatusChanged`
    /// GraphQL subscription can push the updated snapshot.
    DaemonStatusChanged {
        status: DaemonStatus,
    },
    /// P017 Phase B: Lead mediation confirmation resolved.
    MediationConfirmationResolved {
        run_id: RunId,
        mediation_record_id: String,
        confirmation_subject_id: String,
        decision: MediationConfirmationDecision,
    },
    /// P060: System routing task completed (succeeded or failed).
    RoutingCompleted {
        run_id: RunId,
        stage_id: String,
        system_execution_id: SystemExecutionId,
        receipt_id: RoutingReceiptId,
        status: String,
        plan_hash: Option<String>,
    },
    /// Durable scheduler backpressure notification changed (P061).
    /// Payload mirrors the operator readback shape so GraphQL and MCP can
    /// push state without re-deriving freshness from event-only data.
    SchedulerBackpressureChanged {
        run_id: Option<String>,
        stage_execution_id: Option<String>,
        provider_family: Option<String>,
        top_reason: String,
        queued_count: i64,
        oldest_queued_age_ms: i64,
        global_queue_depth: i64,
        state: String,
        updated_at: chrono::DateTime<chrono::Utc>,
        stale_after_ms: i64,
    },
    /// P087: CAS repair failed for a maintenance slot.
    MaintenanceSlotReleaseCasFailed {
        operation_id: String,
        slot_generation: i64,
        error: String,
    },
    /// P046: A session_events row was persisted. Wakes sessionStatusChanged subscribers
    /// so live status changes are delivered without waiting for unrelated runtime events.
    SessionEventRecorded {
        run_id: RunId,
    },
}
