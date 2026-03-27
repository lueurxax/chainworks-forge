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
    var providerBindingSnapshotJSON: Data?
    var startOptionsJSON: Data?

    // Proposal 011 (REQ-009): Frozen binding provenance per agent, keyed by agent ID.
    var bindingProvenanceJSON: Data?

    // Proposal 011 (REQ-007): Frozen idea workspace root path at run creation time.
    // Set once during startRun, not mutated afterward.
    var frozenWorkspaceRootPath: String?

    // Proposal 011: Cancellation settlement (REQ-002 — truthful run control)
    var cancellationRequestedAt: Date?       // when operator pressed stop
    var cancellationSettledAt: Date?         // when coordinator confirmed propagation
    var cancellationSettlementLog: Data?     // JSON array of per-agent settlement entries

    // Proposal 007: Delivery configuration (frozen pre-run contract — ARCH-067 through ARCH-075)
    var deliveryConfigurationJSON: Data?
    var deliveryPreflightJSON: Data?
    var worktreeRoot: String?
    var repoIdentifier: String?
    var repoRoot: String?
    var baseBranch: String?
    var baseRevision: String?
    var targetBranch: String?
    var releaseTargetID: String?
    var releaseMode: String?

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
    case cancelling
}

// MARK: - Run Presentation Status (Proposal 011 — REQ-002)

extension Run {
    /// Truthful presentation status: shows `.cancelling` when a cancellation has been
    /// requested but not yet settled, and `.cancelled` only after full settlement.
    var presentationStatus: RunStatus {
        if cancellationRequestedAt != nil && cancellationSettledAt == nil {
            return .cancelling
        }
        return status
    }

    /// Human-readable label for the current presentation status.
    var presentationStatusLabel: String {
        presentationStatus.rawValue.replacingOccurrences(of: "_", with: " ")
    }
}

// MARK: - CancellationSettlementEntry (Proposal 011 — REQ-002)

/// Records per-agent settlement details during cancellation propagation.
struct CancellationSettlementEntry: Codable, Sendable {
    let agentExecutionID: UUID
    let agentID: String
    let priorStatus: String              // status at cancellation-request time
    let terminalStatus: String           // status after propagation
    let sessionCloseAttempted: Bool
    let sessionCloseSucceeded: Bool?     // nil if no session was open
    let settledAt: Date
}

// MARK: - FrozenBindingProvenance (Proposal 011 — REQ-009)

/// How the resolved model was determined for a single agent binding.
/// Frozen at run start, never reconstructed from mutable current settings.
enum BindingProvenanceSource: String, Codable, Sendable {
    case backendProfileDefault = "backend_profile"
    case configuredProviderDefault = "configured_provider"
    case runOverride = "run_override"
    case unverifiable = "unverifiable"
}

/// Per-agent frozen provenance snapshot persisted in `Run.bindingProvenanceJSON`.
struct FrozenBindingProvenance: Codable, Sendable {
    /// The source that determined the resolved model.
    let source: BindingProvenanceSource
    /// The backend profile ID from the agent catalog.
    let backendProfileID: String
    /// The backend profile's declared model (may be "default" or explicit).
    let backendProfileModel: String
    /// The configured provider ID selected at run start (if any).
    let configuredProviderID: UUID?
    /// The configured provider's default model at the time of run start.
    let configuredProviderDefaultModel: String?
    /// The explicit run-start override model (if any).
    let runOverrideModel: String?
    /// The final resolved model actually sent to Goose.
    let resolvedModel: String
    /// The final resolved provider family.
    let resolvedProviderFamily: String
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
