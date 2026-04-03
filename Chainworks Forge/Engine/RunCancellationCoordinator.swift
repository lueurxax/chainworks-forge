import Foundation
import SwiftData

// MARK: - RunCancellationCoordinator (Proposal 011 — REQ-001, REQ-002)

/// Coordinates truthful cancellation settlement for a run.
///
/// Settlement contract (§4.2.1) — two-phase design:
///
/// **Phase 1 — `beginSettlement()` (synchronous, MainActor):**
/// 1. `cancellationRequestedAt` is set immediately when the operator presses stop.
/// 2. The orchestrator is signalled to stop advancing stages.
/// 3. Each active agent execution transitions to `.cancelled`.
/// 4. A preliminary settlement log is written (session outcomes pending).
/// 5. `pendingSessionIDs` is populated for async cleanup.
/// NOTE: `cancellationSettledAt` is NOT written yet. `presentationStatus` returns `.cancelling`.
///
/// **Phase 2 — `finalizeSettlement(sessionOutcomes:)` (synchronous, MainActor, called after async session close):**
/// 6. The settlement log is updated with actual session-close outcomes.
/// 7. `cancellationSettledAt` is written — settlement is now truthful.
/// 8. `run.status = .cancelled` and `run.completedAt` are set.
///
/// Session close happens between Phase 1 and Phase 2, with per-session timeouts.
/// This ensures the settlement log records *observed* session-close outcomes,
/// not optimistic placeholders.
@MainActor
final class RunCancellationCoordinator {

    private let run: Run
    private let orchestrator: WorkflowOrchestrator?

    /// Session IDs that need async cleanup (collected during Phase 1).
    private(set) var pendingSessionIDs: [String] = []

    /// Maps session ID → agent execution ID for settlement log updates in Phase 2.
    private var sessionToAgentExecutionID: [String: UUID] = [:]

    init(run: Run, orchestrator: WorkflowOrchestrator?) {
        self.run = run
        self.orchestrator = orchestrator
    }

    // MARK: - Phase 1: Begin Settlement (synchronous)

    /// Begin cancellation settlement. After this call:
    /// - `cancellationRequestedAt` is set.
    /// - All active agents are `.cancelled`.
    /// - A preliminary settlement log is written (with `sessionCloseSucceeded: nil`).
    /// - `pendingSessionIDs` is populated for async session cleanup.
    ///
    /// `cancellationSettledAt` is NOT set yet — call `finalizeSettlement(sessionOutcomes:)`
    /// after session cleanup to complete the settlement contract.
    func beginSettlement() {
        // Criterion 1: Record the cancellation request.
        run.cancellationRequestedAt = run.cancellationRequestedAt ?? Date()

        // Criterion 1 (cont.): Signal the orchestrator to stop advancing stages.
        orchestrator?.signalCancellation()

        // Criterion 2: Transition all active agents to .cancelled.
        let activeAgentExecutions = run.stageExecutions
            .flatMap(\.agentExecutions)
            .filter { [AgentStatus.running, .pending, .ready].contains($0.status) }

        var entries: [CancellationSettlementEntry] = []

        for agentExec in activeAgentExecutions {
            let priorStatus = agentExec.status.rawValue
            agentExec.status = .cancelled
            agentExec.completedAt = agentExec.completedAt ?? Date()

            // Collect session IDs for async cleanup.
            let hasSession = agentExec.gooseSessionID != nil && !(agentExec.gooseSessionID?.isEmpty ?? true)
            if let sessionID = agentExec.gooseSessionID, !sessionID.isEmpty {
                pendingSessionIDs.append(sessionID)
                sessionToAgentExecutionID[sessionID] = agentExec.id
            }

            // Mark parent stage if all agents are now terminal.
            if let stageExec = agentExec.stageExecution {
                let allTerminal = stageExec.agentExecutions.allSatisfy {
                    [AgentStatus.completed, .failed, .cancelled, .skipped].contains($0.status)
                }
                if allTerminal && stageExec.status != .completed && stageExec.status != .failed {
                    stageExec.status = .failed
                    stageExec.completedAt = stageExec.completedAt ?? Date()
                }
            }

            entries.append(CancellationSettlementEntry(
                agentExecutionID: agentExec.id,
                agentID: agentExec.agentID,
                priorStatus: priorStatus,
                terminalStatus: agentExec.status.rawValue,
                sessionCloseAttempted: hasSession,
                sessionCloseSucceeded: nil,  // Pending — updated in Phase 2.
                settledAt: Date()
            ))
        }

        // Write preliminary settlement log (session outcomes pending).
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        run.cancellationSettlementLog = try? encoder.encode(entries)

        // NOTE: Do NOT write cancellationSettledAt, .cancelled, or completedAt here.
        // presentationStatus will return .cancelling until finalizeSettlement() is called.
    }

    // MARK: - Phase 2: Finalize Settlement (synchronous, called after async session close)

    /// Complete the settlement contract by recording observed session-close outcomes
    /// and writing the terminal run state.
    ///
    /// - Parameter sessionOutcomes: Actual per-session close results from `closeGooseSessionsWithOutcomes()`.
    func finalizeSettlement(sessionOutcomes: [SessionCloseOutcome]) {
        // Build outcome lookup: sessionID → succeeded.
        let outcomeLookup = Dictionary(
            sessionOutcomes.map { ($0.sessionID, $0.succeeded) },
            uniquingKeysWith: { _, last in last }
        )

        // Decode preliminary entries.
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        var entries: [CancellationSettlementEntry] = []
        if let data = run.cancellationSettlementLog {
            entries = (try? decoder.decode([CancellationSettlementEntry].self, from: data)) ?? []
        }

        // Update entries with actual session-close outcomes.
        entries = entries.map { entry in
            guard entry.sessionCloseAttempted,
                  let sessionID = sessionToAgentExecutionID.first(where: { $0.value == entry.agentExecutionID })?.key,
                  let succeeded = outcomeLookup[sessionID] else {
                return entry
            }
            return CancellationSettlementEntry(
                agentExecutionID: entry.agentExecutionID,
                agentID: entry.agentID,
                priorStatus: entry.priorStatus,
                terminalStatus: entry.terminalStatus,
                sessionCloseAttempted: true,
                sessionCloseSucceeded: succeeded,
                settledAt: entry.settledAt
            )
        }

        // Write finalized settlement log with truthful session outcomes.
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        run.cancellationSettlementLog = try? encoder.encode(entries)

        // Settlement is now truthful — write terminal state.
        run.cancellationSettledAt = Date()
        run.status = .cancelled
        run.completedAt = Date()
    }

    // MARK: - Session Close with Outcomes

    /// Per-session close outcome for truthful settlement recording.
    struct SessionCloseOutcome: Sendable {
        let sessionID: String
        let attempted: Bool
        let succeeded: Bool
    }

    /// Close Goose sessions and return per-session outcomes. Does NOT reference any SwiftData models.
    /// Each session close is bounded by `perSessionTimeout` to prevent cancellation UX from hanging.
    nonisolated static func closeGooseSessionsWithOutcomes(
        sessionIDs: [String],
        executor: AgentExecutor,
        perSessionTimeout: Duration = .seconds(10)
    ) async -> [SessionCloseOutcome] {
        guard let gooseExecutor = executor as? GooseAgentExecutor else {
            // No Goose executor — nothing to close (simulated executor).
            return []
        }

        var outcomes: [SessionCloseOutcome] = []
        for sessionID in sessionIDs {
            let succeeded = await closeSessionWithTimeout(
                sessionID: sessionID,
                transport: gooseExecutor.sessionBridge.transport,
                timeout: perSessionTimeout
            )
            outcomes.append(SessionCloseOutcome(sessionID: sessionID, attempted: true, succeeded: succeeded))
        }
        return outcomes
    }

    /// Attempt to close a single session with a timeout guard.
    private nonisolated static func closeSessionWithTimeout(
        sessionID: String,
        transport: any GooseTransportProtocol,
        timeout: Duration
    ) async -> Bool {
        do {
            return try await withThrowingTaskGroup(of: Bool.self) { group in
                group.addTask {
                    try await transport.closeSession(sessionID: sessionID)
                    return true
                }
                group.addTask {
                    try await Task.sleep(for: timeout)
                    throw SessionCloseTimeoutError()
                }
                let result = try await group.next()!
                group.cancelAll()
                return result
            }
        } catch {
            print("[RunCancellationCoordinator] Session close failed for \(sessionID): \(error.localizedDescription)")
            return false
        }
    }

    private struct SessionCloseTimeoutError: Error {}
}
