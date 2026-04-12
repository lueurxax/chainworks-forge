import Foundation
import SwiftData

// MARK: - SessionLineageReportBridge (Proposal 018, Layer D)

/// Makes run reports and blocked-run views show session reuse truth explicitly.
///
/// This bridge reads persisted session lineage, generation, and event data
/// to produce structured report entries for inclusion in run reports, receipts,
/// and operator export surfaces.
final class SessionLineageReportBridge {
    struct ReportEnvelope: Codable, Sendable {
        let reports: [AgentSessionReport]
        let errorMessage: String?
    }

    /// Report entry for a single agent's session lineage within a run.
    struct AgentSessionReport: Codable, Sendable {
        let agentID: String
        let lineageID: String
        let reuseScope: String
        let familyID: String?
        let totalGenerations: Int
        let totalEvents: Int
        let activeGenerationNumber: Int?
        let activeStatus: String?
        let totalTurns: Int
        let totalPromptTokens: Int64
        let totalCostCents: Int64
        let dispositionHistory: [String]
        let resetCount: Int
        let budgetExceededCount: Int
        let compactionCount: Int
    }

    static func generateEnvelope(
        for runID: UUID,
        fetchLineages: () throws -> [AgentSessionLineage]
    ) -> ReportEnvelope {
        do {
            let lineages = try fetchLineages()
            return ReportEnvelope(
                reports: buildReports(from: lineages),
                errorMessage: nil
            )
        } catch {
            let message = "Failed to fetch session lineage reports for run \(runID): \(error.localizedDescription)"
            ForgeLogger.recovery.error(message)
            return ReportEnvelope(reports: [], errorMessage: message)
        }
    }

    /// Generate session lineage reports for all agents in a run.
    static func generateReports(for runID: UUID, context: ModelContext) -> [AgentSessionReport] {
        generateEnvelope(
            for: runID,
            fetchLineages: {
                let predicate = #Predicate<AgentSessionLineage> { $0.runID == runID }
                let descriptor = FetchDescriptor<AgentSessionLineage>(predicate: predicate)
                return try context.fetch(descriptor)
            }
        ).reports
    }

    private static func buildReports(from lineages: [AgentSessionLineage]) -> [AgentSessionReport] {
        lineages.map { lineage in
            let sortedGenerations = lineage.generations.sorted(by: { $0.generation < $1.generation })
            let activeGen = lineage.activeGenerationID.flatMap { activeID in
                lineage.generations.first(where: { $0.id == activeID })
            }

            let totalTurns = sortedGenerations.reduce(0) { $0 + $1.turnCount }
            let totalTokens = sortedGenerations.reduce(Int64(0)) { $0 + $1.cumulativePromptTokens }
            let totalCost = sortedGenerations.reduce(Int64(0)) { $0 + $1.cumulativeCostCents }

            let dispositions = lineage.events
                .sorted(by: { $0.recordedAt < $1.recordedAt })
                .map(\.eventType.rawValue)

            let resetCount = lineage.events.filter { $0.eventType == .operator_reset }.count
            let budgetCount = lineage.events.filter { $0.eventType == .budget_exceeded }.count
            let compactCount = lineage.events.filter { $0.eventType == .compacted }.count

            return AgentSessionReport(
                agentID: lineage.agentID,
                lineageID: lineage.lineageID,
                reuseScope: lineage.sessionReuseScope.rawValue,
                familyID: lineage.sessionFamilyID,
                totalGenerations: sortedGenerations.count,
                totalEvents: lineage.events.count,
                activeGenerationNumber: activeGen?.generation,
                activeStatus: activeGen?.status.rawValue,
                totalTurns: totalTurns,
                totalPromptTokens: totalTokens,
                totalCostCents: totalCost,
                dispositionHistory: dispositions,
                resetCount: resetCount,
                budgetExceededCount: budgetCount,
                compactionCount: compactCount
            )
        }
    }

    /// Generate a JSON-serializable summary for inclusion in run reports.
    static func generateReportJSON(for runID: UUID, context: ModelContext) -> Data? {
        let envelope = generateEnvelope(
            for: runID,
            fetchLineages: {
                let predicate = #Predicate<AgentSessionLineage> { $0.runID == runID }
                let descriptor = FetchDescriptor<AgentSessionLineage>(predicate: predicate)
                return try context.fetch(descriptor)
            }
        )
        guard !envelope.reports.isEmpty || envelope.errorMessage != nil else { return nil }
        do {
            return try JSONEncoder().encode(envelope)
        } catch {
            ForgeLogger.recovery.error("Failed to encode session lineage report envelope for run \(runID): \(error.localizedDescription)")
            return nil
        }
    }
}
