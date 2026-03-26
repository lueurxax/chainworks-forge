import Foundation
import SwiftData

// MARK: - RunCancellationCoordinator (Proposal 011 — REQ-001, REQ-002)

/// Coordinates truthful cancellation settlement for a run.
///
/// Settlement contract:
/// 1. `cancellationRequestedAt` is set immediately when the operator presses stop.
/// 2. The orchestrator is signalled to stop advancing stages.
/// 3. Each active agent execution is transitioned to `.cancelled` and its Goose session
///    is closed where possible.
/// 4. Per-agent settlement entries are collected.
/// 5. Only after all agents are settled does `cancellationSettledAt` get written,
///    along with the `cancellationSettlementLog`.
/// 6. The run's `status` transitions to `.cancelled` only after settlement.
@MainActor
final class RunCancellationCoordinator {

    let run: Run
    let orchestrator: WorkflowOrchestrator
    let modelContext: ModelContext

    init(run: Run, orchestrator: WorkflowOrchestrator, modelContext: ModelContext) {
        self.run = run
        self.orchestrator = orchestrator
        self.modelContext = modelContext
    }

    // MARK: - Settlement

    /// Begin the cancellation settlement process.
    /// Returns after all agents are settled and the run is marked `.cancelled`.
    func settle() async {
        // Step 1: Record the cancellation request timestamp.
        run.cancellationRequestedAt = Date()

        // Step 2: Signal the orchestrator to stop advancing stages.
        // This sets isCancelled = true and isRunning = false without touching run.status.
        orchestrator.signalCancellation()

        // Step 3: Collect all active agent executions across all stages.
        let activeAgentExecutions = run.stageExecutions
            .flatMap(\.agentExecutions)
            .filter { [AgentStatus.running, .pending, .ready].contains($0.status) }

        // Step 4: Settle each agent execution, collecting settlement entries.
        var entries: [CancellationSettlementEntry] = []

        for agentExec in activeAgentExecutions {
            let entry = await settleAgentExecution(agentExec)
            entries.append(entry)
        }

        // Step 5: Also record already-terminal agents that were in flight during the run.
        // (They don't need settlement, but we record them for completeness.)

        // Step 6: Write the settlement log.
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        run.cancellationSettlementLog = try? encoder.encode(entries)

        // Step 7: Mark settlement complete and transition to terminal .cancelled.
        run.cancellationSettledAt = Date()
        run.status = .cancelled
        run.completedAt = Date()
    }

    // MARK: - Per-Agent Settlement

    private func settleAgentExecution(_ agentExec: AgentExecution) async -> CancellationSettlementEntry {
        let priorStatus = agentExec.status.rawValue
        let sessionID = agentExec.gooseSessionID

        // Transition the agent execution to cancelled.
        agentExec.status = .cancelled
        agentExec.completedAt = agentExec.completedAt ?? Date()

        // Attempt to close the Goose session if one was open.
        var sessionCloseAttempted = false
        var sessionCloseSucceeded: Bool? = nil

        if let sessionID, !sessionID.isEmpty {
            sessionCloseAttempted = true
            sessionCloseSucceeded = await closeGooseSession(sessionID: sessionID)
        }

        // Also mark the parent stage execution if all its agents are now terminal.
        if let stageExec = agentExec.stageExecution {
            let allTerminal = stageExec.agentExecutions.allSatisfy {
                [AgentStatus.completed, .failed, .cancelled, .skipped].contains($0.status)
            }
            if allTerminal && stageExec.status != .completed && stageExec.status != .failed {
                stageExec.status = .failed
                stageExec.completedAt = stageExec.completedAt ?? Date()
            }
        }

        return CancellationSettlementEntry(
            agentExecutionID: agentExec.id,
            agentID: agentExec.agentID,
            priorStatus: priorStatus,
            terminalStatus: agentExec.status.rawValue,
            sessionCloseAttempted: sessionCloseAttempted,
            sessionCloseSucceeded: sessionCloseSucceeded,
            settledAt: Date()
        )
    }

    /// Attempt to close a Goose session, returning whether it succeeded.
    private func closeGooseSession(sessionID: String) async -> Bool {
        // Access the transport through the orchestrator's executor if it's a GooseAgentExecutor.
        guard let gooseExecutor = orchestrator.executor as? GooseAgentExecutor else {
            return false
        }

        do {
            try await gooseExecutor.sessionBridge.transport.closeSession(sessionID: sessionID)
            return true
        } catch {
            print("[RunCancellationCoordinator] Failed to close session \(sessionID): \(error.localizedDescription)")
            return false
        }
    }
}
