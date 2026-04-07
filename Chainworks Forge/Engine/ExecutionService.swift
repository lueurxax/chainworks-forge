import Foundation
import SwiftData
import Observation

// MARK: - GooseTransportAPI (Proposal 005, Section 5.5)

/// Selects which transport protocol implementation to use for live execution.
enum GooseTransportAPI: String, Codable, Sendable {
    /// Original bespoke /api/sessions contract (never implemented server-side).
    case bespoke
    /// Real goosed /agent/start + /reply contract (Proposal 005).
    case gooseServer = "goose_server"
}

enum LiveTransportMode: Sendable {
    case network
    case fixtureProposalLoopSuccess
    case fixtureProposal022FeedbackCycle
    case fixtureProposal013AggregateFailure
    case fixtureFullMVPSuccess
}

struct LiveRuntimeConfiguration: Sendable {
    let baseURL: URL
    let apiKey: String?
    let override: LiveExecutionOverride?
    let transportMode: LiveTransportMode
    /// Proposal 005: which transport API to use for network mode.
    /// Defaults to `.gooseServer` when `CHAINWORKS_GOOSE_BASE_URL` is set.
    let transportAPI: GooseTransportAPI

    var summary: String {
        if let override {
            return "\(override.provider) / \(override.model) / \(override.effort)"
        }
        return "agent-defined provider/model"
    }

    var sourceDescription: String {
        switch transportMode {
        case .network:
            switch transportAPI {
            case .bespoke:
                return "Goose backend (bespoke)"
            case .gooseServer:
                return "Goose server (goosed)"
            }
        case .fixtureProposalLoopSuccess:
            return "Fixture backend"
        case .fixtureProposal022FeedbackCycle:
            return "Fixture backend"
        case .fixtureProposal013AggregateFailure:
            return "Fixture backend"
        case .fixtureFullMVPSuccess:
            return "Fixture backend"
        }
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

    /// UI-test-only override used to force the unavailable recovery lane while
    /// keeping the live entrypoint visible for owner-path proofs.
    private let forceUITestLiveRuntimeUnavailable: Bool

    /// Optional managed Goose server bridge.
    let gooseServerManager: GooseServerManager?

    /// Steward config (loaded at app init, nil if not present).
    var stewardConfig: StewardConfig?

    /// P005-OPS §10: Notification service.
    let notificationService: NotificationService

    /// P005-OPS §6: Report builder.
    private(set) var reportBuilder: RunReportBuilder?

    /// Counter for post-run hook trigger.
    private var completedRunsSinceLastAnalysis: Int = 0

    /// REQ-008: Flag set when a config change is detected and analysis should run after next completed run.
    private var configChangeAnalysisScheduled: Bool = false

    init(
        modelContext: ModelContext,
        executor: AgentExecutor,
        catalog: AgentCatalog? = nil,
        stewardConfig: StewardConfig? = nil,
        liveRuntimeConfiguration: LiveRuntimeConfiguration? = nil,
        gooseServerManager: GooseServerManager? = nil,
        notificationService: NotificationService? = nil
    ) {
        self.modelContext = modelContext
        self.executor = executor
        self.catalog = catalog
        self.stewardConfig = stewardConfig
        self.fixedLiveRuntimeConfiguration = liveRuntimeConfiguration
        self.forceUITestLiveRuntimeUnavailable =
            ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_FORCE_LIVE_RUNTIME_UNAVAILABLE"] == "1"
        self.gooseServerManager = gooseServerManager
        self.notificationService = notificationService ?? MainActor.assumeIsolated {
            NotificationService()
        }
    }

    var liveRuntimeConfiguration: LiveRuntimeConfiguration? {
        fixedLiveRuntimeConfiguration ?? gooseServerManager?.liveRuntimeConfiguration
    }

    // MARK: - Start Run

    /// Start a new workflow run. Creates an orchestrator and begins execution.
    func startRun(
        run: Run,
        plan: RunPlan,
        workspace: RunWorkspace
    ) {
        guard let orchestrator = prepareOrchestrator(run: run, plan: plan, workspace: workspace) else { return }
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

        let resumeStateID = run.currentStageID
        ForgeLogger.execution.info("Starting orchestrator from state=\(resumeStateID ?? "nil") for run \(run.id)")
        synchronizeIdeaStatus(for: run)

        Task { @MainActor in
            await orchestrator.start(from: resumeStateID)
        }
    }

    // MARK: - Resume Interrupted Runs

    /// Resume all interrupted runs on app launch (ARCH-029).
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

                    let resumeStateID = run.currentStageID
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
            try? persistApprovalResolutionDiagnostic(diagnostic, for: run)
            try? modelContext.save()
            refreshDockBadge()
        }
    }

    // MARK: - Cancel Run

    /// Cancel an active run using two-phase settlement (Proposal 011 — REQ-001, REQ-002).
    ///
    /// **Phase 1** (synchronous): `beginSettlement()` — agents cancelled, preliminary log written,
    /// `presentationStatus` returns `.cancelling`.
    ///
    /// **Session close** (async): Goose sessions are closed with per-session timeouts.
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
            outcomes = await RunCancellationCoordinator.closeGooseSessionsWithOutcomes(
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
        reconcileStalledOrchestratorsIfNeeded()
        return !activeOrchestrators.isEmpty
    }

    /// Number of pending approvals.
    var pendingApprovalCount: Int {
        pendingApprovals.count
    }

    /// P005-OPS §10: Number of blocked runs requiring attention.
    var blockedRunCount: Int {
        reconcileStalledOrchestratorsIfNeeded()
        return activeOrchestrators.values.filter { $0.run.status == .blocked }.count
    }

    /// P005-OPS §10: Number of failed runs requiring attention.
    var failedRunCount: Int {
        reconcileStalledOrchestratorsIfNeeded()
        return activeOrchestrators.values.filter { $0.run.status == .failed }.count
    }

    /// Get the orchestrator for a specific run.
    func orchestrator(for runID: UUID) -> WorkflowOrchestrator? {
        reconcileStalledOrchestratorsIfNeeded()
        return activeOrchestrators[runID]
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
        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: resolvedExecutor,
            modelContext: modelContext,
            catalog: catalog
        )

        orchestrator.onApprovalRequest = { [weak self] request in
            self?.pendingApprovals[request.id] = request
            let stageLabel = run.stageExecutions.first(where: { $0.stageID == request.stageID })?.label ?? request.stageID
            self?.notificationService.notifyApprovalRequired(run: run, stageLabel: stageLabel)
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

    var supportsLiveExecution: Bool {
        forceUITestLiveRuntimeUnavailable || liveRuntimeConfiguration != nil
    }

    private func synchronizeIdeaStatus(for run: Run) {
        guard let idea = run.idea else { return }
        idea.synchronizePersistedStatusFromRuns()
        try? modelContext.save()
    }

    private func reconcileStalledOrchestratorsIfNeeded(now: Date = Date()) {
        let stalledRunIDs = activeOrchestrators.compactMap { runID, orchestrator -> UUID? in
            let hasPendingApproval = pendingApprovals.values.contains { $0.runID == runID }
            let latestEvent = Self.latestMeaningfulEvent(in: orchestrator.liveTimeline)
            let hasRunningAgents = orchestrator.run.stageExecutions
                .flatMap(\.agentExecutions)
                .contains { $0.status == .running }

            guard Self.shouldReconcileStalledRun(
                run: orchestrator.run,
                hasPendingApproval: hasPendingApproval,
                hasRunningAgents: hasRunningAgents,
                latestLiveEvent: latestEvent,
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
        try? modelContext.save()
    }

    private func stalledStage(for run: Run) -> StageExecution? {
        let sorted = run.stageExecutions.sorted { lhs, rhs in
            if lhs.startedAt == rhs.startedAt {
                if lhs.iteration == rhs.iteration {
                    return lhs.attemptNumber < rhs.attemptNumber
                }
                return lhs.iteration < rhs.iteration
            }
            return lhs.startedAt < rhs.startedAt
        }

        if let currentStageID = run.currentStageID,
           let match = sorted.last(where: { $0.stageID == currentStageID }) {
            return match
        }

        return sorted.last
    }

    static func shouldReconcileStalledRun(
        run: Run,
        hasPendingApproval: Bool,
        hasRunningAgents: Bool,
        latestLiveEvent: ExecutionEvent?,
        now: Date,
        graceInterval: TimeInterval = 30
    ) -> Bool {
        guard run.status == .running || run.status == .ready || run.status == .pending else {
            return false
        }
        guard !hasPendingApproval else { return false }
        guard !hasRunningAgents else { return false }
        guard let latestLiveEvent else { return false }
        guard latestLiveEvent.type == .sessionClosed else { return false }
        return now.timeIntervalSince(latestLiveEvent.timestamp) >= graceInterval
    }

    private static func latestMeaningfulEvent(in timeline: [LiveExecutionTimelineEntry]) -> ExecutionEvent? {
        timeline.reversed().first(where: { $0.event.type != .textChunk })?.event ?? timeline.last?.event
    }

    var liveRuntimeReadiness: LiveRuntimeReadiness {
        if forceUITestLiveRuntimeUnavailable {
            return .unavailable(
                reason: "Live runtime is unavailable for this proof lane.",
                recovery: "Connect a Goose backend or enable the fixture backend to unlock live workflows."
            )
        }

        if let liveRuntimeConfiguration,
           liveRuntimeConfiguration.transportMode != .network {
            return .ready(
                summary: liveRuntimeConfiguration.summary,
                source: liveRuntimeConfiguration.sourceDescription
            )
        }

        if let gooseServerManager, let liveRuntimeConfiguration {
            switch gooseServerManager.launchState {
            case .running, .external:
                return .ready(
                    summary: liveRuntimeConfiguration.summary,
                    source: liveRuntimeConfiguration.sourceDescription
                )
            case .starting:
                return .unavailable(
                    reason: "Managed Goose server is still starting",
                    recovery: "Wait for the managed Goose server to finish booting, or refresh server status in Provider Settings."
                )
            case .failed(let reason):
                return .unavailable(
                    reason: "Managed Goose server is unavailable",
                    recovery: reason
                )
            case .idle:
                return .unavailable(
                    reason: "Managed Goose server is not running",
                    recovery: "Start the managed Goose server in Provider Settings or First Run Setup."
                )
            }
        }

        if let liveRuntimeConfiguration {
            return .ready(
                summary: liveRuntimeConfiguration.summary,
                source: liveRuntimeConfiguration.sourceDescription
            )
        }

        return .unavailable(
            reason: "Live runtime is unavailable",
            recovery: "Connect a Goose backend or enable the fixture backend, then relaunch the app. Advanced setup: CHAINWORKS_GOOSE_BASE_URL or CHAINWORKS_GOOSE_FIXTURE_MODE=proposal_loop_success."
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
        // they do NOT require a Goose server to be configured.
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
        // Goose transport shared for all Goose agents; ACP transports cached per adapter family.
        let gooseTransport: (any RuntimeTransportProtocol)?
        if let liveRuntimeConfiguration {
            gooseTransport = resolveGooseTransport(liveRuntimeConfiguration)
        } else {
            gooseTransport = nil // ACP-only runs don't need Goose
        }

        let factory = DefaultRuntimeTransportFactory(gooseTransport: gooseTransport)
        let sessionManager = AgentSessionManager(container: modelContext.container)
        return RuntimeAgentExecutor(
            transportFactory: factory,
            override: liveRuntimeConfiguration?.override,
            sessionManager: sessionManager
        )
    }

    /// Proposal 026: Resolve Goose-backed transport from live runtime configuration.
    private func resolveGooseTransport(_ config: LiveRuntimeConfiguration) -> any RuntimeTransportProtocol {
        switch config.transportMode {
        case .network:
            switch config.transportAPI {
            case .bespoke:
                return GooseTransport(
                    baseURL: config.baseURL,
                    apiKey: config.apiKey
                )
            case .gooseServer:
                return GooseServerTransport(
                    baseURL: config.baseURL,
                    secretKey: config.apiKey,
                    provider: config.override?.provider,
                    model: config.override?.model
                )
            }
        case .fixtureProposalLoopSuccess:
            return FixtureGooseTransport(scenario: .proposalLoopSuccess)
        case .fixtureProposal022FeedbackCycle:
            return FixtureGooseTransport(scenario: .proposal022FeedbackCycle)
        case .fixtureProposal013AggregateFailure:
            return FixtureGooseTransport(scenario: .proposal013AggregateFailure)
        case .fixtureFullMVPSuccess:
            return FixtureGooseTransport(scenario: .fullMVPSuccess)
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
}

// MARK: - DefaultRuntimeTransportFactory (Proposal 026 — per-agent transport resolution)

/// Resolves the correct transport for each agent based on adapter family from its provider binding.
/// Transports are cached by adapter family — max one instance per family per run.
/// Goose transport is shared (created once). ACP transports are created on demand and cached.
final class DefaultRuntimeTransportFactory: RuntimeTransportFactory, @unchecked Sendable {
    let gooseTransport: (any RuntimeTransportProtocol)?
    private let lock = NSLock()
    private var transportsByFamily: [String: any RuntimeTransportProtocol] = [:]

    init(gooseTransport: (any RuntimeTransportProtocol)?) {
        self.gooseTransport = gooseTransport
    }

    func transport(for agent: ResolvedAgent, binding: ResolvedProviderBinding?) -> any RuntimeTransportProtocol {
        let family = binding?.adapterFamily ?? "goose"
        guard family != "goose" && !family.isEmpty else {
            guard let gooseTransport else {
                fatalError("[DefaultRuntimeTransportFactory] Goose transport required but not configured. Agent '\(agent.id)' needs Goose but liveRuntimeConfiguration is absent.")
            }
            return gooseTransport
        }

        lock.lock()
        defer { lock.unlock() }
        if let existing = transportsByFamily[family] { return existing }

        let created: any RuntimeTransportProtocol
        switch family {
        case "claude_agent_acp":
            print("[TransportFactory] Creating ClaudeAgentACPTransport for family '\(family)'")
            created = ClaudeAgentACPTransport()
        case "gemini_cli_acp":
            print("[TransportFactory] Creating GeminiCLIACPTransport for family '\(family)'")
            created = GeminiCLIACPTransport()
        default:
            print("[TransportFactory] Unknown adapter family '\(family)', falling back to Goose")
            guard let gooseTransport else {
                fatalError("[DefaultRuntimeTransportFactory] Goose fallback required but not configured for unknown family '\(family)'.")
            }
            return gooseTransport
        }
        transportsByFamily[family] = created
        return created
    }
}
