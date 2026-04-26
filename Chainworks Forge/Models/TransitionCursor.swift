import Foundation
import SwiftData

// MARK: - Proposal 032: Atomic Transition Settlement and Durable Resume Cursor

/// The canonical durable continuation truth for a run's workflow progression.
/// Persisted as JSON on `Run.transitionCursorJSON`. All resume, recovery, and report
/// surfaces must read this cursor first before falling back to heuristic reconstruction.
///
/// Invariants:
/// - One cursor per run, owned by the engine (not by workflow agents or `run_state` artifacts).
/// - Each settlement creates a new cursor value (immutable struct, never mutated in place).
/// - `sequenceNumber` increments monotonically on every settlement.
struct TransitionCursor: Codable, Sendable, Equatable {
    /// Monotonically increasing counter. Increments on every settlement,
    /// enabling stale-read detection and ordering.
    let sequenceNumber: Int

    /// The state ID of the last stage that completed successfully. Nil before first stage completes.
    let lastCompletedStateID: String?

    /// The StageExecution.id of the last completed stage. Nil before first stage completes.
    let lastCompletedStageExecutionID: UUID?

    /// The state ID of the next scheduled continuation point. Nil when terminal or idle.
    let nextScheduledStateID: String?

    /// The iteration number for the next scheduled state (loop support).
    let nextScheduledIteration: Int?

    /// The attempt number for the next scheduled state.
    let nextScheduledAttemptNumber: Int?

    /// The StageExecution.id for the scheduled downstream stage, if pre-materialized.
    let scheduledStageExecutionID: UUID?

    /// The settlement phase at the transition boundary.
    let settlementPhase: TransitionSettlementPhase

    /// When this cursor was persisted.
    let updatedAt: Date

    /// Terminal workflow-conflict reason surfaced when no same-run continuation is safe.
    var terminalFailureReason: String? = nil
}

/// Describes the phase of the transition boundary.
///
/// The cursor owns only transition-boundary truth. Stage-level running/completed
/// status is tracked by `StageExecution.status` and is not duplicated here.
///
/// Progression: `awaitingFirstState` → `transitionSettled` → `transitionStarted` → `terminal`
/// Blocking workflow conflicts pause at `awaitingConflictResolution` until operator or
/// lead mediation resolves the conflict.
enum TransitionSettlementPhase: String, Codable, Sendable, Equatable {
    /// Run has been created but no state has begun execution yet.
    case awaitingFirstState = "awaiting_first_state"

    /// A transition has been durably settled: the completed state and the next
    /// scheduled state are both recorded. The next state has NOT started execution.
    /// Startup normalization must preserve this as resumable, not rewrite to blocked.
    case transitionSettled = "transition_settled"

    /// The scheduled next state has begun execution (agent work has started).
    case transitionStarted = "transition_started"

    /// A workflow conflict blocked legal graph advancement at the current state.
    case awaitingConflictResolution = "await_conflict_resolution"

    /// The run has reached a terminal state (completed, failed, or cancelled).
    case terminal = "terminal"
}

// MARK: - Cursor Construction Helpers

extension TransitionCursor {
    /// Initial cursor for a brand-new run before any state executes.
    static func initial() -> TransitionCursor {
        TransitionCursor(
            sequenceNumber: 0,
            lastCompletedStateID: nil,
            lastCompletedStageExecutionID: nil,
            nextScheduledStateID: nil,
            nextScheduledIteration: nil,
            nextScheduledAttemptNumber: nil,
            scheduledStageExecutionID: nil,
            settlementPhase: .awaitingFirstState,
            updatedAt: Date()
        )
    }

    /// Seed a cursor for a pre-P032 legacy run being resumed from a heuristic continuation.
    /// Records the resume target as the next scheduled state so we don't lose the
    /// heuristic computation by writing `.initial()`.
    static func seededForResume(
        nextScheduledStateID: String,
        nextScheduledIteration: Int? = nil,
        nextScheduledAttemptNumber: Int? = nil,
        scheduledStageExecutionID: UUID? = nil
    ) -> TransitionCursor {
        TransitionCursor(
            sequenceNumber: 0,
            lastCompletedStateID: nil,
            lastCompletedStageExecutionID: nil,
            nextScheduledStateID: nextScheduledStateID,
            nextScheduledIteration: nextScheduledIteration,
            nextScheduledAttemptNumber: nextScheduledAttemptNumber,
            scheduledStageExecutionID: scheduledStageExecutionID,
            settlementPhase: .transitionSettled,
            updatedAt: Date()
        )
    }

    /// Cursor after atomically settling a transition from completedState → nextState.
    func settlingTransition(
        completedStateID: String,
        completedStageExecutionID: UUID?,
        nextStateID: String,
        nextIteration: Int = 1,
        nextAttemptNumber: Int = 1,
        scheduledStageExecutionID: UUID? = nil
    ) -> TransitionCursor {
        TransitionCursor(
            sequenceNumber: sequenceNumber + 1,
            lastCompletedStateID: completedStateID,
            lastCompletedStageExecutionID: completedStageExecutionID,
            nextScheduledStateID: nextStateID,
            nextScheduledIteration: nextIteration,
            nextScheduledAttemptNumber: nextAttemptNumber,
            scheduledStageExecutionID: scheduledStageExecutionID,
            settlementPhase: .transitionSettled,
            updatedAt: Date()
        )
    }

    /// Cursor marking that the scheduled next state has actually begun execution.
    func markingTransitionStarted() -> TransitionCursor {
        TransitionCursor(
            sequenceNumber: sequenceNumber + 1,
            lastCompletedStateID: lastCompletedStateID,
            lastCompletedStageExecutionID: lastCompletedStageExecutionID,
            nextScheduledStateID: nextScheduledStateID,
            nextScheduledIteration: nextScheduledIteration,
            nextScheduledAttemptNumber: nextScheduledAttemptNumber,
            scheduledStageExecutionID: scheduledStageExecutionID,
            settlementPhase: .transitionStarted,
            updatedAt: Date()
        )
    }

    /// Cursor marking that transition authority blocked at a workflow conflict.
    func markingWorkflowConflictBlocked(
        currentStateID: String,
        currentStageExecutionID: UUID?
    ) -> TransitionCursor {
        TransitionCursor(
            sequenceNumber: sequenceNumber + 1,
            lastCompletedStateID: currentStateID,
            lastCompletedStageExecutionID: currentStageExecutionID,
            nextScheduledStateID: nil,
            nextScheduledIteration: nil,
            nextScheduledAttemptNumber: nil,
            scheduledStageExecutionID: nil,
            settlementPhase: .awaitingConflictResolution,
            updatedAt: Date()
        )
    }

    /// Cursor marking the run as terminal (completed, failed, or cancelled).
    func markingTerminal(
        lastCompletedStateID: String? = nil,
        lastCompletedStageExecutionID: UUID? = nil,
        terminalFailureReason: String? = nil
    ) -> TransitionCursor {
        TransitionCursor(
            sequenceNumber: sequenceNumber + 1,
            lastCompletedStateID: lastCompletedStateID ?? self.lastCompletedStateID,
            lastCompletedStageExecutionID: lastCompletedStageExecutionID ?? self.lastCompletedStageExecutionID,
            nextScheduledStateID: nil,
            nextScheduledIteration: nil,
            nextScheduledAttemptNumber: nil,
            scheduledStageExecutionID: nil,
            settlementPhase: .terminal,
            updatedAt: Date(),
            terminalFailureReason: terminalFailureReason ?? self.terminalFailureReason
        )
    }
}

// MARK: - Run Cursor Accessor

@MainActor
extension Run {
    /// Decode the durable transition cursor from persisted JSON.
    /// Returns nil for pre-P032 runs that have no cursor yet.
    var transitionCursor: TransitionCursor? {
        guard let data = transitionCursorJSON else { return nil }
        return try? JSONDecoder().decode(TransitionCursor.self, from: data)
    }

    /// Persist a new transition cursor value. Each call replaces the prior cursor atomically.
    func persistTransitionCursor(_ cursor: TransitionCursor) {
        transitionCursorJSON = try? JSONEncoder().encode(cursor)
    }
}
