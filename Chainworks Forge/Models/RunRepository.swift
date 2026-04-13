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
    private static let terminalCleanupStatuses: Set<RunStatus> = [.completed, .failed, .cancelled]
    private static let preservedIdeaAttachmentFolder = ".chainworks/idea-attachments"

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

    func terminalRunsEligibleForCleanup() -> [Run] {
        let descriptor = FetchDescriptor<Run>(sortBy: [SortDescriptor(\.startedAt, order: .reverse)])
        let allRuns = (try? context.fetch(descriptor)) ?? []
        return allRuns.filter { Self.terminalCleanupStatuses.contains($0.status) }
    }

    func prepareTerminalRunCleanup() throws -> RunCleanupPlan {
        let candidateRuns = terminalRunsEligibleForCleanup()
        let migrationResult = try preserveIdeaAttachmentReferences(for: candidateRuns)
        let runs = candidateRuns.filter { !migrationResult.protectedRunIDs.contains($0.id) }
        let paths = runs.flatMap(Self.cleanupPaths(for:))
        let deletedRunIDs = runs.map(\.id)

        materializeCleanupFaults(for: runs)

        for run in runs {
            context.delete(run)
        }

        if context.hasChanges {
            try context.save()
        }

        return RunCleanupPlan(
            deletedRunCount: runs.count,
            deletedRunIDs: deletedRunIDs,
            filesystemPaths: Array(Set(paths)).sorted { $0.count > $1.count },
            migratedAttachmentCount: migrationResult.migratedAttachmentCount,
            protectedRunCount: migrationResult.protectedRunIDs.count,
            protectedRunIDs: migrationResult.protectedRunIDs.sorted { $0.uuidString < $1.uuidString }
        )
    }

    private func materializeCleanupFaults(for runs: [Run]) {
        for run in runs {
            for stage in run.stageExecutions {
                _ = stage.stageID
                _ = stage.label
                _ = stage.startedAt
                _ = stage.completedAt
                _ = stage.status
                _ = stage.iteration
                _ = stage.attemptNumber
                _ = stage.lineageID
                _ = stage.settlementKind
                _ = stage.settledAt
                _ = stage.activeOwnerToken
                _ = stage.retryMode
                _ = stage.triggerReason
                _ = stage.supersedesAttemptNumber
                _ = stage.validationFailureJSON
                _ = stage.evidencePacketJSON
                _ = stage.recoverySnapshotJSON

                for agent in stage.agentExecutions {
                    _ = agent.agentID
                    _ = agent.agentTitle
                    _ = agent.taskName
                    _ = agent.startedAt
                    _ = agent.completedAt
                    _ = agent.status
                    _ = agent.provider
                    _ = agent.effort
                    _ = agent.costCents
                    _ = agent.logSnippet
                    _ = agent.runtimeSessionID
                    _ = agent.providerSessionID
                    _ = agent.providerRequestID
                    _ = agent.transcriptArtifactPath
                    _ = agent.resolvedBackendProfileID
                    _ = agent.providerReceiptJSON
                    _ = agent.resolvedModel
                    _ = agent.adapterVersion
                    _ = agent.retryReason
                    _ = agent.agentAttemptNumber
                    _ = agent.supersedesAgentExecutionID
                    _ = agent.validationFailureJSON
                    _ = agent.outputEnvelopesJSON
                    _ = agent.compactionMetadataJSON
                    _ = agent.canonicalOutcome
                    _ = agent.supervisionClassification
                    _ = agent.transportErrorKind
                    _ = agent.providerStopReason
                    _ = agent.outputPresence
                    _ = agent.settledAt
                    _ = agent.runtimeProvider
                    _ = agent.runtimeModel
                    _ = agent.outcomeEnvelopeJSON
                    _ = agent.runtimeProfileID
                    _ = agent.actualAdapterFamily
                    _ = agent.actualCapabilityClass
                    _ = agent.repoRevisionBefore
                    _ = agent.repoRevisionAfter
                    _ = agent.sessionLineageID
                    _ = agent.sessionGenerationID
                    _ = agent.rehydratedFromCheckpointArtifactID
                    _ = agent.invocationOwnerKey
                    _ = agent.sessionReuseScope
                    _ = agent.sessionFamilyID
                    _ = agent.sessionReuseDisposition
                    _ = agent.sessionResetReason
                    _ = agent.inputPayloadBytes
                    _ = agent.handoffMode
                    _ = agent.modelTierUsed
                }
            }

            for approval in run.approvals {
                _ = approval.decision
                _ = approval.stageID
                _ = approval.requestedAt
                _ = approval.decidedAt
                _ = approval.comment
            }
        }
    }

    nonisolated static func removeFilesystemRoots(_ plan: RunCleanupPlan) async -> Int {
        guard !plan.filesystemPaths.isEmpty else { return 0 }

        return await Task.detached(priority: .utility) {
            let fileManager = FileManager.default
            var removedCount = 0

            for path in plan.filesystemPaths where shouldRemoveOwnedPath(path) {
                guard fileManager.fileExists(atPath: path) else { continue }
                do {
                    try fileManager.removeItem(atPath: path)
                    removedCount += 1
                } catch {
                    await ForgeLogger.app.error("Run cleanup failed to remove path \(path): \(error.localizedDescription)")
                }
            }

            return removedCount
        }.value
    }

    // MARK: - Cohort Metadata Derivation (Proposal 003 — REQ-002)

    private static func cleanupPaths(for run: Run) -> [String] {
        let workspaceRoot = normalizedCleanupPath(run.workspaceRoot)
        let artifactRoot = normalizedCleanupPath(run.artifactRoot)
        let worktreeRoot = normalizedCleanupPath(run.worktreeRoot)

        var paths: [String] = []
        if let workspaceRoot {
            paths.append(workspaceRoot)
        }

        if let artifactRoot,
           paths.contains(where: { artifactRoot.hasPrefix($0 + "/") || artifactRoot == $0 }) == false {
            paths.append(artifactRoot)
        }

        if let worktreeRoot,
           paths.contains(where: { worktreeRoot.hasPrefix($0 + "/") || worktreeRoot == $0 }) == false {
            paths.append(worktreeRoot)
        }

        return paths
    }

    private func preserveIdeaAttachmentReferences(for runs: [Run]) throws -> AttachmentMigrationResult {
        guard !runs.isEmpty else {
            return AttachmentMigrationResult(migratedAttachmentCount: 0, protectedRunIDs: [])
        }

        let runCleanupRoots = Dictionary(uniqueKeysWithValues: runs.map { run in
            let ownedRoots = Self.cleanupPaths(for: run).filter(Self.shouldRemoveOwnedPath)
            return (run.id, ownedRoots)
        })
        let runByID = Dictionary(uniqueKeysWithValues: runs.map { ($0.id, $0) })

        let descriptor = FetchDescriptor<Idea>(sortBy: [SortDescriptor(\.createdAt, order: .reverse)])
        let ideas = try context.fetch(descriptor)

        var migratedAttachmentCount = 0
        var protectedRunIDs = Set<UUID>()
        let fileManager = FileManager.default

        for idea in ideas {
            guard let attachmentPath = Self.normalizedCleanupPath(idea.attachmentPath) else { continue }
            guard let runID = owningCleanupRunID(for: attachmentPath, cleanupRootsByRunID: runCleanupRoots) else { continue }
            guard protectedRunIDs.contains(runID) == false else { continue }

            guard let run = runByID[runID] else { continue }
            guard let workspaceRootPath = Self.normalizedCleanupPath(idea.workspaceRootPath) else {
                protectedRunIDs.insert(runID)
                continue
            }

            var isDirectory: ObjCBool = false
            guard fileManager.fileExists(atPath: workspaceRootPath, isDirectory: &isDirectory), isDirectory.boolValue else {
                protectedRunIDs.insert(runID)
                continue
            }

            guard fileManager.fileExists(atPath: attachmentPath) else {
                continue
            }

            let destinationURL = uniqueIdeaAttachmentDestination(
                ideaID: idea.id,
                workspaceRootPath: workspaceRootPath,
                sourcePath: attachmentPath
            )
            try fileManager.createDirectory(at: destinationURL.deletingLastPathComponent(), withIntermediateDirectories: true)
            try fileManager.copyItem(at: URL(fileURLWithPath: attachmentPath), to: destinationURL)

            idea.attachmentPath = destinationURL.path
            run.driftDetails = mergedPreservedAttachmentNote(existing: run.driftDetails, destinationPath: destinationURL.path)
            migratedAttachmentCount += 1
        }

        return AttachmentMigrationResult(
            migratedAttachmentCount: migratedAttachmentCount,
            protectedRunIDs: Array(protectedRunIDs)
        )
    }

    private func owningCleanupRunID(
        for attachmentPath: String,
        cleanupRootsByRunID: [UUID: [String]]
    ) -> UUID? {
        cleanupRootsByRunID.first { _, roots in
            roots.contains(where: { root in
                attachmentPath == root || attachmentPath.hasPrefix(root + "/")
            })
        }?.key
    }

    private func uniqueIdeaAttachmentDestination(
        ideaID: UUID,
        workspaceRootPath: String,
        sourcePath: String
    ) -> URL {
        let sourceURL = URL(fileURLWithPath: sourcePath)
        let baseDirectory = URL(fileURLWithPath: workspaceRootPath, isDirectory: true)
            .appendingPathComponent(Self.preservedIdeaAttachmentFolder, isDirectory: true)
            .appendingPathComponent(ideaID.uuidString, isDirectory: true)

        let rawBaseName = sourceURL.deletingPathExtension().lastPathComponent.trimmingCharacters(in: .whitespacesAndNewlines)
        let baseName = rawBaseName.isEmpty ? "attachment" : rawBaseName
        let ext = sourceURL.pathExtension
        var candidate = baseDirectory.appendingPathComponent(sourceURL.lastPathComponent)
        var suffix = 1

        while FileManager.default.fileExists(atPath: candidate.path) {
            let nextName = ext.isEmpty ? "\(baseName)-\(suffix)" : "\(baseName)-\(suffix).\(ext)"
            candidate = baseDirectory.appendingPathComponent(nextName)
            suffix += 1
        }

        return candidate
    }

    private func mergedPreservedAttachmentNote(existing: String?, destinationPath: String) -> String {
        let note = "Referenced attachment preserved at \(destinationPath) before terminal run cleanup."
        let existing = existing?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !existing.isEmpty else { return note }
        guard !existing.localizedCaseInsensitiveContains("attachment preserved at") else { return existing }
        return "\(existing) \(note)"
    }

    private static func normalizedCleanupPath(_ rawPath: String?) -> String? {
        guard let rawPath else { return nil }
        let trimmed = rawPath.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        return URL(fileURLWithPath: trimmed).standardizedFileURL.path
    }

    nonisolated private static func shouldRemoveOwnedPath(_ path: String) -> Bool {
        guard path != "/", path.isEmpty == false else { return false }
        let standardized = URL(fileURLWithPath: path).standardizedFileURL.path
        let tempRoot = FileManager.default.temporaryDirectory.standardizedFileURL.path
        if standardized.hasPrefix(tempRoot + "/") {
            return true
        }
        return standardized.contains("/Library/Application Support/Chainworks Forge/runs/")
            || standardized.contains("/Library/Application Support/Chainworks Forge/worktrees/")
    }

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

struct RunCleanupPlan: Sendable {
    let deletedRunCount: Int
    let deletedRunIDs: [UUID]
    let filesystemPaths: [String]
    let migratedAttachmentCount: Int
    let protectedRunCount: Int
    let protectedRunIDs: [UUID]
}

private struct AttachmentMigrationResult: Sendable {
    let migratedAttachmentCount: Int
    let protectedRunIDs: [UUID]
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
