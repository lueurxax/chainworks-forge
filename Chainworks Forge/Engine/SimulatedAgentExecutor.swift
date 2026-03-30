import Foundation

// MARK: - SimulatedAgentExecutor

/// Deterministic mock executor that produces structurally valid outputs
/// for all agent tasks using OutputContractTemplates.
/// Used for testing the orchestrator without real LLM backends.
final class SimulatedAgentExecutor: AgentExecutor, @unchecked Sendable {
    /// Simulated delay per task in seconds. Set to 0 for instant execution in tests.
    let simulatedDelay: TimeInterval

    /// Optional catalog for contract-aware output generation.
    let catalog: AgentCatalog?

    /// Track executed tasks for test assertions.
    private let _lock = NSLock()
    private var _executedTasks: [(agentID: String, task: String, stageID: String)] = []

    /// Inject failures: agent IDs in this set will fail execution.
    var failingAgentIDs: Set<String> = []

    var executedTasks: [(agentID: String, task: String, stageID: String)] {
        _lock.lock()
        defer { _lock.unlock() }
        return _executedTasks
    }

    init(simulatedDelay: TimeInterval = 0, catalog: AgentCatalog? = nil) {
        self.simulatedDelay = simulatedDelay
        self.catalog = catalog
    }

    func execute(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext
    ) async throws -> AgentResult {
        // Proposal 013: V2 resolver — catalog-driven contract resolution
        let expectedOutputs = OutputContractResolverV2.expectedOutputs(for: task, agent: agent)

        // Record execution
        _lock.lock()
        _executedTasks.append((agentID: agent.id, task: task.task, stageID: context.stageID))
        _lock.unlock()

        // Simulated delay
        if simulatedDelay > 0 {
            try await Task.sleep(nanoseconds: UInt64(simulatedDelay * 1_000_000_000))
        }

        // Check for injected failures
        if failingAgentIDs.contains(agent.id) {
            let binding = context.providerBinding
            return AgentResult(
                outputs: [:],
                logSnippet: "Simulated failure for agent '\(agent.id)'",
                costCents: 0,
                succeeded: false,
                errorMessage: "Simulated failure for agent '\(agent.id)'",
                sessionID: nil,
                durationSeconds: simulatedDelay,
                providerReceipt: UsageReceiptNormalizer.makeReceipt(
                    providerFamily: binding?.providerFamily ?? agent.provider,
                    configuredProviderID: binding?.configuredProviderID,
                    model: binding?.model ?? agent.model,
                    effort: binding?.effort ?? agent.effort,
                    transport: binding?.transport ?? "simulated",
                    costCents: 0,
                    durationSeconds: simulatedDelay
                ),
                resolvedModel: binding?.model ?? agent.model,
                configuredProviderID: binding?.configuredProviderID,
                adapterVersion: binding?.adapterVersion
            )
        }

        // Generate outputs based on agent's declared outputs
        var outputs: [String: Data] = [:]

        for outputName in expectedOutputs {
            let (data, _) = OutputContractTemplates.generateForOutput(
                outputName: outputName,
                agent: agent,
                stageID: context.stageID,
                catalog: catalog
            )
            outputs[outputName] = data
        }

        // If no declared outputs, generate a default markdown artifact
        if outputs.isEmpty {
            let defaultOutput = OutputContractTemplates.generate(
                contractID: agent.outputContract ?? "default",
                agentID: agent.id,
                stageID: context.stageID
            )
            outputs["\(agent.id)_output"] = defaultOutput.data
        }

        let binding = context.providerBinding
        return AgentResult(
            outputs: outputs,
            logSnippet: "Simulated execution of '\(agent.id)' for task '\(task.task)' completed successfully",
            costCents: 100,  // §6.2: default 100 cents per execution
            succeeded: true,
            errorMessage: nil,
            sessionID: "sim-\(UUID().uuidString.prefix(8))",
            durationSeconds: simulatedDelay,
            providerReceipt: UsageReceiptNormalizer.makeReceipt(
                providerFamily: binding?.providerFamily ?? agent.provider,
                configuredProviderID: binding?.configuredProviderID,
                model: binding?.model ?? agent.model,
                effort: binding?.effort ?? agent.effort,
                transport: binding?.transport ?? "simulated",
                costCents: 100,
                durationSeconds: simulatedDelay
            ),
            resolvedModel: binding?.model ?? agent.model,
            configuredProviderID: binding?.configuredProviderID,
            adapterVersion: binding?.adapterVersion
        )
    }

    /// Reset tracking state (for test setup).
    func reset() {
        _lock.lock()
        _executedTasks.removeAll()
        failingAgentIDs.removeAll()
        _lock.unlock()
    }
}
