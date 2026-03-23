import Foundation
import SwiftData
import Observation

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

    init(modelContext: ModelContext, executor: AgentExecutor, catalog: AgentCatalog? = nil) {
        self.modelContext = modelContext
        self.executor = executor
        self.catalog = catalog
    }

    // MARK: - Start Run

    /// Start a new workflow run. Creates an orchestrator and begins execution.
    func startRun(
        run: Run,
        plan: RunPlan,
        workspace: RunWorkspace
    ) {
        guard activeOrchestrators[run.id] == nil else { return }

        let orchestrator = WorkflowOrchestrator(
            run: run,
            plan: plan,
            workspace: workspace,
            executor: executor,
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
                    let orchestrator = WorkflowOrchestrator(
                        run: run,
                        plan: plan,
                        workspace: workspace,
                        executor: executor,
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
            print("Resume failed: \(error.localizedDescription)")
        }
    }

    // MARK: - Approval Resolution

    /// Resolve a pending approval.
    func resolveApproval(approvalID: UUID, granted: Bool, comment: String? = nil) {
        guard let request = pendingApprovals[approvalID] else { return }

        pendingApprovals.removeValue(forKey: approvalID)

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

    var hasActiveRuns: Bool {
        !activeOrchestrators.isEmpty
    }

    var pendingApprovalCount: Int {
        pendingApprovals.count
    }

    func orchestrator(for runID: UUID) -> WorkflowOrchestrator? {
        activeOrchestrators[runID]
    }
}
