import Foundation
import SwiftData

// MARK: - ResumeManager (ARCH-029)

/// Detects interrupted runs at app launch and classifies whether they can be resumed.
/// Checks:
///   - Compiler version compatibility (ARCH-029)
///   - Drift detection (hash comparison)
///   - Side-effect stage detection
@MainActor
final class ResumeManager {
    private let modelContext: ModelContext

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

        // Check 3: Drift detection — compare current source hashes with frozen hashes
        let driftResult = detectDrift(run: run)
        if let driftReason = driftResult {
            // If the run was in a state with side effects, require user decision
            if hasSideEffectStages(run: run, plan: plan) {
                return .needsDecision(
                    run,
                    reason: "Drift detected and run has executed side-effect stages: \(driftReason)"
                )
            }
            // Drift without side effects — still needs decision
            return .needsDecision(run, reason: driftReason)
        }

        // Check 4: Side-effect stage detection (e.g., git push, release)
        // Even without drift, if the run was interrupted mid-side-effect, flag it
        if wasInterruptedDuringSideEffect(run: run, plan: plan) {
            return .needsDecision(
                run,
                reason: "Run was interrupted during a side-effect stage"
            )
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
            if FileManager.default.fileExists(atPath: sourceURL.path) {
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
}
