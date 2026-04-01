import Foundation
import SwiftData

// MARK: - SessionResetAuditTrail (Proposal 018, Layer D)

/// Persists operator-triggered session reset history for later debugging.
///
/// This audit trail supplements the append-only `AgentSessionEvent` stream
/// with a structured, operator-visible audit record that can be surfaced
/// in recovery views, run reports, and export pipelines.
struct SessionResetAuditTrail {

    /// A single audit entry for an operator-triggered reset.
    struct ResetAuditEntry: Codable, Sendable {
        let timestamp: Date
        let runID: UUID
        let agentID: String
        let lineageID: String
        let generationNumber: Int
        let resetReason: String
        let priorTurnCount: Int
        let priorCumulativeTokens: Int64
        let priorCumulativeCostCents: Int64
        let checkpointEmitted: Bool
    }

    /// Build an audit entry from a lineage and the generation that was reset.
    static func buildEntry(
        runID: UUID,
        lineage: AgentSessionLineage,
        resetGeneration: AgentSessionGeneration,
        reason: String,
        checkpointEmitted: Bool
    ) -> ResetAuditEntry {
        ResetAuditEntry(
            timestamp: Date(),
            runID: runID,
            agentID: lineage.agentID,
            lineageID: lineage.lineageID,
            generationNumber: resetGeneration.generation,
            resetReason: reason,
            priorTurnCount: resetGeneration.turnCount,
            priorCumulativeTokens: resetGeneration.cumulativePromptTokens,
            priorCumulativeCostCents: resetGeneration.cumulativeCostCents,
            checkpointEmitted: checkpointEmitted
        )
    }

    /// Fetch all reset audit entries for a run from persisted session events.
    static func fetchResetHistory(for runID: UUID, context: ModelContext) -> [ResetAuditEntry] {
        let predicate = #Predicate<AgentSessionLineage> { $0.runID == runID }
        let descriptor = FetchDescriptor<AgentSessionLineage>(predicate: predicate)
        guard let lineages = try? context.fetch(descriptor) else { return [] }

        var entries: [ResetAuditEntry] = []
        for lineage in lineages {
            let resetEvents = lineage.events.filter { $0.eventType == .operator_reset }
            for event in resetEvents {
                // Find the generation that was active at the time of reset
                if let generation = lineage.generations.first(where: { $0.id == event.generationID }) {
                    let hadCheckpoint = lineage.events.contains {
                        $0.eventType == .checkpoint_created &&
                        $0.generationID == event.generationID &&
                        $0.recordedAt <= event.recordedAt
                    }
                    entries.append(buildEntry(
                        runID: runID,
                        lineage: lineage,
                        resetGeneration: generation,
                        reason: generation.endReason ?? "operator_reset",
                        checkpointEmitted: hadCheckpoint
                    ))
                }
            }
        }

        return entries.sorted(by: { $0.timestamp < $1.timestamp })
    }

    /// Serialize the full reset audit trail for a run as JSON.
    static func exportJSON(for runID: UUID, context: ModelContext) -> Data? {
        let entries = fetchResetHistory(for: runID, context: context)
        guard !entries.isEmpty else { return nil }
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        return try? encoder.encode(entries)
    }
}
