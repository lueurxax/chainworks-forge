import Foundation
import SwiftData

// MARK: - ResumeManager (ARCH-029)

/// Detects interrupted runs at app launch and classifies whether they can be resumed.
/// Checks:
///   - Compiler version compatibility (ARCH-029)
///   - Side-effect stage detection
///   - Source drift as informational operator context only
@MainActor
final class ResumeManager {
    private let modelContext: ModelContext
    private static let startupInterruptionReason =
        "Run was interrupted by app restart before reaching a terminal state. Use Resume Interrupted to continue."

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    // MARK: - Resume Actions

    enum ResumeAction {
        /// Safe to resume — plan rebuilt successfully, no drift detected.
        case resume(Run, RunPlan, RunWorkspace)
        /// Needs user decision — drift detected or side-effect stage.
        case needsDecision(Run, reason: String)
        /// Cannot resume — compiler version mismatch or snapshot corruption.
        case cannotResume(Run, reason: String)
    }

    // MARK: - Startup Normalization

    /// Normalizes stale in-flight runs after a fresh app launch when auto-resume is disabled.
    /// Runs that still appear `.pending`, `.ready`, or `.running` cannot be truthful after a new
    /// process start, so they become recoverable `.blocked` runs that require explicit operator resume.
    ///
    /// Proposal 032: If the run has a durable transition cursor showing `transitionSettled`
    /// (next state was scheduled but never started), the run is marked `.blocked` for operator
    /// resume but the scheduled-but-not-started stage is preserved as `.ready` rather than
    /// being rewritten to `.blocked`. This distinguishes "interrupted before work began"
    /// from "interrupted mid-execution" and enables clean resume targeting.
    @discardableResult
    func normalizeInterruptedRunsForManualResume() throws -> Int {
        let interruptedRuns = try findInterruptedRuns()
        let now = Date()
        var normalizedCount = 0

        for run in interruptedRuns {
            guard [.pending, .ready, .running].contains(run.status) else { continue }

            run.status = .blocked
            run.driftDetails = mergedInterruptionReason(existing: run.driftDetails)

            // Proposal 032: Check cursor for scheduled-but-not-started continuation truth.
            let cursor = run.transitionCursor
            let scheduledNotStartedStageID: String?
            if let cursor, cursor.settlementPhase == .transitionSettled {
                scheduledNotStartedStageID = cursor.nextScheduledStateID
            } else {
                scheduledNotStartedStageID = nil
            }

            for stageExecution in run.stageExecutions where [.pending, .ready, .running].contains(stageExecution.status) {
                // Proposal 032 §5.3: If this stage was scheduled but never started,
                // preserve it as .ready so resume can target it cleanly.
                if let scheduledID = scheduledNotStartedStageID,
                   stageExecution.stageID == scheduledID,
                   stageExecution.status == .ready,
                   !stageExecution.agentExecutions.contains(where: { !$0.artifacts.isEmpty }) {
                    // Preserve as resumable — do not flatten to blocked.
                    continue
                }

                stageExecution.status = .blocked
                stageExecution.settlementKind = .blocked
                stageExecution.settledAt = stageExecution.settledAt ?? now

                for agentExecution in stageExecution.agentExecutions where [.pending, .ready, .running].contains(agentExecution.status) {
                    agentExecution.status = .failed
                    agentExecution.completedAt = agentExecution.completedAt ?? now
                    agentExecution.settledAt = agentExecution.settledAt ?? now
                    agentExecution.canonicalOutcome = agentExecution.canonicalOutcome ?? .failedBeforeOutput
                    agentExecution.transportErrorKind = agentExecution.transportErrorKind ?? .unknown
                    agentExecution.providerStopReason = agentExecution.providerStopReason ?? "interrupted_on_app_restart"
                    agentExecution.logSnippet = mergedAgentInterruptionLog(existing: agentExecution.logSnippet)
                }
            }

            normalizedCount += 1
        }

        if normalizedCount > 0 {
            try modelContext.save()
        }

        return normalizedCount
    }

    // MARK: - Classify Interrupted Runs

    /// Find all interrupted runs and classify their resume eligibility.
    func classifyInterruptedRuns(compiler: RunPlanCompiler) throws -> [ResumeAction] {
        let interruptedRuns = try findInterruptedRuns()
        var actions: [ResumeAction] = []

        for run in interruptedRuns {
            let action = classifyRun(run, compiler: compiler)
            actions.append(action)
        }

        return actions
    }

    /// Find runs that were active when the app was last terminated.
    func findInterruptedRuns() throws -> [Run] {
        // Flush pending changes so the fetch sees the latest state.
        // This is critical for in-memory stores where auto-save may not have fired yet.
        try modelContext.save()

        // Fetch all runs and filter in-memory to avoid SwiftData #Predicate
        // limitations with enum rawValue and array contains.
        let descriptor = FetchDescriptor<Run>(
            sortBy: [SortDescriptor(\.startedAt)]
        )

        let allRuns = try modelContext.fetch(descriptor)
        let interruptibleStatuses: Set<RunStatus> = [
            .running, .waitingApproval, .blocked, .pending, .ready
        ]

        return allRuns.filter { interruptibleStatuses.contains($0.status) }
    }

    // MARK: - Single Run Classification

    private func classifyRun(_ run: Run, compiler: RunPlanCompiler) -> ResumeAction {
        // Check 1: Compiler version (ARCH-029)
        guard run.planCompilerVersion == RunPlan.currentCompilerVersion else {
            return .cannotResume(
                run,
                reason: "Compiler version mismatch: run compiled with v\(run.planCompilerVersion), current is v\(RunPlan.currentCompilerVersion)"
            )
        }

        // Check 2: Try to rebuild the plan from frozen snapshots
        let plan: RunPlan
        let workspace: RunWorkspace
        do {
            (plan, workspace) = try compiler.rebuildPlanFromSnapshot(run: run)
        } catch {
            return .cannotResume(
                run,
                reason: "Failed to rebuild plan from snapshot: \(error.localizedDescription)"
            )
        }

        // Check 3: Source drift is informational only.
        // Existing runs always resume from frozen workflow/catalog snapshots already
        // stored on `Run`; mutable source files are only a comparison surface.
        let driftResult = detectDrift(run: run)
        if let driftReason = driftResult {
            run.driftDetectedAt = Date()
            run.driftDetails = driftReason
        } else if run.driftDetails?.contains("hash mismatch") == true {
            run.driftDetails = nil
        }

        // Check 4: Side-effect stage detection (e.g., git push, release)
        // Even without drift, if the run was interrupted mid-side-effect, flag it
        if wasInterruptedDuringSideEffect(run: run, plan: plan) {
            return .needsDecision(
                run,
                reason: "Run was interrupted during a side-effect stage"
            )
        }

        // Check 5 (Proposal 011 — REQ-005): Validate frozen workspace directory still exists.
        if plan.requiresProjectAccess {
            if let frozenPath = run.frozenWorkspaceRootPath, !frozenPath.isEmpty {
                var isDirectory: ObjCBool = false
                let exists = FileManager.default.fileExists(atPath: frozenPath, isDirectory: &isDirectory)
                if !exists || !isDirectory.boolValue {
                    return .cannotResume(
                        run,
                        reason: "Frozen workspace directory no longer exists or is not accessible: \(frozenPath)"
                    )
                }
            } else {
                return .cannotResume(
                    run,
                    reason: "Workflow requires project access but the run has no frozen workspace root path"
                )
            }
        }

        // Safe to resume
        return .resume(run, plan, workspace)
    }

    // MARK: - Drift Detection

    /// Detect if the workflow/catalog source files have changed since the run was created.
    /// Returns nil if no drift, or a description of what drifted.
    private func detectDrift(run: Run) -> String? {
        var driftReasons: [String] = []

        // Check workflow source drift
        if !run.workflowSourcePath.isEmpty {
            let sourceURL = URL(fileURLWithPath: run.workflowSourcePath)
            if SecurityScopedAccess.fileExists(at: sourceURL) {
                do {
                    let currentWorkflow = try YAMLParser.loadWorkflow(from: sourceURL)
                    let (_, currentHash) = try DefinitionHasher.hash(currentWorkflow)
                    if currentHash != run.workflowSnapshotHash {
                        driftReasons.append("Workflow source has changed (hash mismatch)")
                    }
                } catch {
                    // Can't read source — don't flag as drift, could be benign
                }
            }
        }

        // Check catalog source drift
        if !run.catalogSourcePath.isEmpty {
            let sourceURL = URL(fileURLWithPath: run.catalogSourcePath)
            if FileManager.default.fileExists(atPath: sourceURL.path) {
                do {
                    let currentCatalog = try YAMLParser.loadAgentCatalog(from: sourceURL)
                    let (_, currentHash) = try DefinitionHasher.hash(currentCatalog)
                    if currentHash != run.catalogSnapshotHash {
                        driftReasons.append("Agent catalog source has changed (hash mismatch)")
                    }
                } catch {
                    // Can't read source — don't flag as drift
                }
            }
        }

        return driftReasons.isEmpty ? nil : driftReasons.joined(separator: "; ")
    }

    // MARK: - Side-Effect Detection (§10.2)

    /// Stage-name patterns that indicate irreversible side effects (git push, release, etc.).
    private static let sideEffectStagePatterns = [
        "commit", "push", "release", "publish", "deploy"
    ]

    /// Permission profiles that indicate side-effect stages (§10.2).
    private static let sideEffectPermissionProfiles: Set<String> = [
        "RELEASE_GIT", "RELEASE_PUBLISH"
    ]

    /// Determine if a state produces irreversible side effects per §10.2:
    /// - Agents with `requires_human_approval: true`
    /// - Agents with permission profiles `RELEASE_GIT` or `RELEASE_PUBLISH`
    /// - Stage ID matching side-effect name patterns (fallback heuristic)
    private func isSideEffectState(_ stateID: String, plan: RunPlan) -> Bool {
        // Check stage name patterns (heuristic fallback)
        if Self.sideEffectStagePatterns.contains(where: { stateID.lowercased().contains($0) }) {
            return true
        }

        // Check agents referenced by this state (§10.2 contract)
        guard let state = plan.states[stateID] else { return false }

        let agentIDs = collectAgentIDsFromState(state)
        for agentID in agentIDs {
            guard let agent = plan.agentBindings[agentID] else { continue }
            // requires_human_approval flag
            if agent.requiresHumanApproval {
                return true
            }
            // RELEASE_GIT / RELEASE_PUBLISH permission profiles
            if Self.sideEffectPermissionProfiles.contains(agent.permissionProfile) {
                return true
            }
        }
        return false
    }

    /// Collect all agent IDs referenced by a state's run blocks.
    private func collectAgentIDsFromState(_ state: ExecutableState) -> Set<String> {
        var ids = Set<String>()
        ids.insert(state.ownerAgentID)
        if let block = state.runBlock {
            for phase in block.phases {
                switch phase {
                case .sequential(let tasks): tasks.forEach { ids.insert($0.agent) }
                case .parallel(let tasks): tasks.forEach { ids.insert($0.agent) }
                }
            }
        }
        if let block = state.runAfterApproval {
            for phase in block.phases {
                switch phase {
                case .sequential(let tasks): tasks.forEach { ids.insert($0.agent) }
                case .parallel(let tasks): tasks.forEach { ids.insert($0.agent) }
                }
            }
        }
        return ids
    }

    /// Check if the run has already executed any side-effect stages.
    private func hasSideEffectStages(run: Run, plan: RunPlan) -> Bool {
        let completedStageIDs = Set(
            run.stageExecutions
                .filter { $0.status == .completed }
                .map(\.stageID)
        )

        return completedStageIDs.contains { isSideEffectState($0, plan: plan) }
    }

    /// Check if the run was interrupted during a side-effect stage.
    private func wasInterruptedDuringSideEffect(run: Run, plan: RunPlan) -> Bool {
        let runningStageIDs = run.stageExecutions
            .filter { $0.status == .running }
            .map(\.stageID)

        return runningStageIDs.contains { isSideEffectState($0, plan: plan) }
    }

    private func mergedInterruptionReason(existing: String?) -> String {
        let existing = existing?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !existing.isEmpty else { return Self.startupInterruptionReason }
        guard !existing.localizedCaseInsensitiveContains("interrupted by app restart") else { return existing }
        return "\(existing) \(Self.startupInterruptionReason)"
    }

    private func mergedAgentInterruptionLog(existing: String?) -> String {
        let note = "Interrupted by app restart before settlement. Manual resume required."
        let existing = existing?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !existing.isEmpty else { return note }
        guard !existing.localizedCaseInsensitiveContains("manual resume required") else { return existing }
        return "\(existing) \(note)"
    }
}
