import Foundation
import SwiftData
import Observation

enum LiveTransportMode: Sendable {
    case network
    case fixtureProposalLoopSuccess
}

struct LiveRuntimeConfiguration: Sendable {
    let baseURL: URL
    let apiKey: String?
    let override: LiveExecutionOverride?
    let transportMode: LiveTransportMode

    var summary: String {
        if let override {
            return "\(override.provider) / \(override.model) / \(override.effort)"
        }
        return "agent-defined provider/model"
    }

    var sourceDescription: String {
        switch transportMode {
        case .network:
            return "Goose backend"
        case .fixtureProposalLoopSuccess:
            return "Fixture backend"
        }
    }
}

enum LiveRuntimeReadiness: Sendable {
    case ready(summary: String, source: String)
    case unavailable(reason: String, recovery: String)
}

// MARK: - ExecutionService (app-scoped @Observable singleton — ARCH-022)

/// App-scoped service managing all active workflow orchestrators.
/// NOT a per-run singleton — maintains a collection of orchestrators (ARCH-028).
@MainActor
@Observable
final class ExecutionService {
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

    /// Optional live runtime configuration for Proposal 004 runs.
    let liveRuntimeConfiguration: LiveRuntimeConfiguration?

    /// Steward config (loaded at app init, nil if not present).
    var stewardConfig: StewardConfig?

    /// Counter for post-run hook trigger.
    private var completedRunsSinceLastAnalysis: Int = 0

    /// REQ-008: Flag set when a config change is detected and analysis should run after next completed run.
    private var configChangeAnalysisScheduled: Bool = false

    init(
        modelContext: ModelContext,
        executor: AgentExecutor,
        catalog: AgentCatalog? = nil,
        stewardConfig: StewardConfig? = nil,
        liveRuntimeConfiguration: LiveRuntimeConfiguration? = nil
    ) {
        self.modelContext = modelContext
        self.executor = executor
        self.catalog = catalog
        self.stewardConfig = stewardConfig
        self.liveRuntimeConfiguration = liveRuntimeConfiguration
    }

    // MARK: - Start Run

    /// Start a new workflow run. Creates an orchestrator and begins execution.
    func startRun(
        run: Run,
        plan: RunPlan,
        workspace: RunWorkspace
    ) {
        guard activeOrchestrators[run.id] == nil else { return }
        guard !requiresLiveRuntimeConfiguration(for: plan) else {
            run.status = .blocked
            run.driftDetails = "Live runtime is not configured for workflow \(plan.workflowID)"
            return
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

        // Wire up approval callback
        orchestrator.onApprovalRequest = { [weak self] request in
            self?.pendingApprovals[request.id] = request
        }

        // Wire up completion callback
        orchestrator.onComplete = { [weak self] _ in
            self?.activeOrchestrators.removeValue(forKey: run.id)
            // Clean up any pending approvals for this run
            self?.pendingApprovals = self?.pendingApprovals.filter { $0.value.runID != run.id } ?? [:]
            // Steward V1: post-run hook trigger
            if run.status == .completed {
                self?.notifyRunCompleted()
            }
        }

        activeOrchestrators[run.id] = orchestrator

        Task { @MainActor in
            await orchestrator.start()
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
                    guard !requiresLiveRuntimeConfiguration(for: plan) else {
                        run.status = .blocked
                        run.driftDetails = "Live runtime is not configured for workflow \(plan.workflowID)"
                        continue
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
                    }

                    orchestrator.onComplete = { [weak self] _ in
                        self?.activeOrchestrators.removeValue(forKey: run.id)
                        self?.pendingApprovals = self?.pendingApprovals.filter { $0.value.runID != run.id } ?? [:]
                    }

                    activeOrchestrators[run.id] = orchestrator

                    let resumeStateID = run.currentStageID
                    Task { @MainActor in
                        await orchestrator.start(from: resumeStateID)
                    }

                case .needsDecision(let run, let reason):
                    // Mark as blocked, needs user intervention
                    run.status = .blocked
                    run.driftDetectedAt = Date()
                    run.driftDetails = reason

                case .cannotResume(let run, let reason):
                    run.status = .failed
                    run.driftDetectedAt = Date()
                    run.driftDetails = reason
                }
            }
        } catch {
            // Log but don't crash
            print("Resume failed: \(error.localizedDescription)")
        }
    }

    // MARK: - Approval Resolution

    /// Resolve a pending approval.
    func resolveApproval(approvalID: UUID, granted: Bool, comment: String? = nil) {
        guard let request = pendingApprovals[approvalID] else { return }

        // Remove from pending
        pendingApprovals.removeValue(forKey: approvalID)

        // Find the orchestrator and forward the resolution
        if let orchestrator = activeOrchestrators[request.runID] {
            orchestrator.resolveApproval(stageID: request.stageID, granted: granted, comment: comment)
        }
    }

    // MARK: - Cancel Run

    /// Cancel an active run.
    func cancelRun(runID: UUID) {
        if let orchestrator = activeOrchestrators[runID] {
            orchestrator.cancel()
            activeOrchestrators.removeValue(forKey: runID)
            pendingApprovals = pendingApprovals.filter { $0.value.runID != runID }
        }
    }

    // MARK: - Query

    /// Whether any runs are active.
    var hasActiveRuns: Bool {
        !activeOrchestrators.isEmpty
    }

    /// Number of pending approvals.
    var pendingApprovalCount: Int {
        pendingApprovals.count
    }

    /// Get the orchestrator for a specific run.
    func orchestrator(for runID: UUID) -> WorkflowOrchestrator? {
        activeOrchestrators[runID]
    }

    var supportsLiveExecution: Bool {
        liveRuntimeConfiguration != nil
    }

    var liveRuntimeReadiness: LiveRuntimeReadiness {
        if let liveRuntimeConfiguration {
            return .ready(
                summary: liveRuntimeConfiguration.summary,
                source: liveRuntimeConfiguration.sourceDescription
            )
        }

        return .unavailable(
            reason: "Live runtime is unavailable",
            recovery: "Set CHAINWORKS_GOOSE_BASE_URL or CHAINWORKS_GOOSE_FIXTURE_MODE=proposal_loop_success before launching the app."
        )
    }

    func isLiveWorkflow(_ workflowID: String) -> Bool {
        workflowID == "proposal_loop_live"
    }

    private func requiresLiveRuntimeConfiguration(for plan: RunPlan) -> Bool {
        isLiveWorkflow(plan.workflowID) && liveRuntimeConfiguration == nil
    }

    private func executorForRun(plan: RunPlan) -> AgentExecutor {
        guard isLiveWorkflow(plan.workflowID), let liveRuntimeConfiguration else {
            return executor
        }

        let transport: GooseTransport
        switch liveRuntimeConfiguration.transportMode {
        case .network:
            transport = GooseTransport(
                baseURL: liveRuntimeConfiguration.baseURL,
                apiKey: liveRuntimeConfiguration.apiKey
            )
        case .fixtureProposalLoopSuccess:
            transport = FixtureGooseTransport(
                scenario: .proposalLoopSuccess,
                baseURL: liveRuntimeConfiguration.baseURL
            )
        }

        return GooseAgentExecutor(
            transport: transport,
            override: liveRuntimeConfiguration.override
        )
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
            print("Steward analysis failed: \(error.localizedDescription)")
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
            print("[Steward] No previous analysis found. Scheduling config-change analysis.")
            return
        }

        if lastAnalysis.stewardConfigSnapshotHash != currentStewardHash
            || lastAnalysis.workflowCatalogSnapshotHash != currentCatalogHash {
            configChangeAnalysisScheduled = true
            print("[Steward] Config change detected (steward: \(lastAnalysis.stewardConfigSnapshotHash != currentStewardHash), catalog: \(lastAnalysis.workflowCatalogSnapshotHash != currentCatalogHash)). Scheduling analysis after next completed run.")
        }
    }
}
