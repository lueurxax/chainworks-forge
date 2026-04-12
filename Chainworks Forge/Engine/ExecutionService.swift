import Foundation
import SwiftData
import Observation

enum LiveTransportMode: Sendable {
    case fixtureProposalLoopSuccess
    case fixtureProposal022FeedbackCycle
    case fixtureProposal013AggregateFailure
    case fixtureFullMVPSuccess
}

struct LiveRuntimeConfiguration: Sendable {
    let override: LiveExecutionOverride?
    let transportMode: LiveTransportMode

    var summary: String {
        if let override {
            return "\(override.provider) / \(override.model) / \(override.effort)"
        }
        return "agent-defined provider/model"
    }

    var sourceDescription: String {
        "Fixture backend"
    }
}

enum LiveRuntimeReadiness: Sendable {
    case ready(summary: String, source: String)
    case unavailable(reason: String, recovery: String)
}

private struct ApprovalResolutionDiagnostic: Codable, Sendable {
    struct ApprovalSnapshot: Codable, Sendable {
        let approvalID: UUID
        let stageID: String
        let decision: String
        let requestedAt: Date
        let decidedAt: Date?
        let comment: String?
    }

    let runID: UUID
    let workflowID: String
    let approvalRequestID: UUID
    let stageID: String
    let granted: Bool
    let recordedAt: Date
    let runStatusBefore: String
    let runStatusAfter: String
    let currentStageBefore: String?
    let currentStageAfter: String?
    let stageStatusBefore: String?
    let stageStatusAfter: String?
    let approvalBefore: ApprovalSnapshot?
    let approvalAfter: ApprovalSnapshot?
}

// MARK: - ExecutionService (app-scoped @Observable singleton — ARCH-022)

/// App-scoped service managing all active workflow orchestrators.
/// NOT a per-run singleton — maintains a collection of orchestrators (ARCH-028).
@MainActor
@Observable
final class ExecutionService {
    private static let stalledRunGraceInterval: TimeInterval = 30
    private static let maintenanceTickInterval: Duration = .seconds(5)

    /// Active orchestrators keyed by run ID.
    private(set) var activeOrchestrators: [UUID: WorkflowOrchestrator] = [:]

    /// Pending approval requests across all runs (ARCH-028: collection, not singleton).
    private(set) var pendingApprovals: [UUID: ApprovalRequest] = [:]

    /// The model context for SwiftData operations.
    let modelContext: ModelContext

    /// The agent executor to use (injectable for testing).
    let executor: AgentExecutor

    /// Optional catalog for contract-aware output generation.
    let catalog: AgentCatalog?

    /// Optional fixture or externally injected live runtime configuration.
    private let fixedLiveRuntimeConfiguration: LiveRuntimeConfiguration?
    private let providerRegistry: ProviderRegistry?

    /// UI-test-only override used to force the unavailable recovery lane while
    /// keeping the live entrypoint visible for owner-path proofs.
    private let forceUITestLiveRuntimeUnavailable: Bool

    /// Steward config (loaded at app init, nil if not present).
    var stewardConfig: StewardConfig?

    /// P005-OPS §10: Notification service.
    let notificationService: NotificationService

    /// P005-OPS §6: Report builder.
    private(set) var reportBuilder: RunReportBuilder?

    /// Counter for post-run hook trigger.
    private var completedRunsSinceLastAnalysis: Int = 0
    private(set) var reconcileInvocationCountForTesting: Int = 0

    /// REQ-008: Flag set when a config change is detected and analysis should run after next completed run.
    private var configChangeAnalysisScheduled: Bool = false
    private var maintenanceTask: Task<Void, Never>?
    private var reportHistoryCompactionTask: Task<Void, Never>?

    init(
        modelContext: ModelContext,
        executor: AgentExecutor,
        catalog: AgentCatalog? = nil,
        stewardConfig: StewardConfig? = nil,
        liveRuntimeConfiguration: LiveRuntimeConfiguration? = nil,
        notificationService: NotificationService? = nil,
        providerRegistry: ProviderRegistry? = nil
    ) {
        self.modelContext = modelContext
        self.executor = executor
        self.catalog = catalog
        self.stewardConfig = stewardConfig
        self.fixedLiveRuntimeConfiguration = liveRuntimeConfiguration
        self.providerRegistry = providerRegistry
        self.forceUITestLiveRuntimeUnavailable =
            ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_FORCE_LIVE_RUNTIME_UNAVAILABLE"] == "1"
        self.notificationService = notificationService ?? MainActor.assumeIsolated {
            NotificationService()
        }
        RuntimeHelperProcessJanitor.live.sweepStaleHelpers()
        rebuildPersistedPendingApprovals()
        startMaintenanceLoop()
        scheduleReportHistoryCompaction()
    }

    var liveRuntimeConfiguration: LiveRuntimeConfiguration? {
        fixedLiveRuntimeConfiguration
    }

    func runMaintenanceTick(now: Date = Date()) {
        reconcileStalledOrchestratorsIfNeeded(now: now)
    }

    func prepareForTermination() {
        maintenanceTask?.cancel()
        maintenanceTask = nil
        reportHistoryCompactionTask?.cancel()
        reportHistoryCompactionTask = nil

        for orchestrator in activeOrchestrators.values {
            guard let runtimeExecutor = orchestrator.executor as? RuntimeAgentExecutor else { continue }
            runtimeExecutor.prepareForAppTermination()
        }
    }

    private func scheduleReportHistoryCompaction() {
        reportHistoryCompactionTask?.cancel()
        reportHistoryCompactionTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(2))
            guard !Task.isCancelled else { return }

            let builder = reportBuilder ?? RunReportBuilder(modelContext: modelContext)
            reportBuilder = builder

            do {
                try builder.pruneImmutableHistoryForAllRuns()
            } catch {
                ForgeLogger.execution.error("Run report history compaction failed: \(error.localizedDescription)")
            }
        }
    }

    // MARK: - Start Run

    /// Start a new workflow run. Creates an orchestrator and begins execution.
    func startRun(
        run: Run,
        plan: RunPlan,
        workspace: RunWorkspace
    ) {
        guard let orchestrator = prepareOrchestrator(run: run, plan: plan, workspace: workspace) else { return }
        prepareRunForLiveAttachment(run)
        synchronizeIdeaStatus(for: run)

        Task { @MainActor in
            await orchestrator.start()
        }
    }

    /// Re-attach orchestration to an existing run after an explicit recovery action created
    /// a new ready/running execution path. This avoids leaving a run in inert `.ready`
    /// after retry-in-place mutated SwiftData but no orchestrator was attached.
    func resumeRun(run: Run, compiler: RunPlanCompiler) throws {
        guard activeOrchestrators[run.id] == nil else {
            ForgeLogger.execution.info("resumeRun skipped: orchestrator already active for run \(run.id)")
            return
        }

        let (plan, workspace) = try compiler.rebuildPlanFromSnapshot(run: run)
        guard let orchestrator = prepareOrchestrator(run: run, plan: plan, workspace: workspace) else {
            ForgeLogger.execution.error("resumeRun failed: prepareOrchestrator returned nil for run \(run.id), status=\(run.status.rawValue)")
            return
        }

        let resumeStateID = run.resumeContinuationStateID
        ForgeLogger.execution.info("Starting orchestrator from state=\(resumeStateID ?? "nil") for run \(run.id)")
        prepareRunForLiveAttachment(run)
        synchronizeIdeaStatus(for: run)

        Task { @MainActor in
            await orchestrator.start(from: resumeStateID)
        }
    }

    // MARK: - Resume Interrupted Runs

    /// Resume interrupted runs on explicit operator action.
    /// Uses ResumeManager to classify which runs can be resumed.
    func resumeInterruptedRuns(compiler: RunPlanCompiler) {
        let resumeManager = ResumeManager(modelContext: modelContext)

        do {
            let actions = try resumeManager.classifyInterruptedRuns(compiler: compiler)
            for action in actions {
                switch action {
                case .resume(let run, let plan, let workspace):
                    guard let orchestrator = prepareOrchestrator(run: run, plan: plan, workspace: workspace) else {
                        continue
                    }

                    let resumeStateID = run.resumeContinuationStateID
                    prepareRunForLiveAttachment(run)
                    Task { @MainActor in
                        await orchestrator.start(from: resumeStateID)
                    }

                case .needsDecision(let run, let reason):
                    // Mark as blocked, needs user intervention
                    run.status = .blocked
                    run.driftDetectedAt = Date()
                    run.driftDetails = reason
                    synchronizeIdeaStatus(for: run)

                case .cannotResume(let run, let reason):
                    run.status = .failed
                    run.driftDetectedAt = Date()
                    run.driftDetails = reason
                    synchronizeIdeaStatus(for: run)
                }
            }
        } catch {
            // Log but don't crash
            ForgeLogger.execution.error("Resume interrupted runs failed: \(error.localizedDescription)")
        }
    }

    // MARK: - Approval Resolution

    /// Resolve a pending approval.
    func resolveApproval(approvalID: UUID, granted: Bool, comment: String? = nil) {
        rebuildPersistedPendingApprovals()
        guard let request = pendingApprovals[approvalID] else { return }
        let descriptor = FetchDescriptor<Run>()
        guard let run = (try? modelContext.fetch(descriptor))?.first(where: { $0.id == request.runID }) else { return }

        let priorApproval = run.approvals.first(where: { $0.id == approvalID || $0.stageID == request.stageID })
        let beforeDiagnostic = ApprovalResolutionDiagnostic(
            runID: run.id,
            workflowID: run.workflowID,
            approvalRequestID: approvalID,
            stageID: request.stageID,
            granted: granted,
            recordedAt: Date(),
            runStatusBefore: run.status.rawValue,
            runStatusAfter: run.status.rawValue,
            currentStageBefore: run.currentStageID,
            currentStageAfter: run.currentStageID,
            stageStatusBefore: run.stageExecutions.first(where: { $0.stageID == request.stageID })?.status.rawValue,
            stageStatusAfter: run.stageExecutions.first(where: { $0.stageID == request.stageID })?.status.rawValue,
            approvalBefore: priorApproval.map(Self.makeApprovalSnapshot),
            approvalAfter: priorApproval.map(Self.makeApprovalSnapshot)
        )

        // Remove from pending
        pendingApprovals.removeValue(forKey: approvalID)

        // Find the orchestrator and forward the resolution
        if let orchestrator = activeOrchestrators[request.runID] {
            orchestrator.resolveApproval(stageID: request.stageID, granted: granted, comment: comment)
            let updatedApproval = run.approvals.first(where: { $0.id == approvalID || $0.stageID == request.stageID })
            let diagnostic = ApprovalResolutionDiagnostic(
                runID: run.id,
                workflowID: run.workflowID,
                approvalRequestID: approvalID,
                stageID: request.stageID,
                granted: granted,
                recordedAt: Date(),
                runStatusBefore: beforeDiagnostic.runStatusBefore,
                runStatusAfter: run.status.rawValue,
                currentStageBefore: beforeDiagnostic.currentStageBefore,
                currentStageAfter: run.currentStageID,
                stageStatusBefore: beforeDiagnostic.stageStatusBefore,
                stageStatusAfter: run.stageExecutions.first(where: { $0.stageID == request.stageID })?.status.rawValue,
                approvalBefore: beforeDiagnostic.approvalBefore,
                approvalAfter: updatedApproval.map(Self.makeApprovalSnapshot)
            )
            persistApprovalResolution(diagnostic, for: run, approvalID: approvalID)
            refreshDockBadge()
            return
        }

        guard run.status == .waitingApproval else {
            refreshDockBadge()
            return
        }

        Task { @MainActor in
            do {
                let compiler = RunPlanCompiler(modelContext: modelContext)
                let (plan, workspace) = try compiler.rebuildPlanFromSnapshot(run: run)
                guard let orchestrator = prepareOrchestrator(run: run, plan: plan, workspace: workspace) else {
                    pendingApprovals[approvalID] = request
                    refreshDockBadge()
                    return
                }

                await orchestrator.start(from: request.stageID)
                pendingApprovals.removeValue(forKey: approvalID)
                orchestrator.resolveApproval(stageID: request.stageID, granted: granted, comment: comment)

                let updatedApproval = run.approvals.first(where: { $0.id == approvalID || $0.stageID == request.stageID })
                let diagnostic = ApprovalResolutionDiagnostic(
                    runID: run.id,
                    workflowID: run.workflowID,
                    approvalRequestID: approvalID,
                    stageID: request.stageID,
                    granted: granted,
                    recordedAt: Date(),
                    runStatusBefore: beforeDiagnostic.runStatusBefore,
                    runStatusAfter: run.status.rawValue,
                    currentStageBefore: beforeDiagnostic.currentStageBefore,
                    currentStageAfter: run.currentStageID,
                    stageStatusBefore: beforeDiagnostic.stageStatusBefore,
                    stageStatusAfter: run.stageExecutions.first(where: { $0.stageID == request.stageID })?.status.rawValue,
                    approvalBefore: beforeDiagnostic.approvalBefore,
                    approvalAfter: updatedApproval.map(Self.makeApprovalSnapshot)
                )
                persistApprovalResolution(diagnostic, for: run, approvalID: approvalID)
            } catch {
                pendingApprovals[approvalID] = request
                ForgeLogger.execution.error("Failed to resolve persisted approval \(approvalID): \(error.localizedDescription)")
            }
            refreshDockBadge()
        }
    }

    // MARK: - Cancel Run

    /// Cancel an active run using two-phase settlement (Proposal 011 — REQ-001, REQ-002).
    ///
    /// **Phase 1** (synchronous): `beginSettlement()` — agents cancelled, preliminary log written,
    /// `presentationStatus` returns `.cancelling`.
    ///
    /// **Session close** (async): Runtime sessions are closed with per-session timeouts.
    /// Outcomes are recorded as observed truth, not optimistic placeholders.
    ///
    /// **Phase 2** (synchronous): `finalizeSettlement()` — settlement log updated with real outcomes,
    /// `cancellationSettledAt` written, `run.status = .cancelled`.
    func cancelRun(runID: UUID) async {
        guard let run = fetchRun(id: runID) else { return }

        guard ![RunStatus.completed, .failed, .cancelled].contains(run.status) else { return }

        guard let orchestrator = activeOrchestrators[runID] else {
            let coordinator = RunCancellationCoordinator(run: run, orchestrator: nil)
            coordinator.beginSettlement()
            coordinator.finalizeSettlement(sessionOutcomes: [])

            pendingApprovals = pendingApprovals.filter { $0.value.runID != runID }
            synchronizeIdeaStatus(for: run)
            refreshDockBadge()
            return
        }

        // Phase 1: Synchronous settlement — agents cancelled, preliminary log written.
        let coordinator = RunCancellationCoordinator(
            run: run,
            orchestrator: orchestrator
        )
        coordinator.beginSettlement()

        // Collect session cleanup data before removing orchestrator reference.
        let sessionIDs = coordinator.pendingSessionIDs
        let executor = orchestrator.executor

        // Remove from active orchestrators and clean up approvals.
        activeOrchestrators.removeValue(forKey: runID)
        pendingApprovals = pendingApprovals.filter { $0.value.runID != runID }

        // Async session close — bounded per-session timeouts, outcomes recorded.
        let outcomes: [RunCancellationCoordinator.SessionCloseOutcome]
        if !sessionIDs.isEmpty {
            outcomes = await RunCancellationCoordinator.closeRuntimeSessionsWithOutcomes(
                sessionIDs: sessionIDs,
                executor: executor
            )
        } else {
            outcomes = []
        }

        // Phase 2: Finalize settlement with truthful session close outcomes.
        coordinator.finalizeSettlement(sessionOutcomes: outcomes)
        synchronizeIdeaStatus(for: orchestrator.run)
    }

    // MARK: - Query

    /// Whether any runs are active.
    var hasActiveRuns: Bool {
        return !activeOrchestrators.isEmpty
    }

    /// Number of pending approvals.
    var pendingApprovalCount: Int {
        pendingApprovals.count
    }

    /// P005-OPS §10: Number of blocked runs requiring attention.
    var blockedRunCount: Int {
        return activeOrchestrators.values.filter { $0.run.status == .blocked }.count
    }

    /// P005-OPS §10: Number of failed runs requiring attention.
    var failedRunCount: Int {
        return activeOrchestrators.values.filter { $0.run.status == .failed }.count
    }

    /// Get the orchestrator for a specific run.
    func orchestrator(for runID: UUID) -> WorkflowOrchestrator? {
        reconcileStalledOrchestratorsIfNeeded()
        return activeOrchestrators[runID]
    }

    /// Read-only lookup used by UI/projection paths that must not trigger
    /// reconcile side effects on the main thread.
    func peekOrchestrator(for runID: UUID) -> WorkflowOrchestrator? {
        activeOrchestrators[runID]
    }

    func registerTestingOrchestrator(_ orchestrator: WorkflowOrchestrator) {
        activeOrchestrators[orchestrator.run.id] = orchestrator
    }

    private func fetchRun(id: UUID) -> Run? {
        let descriptor = FetchDescriptor<Run>()
        return (try? modelContext.fetch(descriptor))?.first(where: { $0.id == id })
    }

    @discardableResult
    private func prepareOrchestrator(
        run: Run,
        plan: RunPlan,
        workspace: RunWorkspace
    ) -> WorkflowOrchestrator? {
        guard activeOrchestrators[run.id] == nil else { return activeOrchestrators[run.id] }
        guard !requiresLiveRuntimeConfiguration(for: plan) else {
            run.status = .blocked
            run.driftDetails = "Live runtime is not configured for workflow \(plan.workflowID)"
            return nil
        }

        let resolvedExecutor = executorForRun(plan: plan)
        let orchestratorCatalog = resolvedCatalog(for: plan)
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: resolvedExecutor,
            modelContext: modelContext,
            catalog: orchestratorCatalog
        )

        orchestrator.onApprovalRequest = { [weak self] request in
            self?.pendingApprovals[request.id] = request
            let stageLabel = run.stageExecutions.first(where: { $0.stageID == request.stageID })?.label ?? request.stageID
            self?.notificationService.notifyApprovalRequired(run: run, stageLabel: stageLabel)
            self?.emitReportIfNeeded(for: run)
            self?.refreshDockBadge()
        }

        orchestrator.onComplete = { [weak self] _ in
            self?.activeOrchestrators.removeValue(forKey: run.id)
            self?.pendingApprovals = self?.pendingApprovals.filter { $0.value.runID != run.id } ?? [:]
            self?.synchronizeIdeaStatus(for: run)
            
            // Proposal 018: Final audit trail update
            orchestrator.updateSessionAuditTrailOnCompletion()
            
            do {
                try self?.modelContext.save()
            } catch {
                ForgeLogger.execution.error("Failed to persist run completion state: \(error.localizedDescription)")
            }
            self?.emitReportIfNeeded(for: run)

            if let modelContext = self?.modelContext {
                self?.recordBenchmarkExecutionIfNeeded(run: run, modelContext: modelContext)
            }

            self?.fireCompletionNotification(for: run)

            if run.status == .completed {
                self?.notifyRunCompleted()
            }
        }

        activeOrchestrators[run.id] = orchestrator
        return orchestrator
    }

    private func resolvedCatalog(for plan: RunPlan) -> AgentCatalog? {
        if let catalog {
            return catalog
        }
        return try? JSONDecoder().decode(AgentCatalog.self, from: plan.catalogSnapshotJSON)
    }

    private var hasACPBackedRuntimeConfiguration: Bool {
        guard let catalog else { return false }
        let hasRuntimeProfiles = !catalog.runtimeProfiles.isEmpty
        let hasACPBackends = catalog.backendProfiles.values.contains { profile in
            guard let runtimeProfileID = profile.runtimeProfile,
                  let runtimeProfile = catalog.runtimeProfiles[runtimeProfileID] else {
                return false
            }
            return runtimeProfile.transportKind == "acp_stdio"
        }
        let hasConfiguredProviders = !(providerRegistry?.configuredProviders.isEmpty ?? true)
        return hasRuntimeProfiles && hasACPBackends && hasConfiguredProviders
    }

    var supportsLiveExecution: Bool {
        if forceUITestLiveRuntimeUnavailable { return true }
        return liveRuntimeConfiguration != nil || hasACPBackedRuntimeConfiguration
    }

    private func synchronizeIdeaStatus(for run: Run) {
        guard let idea = run.idea else { return }
        idea.synchronizePersistedStatusFromRuns()
        do {
            try modelContext.save()
        } catch {
            ForgeLogger.execution.error(
                "Failed to persist synchronized idea status for run \(run.id): \(error.localizedDescription)"
            )
        }
    }

    private func startMaintenanceLoop() {
        maintenanceTask?.cancel()
        maintenanceTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: Self.maintenanceTickInterval)
                guard !Task.isCancelled else { return }
                guard let self else { return }
                self.runMaintenanceTick()
            }
        }
    }

    private func prepareRunForLiveAttachment(_ run: Run) {
        guard run.status != .waitingApproval else {
            do {
                try modelContext.save()
            } catch {
                ForgeLogger.execution.error(
                    "Failed to persist waiting-approval live attachment for run \(run.id): \(error.localizedDescription)"
                )
            }
            emitLatestSummarySnapshot(for: run)
            return
        }

        run.status = .running
        run.completedAt = nil

        if let details = run.driftDetails,
           details.localizedCaseInsensitiveContains("execution stalled after") {
            run.driftDetails = nil
        }

        do {
            try modelContext.save()
        } catch {
            ForgeLogger.execution.error(
                "Failed to persist active run status for run \(run.id): \(error.localizedDescription)"
            )
        }

        emitLatestSummarySnapshot(for: run)
    }

    func rebuildPersistedPendingApprovals() {
        let runDescriptor = FetchDescriptor<Run>()
        let artifactDescriptor = FetchDescriptor<Artifact>()

        let allRuns = (try? modelContext.fetch(runDescriptor)) ?? []
        let allArtifacts = (try? modelContext.fetch(artifactDescriptor)) ?? []
        let artifactNamesByRunID = Dictionary(grouping: allArtifacts, by: \.runID)
            .mapValues { artifacts in
                Array(Set(artifacts.map(\.name))).sorted()
            }

        var rebuilt: [UUID: ApprovalRequest] = [:]
        for run in allRuns where run.status == .waitingApproval {
            guard let stage = run.stageExecutions
                .filter({ $0.status == .waitingApproval })
                .sorted(by: Self.compareStageExecutions)
                .last else {
                continue
            }

            guard let approval = run.approvals
                .filter({ $0.stageID == stage.stageID && $0.decision == .requested })
                .sorted(by: { $0.requestedAt < $1.requestedAt })
                .last else {
                continue
            }

            rebuilt[approval.id] = ApprovalRequest(
                id: approval.id,
                runID: run.id,
                stageID: stage.stageID,
                stageLabel: stage.label,
                precedingArtifacts: artifactNamesByRunID[run.id] ?? [],
                requestedAt: approval.requestedAt,
                approvalPolicy: nil
            )
        }

        pendingApprovals = rebuilt.merging(pendingApprovals) { persisted, active in
            active
        }
        refreshDockBadge()
    }

    private func reconcileStalledOrchestratorsIfNeeded(now: Date = Date()) {
        reconcileInvocationCountForTesting += 1
        let stalledRunIDs = activeOrchestrators.compactMap { runID, orchestrator -> UUID? in
            let hasPendingApproval = pendingApprovals.values.contains { $0.runID == runID }
            let latestEventEntry = Self.latestMeaningfulTimelineEntry(in: orchestrator.liveTimeline)
            let stalledStage = stalledStage(for: orchestrator.run)
            let hasRunningAgents = Self.hasLiveRunningAgents(in: orchestrator.liveTimeline)
            let hasSettledParallelFanout = Self.hasSettledParallelFanout(stage: stalledStage)
            let hasStartedStageWithoutAgentWork = Self.hasStartedStageWithoutAgentWork(
                run: orchestrator.run,
                stage: stalledStage
            )
            let hasLaterLiveActivity = latestEventEntry.map {
                Self.hasLaterLiveActivity(in: orchestrator.liveTimeline, after: $0)
            } ?? false

            guard Self.shouldReconcileStalledRun(
                run: orchestrator.run,
                hasPendingApproval: hasPendingApproval,
                hasRunningAgents: hasRunningAgents,
                hasSettledParallelFanout: hasSettledParallelFanout,
                hasStartedStageWithoutAgentWork: hasStartedStageWithoutAgentWork,
                stalledStageStatus: stalledStage?.status,
                stalledStageStartedAt: stalledStage?.startedAt,
                latestLiveEvent: latestEventEntry?.event,
                hasLaterLiveActivityAfterLatestEvent: hasLaterLiveActivity,
                now: now
            ) else {
                return nil
            }

            return runID
        }

        for runID in stalledRunIDs {
            reconcileStalledRun(runID: runID, now: now)
        }
    }

    private func reconcileStalledRun(runID: UUID, now: Date) {
        guard let orchestrator = activeOrchestrators.removeValue(forKey: runID) else { return }

        let run = orchestrator.run
        let stage = stalledStage(for: run)

        if let stage, stage.status == .running || stage.status == .ready || stage.status == .completed {
            stage.status = .blocked
            stage.completedAt = stage.completedAt ?? now

            for agentExecution in stage.agentExecutions where [.pending, .ready, .running].contains(agentExecution.status) {
                agentExecution.status = .failed
                agentExecution.completedAt = agentExecution.completedAt ?? now
                agentExecution.settledAt = agentExecution.settledAt ?? now
                agentExecution.canonicalOutcome = agentExecution.canonicalOutcome ?? .failedBeforeOutput
                agentExecution.transportErrorKind = agentExecution.transportErrorKind ?? .unknown
                agentExecution.providerStopReason = agentExecution.providerStopReason ?? "session_closed_without_transition"
                agentExecution.logSnippet = mergedStalledExecutionLog(existing: agentExecution.logSnippet)
            }
        }

        run.status = .blocked
        if run.driftDetails?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty != false {
            run.driftDetails = "Execution stalled after the last session closed before the workflow could transition. Resume the run to continue from the current stage."
        }

        pendingApprovals = pendingApprovals.filter { $0.value.runID != runID }
        synchronizeIdeaStatus(for: run)
        orchestrator.updateSessionAuditTrailOnCompletion()
        emitReportIfNeeded(for: run)
        fireCompletionNotification(for: run)
        do {
            try modelContext.save()
        } catch {
            ForgeLogger.execution.error(
                "Failed to persist stalled-run reconciliation for run \(runID): \(error.localizedDescription)"
            )
        }
    }

    private func stalledStage(for run: Run) -> StageExecution? {
        let sorted = run.stageExecutions.sorted { lhs, rhs in
            Self.compareStageExecutions(lhs, rhs)
        }

        if let currentStageID = run.currentStageID,
           let match = sorted.last(where: { $0.stageID == currentStageID }) {
            return match
        }

        return sorted.last
    }

    private static func compareStageExecutions(_ lhs: StageExecution, _ rhs: StageExecution) -> Bool {
        if lhs.startedAt == rhs.startedAt {
            if lhs.iteration == rhs.iteration {
                return lhs.attemptNumber < rhs.attemptNumber
            }
            return lhs.iteration < rhs.iteration
        }
        return lhs.startedAt < rhs.startedAt
    }

    static func shouldReconcileStalledRun(
        run: Run,
        hasPendingApproval: Bool,
        hasRunningAgents: Bool,
        hasSettledParallelFanout: Bool = false,
        hasStartedStageWithoutAgentWork: Bool = false,
        stalledStageStatus: StageStatus?,
        stalledStageStartedAt: Date?,
        latestLiveEvent: ExecutionEvent?,
        hasLaterLiveActivityAfterLatestEvent: Bool = false,
        now: Date,
        graceInterval: TimeInterval = 30,
        runningAgentsGraceInterval: TimeInterval = 300
    ) -> Bool {
        guard run.status == .running || run.status == .ready || run.status == .pending else {
            return false
        }
        guard !hasPendingApproval else { return false }
        guard let latestLiveEvent else { return false }
        guard latestLiveEvent.type == .sessionClosed else { return false }
        guard !hasLaterLiveActivityAfterLatestEvent else { return false }

        // Proposal 032: If the durable cursor shows the transition was settled
        // (next state scheduled but no agent work started), this run has a clean
        // resumable continuation and must NOT be reconciled into blocked/failed.
        if let cursor = run.transitionCursor, cursor.settlementPhase == .transitionSettled {
            return false
        }

        if stalledStageStatus == .completed || stalledStageStatus == .skipped {
            return false
        }

        let extendedTransitionGraceRequired =
            hasRunningAgents || hasSettledParallelFanout || hasStartedStageWithoutAgentWork
        let effectiveGraceInterval = extendedTransitionGraceRequired
            ? max(graceInterval, runningAgentsGraceInterval)
            : graceInterval

        // If a new stage has already started after the last meaningful live event,
        // treat that stage start as the new stall baseline instead of immediately
        // reconciling off a stale sessionClosed from the previous stage.
        if let stalledStageStartedAt, stalledStageStartedAt > latestLiveEvent.timestamp {
            return now.timeIntervalSince(stalledStageStartedAt) >= effectiveGraceInterval
        }

        if extendedTransitionGraceRequired {
            let stallBaseline = max(latestLiveEvent.timestamp, stalledStageStartedAt ?? .distantPast)
            return now.timeIntervalSince(stallBaseline) >= effectiveGraceInterval
        }
        return now.timeIntervalSince(latestLiveEvent.timestamp) >= graceInterval
    }

    static func shouldReconcileStalledRun(
        run: Run,
        hasPendingApproval: Bool,
        hasRunningAgents: Bool,
        stalledStageStatus: StageStatus?,
        latestLiveEvent: ExecutionEvent?,
        hasLaterLiveActivityAfterLatestEvent: Bool = false,
        now: Date,
        graceInterval: TimeInterval = 30
    ) -> Bool {
        shouldReconcileStalledRun(
            run: run,
            hasPendingApproval: hasPendingApproval,
            hasRunningAgents: hasRunningAgents,
            hasSettledParallelFanout: false,
            hasStartedStageWithoutAgentWork: false,
            stalledStageStatus: stalledStageStatus,
            stalledStageStartedAt: nil,
            latestLiveEvent: latestLiveEvent,
            hasLaterLiveActivityAfterLatestEvent: hasLaterLiveActivityAfterLatestEvent,
            now: now,
            graceInterval: graceInterval
        )
    }

    private static func hasSettledParallelFanout(stage: StageExecution?) -> Bool {
        guard let stage else { return false }
        let agentExecutions = stage.agentExecutions
        guard agentExecutions.count > 1 else { return false }
        return agentExecutions.allSatisfy { execution in
            switch execution.status {
            case .completed, .failed, .cancelled, .skipped:
                return true
            case .pending, .ready, .running:
                return false
            }
        }
    }

    private static func hasStartedStageWithoutAgentWork(run: Run, stage: StageExecution?) -> Bool {
        guard
            let stage,
            let cursor = run.transitionCursor,
            cursor.settlementPhase == .transitionStarted,
            cursor.nextScheduledStateID == stage.stageID
        else {
            return false
        }

        guard stage.status == .running || stage.status == .ready else {
            return false
        }

        return stage.agentExecutions.isEmpty
    }

    private static func latestMeaningfulTimelineEntry(in timeline: [LiveExecutionTimelineEntry]) -> LiveExecutionTimelineEntry? {
        timeline.reversed().first(where: { $0.event.type != .textChunk }) ?? timeline.last
    }

    private static func hasLaterLiveActivity(
        in timeline: [LiveExecutionTimelineEntry],
        after latestEntry: LiveExecutionTimelineEntry
    ) -> Bool {
        timeline.contains { entry in
            entry.event.timestamp > latestEntry.event.timestamp
        }
    }

    private static func hasLiveRunningAgents(in timeline: [LiveExecutionTimelineEntry]) -> Bool {
        let latestEventByAgentID = Dictionary(
            grouping: timeline,
            by: \.agentID
        ).compactMapValues { entries in
            entries.max(by: { lhs, rhs in
                if lhs.event.timestamp != rhs.event.timestamp {
                    return lhs.event.timestamp < rhs.event.timestamp
                }
                return lhs.id.uuidString < rhs.id.uuidString
            })
        }

        return latestEventByAgentID.values.contains { entry in
            entry.event.type != .sessionClosed
        }
    }

    private func mergedStalledExecutionLog(existing: String?) -> String {
        let note = "Execution stalled after the runtime session closed before the workflow could transition. Resume required."
        guard let existing, !existing.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return note
        }
        if existing.contains(note) { return existing }
        return existing + "\n" + note
    }

    var liveRuntimeReadiness: LiveRuntimeReadiness {
        if forceUITestLiveRuntimeUnavailable {
            return .unavailable(
                reason: "Live runtime is unavailable for this proof lane.",
                recovery: "Enable a runtime backend or the fixture backend to unlock live workflows."
            )
        }

        if let liveRuntimeConfiguration {
            return .ready(
                summary: liveRuntimeConfiguration.summary,
                source: liveRuntimeConfiguration.sourceDescription
            )
        }

        if hasACPBackedRuntimeConfiguration {
            return .ready(
                summary: "ACP-backed agent-defined runtime",
                source: "Agent catalog"
            )
        }

        return .unavailable(
            reason: "Live runtime is unavailable",
            recovery: "Configure at least one ACP provider in Settings or enable the fixture backend, then relaunch the app. Advanced setup: CHAINWORKS_FIXTURE_MODE=proposal_loop_success."
        )
    }

    func isLiveWorkflow(_ workflowID: String) -> Bool {
        workflowID == "proposal_loop_live" || workflowID == "full_mvp_live"
    }

    /// Proposal 007: Whether a workflow is a repo-backed delivery workflow.
    func isDeliveryWorkflow(_ workflowID: String) -> Bool {
        workflowID == "full_mvp_live"
    }

    private func requiresLiveRuntimeConfiguration(for plan: RunPlan) -> Bool {
        // Proposal 026: ACP-profiled runs launch their own subprocess —
        // they do NOT require a legacy runtime server to be configured.
        if planHasACPRuntime(plan) { return false }
        return isLiveWorkflow(plan.workflowID) && liveRuntimeConfiguration == nil
    }

    /// Proposal 026: Whether the plan contains any agent with a non-nil ACP runtime profile.
    private func planHasACPRuntime(_ plan: RunPlan) -> Bool {
        plan.agentBindings.values.contains { $0.runtimeProfileID != nil }
    }

    private func executorForRun(plan: RunPlan) -> AgentExecutor {
        guard isLiveWorkflow(plan.workflowID) else {
            return executor
        }

        // Proposal 026: Per-agent transport via factory.
        // ACP transports are cached per adapter family. Fixture transport used for non-network modes.
        let fixtureTransport: (any RuntimeTransportProtocol)?
        if let liveRuntimeConfiguration {
            fixtureTransport = resolveFixtureTransport(liveRuntimeConfiguration)
        } else {
            fixtureTransport = nil
        }

        let factory = DefaultRuntimeTransportFactory(fixtureTransport: fixtureTransport)
        let sessionManager = AgentSessionManager(container: modelContext.container)
        return RuntimeAgentExecutor(
            transportFactory: factory,
            override: liveRuntimeConfiguration?.override,
            sessionManager: sessionManager
        )
    }

    /// Resolve fixture transport from live runtime configuration.
    private func resolveFixtureTransport(_ config: LiveRuntimeConfiguration) -> any RuntimeTransportProtocol {
        switch config.transportMode {
        case .fixtureProposalLoopSuccess:
            return FixtureACPTransport(scenario: .proposalLoopSuccess)
        case .fixtureProposal022FeedbackCycle:
            return FixtureACPTransport(scenario: .proposal022FeedbackCycle)
        case .fixtureProposal013AggregateFailure:
            return FixtureACPTransport(scenario: .proposal013AggregateFailure)
        case .fixtureFullMVPSuccess:
            return FixtureACPTransport(scenario: .fullMVPSuccess)
        }
    }

    // MARK: - Steward V1 Trigger Mechanism (Proposal 003)

    /// Manual trigger entry point for Steward analysis.
    func runStewardAnalysis() async {
        guard let config = stewardConfig else { return }
        let service = StewardAnalysisService(
            modelContext: modelContext,
            stewardConfig: config,
            executor: executor,
            catalog: catalog
        )
        do {
            _ = try await service.runAnalysis()
        } catch {
            ForgeLogger.steward.error("Steward analysis failed: \(error.localizedDescription)")
        }
    }

    /// Called after a run completes. Increments counter and optionally triggers Steward.
    func notifyRunCompleted() {
        completedRunsSinceLastAnalysis += 1

        // REQ-008: Config-change trigger takes priority.
        if configChangeAnalysisScheduled {
            configChangeAnalysisScheduled = false
            completedRunsSinceLastAnalysis = 0
            Task { @MainActor in
                await runStewardAnalysis()
            }
            return
        }

        // Post-run hook trigger.
        guard let config = stewardConfig,
              config.triggers.postRunHook.enabled,
              completedRunsSinceLastAnalysis >= config.triggers.postRunHook.runInterval else {
            return
        }
        completedRunsSinceLastAnalysis = 0
        Task { @MainActor in
            await runStewardAnalysis()
        }
    }

    // MARK: - Config-Change Trigger (Proposal 003 — REQ-008)

    /// Check if configuration has changed since the last Steward analysis.
    /// If so, schedule an analysis after the next completed run.
    ///
    /// Compares current `stewardConfigSnapshotHash` and `workflowCatalogSnapshotHash`
    /// against the most recent `StewardAnalysis` record. A change to any of the three
    /// config surfaces (agents.yaml, workflow.yaml, steward_config.yaml) triggers a
    /// follow-up analysis.
    func checkForConfigChange() {
        guard let config = stewardConfig, config.triggers.onConfigChange.enabled else { return }

        // Compute current hashes
        let currentStewardHash = (try? DefinitionHasher.hash(config).sha256) ?? "unknown"
        let currentCatalogHash = catalog.flatMap { try? DefinitionHasher.hash($0).sha256 } ?? "no-catalog"

        // Fetch the most recent analysis
        var descriptor = FetchDescriptor<StewardAnalysis>(
            sortBy: [SortDescriptor(\.createdAt, order: .reverse)]
        )
        descriptor.fetchLimit = 1

        guard let lastAnalysis = (try? modelContext.fetch(descriptor))?.first else {
            // No previous analysis exists — schedule one after the next completed run.
            configChangeAnalysisScheduled = true
            ForgeLogger.steward.info("No previous analysis found. Scheduling config-change analysis.")
            return
        }

        if lastAnalysis.stewardConfigSnapshotHash != currentStewardHash
            || lastAnalysis.workflowCatalogSnapshotHash != currentCatalogHash {
            configChangeAnalysisScheduled = true
            ForgeLogger.steward.info("Config change detected (steward: \(lastAnalysis.stewardConfigSnapshotHash != currentStewardHash), catalog: \(lastAnalysis.workflowCatalogSnapshotHash != currentCatalogHash)). Scheduling analysis after next completed run.")
        }
    }

    // MARK: - P005-OPS Report & Notification Hooks

    /// Emit an immutable report if the run is at a stable checkpoint (§6.3).
    private func emitReportIfNeeded(for run: Run) {
        if reportBuilder == nil {
            reportBuilder = RunReportBuilder(modelContext: modelContext)
        }
        guard let builder = reportBuilder, builder.shouldEmitReport(for: run) else { return }
        do {
            _ = try builder.emitReport(for: run)
        } catch {
            ForgeLogger.execution.error("Report emission failed: \(error.localizedDescription)")
        }
    }

    private func emitLatestSummarySnapshot(for run: Run) {
        if reportBuilder == nil {
            reportBuilder = RunReportBuilder(modelContext: modelContext)
        }
        guard let builder = reportBuilder else { return }

        do {
            try builder.emitLatestSummarySnapshot(for: run)
        } catch {
            ForgeLogger.execution.error(
                "Failed to emit latest run summary snapshot for run \(run.id): \(error.localizedDescription)"
            )
        }
    }

    // MARK: - Proposal 008 (REQ-006): Benchmark Run Recording

    /// If the completed run is linked to a benchmark cohort, record its execution
    /// into the benchmark subsystem using `BenchmarkRunRecorder`.
    private func recordBenchmarkExecutionIfNeeded(run: Run, modelContext: ModelContext) {
        guard run.experimentCohortID != nil else { return }
        guard [RunStatus.completed, .failed].contains(run.status) else { return }

        // Find the benchmark pair linked to this run via cohort membership.
        let pairDescriptor = FetchDescriptor<BenchmarkPair>()
        guard let allPairs = try? modelContext.fetch(pairDescriptor) else { return }
        guard let pair = allPairs.first(where: { pair in
            pair.appDrivenRecord == nil && pair.cohort?.id == run.experimentCohortID
        }) else { return }

        let recorder = BenchmarkRunRecorder(modelContext: modelContext)
        do {
            try recorder.recordAppDrivenExecution(pair: pair, run: run)
            ForgeLogger.execution.info("Benchmark execution recorded for run \(run.id.uuidString.prefix(8))")
        } catch {
            ForgeLogger.execution.error("Benchmark recording failed: \(error.localizedDescription)")
        }
    }

    /// Fire the appropriate notification for a run status change (§10).
    private func fireCompletionNotification(for run: Run) {
        switch run.status {
        case .completed:
            notificationService.notifyRunCompleted(run: run)
        case .failed:
            notificationService.notifyRunFailed(run: run)
        case .blocked:
            notificationService.notifyRunBlocked(run: run, reason: run.driftDetails ?? "Unknown")
        default:
            break
        }
        refreshDockBadge()
    }

    /// Refresh dock badge based on current run states (§10).
    func refreshDockBadge() {
        let descriptor = FetchDescriptor<Run>()
        let allRuns = (try? modelContext.fetch(descriptor)) ?? []
        let waitingApproval = allRuns.filter { $0.status == .waitingApproval }.count
        let blocked = allRuns.filter { $0.status == .blocked }.count
        notificationService.updateDockBadge(waitingApprovalCount: waitingApproval, blockedCount: blocked)
    }

    private static func makeApprovalSnapshot(_ approval: Approval) -> ApprovalResolutionDiagnostic.ApprovalSnapshot {
        ApprovalResolutionDiagnostic.ApprovalSnapshot(
            approvalID: approval.id,
            stageID: approval.stageID,
            decision: approval.decision.rawValue,
            requestedAt: approval.requestedAt,
            decidedAt: approval.decidedAt,
            comment: approval.comment
        )
    }

    private func persistApprovalResolutionDiagnostic(
        _ diagnostic: ApprovalResolutionDiagnostic,
        for run: Run
    ) throws {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(diagnostic)

        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        let timestamp = formatter.string(from: diagnostic.recordedAt).replacingOccurrences(of: ":", with: "")
        let name = "approval_resolution_diagnostic_\(timestamp).json"

        let stageExecutions = run.stageExecutions.filter { $0.stageID == diagnostic.stageID }
        let latestStage = stageExecutions.sorted {
            if $0.iteration == $1.iteration {
                if $0.attemptNumber == $1.attemptNumber {
                    return $0.startedAt < $1.startedAt
                }
                return $0.attemptNumber < $1.attemptNumber
            }
            return $0.iteration < $1.iteration
        }.last

        let workspace = RunWorkspace(
            runID: run.id,
            workspaceRoot: URL(fileURLWithPath: run.workspaceRoot, isDirectory: true),
            artifactRoot: URL(fileURLWithPath: run.artifactRoot, isDirectory: true),
            worktreeRoot: run.worktreeRoot.flatMap { URL(fileURLWithPath: $0, isDirectory: true) }
        )

        _ = try ArtifactManager(modelContext: modelContext).persistSystemArtifact(
            name: name,
            data: data,
            contractID: "approval_resolution_diagnostic_v1",
            format: .json,
            workspace: workspace,
            stageID: diagnostic.stageID,
            iteration: latestStage?.iteration ?? 1,
            agentID: "system_approval",
            provider: "system",
            model: nil,
            effort: nil,
            attemptNumber: latestStage?.attemptNumber ?? 1
        )
    }

    private func persistApprovalResolution(
        _ diagnostic: ApprovalResolutionDiagnostic,
        for run: Run,
        approvalID: UUID
    ) {
        do {
            try persistApprovalResolutionDiagnostic(diagnostic, for: run)
        } catch {
            ForgeLogger.execution.error(
                "Failed to persist approval resolution diagnostic for approval \(approvalID): \(error.localizedDescription)"
            )
        }

        do {
            try modelContext.save()
        } catch {
            ForgeLogger.execution.error(
                "Failed to persist approval resolution state for approval \(approvalID): \(error.localizedDescription)"
            )
        }
    }
}

extension ExecutionService: ExecutionTerminationControlling {}

// MARK: - DefaultRuntimeTransportFactory (Proposal 026 — per-agent transport resolution)

/// Resolves the correct transport for each agent based on adapter family from its provider binding.
/// Transports are cached by adapter family — max one instance per family per run.
/// Fixture transport is shared (created once). ACP transports are created on demand and cached.
final class DefaultRuntimeTransportFactory: RuntimeTransportFactory, @unchecked Sendable {
    let fixtureTransport: (any RuntimeTransportProtocol)?
    private let helperProcessJanitor: any RuntimeHelperProcessJanitorProtocol
    private let lock = NSLock()
    private var transportsByFamily: [String: any RuntimeTransportProtocol] = [:]

    init(
        fixtureTransport: (any RuntimeTransportProtocol)?,
        helperProcessJanitor: any RuntimeHelperProcessJanitorProtocol = RuntimeHelperProcessJanitor.live
    ) {
        self.fixtureTransport = fixtureTransport
        self.helperProcessJanitor = helperProcessJanitor
    }

    func transport(for agent: ResolvedAgent, binding: ResolvedProviderBinding?) throws -> any RuntimeTransportProtocol {
        let family = binding?.adapterFamily ?? ""
        guard !family.isEmpty else {
            guard let fixtureTransport else {
                throw RuntimeTransportError.sessionCreationFailed(reason: "Fixture transport required but not configured. Agent '\(agent.id)' has no adapter family and liveRuntimeConfiguration is absent.")
            }
            return fixtureTransport
        }

        lock.lock()
        defer { lock.unlock() }
        if let existing = transportsByFamily[family] { return existing }

        helperProcessJanitor.sweepStaleHelpers()

        let created: any RuntimeTransportProtocol
        switch family {
        case "claude_agent_acp":
            print("[TransportFactory] Creating ClaudeAgentACPTransport for family '\(family)'")
            created = ClaudeAgentACPTransport()
        case "gemini_cli_acp":
            print("[TransportFactory] Creating GeminiCLIACPTransport for family '\(family)'")
            created = GeminiCLIACPTransport()
        case "codex_acp":
            print("[TransportFactory] Creating CodexACPTransport for family '\(family)'")
            created = CodexACPTransport()
        case "auggie_cli_acp":
            print("[TransportFactory] Creating AuggieCLIACPTransport for family '\(family)'")
            created = AuggieCLIACPTransport()
        case "junie_cli_acp":
            print("[TransportFactory] Creating JunieCLIACPTransport for family '\(family)'")
            created = JunieCLIACPTransport()
        default:
            throw RuntimeTransportError.unknownAdapterFamily(family)
        }
        transportsByFamily[family] = created
        return created
    }
}

extension DefaultRuntimeTransportFactory: RuntimeTransportFactoryTerminationControlling {
    func terminateActiveTransportsForAppShutdown() {
        lock.lock()
        let transports = Array(transportsByFamily.values)
        lock.unlock()

        for transport in transports {
            (transport as? RuntimeTransportTerminationControlling)?.terminateActiveSessionsForAppShutdown()
        }
    }
}
