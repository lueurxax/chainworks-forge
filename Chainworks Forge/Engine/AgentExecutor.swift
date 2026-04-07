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
    /// Proposal 018: The stage lineage ID for this execution.
    let stageLineageID: String?
    /// Proposal 018: The execution ID (agent attempt) that owns this session.
    let ownerExecutionLineageID: UUID
    /// The current iteration within this stage (for loop support).
    let iteration: Int
    /// The attempt number for this execution.
    let attemptNumber: Int
    /// Input artifacts available to this agent (name -> data).
    let inputArtifacts: [String: Data]
    /// Optional absolute file paths for input artifacts when persisted artifacts already exist on disk.
    let inputArtifactPaths: [String: String]
    /// Runtime variables (for prompt interpolation, etc.).
    let variables: [String: AnyCodableValue]
    /// The idea body text for the run.
    let ideaBody: String
    /// Optional absolute path to a file attached to the idea (e.g. a prior proposal).
    let ideaAttachmentPath: String?
    /// Resolved provider binding frozen at run start.
    let providerBinding: ResolvedProviderBinding?
    /// Optional contract catalog for catalog-driven prompt/runtime hints.
    let catalog: AgentCatalog?
    /// Frozen strategy profile ID selected for the run.
    let contextStrategyProfileID: String?
    /// How the strategy was assigned for this run.
    let strategyAssignmentMode: String?
    /// Frozen strategy profile used for this execution context.
    let contextStrategyProfile: ContextStrategyProfile?
    /// Strategy-compiled context handoff payload for this execution.
    let handoffPacket: HandoffPacket?
    
    /// Proposal 018: Optional session lineage ID to force reuse or inspect.
    var sessionLineageID: UUID?

    init(
        workspace: RunWorkspace,
        projectRoot: URL? = nil,
        stageID: String,
        stageLineageID: String? = nil,
        ownerExecutionLineageID: UUID,
        iteration: Int,
        attemptNumber: Int,
        inputArtifacts: [String: Data],
        inputArtifactPaths: [String: String] = [:],
        variables: [String: AnyCodableValue],
        ideaBody: String,
        ideaAttachmentPath: String? = nil,
        providerBinding: ResolvedProviderBinding?,
        catalog: AgentCatalog? = nil,
        contextStrategyProfileID: String? = nil,
        strategyAssignmentMode: String? = nil,
        contextStrategyProfile: ContextStrategyProfile? = nil,
        handoffPacket: HandoffPacket? = nil,
        sessionLineageID: UUID? = nil
    ) {
        self.workspace = workspace
        self.projectRoot = projectRoot
        self.stageID = stageID
        self.stageLineageID = stageLineageID
        self.ownerExecutionLineageID = ownerExecutionLineageID
        self.iteration = iteration
        self.attemptNumber = attemptNumber
        self.inputArtifacts = inputArtifacts
        self.inputArtifactPaths = inputArtifactPaths
        self.variables = variables
        self.ideaBody = ideaBody
        self.ideaAttachmentPath = ideaAttachmentPath
        self.providerBinding = providerBinding
        self.catalog = catalog
        self.contextStrategyProfileID = contextStrategyProfileID
        self.strategyAssignmentMode = strategyAssignmentMode
        self.contextStrategyProfile = contextStrategyProfile
        self.handoffPacket = handoffPacket
        self.sessionLineageID = sessionLineageID
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
    /// Canonical terminal outcome for this attempt.
    let canonicalOutcome: AgentCanonicalOutcome?
    /// Proposal 018: Session lineage results
    let sessionLineageID: UUID?
    let sessionGenerationID: UUID?
    let sessionReuseDisposition: SessionReuseDisposition?
    /// Proposal 018: Optional checkpoint data if session was invalidated/compacted
    let sessionCheckpoint: AgentSessionCheckpoint?
    /// Normalized transport failure class, if any.
    let transportErrorKind: TransportErrorKind?
    /// Normalized provider/app stop reason, if any.
    let providerStopReason: String?
    /// Whether durable output existed before settlement.
    let outputPresence: OutputPresence
    /// Actual runtime provider identity when known.
    let runtimeProvider: String?
    /// Actual runtime model identity when known.
    let runtimeModel: String?
    /// Resolved MCP profile used for this execution attempt.
    let mcpProfileID: String?
    /// Requested conceptual MCP extensions for this execution attempt.
    let requestedMCPExtensions: [String]
    /// Effective runtime extension IDs expected/enabled for this execution attempt.
    let effectiveMCPRuntimeExtensionIDs: [String]
    /// Requested conceptual MCP extensions denied during resolution/reconciliation.
    let deniedMCPExtensions: [String]
    /// Measured latency for fresh session startup plus MCP reconciliation.
    let mcpSessionStartupLatencyMilliseconds: Int?
    /// Per-server MCP usage measured during this execution.
    let mcpServerMetrics: [MCPServerExecutionMetric]
    /// Raw accumulated text from the agent's stream (for error classification when errors arrive as text, not transport failures).
    let accumulatedText: String?
    /// Supporting diagnostic envelope for later readers.
    let outcomeEnvelope: OutcomeEnvelope?
    /// Unique lazy-evidence artifacts actually fetched during execution.
    let lazyEvidenceArtifactHits: [String]

    init(
        outputs: [String: Data],
        logSnippet: String?,
        costCents: Int64?,
        succeeded: Bool,
        errorMessage: String?,
        sessionID: String?,
        durationSeconds: Double,
        providerReceipt: ProviderExecutionReceipt?,
        resolvedModel: String?,
        configuredProviderID: UUID?,
        adapterVersion: String?,
        canonicalOutcome: AgentCanonicalOutcome? = nil,
        sessionLineageID: UUID? = nil,
        sessionGenerationID: UUID? = nil,
        sessionReuseDisposition: SessionReuseDisposition? = nil,
        sessionCheckpoint: AgentSessionCheckpoint? = nil,
        transportErrorKind: TransportErrorKind? = nil,
        providerStopReason: String? = nil,
        outputPresence: OutputPresence = .none,
        runtimeProvider: String? = nil,
        runtimeModel: String? = nil,
        mcpProfileID: String? = nil,
        requestedMCPExtensions: [String] = [],
        effectiveMCPRuntimeExtensionIDs: [String] = [],
        deniedMCPExtensions: [String] = [],
        mcpSessionStartupLatencyMilliseconds: Int? = nil,
        mcpServerMetrics: [MCPServerExecutionMetric] = [],
        accumulatedText: String? = nil,
        outcomeEnvelope: OutcomeEnvelope? = nil,
        lazyEvidenceArtifactHits: [String] = []
    ) {
        self.outputs = outputs
        self.logSnippet = logSnippet
        self.costCents = costCents
        self.succeeded = succeeded
        self.errorMessage = errorMessage
        self.sessionID = sessionID
        self.durationSeconds = durationSeconds
        self.providerReceipt = providerReceipt
        self.resolvedModel = resolvedModel
        self.configuredProviderID = configuredProviderID
        self.adapterVersion = adapterVersion
        self.canonicalOutcome = canonicalOutcome
        self.sessionLineageID = sessionLineageID
        self.sessionGenerationID = sessionGenerationID
        self.sessionReuseDisposition = sessionReuseDisposition
        self.sessionCheckpoint = sessionCheckpoint
        self.transportErrorKind = transportErrorKind
        self.providerStopReason = providerStopReason
        self.outputPresence = outputPresence
        self.runtimeProvider = runtimeProvider
        self.runtimeModel = runtimeModel
        self.mcpProfileID = mcpProfileID
        self.requestedMCPExtensions = requestedMCPExtensions
        self.effectiveMCPRuntimeExtensionIDs = effectiveMCPRuntimeExtensionIDs
        self.deniedMCPExtensions = deniedMCPExtensions
        self.mcpSessionStartupLatencyMilliseconds = mcpSessionStartupLatencyMilliseconds
        self.mcpServerMetrics = mcpServerMetrics
        self.accumulatedText = accumulatedText
        self.outcomeEnvelope = outcomeEnvelope
        self.lazyEvidenceArtifactHits = lazyEvidenceArtifactHits
    }
}

struct MCPServerExecutionMetric: Codable, Sendable, Equatable {
    let serverID: String
    let toolCallCount: Int
    let requestBytes: Int64
    let responseBytes: Int64
    let promptContextDeltaBytes: Int64
}

// MARK: - Output Contract Resolution

enum OutputContractResolver {
    static func expectedOutputs(for task: AgentTask, agent: ResolvedAgent) -> [String] {
        OutputContractResolverV2.expectedOutputs(for: task, agent: agent)
    }

    static func contractID(
        for outputName: String,
        agent: ResolvedAgent,
        catalog: AgentCatalog?
    ) -> String? {
        OutputContractResolverV2.resolveContractID(
            for: outputName,
            agent: agent,
            catalog: catalog
        )
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
