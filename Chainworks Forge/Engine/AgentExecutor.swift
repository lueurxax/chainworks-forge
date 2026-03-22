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
