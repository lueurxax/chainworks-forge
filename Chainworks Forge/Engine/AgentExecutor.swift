import Foundation

// MARK: - AgentExecutor Protocol (ARCH-030)

/// Protocol for agent execution backends.
/// Executors return [String: Data], NOT file URLs — ArtifactManager is the sole disk writer (ARCH-030).
protocol AgentExecutor: Sendable {
    /// Execute an agent task and return output artifacts as in-memory data.
    /// - Parameters:
    ///   - task: The agent task to execute.
    ///   - agent: The resolved agent definition.
    ///   - context: Execution context with workspace and input artifacts.
    /// - Returns: An AgentResult containing output data and metadata.
    func execute(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext
    ) async throws -> AgentResult
}

// MARK: - ExecutionContext

/// Context provided to an agent executor for a single task execution.
struct ExecutionContext: Sendable {
    /// The run's workspace (paths, isolation boundaries).
    let workspace: RunWorkspace
    /// Preferred read-only project root for repo-backed runs before a writable worktree exists.
    /// When present, agents should read source from here instead of from the ephemeral run workspace.
    let projectRoot: URL?
    /// The current stage ID.
    let stageID: String
    /// The current iteration within this stage (for loop support).
    let iteration: Int
    /// The attempt number for this execution.
    let attemptNumber: Int
    /// Input artifacts available to this agent (name -> data).
    let inputArtifacts: [String: Data]
    /// Runtime variables (for prompt interpolation, etc.).
    let variables: [String: AnyCodableValue]
    /// The idea body text for the run.
    let ideaBody: String
    /// Resolved provider binding frozen at run start.
    let providerBinding: ResolvedProviderBinding?

    init(
        workspace: RunWorkspace,
        projectRoot: URL? = nil,
        stageID: String,
        iteration: Int,
        attemptNumber: Int,
        inputArtifacts: [String: Data],
        variables: [String: AnyCodableValue],
        ideaBody: String,
        providerBinding: ResolvedProviderBinding?
    ) {
        self.workspace = workspace
        self.projectRoot = projectRoot
        self.stageID = stageID
        self.iteration = iteration
        self.attemptNumber = attemptNumber
        self.inputArtifacts = inputArtifacts
        self.variables = variables
        self.ideaBody = ideaBody
        self.providerBinding = providerBinding
    }
}

// MARK: - AgentResult

/// Result of an agent execution.
/// Contains in-memory artifact data (ARCH-030: executor returns Data, not file URLs).
struct AgentResult: Sendable {
    /// Output artifacts produced by the agent. Key: artifact name, Value: raw data.
    let outputs: [String: Data]
    /// Optional log snippet for display.
    let logSnippet: String?
    /// Estimated cost in cents (for cost tracking).
    let costCents: Int64?
    /// Whether execution succeeded.
    let succeeded: Bool
    /// Error message if execution failed.
    let errorMessage: String?
    /// Provider session identifier (§6.1).
    let sessionID: String?
    /// Wall-clock duration of execution in seconds (§6.1).
    let durationSeconds: Double
    /// Normalized provider receipt for operator/debug surfaces.
    let providerReceipt: ProviderExecutionReceipt?
    /// Resolved model actually used for execution.
    let resolvedModel: String?
    /// Configured provider that satisfied the binding.
    let configuredProviderID: UUID?
    /// Adapter version used to execute.
    let adapterVersion: String?
}

// MARK: - Output Contract Resolution

enum OutputContractResolver {
    static func expectedOutputs(for task: AgentTask, agent: ResolvedAgent) -> [String] {
        task.outputs ?? agent.outputs
    }

    static func contractID(
        for outputName: String,
        agent: ResolvedAgent,
        catalog: AgentCatalog?
    ) -> String? {
        switch outputName {
        case "proposal_review_po", "proposal_review_ux", "proposal_review_ui", "proposal_review_architect":
            return "proposal_review_v1"
        case "proposal_review_summary":
            return "proposal_review_summary_v1"
        case "prepush_review_report":
            return "prepush_review_v1"
        case "final_feature_report":
            return "final_feature_report_v1"
        default:
            break
        }

        guard let catalog else { return nil }
        if catalog.contracts[outputName] != nil {
            return outputName
        }
        let versioned = "\(outputName)_v1"
        if catalog.contracts[versioned] != nil {
            return versioned
        }

        if let explicit = agent.outputContract,
           explicitContract(explicit, matches: outputName) {
            return explicit
        }

        return nil
    }

    static func contract(
        for outputName: String,
        agent: ResolvedAgent,
        catalog: AgentCatalog?
    ) -> ArtifactContract? {
        guard let catalog,
              let contractID = contractID(for: outputName, agent: agent, catalog: catalog) else {
            return nil
        }
        return catalog.contracts[contractID]
    }

    private static func explicitContract(_ contractID: String, matches outputName: String) -> Bool {
        guard let stem = contractID.range(of: #"_v\d+$"#, options: .regularExpression).map({
            String(contractID[..<$0.lowerBound])
        }) else {
            return contractID == outputName
        }
        return stem == outputName
    }
}

// MARK: - Execution Errors

enum ExecutionError: Error, LocalizedError {
    case agentFailed(agentID: String, message: String)
    case timeout(agentID: String, seconds: Int)
    case cancelled(agentID: String)
    case outputContractViolation(agentID: String, contractID: String, details: String)

    var errorDescription: String? {
        switch self {
        case .agentFailed(let agentID, let message):
            return "Agent '\(agentID)' failed: \(message)"
        case .timeout(let agentID, let seconds):
            return "Agent '\(agentID)' timed out after \(seconds)s"
        case .cancelled(let agentID):
            return "Agent '\(agentID)' was cancelled"
        case .outputContractViolation(let agentID, let contractID, let details):
            return "Agent '\(agentID)' output contract '\(contractID)' violation: \(details)"
        }
    }
}
