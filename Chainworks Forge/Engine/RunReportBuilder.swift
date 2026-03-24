import Foundation
import SwiftData

// MARK: - P005-OPS §6: Run Report Builder

/// Deterministic immutable reports plus latest summary view.
/// Every stable checkpoint emits immutable `run_report_v{n}` artifacts.
/// Latest summary artifacts exist separately from immutable history.
@MainActor
final class RunReportBuilder {

    private let modelContext: ModelContext

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    // MARK: - Report Generation (§6.3)

    /// Emit a new immutable report version when the run reaches a stable checkpoint.
    /// A new version is created on: terminal state, explicit recovery action, or approval re-arm.
    func emitReport(for run: Run) throws -> (markdownArtifact: Artifact, jsonArtifact: Artifact) {
        let nextVersion = run.latestReportVersion + 1
        let report = buildReportPayload(for: run, version: nextVersion)

        // Immutable markdown report
        let mdContent = renderMarkdown(from: report)
        let mdArtifact = Artifact(
            name: "run_report_v\(nextVersion).md",
            contractID: "run_report",
            format: .markdown,
            filePath: reportFilePath(run: run, name: "run_report_v\(nextVersion).md"),
            runID: run.id,
            stageID: run.currentStageID ?? "unknown",
            agentID: "system",
            provider: "system"
        )
        mdArtifact.reportKind = "immutable_history"
        mdArtifact.reportVersion = nextVersion
        if let previousID = run.latestImmutableReportArtifactID {
            mdArtifact.supersedesArtifactID = previousID
        }

        // Immutable JSON report
        let jsonContent = renderJSON(from: report)
        let jsonArtifact = Artifact(
            name: "run_report_v\(nextVersion).json",
            contractID: "run_report",
            format: .json,
            filePath: reportFilePath(run: run, name: "run_report_v\(nextVersion).json"),
            runID: run.id,
            stageID: run.currentStageID ?? "unknown",
            agentID: "system",
            provider: "system"
        )
        jsonArtifact.reportKind = "immutable_history"
        jsonArtifact.reportVersion = nextVersion

        // Write to disk
        try writeReportToDisk(content: mdContent, path: mdArtifact.filePath)
        try writeReportToDisk(content: jsonContent, path: jsonArtifact.filePath)

        // Persist metadata
        modelContext.insert(mdArtifact)
        modelContext.insert(jsonArtifact)

        // Update run pointers
        run.latestReportVersion = nextVersion
        run.latestImmutableReportArtifactID = mdArtifact.id

        // Emit latest summary
        try emitLatestSummary(for: run, basedOn: report)

        try modelContext.save()

        return (mdArtifact, jsonArtifact)
    }

    /// Emit or overwrite the mutable latest summary artifacts.
    func emitLatestSummary(for run: Run, basedOn report: RunReportPayload) throws {
        let mdContent = renderLatestSummaryMarkdown(from: report)
        let jsonContent = renderLatestSummaryJSON(from: report)

        let mdPath = reportFilePath(run: run, name: "run_summary_latest.md")
        let jsonPath = reportFilePath(run: run, name: "run_summary_latest.json")

        try writeReportToDisk(content: mdContent, path: mdPath)
        try writeReportToDisk(content: jsonContent, path: jsonPath)

        // If latest summary artifacts already exist, update them; otherwise create new
        if let existingID = run.latestSummaryArtifactID {
            let descriptor = FetchDescriptor<Artifact>(predicate: #Predicate { $0.id == existingID })
            if let existing = try? modelContext.fetch(descriptor).first {
                existing.reportKind = "latest_summary"
                existing.reportVersion = report.version
            }
        } else {
            let summaryArtifact = Artifact(
                name: "run_summary_latest.md",
                contractID: "run_summary",
                format: .markdown,
                filePath: mdPath,
                runID: run.id,
                stageID: run.currentStageID ?? "unknown",
                agentID: "system",
                provider: "system"
            )
            summaryArtifact.reportKind = "latest_summary"
            summaryArtifact.reportVersion = report.version
            modelContext.insert(summaryArtifact)
            run.latestSummaryArtifactID = summaryArtifact.id
        }
    }

    // MARK: - Should Emit Check

    /// Determines if a report should be emitted based on current run state (§6.3).
    func shouldEmitReport(for run: Run) -> Bool {
        switch run.status {
        case .completed, .failed, .cancelled, .blocked:
            return true
        default:
            return false
        }
    }

    // MARK: - Report Payload Construction (§6.4)

    func buildReportPayload(for run: Run, version: Int) -> RunReportPayload {
        let stages = run.stageExecutions.sorted { $0.startedAt < $1.startedAt }
        let allAgents = stages.flatMap { $0.agentExecutions }
        let approvals = run.approvals.sorted { $0.requestedAt < $1.requestedAt }

        // Stage timeline
        let stageTimeline: [RunReportPayload.StageEntry] = stages.map { stage in
            RunReportPayload.StageEntry(
                label: stage.label,
                status: stage.status.rawValue,
                iteration: stage.iteration,
                attempt: stage.attemptNumber,
                duration: stageDuration(stage)
            )
        }

        // Agents used
        let agentsUsed: [RunReportPayload.AgentEntry] = allAgents.map { agent in
            RunReportPayload.AgentEntry(
                agentID: agent.agentID,
                provider: agent.provider,
                model: agent.resolvedModel ?? agent.resolvedBackendProfileID,
                effort: agent.effort,
                costCents: agent.costCents,
                duration: agentDuration(agent),
                finalStatus: agent.status.rawValue
            )
        }

        // Approval entries
        let approvalEntries: [RunReportPayload.ApprovalEntry] = approvals.map { approval in
            RunReportPayload.ApprovalEntry(
                gateLabel: approval.stageID,
                decision: approval.decision.rawValue,
                comment: approval.comment,
                requestedAt: approval.requestedAt,
                decidedAt: approval.decidedAt
            )
        }

        // Execution summary
        let completedStages = stages.filter { $0.status == .completed }.count
        let skippedStages = stages.filter { $0.status == .skipped }.count
        let failedStages = stages.filter { $0.status == .failed }.count
        let loopsEntered = run.loopCounters.values.reduce(0, +)
        let approvalsRequested = approvals.count
        let approvalsGranted = approvals.filter { $0.decision == .granted }.count
        let approvalsRejected = approvals.filter { $0.decision == .rejected }.count

        // Key artifacts (pinned first, then report-worthy)
        let allArtifacts = allAgents.flatMap { $0.artifacts }
        let pinnedArtifacts = allArtifacts.filter { $0.isPinned }
        let reportArtifacts = allArtifacts.filter { !$0.isPinned && $0.reportKind == nil }

        let keyArtifactEntries: [RunReportPayload.ArtifactEntry] = (pinnedArtifacts + reportArtifacts).map { artifact in
            RunReportPayload.ArtifactEntry(
                name: artifact.name,
                format: artifact.format.rawValue,
                isPinned: artifact.isPinned,
                agentID: artifact.agentID,
                stageID: artifact.stageID
            )
        }

        // §6.5: Retry/recovery narrative
        let retriesPerformed = stages.reduce(0) { $0 + max(0, $1.attemptNumber - 1) }
        let recoveryActionsTaken: [String] = allAgents
            .compactMap { $0.retryReason }

        // §6.5: Compute retry path and resume path from current run state
        let retryPath: String? = {
            if run.status == .failed || run.status == .blocked {
                if let failedStage = stages.last(where: { $0.status == .failed }) {
                    if let failedAgent = failedStage.agentExecutions.first(where: { $0.status == .failed }) {
                        return "Retry agent '\(failedAgent.agentID)' in stage '\(failedStage.label)'"
                    }
                    return "Retry stage '\(failedStage.label)'"
                }
                if let blockedStage = stages.last(where: { $0.status == .blocked }) {
                    return "Retry stage '\(blockedStage.label)'"
                }
            }
            return nil
        }()

        let resumePath: String? = {
            if run.status == .waitingApproval {
                if let gateStage = stages.last(where: { $0.status == .waitingApproval }) {
                    return "Resume from approval gate '\(gateStage.label)'"
                }
            }
            if run.status == .failed || run.status == .blocked {
                return "Clone run (frozen snapshot or current config)"
            }
            return nil
        }()

        return RunReportPayload(
            ideaTitle: run.idea?.title ?? "Unknown",
            workflowTitle: run.workflowTitle,
            runID: run.id,
            runStatus: run.status.rawValue,
            version: version,
            startedAt: run.startedAt,
            completedAt: run.completedAt,
            elapsedSeconds: elapsedTime(for: run),
            totalCostCents: run.totalCostCents,
            workflowSnapshotHash: run.workflowSnapshotHash,
            catalogSnapshotHash: run.catalogSnapshotHash,
            runtimeTrustLevel: run.runtimeTrustLevel ?? "unknown",
            driftNote: run.driftDetails,
            completedStages: completedStages,
            skippedStages: skippedStages,
            failedStages: failedStages,
            loopsEntered: loopsEntered,
            approvalsRequested: approvalsRequested,
            approvalsGranted: approvalsGranted,
            approvalsRejected: approvalsRejected,
            stageTimeline: stageTimeline,
            agentsUsed: agentsUsed,
            approvalEntries: approvalEntries,
            keyArtifacts: keyArtifactEntries,
            blockedReason: run.driftDetails,
            retryPath: retryPath,
            resumePath: resumePath,
            driftDecision: run.driftDecision?.rawValue,
            retriesPerformed: retriesPerformed,
            recoveryActionsTaken: recoveryActionsTaken
        )
    }

    // MARK: - Rendering

    private func renderMarkdown(from payload: RunReportPayload) -> String {
        var lines: [String] = []
        lines.append("# Run Report v\(payload.version)")
        lines.append("")
        lines.append("## 1. Header")
        lines.append("- **Idea:** \(payload.ideaTitle)")
        lines.append("- **Workflow:** \(payload.workflowTitle)")
        lines.append("- **Run ID:** \(payload.runID)")
        lines.append("- **Status:** \(payload.runStatus)")
        lines.append("- **Report Version:** \(payload.version)")
        lines.append("- **Started:** \(payload.startedAt)")
        if let completed = payload.completedAt {
            lines.append("- **Completed:** \(completed)")
        }
        lines.append("- **Elapsed:** \(formattedDuration(payload.elapsedSeconds))")
        if let cost = payload.totalCostCents {
            lines.append("- **Total Cost:** \(cost) cents")
        }
        lines.append("")
        lines.append("## 2. Snapshot and Runtime Provenance")
        lines.append("- **Workflow Hash:** \(payload.workflowSnapshotHash)")
        lines.append("- **Catalog Hash:** \(payload.catalogSnapshotHash)")
        lines.append("- **Runtime Trust:** \(payload.runtimeTrustLevel)")
        if let drift = payload.driftNote {
            lines.append("- **Drift Note:** \(drift)")
        }
        lines.append("")
        lines.append("## 3. Execution Summary")
        lines.append("- Stages completed: \(payload.completedStages)")
        lines.append("- Stages skipped: \(payload.skippedStages)")
        lines.append("- Stages failed: \(payload.failedStages)")
        lines.append("- Loops entered: \(payload.loopsEntered)")
        lines.append("- Approvals requested: \(payload.approvalsRequested)")
        lines.append("- Approvals granted: \(payload.approvalsGranted)")
        lines.append("- Approvals rejected: \(payload.approvalsRejected)")
        lines.append("")
        lines.append("## 4. Stage Timeline")
        for stage in payload.stageTimeline {
            lines.append("- **\(stage.label)** — \(stage.status) (iter \(stage.iteration), attempt \(stage.attempt), \(formattedDuration(stage.duration)))")
        }
        lines.append("")
        lines.append("## 5. Agents Used")
        for agent in payload.agentsUsed {
            var line = "- **\(agent.agentID)** — \(agent.provider)"
            if let model = agent.model { line += " / \(model)" }
            line += " / \(agent.effort)"
            if let cost = agent.costCents { line += " (\(cost)c)" }
            line += " — \(agent.finalStatus)"
            lines.append(line)
        }
        lines.append("")
        lines.append("## 6. Approvals")
        for approval in payload.approvalEntries {
            var line = "- **\(approval.gateLabel)** — \(approval.decision)"
            if let comment = approval.comment { line += " (\(comment))" }
            lines.append(line)
        }
        lines.append("")
        lines.append("## 7. Key Artifacts")
        for artifact in payload.keyArtifacts {
            let pinLabel = artifact.isPinned ? " [PINNED]" : ""
            lines.append("- \(artifact.name)\(pinLabel) (\(artifact.format)) — \(artifact.agentID) / \(artifact.stageID)")
        }
        lines.append("")
        lines.append("## 8. Recovery Notes")
        if let reason = payload.blockedReason { lines.append("- Blocked reason: \(reason)") }
        if let retry = payload.retryPath { lines.append("- Retry path: \(retry)") }
        if let resume = payload.resumePath { lines.append("- Resume path: \(resume)") }
        if let drift = payload.driftDecision { lines.append("- Drift decision: \(drift)") }
        lines.append("- Retries performed: \(payload.retriesPerformed)")
        if !payload.recoveryActionsTaken.isEmpty {
            lines.append("- Recovery actions taken:")
            for action in payload.recoveryActionsTaken {
                lines.append("  - \(action)")
            }
        }
        lines.append("")
        lines.append("## 9. Outcome")
        lines.append("- \(payload.runStatus)")
        lines.append("")
        return lines.joined(separator: "\n")
    }

    private func renderJSON(from payload: RunReportPayload) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        guard let data = try? encoder.encode(payload) else { return "{}" }
        return String(data: data, encoding: .utf8) ?? "{}"
    }

    private func renderLatestSummaryMarkdown(from payload: RunReportPayload) -> String {
        var lines: [String] = []
        lines.append("# Latest Summary — \(payload.ideaTitle)")
        lines.append("")
        lines.append("**Status:** \(payload.runStatus) | **Trust:** \(payload.runtimeTrustLevel) | **Report v\(payload.version)**")
        lines.append("")
        lines.append("Stages: \(payload.completedStages) completed, \(payload.failedStages) failed, \(payload.skippedStages) skipped")
        lines.append("Elapsed: \(formattedDuration(payload.elapsedSeconds))")
        if let cost = payload.totalCostCents { lines.append("Cost: \(cost) cents") }
        if let drift = payload.driftNote { lines.append("Drift: \(drift)") }
        lines.append("")
        return lines.joined(separator: "\n")
    }

    private func renderLatestSummaryJSON(from payload: RunReportPayload) -> String {
        // Emit a minimal summary subset
        let summary: [String: Any] = [
            "runID": payload.runID.uuidString,
            "status": payload.runStatus,
            "version": payload.version,
            "trust": payload.runtimeTrustLevel,
            "completedStages": payload.completedStages,
            "failedStages": payload.failedStages,
            "elapsedSeconds": payload.elapsedSeconds
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: summary, options: [.prettyPrinted, .sortedKeys]) else {
            return "{}"
        }
        return String(data: data, encoding: .utf8) ?? "{}"
    }

    // MARK: - Helpers

    private func elapsedTime(for run: Run) -> Double {
        let end = run.completedAt ?? Date()
        return end.timeIntervalSince(run.startedAt)
    }

    private func stageDuration(_ stage: StageExecution) -> Double {
        let end = stage.completedAt ?? Date()
        return end.timeIntervalSince(stage.startedAt)
    }

    private func agentDuration(_ agent: AgentExecution) -> Double {
        let end = agent.completedAt ?? Date()
        return end.timeIntervalSince(agent.startedAt)
    }

    private func formattedDuration(_ seconds: Double) -> String {
        let mins = Int(seconds) / 60
        let secs = Int(seconds) % 60
        if mins > 0 { return "\(mins)m \(secs)s" }
        return "\(secs)s"
    }

    private func reportFilePath(run: Run, name: String) -> String {
        let base = URL(fileURLWithPath: run.artifactRoot)
            .appendingPathComponent("reports", isDirectory: true)
        return base.appendingPathComponent(name).path
    }

    private func writeReportToDisk(content: String, path: String) throws {
        let url = URL(fileURLWithPath: path)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try content.write(to: url, atomically: true, encoding: .utf8)
    }
}

// MARK: - RunReportPayload (§6.4)

struct RunReportPayload: Codable, Sendable {
    let ideaTitle: String
    let workflowTitle: String
    let runID: UUID
    let runStatus: String
    let version: Int
    let startedAt: Date
    let completedAt: Date?
    let elapsedSeconds: Double
    let totalCostCents: Int64?

    // Snapshot and provenance
    let workflowSnapshotHash: String
    let catalogSnapshotHash: String
    let runtimeTrustLevel: String
    let driftNote: String?

    // Execution summary
    let completedStages: Int
    let skippedStages: Int
    let failedStages: Int
    let loopsEntered: Int
    let approvalsRequested: Int
    let approvalsGranted: Int
    let approvalsRejected: Int

    // Stage timeline
    let stageTimeline: [StageEntry]

    // Agents
    let agentsUsed: [AgentEntry]

    // Approvals
    let approvalEntries: [ApprovalEntry]

    // Key artifacts
    let keyArtifacts: [ArtifactEntry]

    // Recovery notes
    let blockedReason: String?
    let retryPath: String?
    let resumePath: String?
    let driftDecision: String?

    // §6.5: Retry/recovery narrative
    let retriesPerformed: Int
    let recoveryActionsTaken: [String]

    struct StageEntry: Codable, Sendable {
        let label: String
        let status: String
        let iteration: Int
        let attempt: Int
        let duration: Double
    }

    struct AgentEntry: Codable, Sendable {
        let agentID: String
        let provider: String
        let model: String?
        let effort: String
        let costCents: Int64?
        let duration: Double
        let finalStatus: String
    }

    struct ApprovalEntry: Codable, Sendable {
        let gateLabel: String
        let decision: String
        let comment: String?
        let requestedAt: Date
        let decidedAt: Date?
    }

    struct ArtifactEntry: Codable, Sendable {
        let name: String
        let format: String
        let isPinned: Bool
        let agentID: String
        let stageID: String
    }
}
