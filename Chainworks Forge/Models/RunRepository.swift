import Foundation
import SwiftData

/// RunRepository: single owner for Run creation and lifecycle queries.
///
/// ARCH-PA-003: the app is a SINGLE Xcode target (not separate modules).
/// `internal` access does not confine Run.init to RunRepository.
/// Enforcement uses automated checks, not Swift access control:
///
/// 1. @MainActor serialization — prevents TOCTOU races
/// 2. Automated codebase scan test — catches unauthorized Run construction
/// 3. CI pre-commit grep guard — blocks direct Run insertion at commit time
@MainActor
struct RunRepository {
    private let context: ModelContext

    init(context: ModelContext) {
        self.context = context
    }

    /// Atomically checks that no active run exists for the idea, then creates
    /// and inserts the new run. Returns the inserted Run.
    ///
    /// Active run = status in [.pending, .ready, .running, .waitingApproval, .blocked]
    ///
    /// This is the ONLY approved way to create a Run.
    ///
    /// Throws RunRepositoryError.activeRunExists if an active run already exists.
    func createRun(
        for idea: Idea,
        workflow: WorkflowDefinition,
        catalog: AgentCatalog,
        workflowSourcePath: String,
        catalogSourcePath: String
    ) throws -> Run {
        // Check for active run
        let activeStatuses: [RunStatus] = [.pending, .ready, .running, .waitingApproval, .blocked]
        if let existing = idea.runs.first(where: { activeStatuses.contains($0.status) }) {
            throw RunRepositoryError.activeRunExists(runID: existing.id, status: existing.status)
        }

        // Compute provenance hashes and snapshots
        let (workflowData, workflowHash) = try DefinitionHasher.hash(workflow)
        let (catalogData, catalogHash) = try DefinitionHasher.hash(catalog)

        let run = Run(
            workflowID: workflow.workflow.id,
            workflowTitle: workflow.workflow.name,
            workflowSnapshotHash: workflowHash,
            catalogSnapshotHash: catalogHash,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            workflowSnapshotJSON: workflowData,
            catalogSnapshotJSON: catalogData
        )
        run.idea = idea
        context.insert(run)
        return run
    }

    /// Convenience overload that accepts pre-computed provenance fields.
    ///
    /// Useful in tests and when snapshot data is already available.
    func createRun(
        for idea: Idea,
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
    ) throws -> Run {
        let activeStatuses: [RunStatus] = [.pending, .ready, .running, .waitingApproval, .blocked]
        if let existing = idea.runs.first(where: { activeStatuses.contains($0.status) }) {
            throw RunRepositoryError.activeRunExists(runID: existing.id, status: existing.status)
        }

        let run = Run(
            workflowID: workflowID,
            workflowTitle: workflowTitle,
            workflowSnapshotHash: workflowSnapshotHash,
            catalogSnapshotHash: catalogSnapshotHash,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            workflowSnapshotJSON: workflowSnapshotJSON,
            catalogSnapshotJSON: catalogSnapshotJSON,
            workspaceRoot: workspaceRoot,
            artifactRoot: artifactRoot,
            planCompilerVersion: planCompilerVersion
        )
        run.idea = idea
        context.insert(run)
        return run
    }

    /// Proposal 002: Create a Run from a precompiled RunPlan and provisioned RunWorkspace.
    /// This is the Phase 2 persistence boundary for the execution flow (ARCH-021).
    /// StageExecutions are created lazily by the orchestrator (ARCH-027).
    func createRunFromPlan(
        for idea: Idea,
        plan: RunPlan,
        workspace: RunWorkspace,
        workflowSourcePath: String,
        catalogSourcePath: String,
        startSnapshot: RunStartSnapshot
    ) throws -> Run {
        let activeStatuses: [RunStatus] = [.pending, .ready, .running, .waitingApproval, .blocked]
        if let existing = idea.runs.first(where: { activeStatuses.contains($0.status) }) {
            throw RunRepositoryError.activeRunExists(runID: existing.id, status: existing.status)
        }

        let run = Run(
            id: workspace.runID,
            workflowID: plan.workflowID,
            workflowTitle: plan.workflowTitle,
            workflowSnapshotHash: plan.workflowSnapshotHash,
            catalogSnapshotHash: plan.catalogSnapshotHash,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            workflowSnapshotJSON: plan.workflowSnapshotJSON,
            catalogSnapshotJSON: plan.catalogSnapshotJSON,
            workspaceRoot: workspace.workspaceRoot.path,
            artifactRoot: workspace.artifactRoot.path,
            planCompilerVersion: plan.planCompilerVersion
        )
        run.idea = idea

        // Proposal 003 — REQ-002: Populate mandatory cohort metadata at run creation.
        run.workflowFamily = Self.deriveWorkflowFamily(from: plan.workflowID)
        run.projectKey = Self.deriveProjectKey(from: idea)
        run.riskClass = .standard
        run.stack = "unknown"

        // Proposal 008 (REQ-006): Link run to benchmark cohort if the idea has one.
        run.experimentCohortID = idea.experimentCohortID

        startSnapshot.apply(to: run)

        context.insert(run)
        return run
    }

    func createRunFromPlan(
        for idea: Idea,
        plan: RunPlan,
        workspace: RunWorkspace,
        workflowSourcePath: String,
        catalogSourcePath: String
    ) throws -> Run {
        try createRunFromPlan(
            for: idea,
            plan: plan,
            workspace: workspace,
            workflowSourcePath: workflowSourcePath,
            catalogSourcePath: catalogSourcePath,
            startSnapshot: RunStartSnapshot()
        )
    }

    /// Returns the current active run for the idea, or nil.
    func activeRun(for idea: Idea) -> Run? {
        let activeStatuses: [RunStatus] = [.pending, .ready, .running, .waitingApproval, .blocked]
        return idea.runs.first(where: { activeStatuses.contains($0.status) })
    }

    // MARK: - Cohort Metadata Derivation (Proposal 003 — REQ-002)

    /// Derive `workflowFamily` from the workflow ID.
    /// e.g. "proposal_to_release_v1" → "proposal_to_release".
    private static func deriveWorkflowFamily(from workflowID: String) -> String {
        let components = workflowID.split(separator: "_")
        // Strip a trailing version component like "v1", "v2"
        if components.count > 1,
           let last = components.last,
           last.count <= 3,
           last.hasPrefix("v"),
           last.dropFirst().allSatisfy(\.isNumber) {
            return components.dropLast().joined(separator: "_")
        }
        return workflowID
    }

    /// Derive `projectKey` from the idea.
    /// Falls back to `"untagged"` per the cohorting contract.
    private static func deriveProjectKey(from idea: Idea) -> String {
        let title = idea.title
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .replacingOccurrences(of: " ", with: "_")
        return title.isEmpty ? "untagged" : title
    }
}

enum RunRepositoryError: Error, LocalizedError {
    case activeRunExists(runID: UUID, status: RunStatus)

    var errorDescription: String? {
        switch self {
        case .activeRunExists(let runID, let status):
            return "Active run \(runID) already exists with status \(status.rawValue)"
        }
    }
}
