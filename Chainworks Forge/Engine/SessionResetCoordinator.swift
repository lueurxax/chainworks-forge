import Foundation
import SwiftData

final class SessionResetCoordinator {
    let modelContext: ModelContext
    let sessionManager: AgentSessionManager

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
        self.sessionManager = AgentSessionManager(container: modelContext.container)
    }

    /// Reset an agent's reusable session lineage (§6.8, §6.9).
    ///
    /// Steps (per §6.9):
    /// 1. Emit a checkpoint artifact before reset (§6.4 rule 1: always emit before explicit `operator_reset`)
    /// 2. Record `checkpoint_created` event
    /// 3. Mark current generation as ended with `endReason = operator_reset`
    /// 4. Append `operator_reset` event
    /// 5. Set `activeGenerationID` to nil
    /// 6. Next invocation will see `.fresh_after_reset` via `SessionReusePolicy`
    ///
    /// Returns the reset reason string for persisting on `AgentExecution.sessionResetReason`.
    @discardableResult
    func resetAgentSession(runID: UUID, agentID: String, reason: String? = nil) async throws -> String {
        let resetReason = reason ?? "Operator-triggered session reset for agent '\(agentID)'"

        // Find the lineage
        let predicate = #Predicate<AgentSessionLineage> {
            $0.runID == runID && $0.agentID == agentID
        }
        let descriptor = FetchDescriptor<AgentSessionLineage>(predicate: predicate)
        guard let lineage = try modelContext.fetch(descriptor).first else {
            return resetReason // No lineage to reset
        }

        // §6.4 rule 1: Always emit checkpoint before explicit `operator_reset`
        if let activeID = lineage.activeGenerationID,
           let activeGen = lineage.generations.first(where: { $0.id == activeID }),
           activeGen.status == .active {
            // Build a minimal checkpoint for rehydration
            let checkpoint = AgentSessionCheckpointBuilder.buildForReset(
                generation: activeGen,
                lineage: lineage,
                resetReason: resetReason
            )
            // Persist checkpoint reference on the generation
            let checkpointData = try? JSONEncoder().encode(checkpoint)
            try await sessionManager.recordCheckpointCreated(
                lineageID: lineage.id,
                generationID: activeID,
                checkpointData: checkpointData
            )
        }

        // Perform the actual reset (marks generation, appends event, clears activeID)
        try await sessionManager.resetSession(lineageID: lineage.id, reason: resetReason)

        return resetReason
    }
}
