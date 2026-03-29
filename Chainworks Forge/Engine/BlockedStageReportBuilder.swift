import Foundation

// MARK: - Proposal 013 Layer O: Blocked Stage Report Builder

/// Produces one consistent failed-stage report packet for operator
/// and audit consumption. Both blocked-run UI and exported reports
/// read from this same packet.
struct BlockedStageReportBuilder {

    /// Build a blocked stage report from available evidence.
    static func buildReport(
        run: Run,
        stage: StageExecution,
        evidencePacket: FailedStageEvidencePacket?
    ) -> BlockedStageReport {
        let stageHistory = buildStageHistory(run: run, stageID: stage.stageID)
        let agentHistory = buildAgentHistory(stage: stage)

        return BlockedStageReport(
            id: UUID(),
            timestamp: Date(),
            runID: run.id,
            stageID: stage.stageID,
            stageLabel: stage.label,
            currentAttemptNumber: stage.attemptNumber,
            stageStatus: stage.status.rawValue,
            evidencePacket: evidencePacket,
            stageHistory: stageHistory,
            agentHistory: agentHistory,
            retryLineageSummary: buildRetryLineageSummary(stageHistory: stageHistory)
        )
    }

    // MARK: - Stage History

    private static func buildStageHistory(run: Run, stageID: String) -> [StageAttemptHistoryRecord] {
        run.stageExecutions
            .filter { $0.stageID == stageID }
            .sorted { $0.attemptNumber < $1.attemptNumber }
            .map { stage in
                StageAttemptHistoryRecord(
                    stageExecutionID: stage.id,
                    stageID: stage.stageID,
                    attemptNumber: stage.attemptNumber,
                    retryMode: RetryMode(rawValue: stage.retryMode ?? "fresh_execution") ?? .freshExecution,
                    triggerReason: stage.triggerReason ?? "initial_execution",
                    supersedesAttemptNumber: stage.supersedesAttemptNumber,
                    status: stage.status.rawValue,
                    startedAt: stage.startedAt,
                    completedAt: stage.completedAt,
                    agentCount: stage.agentExecutions.count,
                    failedAgentCount: stage.agentExecutions.filter { $0.status == .failed }.count
                )
            }
    }

    // MARK: - Agent History

    private static func buildAgentHistory(stage: StageExecution) -> [AgentAttemptHistoryRecord] {
        // Group by agentID, show attempt lineage
        let grouped = Dictionary(grouping: stage.agentExecutions) { $0.agentID }
        return grouped.flatMap { (agentID, executions) in
            executions
                .sorted { $0.startedAt < $1.startedAt }
                .enumerated()
                .map { (index, exec) in
                    AgentAttemptHistoryRecord(
                        stageExecutionID: stage.id,
                        agentID: agentID,
                        agentExecutionID: exec.id,
                        agentAttemptNumber: exec.agentAttemptNumber ?? (index + 1),
                        supersedesAgentExecutionID: exec.supersedesAgentExecutionID,
                        reusedSiblingExecutionIDs: decodeReusedSiblingIDs(exec.reusedSiblingExecutionIDsJSON),
                        retryReason: exec.retryReason,
                        status: exec.status.rawValue,
                        startedAt: exec.startedAt,
                        completedAt: exec.completedAt
                    )
                }
        }
    }

    private static func decodeReusedSiblingIDs(_ data: Data?) -> [UUID] {
        guard let data else { return [] }
        return (try? JSONDecoder().decode([UUID].self, from: data)) ?? []
    }

    // MARK: - Retry Lineage Summary

    private static func buildRetryLineageSummary(stageHistory: [StageAttemptHistoryRecord]) -> String {
        guard stageHistory.count > 1 else {
            return "No retries — first attempt"
        }

        let attempts = stageHistory.count
        let lastAttempt = stageHistory.last!
        let retryModes = stageHistory.dropFirst().compactMap { $0.retryMode.rawValue }
        let uniqueModes = Set(retryModes)

        var parts: [String] = ["\(attempts) attempts total"]
        if uniqueModes.contains(RetryMode.agentRetry.rawValue) {
            parts.append("includes agent-only retries")
        }
        if uniqueModes.contains(RetryMode.stageRetry.rawValue) {
            parts.append("includes stage retries")
        }
        parts.append("current attempt: \(lastAttempt.attemptNumber) (\(lastAttempt.status))")

        return parts.joined(separator: "; ")
    }
}

// MARK: - Blocked Stage Report

struct BlockedStageReport: Codable, Sendable, Identifiable {
    let id: UUID
    let timestamp: Date
    let runID: UUID
    let stageID: String
    let stageLabel: String
    let currentAttemptNumber: Int
    let stageStatus: String
    let evidencePacket: FailedStageEvidencePacket?
    let stageHistory: [StageAttemptHistoryRecord]
    let agentHistory: [AgentAttemptHistoryRecord]
    let retryLineageSummary: String
}

// MARK: - Stage Attempt History Record (§5.3)

struct StageAttemptHistoryRecord: Codable, Sendable {
    let stageExecutionID: UUID
    let stageID: String
    let attemptNumber: Int
    let retryMode: RetryMode
    let triggerReason: String
    let supersedesAttemptNumber: Int?
    let status: String
    let startedAt: Date
    let completedAt: Date?
    let agentCount: Int
    let failedAgentCount: Int
}

// MARK: - Agent Attempt History Record (§5.3)

struct AgentAttemptHistoryRecord: Codable, Sendable {
    let stageExecutionID: UUID
    let agentID: String
    let agentExecutionID: UUID
    let agentAttemptNumber: Int
    let supersedesAgentExecutionID: UUID?
    let reusedSiblingExecutionIDs: [UUID]
    let retryReason: String?
    let status: String
    let startedAt: Date
    let completedAt: Date?
}
