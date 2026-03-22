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
        catalogSnapshotJSON: Data
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
            catalogSnapshotJSON: catalogSnapshotJSON
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
        catalogSourcePath: String
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
        context.insert(run)
        return run
    }

    /// Returns the current active run for the idea, or nil.
    func activeRun(for idea: Idea) -> Run? {
        let activeStatuses: [RunStatus] = [.pending, .ready, .running, .waitingApproval, .blocked]
        return idea.runs.first(where: { activeStatuses.contains($0.status) })
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
