import Foundation

// MARK: - RunPlan (compiled execution topology, no run-scoped identity — ARCH-024)

/// Compiled execution topology from YAML. Immutable. No run-scoped identity.
/// The orchestrator receives a (Run, RunPlan, RunWorkspace) tuple — identity lives on Run.
struct RunPlan: Sendable {
    let workflowID: String
    let workflowTitle: String

    /// Resolved state machine: state ID -> ExecutableState
    let states: [String: ExecutableState]
    let initialStateID: String

    /// Resolved agent catalog bindings: agent ID -> ResolvedAgent
    let agentBindings: [String: ResolvedAgent]

    /// Variables from workflow definition (frozen at compile time)
    let variables: [String: AnyCodableValue]

    /// Scoring configuration
    let scoring: ScoringConfig?

    /// Failure policy
    let failurePolicy: FailurePolicy?

    /// Provenance
    let workflowSnapshotHash: String
    let catalogSnapshotHash: String
    let workflowSnapshotJSON: Data
    let catalogSnapshotJSON: Data

    /// Compiler version — monotonic integer incremented when compiler semantics change.
    /// Persisted on Run for resume safety (ARCH-029).
    let planCompilerVersion: Int

    /// Current compiler version constant.
    static let currentCompilerVersion: Int = 1
}

// MARK: - RunWorkspace (run-scoped isolation boundary — ARCH-025)

/// Run-scoped workspace context. Created at createRun() time,
/// frozen in the Run's persisted state. Passed to the orchestrator and executor.
struct RunWorkspace: Sendable {
    let runID: UUID
    /// Isolation boundary (from workspace-isolation-risk.md).
    let workspaceRoot: URL
    /// {workspaceRoot}/artifacts/ — already run-scoped, no extra runID nesting (ARCH-026).
    let artifactRoot: URL
    /// Set by Proposal 003 when worktrees are provisioned.
    let worktreeRoot: URL?
}

// MARK: - ExecutableState

struct ExecutableState: Sendable {
    let id: String
    let label: String
    let type: StateType?
    let ownerAgentID: String
    let runBlock: ExecutableRunBlock?
    let runAfterApproval: ExecutableRunBlock?
    let transitions: [ExecutableTransition]
    let approvalRequired: Bool
    let loop: ResolvedLoopConfig?
}

enum StateType: String, Sendable {
    case start
    case end
    case manualGate = "manual_gate"
}

/// Loop config with max resolved at compile time from vars.*.
struct ResolvedLoopConfig: Sendable {
    let counter: String
    let resolvedMax: Int
}

// MARK: - ExecutableRunBlock

struct ExecutableRunBlock: Sendable {
    let phases: [ExecutionPhase]
}

enum ExecutionPhase: Sendable {
    case sequential([AgentTask])
    case parallel([AgentTask])
}

// MARK: - ExecutableTransition

struct ExecutableTransition: Sendable {
    let to: String
    let condition: TransitionCondition
}

enum TransitionCondition: Sendable {
    /// when: 'true'
    case always
    /// when: exists('artifact_name')
    case artifactExists(String)
    /// when: approval.granted == true
    case approvalGranted
    /// when: <complex expression> — evaluated at runtime
    case expression(String)
}

// MARK: - ResolvedAgent

/// Agent definition with backend profile resolved to concrete values.
struct ResolvedAgent: Sendable {
    let id: String
    let title: String
    let mode: String
    let backendProfileID: String?
    let provider: String
    let model: String
    let effort: String
    let maxTurns: Int
    let temperature: Double
    let permissionProfile: String
    let skillRef: String
    let skillRole: String?
    let prompt: String
    let outputContract: String?
    let requiresHumanApproval: Bool
    let inputs: [String]
    let outputs: [String]

    init(
        id: String,
        title: String,
        mode: String,
        backendProfileID: String? = nil,
        provider: String,
        model: String,
        effort: String,
        maxTurns: Int,
        temperature: Double,
        permissionProfile: String,
        skillRef: String,
        skillRole: String?,
        prompt: String,
        outputContract: String?,
        requiresHumanApproval: Bool,
        inputs: [String],
        outputs: [String]
    ) {
        self.id = id
        self.title = title
        self.mode = mode
        self.backendProfileID = backendProfileID
        self.provider = provider
        self.model = model
        self.effort = effort
        self.maxTurns = maxTurns
        self.temperature = temperature
        self.permissionProfile = permissionProfile
        self.skillRef = skillRef
        self.skillRole = skillRole
        self.prompt = prompt
        self.outputContract = outputContract
        self.requiresHumanApproval = requiresHumanApproval
        self.inputs = inputs
        self.outputs = outputs
    }
}

// MARK: - Compilation Errors

enum CompilationError: Error, LocalizedError {
    case validationFailed([ValidationIssue])
    case agentNotFound(agentID: String, stateID: String)
    case backendProfileNotFound(profileID: String, agentID: String)
    case circularTransitions(stateIDs: [String])
    case noInitialState
    case noEndState
    case unreachableStates([String])
    case duplicateStateIDs([String])
    case variableNotFound(name: String, context: String)
    case invalidLoopMax(value: String, counter: String)

    var errorDescription: String? {
        switch self {
        case .validationFailed(let issues):
            let errors = issues.filter { $0.severity == .error }
            return "Validation failed with \(errors.count) error(s)"
        case .agentNotFound(let agentID, let stateID):
            return "Agent '\(agentID)' not found in catalog (referenced in state '\(stateID)')"
        case .backendProfileNotFound(let profileID, let agentID):
            return "Backend profile '\(profileID)' not found (agent '\(agentID)')"
        case .circularTransitions(let stateIDs):
            return "Circular transitions detected: \(stateIDs.joined(separator: " → "))"
        case .noInitialState:
            return "Workflow has no initial state"
        case .noEndState:
            return "Workflow has no end state"
        case .unreachableStates(let ids):
            return "Unreachable states: \(ids.joined(separator: ", "))"
        case .duplicateStateIDs(let ids):
            return "Duplicate state IDs: \(ids.joined(separator: ", "))"
        case .variableNotFound(let name, let context):
            return "Variable '\(name)' not found (\(context))"
        case .invalidLoopMax(let value, let counter):
            return "Invalid loop max '\(value)' for counter '\(counter)' — must resolve to an integer"
        }
    }
}
