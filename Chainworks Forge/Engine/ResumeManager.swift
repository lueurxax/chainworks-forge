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
        if run.status == .blocked {
            return .needsDecision(
                run,
                reason: "Run is blocked and requires explicit operator recovery; blocked runs are not auto-resumed at launch"
            )
        }

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

        let backfillSucceeded = backfillLegacyExecutionTruthIfNeeded(for: run)
        if !backfillSucceeded {
            return .needsDecision(
                run,
                reason: "Runtime truth is legacy or unverifiable; deterministic startup backfill could not classify all required rows"
            )
        }

        repairActiveLineagesIfNeeded(for: run)

        if requiresExplicitResumeDecision(run: run) {
            return .needsDecision(
                run,
                reason: "Runtime truth is legacy or unverifiable; explicit operator resume is required at launch"
            )
        }

        // Check 3: Drift detection — compare current source hashes with frozen hashes
        let driftResult = detectDrift(run: run)
        if let driftReason = driftResult {
            // Proposal 008 (§6.3): Runs interrupted at an approval gate must restore
            // the visible approval context even when workflow sources have drifted.
            // The operator can reject from the restored gate if content is stale.
            // Drift is surfaced as informational detail, not as a resume blocker.
            if run.status == .waitingApproval {
                run.driftDetails = driftReason
                // Fall through to resume — approval gate will be restored with drift notice
            } else if hasSideEffectStages(run: run, plan: plan) {
                return .needsDecision(
                    run,
                    reason: "Drift detected and run has executed side-effect stages: \(driftReason)"
                )
            } else {
                // Drift without side effects — still needs decision
                return .needsDecision(run, reason: driftReason)
            }
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

    private func requiresExplicitResumeDecision(run: Run) -> Bool {
        guard run.status != .waitingApproval else { return false }
        guard let trust = run.runtimeTrustLevel else { return true }
        return trust == RuntimeBindingTrustLevel.unknown.rawValue
            || trust == RuntimeBindingTrustLevel.unverifiable.rawValue
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

    // MARK: - Startup Settlement Repair (Proposal 016)

    private func backfillLegacyExecutionTruthIfNeeded(for run: Run) -> Bool {
        var unresolvedLegacyTruth = false

        for stage in run.stageExecutions {
            if stage.lineageID == nil {
                guard !stage.stageID.isEmpty else {
                    unresolvedLegacyTruth = true
                    continue
                }
                stage.lineageID = stageLineageID(for: stage)
            }

            for agentExec in stage.agentExecutions where agentExec.canonicalOutcome == nil {
                guard let canonicalOutcome = ExecutionTruthSupport.deterministicLegacyOutcome(for: agentExec) else {
                    if !Self.activeAgentStatuses.contains(agentExec.status) {
                        unresolvedLegacyTruth = true
                    }
                    continue
                }

                let outputPresence = ExecutionTruthSupport.derivedOutputPresence(for: agentExec)
                let receipt = ExecutionTruthSupport.decodedReceipt(from: agentExec)
                let runtimeProvider = agentExec.runtimeProvider
                    ?? receipt?.providerFamily
                    ?? agentExec.provider
                let runtimeModel = agentExec.runtimeModel
                    ?? receipt?.model
                    ?? agentExec.resolvedModel

                ExecutionTruthSupport.persistTerminalTruth(
                    for: agentExec,
                    canonicalOutcome: canonicalOutcome,
                    transportErrorKind: agentExec.transportErrorKind,
                    providerStopReason: agentExec.providerStopReason,
                    outputPresence: outputPresence,
                    runtimeProvider: runtimeProvider,
                    runtimeModel: runtimeModel,
                    envelope: decodeOutcomeEnvelope(from: agentExec)
                )
            }
        }

        for approval in run.approvals where approval.lineageID == nil {
            let matchingStages = run.stageExecutions
                .filter { $0.stageID == approval.stageID && $0.lineageID != nil }
                .sorted(by: canonicalStageOrdering)

            guard let canonicalStage = matchingStages.last,
                  let lineageID = canonicalStage.lineageID else {
                unresolvedLegacyTruth = true
                continue
            }

            approval.lineageID = "\(lineageID)::approval"
        }

        if unresolvedLegacyTruth {
            run.runtimeTrustLevel = RuntimeBindingTrustLevel.unverifiable.rawValue
        } else {
            run.runtimeTrustLevel = RuntimeBindingTruthResolver.deriveRunTrustLevel(
                agents: run.stageExecutions.flatMap(\.agentExecutions),
                persisted: run.runtimeTrustLevel
            )
        }

        if modelContext.hasChanges {
            do {
                try modelContext.save()
            } catch {
                run.runtimeTrustLevel = RuntimeBindingTrustLevel.unverifiable.rawValue
                return false
            }
        }

        return !unresolvedLegacyTruth
    }

    private func repairActiveLineagesIfNeeded(for run: Run) {
        let now = Date()

        for stage in run.stageExecutions {
            if stage.lineageID == nil {
                stage.lineageID = stageLineageID(for: stage)
            }
            reconcileStageIfTerminalEvidenceExists(stage, now: now)
        }

        let activeStages = run.stageExecutions.filter { Self.activeStageStatuses.contains($0.status) }
        let stageGroups = Dictionary(grouping: activeStages) { $0.lineageID ?? stageLineageID(for: $0) }
        for (_, siblings) in stageGroups {
            let sorted = siblings.sorted { lhs, rhs in
                if lhs.attemptNumber != rhs.attemptNumber {
                    return lhs.attemptNumber < rhs.attemptNumber
                }
                return lhs.startedAt < rhs.startedAt
            }
            guard let canonical = sorted.last else { continue }
            if canonical.activeOwnerToken == nil {
                canonical.activeOwnerToken = UUID().uuidString
            }
            for stale in sorted.dropLast() {
                stale.status = .blocked
                stale.settlementKind = .repaired
                stale.settledAt = stale.settledAt ?? now
                stale.completedAt = stale.completedAt ?? now
                stale.activeOwnerToken = nil
            }
        }

        for stage in run.stageExecutions where stage.status == .waitingApproval && stage.activeOwnerToken == nil {
            stage.activeOwnerToken = UUID().uuidString
        }

        for approval in run.approvals where approval.lineageID == nil {
            approval.lineageID = approvalLineageID(for: approval, run: run)
        }

        let requestedApprovals = run.approvals.filter { $0.decision == .requested }
        let approvalGroups = Dictionary(grouping: requestedApprovals) { $0.lineageID ?? approvalLineageID(for: $0, run: run) }
        for (_, siblings) in approvalGroups {
            let sorted = siblings.sorted { $0.requestedAt < $1.requestedAt }
            guard sorted.count > 1 else { continue }
            for stale in sorted.dropLast() {
                stale.decision = .expired
                stale.repairedAt = stale.repairedAt ?? now
            }
        }

        if modelContext.hasChanges {
            try? modelContext.save()
        }
    }

    private func stageLineageID(for stage: StageExecution) -> String {
        "\(stage.stageID)::iteration:\(stage.iteration)"
    }

    private func reconcileStageIfTerminalEvidenceExists(_ stage: StageExecution, now: Date) {
        guard Self.activeStageStatuses.contains(stage.status) else { return }
        guard stage.status != .waitingApproval else { return }

        let activeAgentStatuses: Set<AgentStatus> = [.pending, .ready, .running]
        guard !stage.agentExecutions.contains(where: { activeAgentStatuses.contains($0.status) }) else { return }

        let settledAgents = stage.agentExecutions.filter {
            $0.canonicalOutcome != nil && ($0.settledAt != nil || $0.completedAt != nil)
        }
        guard !settledAgents.isEmpty else { return }

        let latestSettlement = settledAgents
            .compactMap { $0.settledAt ?? $0.completedAt }
            .max() ?? now

        if settledAgents.contains(where: \.blocksForwardProgress) {
            stage.status = .failed
            stage.settlementKind = .failed
            stage.completedAt = stage.completedAt ?? latestSettlement
            stage.settledAt = latestSettlement
            stage.activeOwnerToken = nil
            return
        }

        let allTerminal = stage.agentExecutions.allSatisfy { execution in
            if let canonicalOutcome = execution.canonicalOutcome {
                let coarseStatus = canonicalOutcome.coarseStatus
                return coarseStatus != .pending && coarseStatus != .ready && coarseStatus != .running
            }
            return !activeAgentStatuses.contains(execution.status)
        }

        guard allTerminal else {
            stage.status = .blocked
            stage.settlementKind = .blocked
            stage.completedAt = stage.completedAt ?? latestSettlement
            stage.settledAt = latestSettlement
            stage.activeOwnerToken = nil
            return
        }

        stage.status = .completed
        stage.settlementKind = .completed
        stage.completedAt = stage.completedAt ?? latestSettlement
        stage.settledAt = latestSettlement
        stage.activeOwnerToken = nil
    }

    private func approvalLineageID(for approval: Approval, run: Run) -> String {
        if let stage = run.stageExecutions
            .filter({ $0.stageID == approval.stageID })
            .sorted(by: canonicalStageOrdering)
            .last {
            return "\(stage.lineageID ?? stageLineageID(for: stage))::approval"
        }

        return "\(approval.stageID)::approval"
    }

    private static let activeStageStatuses: Set<StageStatus> = [.running, .ready, .waitingApproval]
    private static let activeAgentStatuses: Set<AgentStatus> = [.pending, .ready, .running]

    private func canonicalStageOrdering(_ lhs: StageExecution, _ rhs: StageExecution) -> Bool {
        if lhs.attemptNumber != rhs.attemptNumber {
            return lhs.attemptNumber < rhs.attemptNumber
        }
        if lhs.startedAt != rhs.startedAt {
            return lhs.startedAt < rhs.startedAt
        }
        return lhs.id.uuidString < rhs.id.uuidString
    }

    private func decodeOutcomeEnvelope(from agentExec: AgentExecution) -> OutcomeEnvelope? {
        guard let data = agentExec.outcomeEnvelopeJSON else { return nil }
        return try? JSONDecoder().decode(OutcomeEnvelope.self, from: data)
    }
}
