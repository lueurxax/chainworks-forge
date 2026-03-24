import Foundation
import SwiftData

@Model final class Run {
    @Attribute(.unique) var id: UUID
    var startedAt: Date
    var completedAt: Date?
    var status: RunStatus
    var loopCounters: [String: Int]
    var totalCostCents: Int64?

    // RunPlanSnapshot (immutable after creation — private(set) enforces contract)
    private(set) var workflowID: String
    private(set) var workflowTitle: String
    private(set) var workflowSnapshotHash: String
    private(set) var catalogSnapshotHash: String
    private(set) var workflowSourcePath: String
    private(set) var catalogSourcePath: String
    private(set) var workflowSnapshotJSON: Data
    private(set) var catalogSnapshotJSON: Data

    // Workspace paths (Proposal 002 — ARCH-025, ARCH-026)
    private(set) var workspaceRoot: String
    private(set) var artifactRoot: String
    private(set) var planCompilerVersion: Int

    // Drift detection
    var driftDetectedAt: Date?
    var driftDetails: String?
    var driftDecision: DriftDecision?

    // Steward cohorting metadata (Proposal 003 — optional, lightweight migration)
    var workflowFamily: String?
    var projectKey: String?
    var riskClass: RiskClass?
    var stack: String?
    var experimentCohortID: UUID?

    // P005-OPS §6.5: Report and runtime trust additions
    var latestSummaryArtifactID: UUID?
    var latestImmutableReportArtifactID: UUID?
    var latestReportVersion: Int = 0
    var runtimeTrustLevel: String?    // "fixture_verified" | "server_unverified" | "server_verified"

    @Relationship(inverse: \Idea.runs)
    var idea: Idea?

    @Relationship(deleteRule: .cascade)
    var stageExecutions: [StageExecution] = []

    @Relationship(deleteRule: .cascade)
    var approvals: [Approval] = []

    // Derived current stage (ARCH-PA-002)
    var currentStageID: String? {
        let sorted = stageExecutions.sorted { $0.startedAt < $1.startedAt }
        return sorted.last(where: { $0.status == .running })?.stageID
            ?? sorted.last(where: { $0.status == .waitingApproval })?.stageID
            ?? sorted.last(where: { $0.status == .blocked })?.stageID
            ?? sorted.last(where: { $0.status == .ready })?.stageID
            ?? sorted.last(where: { $0.status == .completed })?.stageID
    }

    init(
        id: UUID = UUID(),
        startedAt: Date = Date(),
        status: RunStatus = .pending,
        loopCounters: [String: Int] = [:],
        workflowID: String,
        workflowTitle: String,
        workflowSnapshotHash: String,
        catalogSnapshotHash: String,
        workflowSourcePath: String,
        catalogSourcePath: String,
        workflowSnapshotJSON: Data,
        catalogSnapshotJSON: Data,
        workspaceRoot: String = "",
        artifactRoot: String = "",
        planCompilerVersion: Int = 0
    ) {
        self.id = id
        self.startedAt = startedAt
        self.status = status
        self.loopCounters = loopCounters
        self.workflowID = workflowID
        self.workflowTitle = workflowTitle
        self.workflowSnapshotHash = workflowSnapshotHash
        self.catalogSnapshotHash = catalogSnapshotHash
        self.workflowSourcePath = workflowSourcePath
        self.catalogSourcePath = catalogSourcePath
        self.workflowSnapshotJSON = workflowSnapshotJSON
        self.catalogSnapshotJSON = catalogSnapshotJSON
        self.workspaceRoot = workspaceRoot
        self.artifactRoot = artifactRoot
        self.planCompilerVersion = planCompilerVersion
    }
}

enum RunStatus: String, Codable {
    case pending
    case ready
    case running
    case waitingApproval
    case blocked
    case completed
    case failed
    case cancelled
}

enum DriftDecision: String, Codable {
    case continueWithOriginal
    case restartWithCurrent
    case cancelled
}

enum RiskClass: String, Codable {
    case standard
    case elevated
    case critical
}
