import Foundation

// MARK: - Full Workflow Definition

struct WorkflowDefinition: Codable, Sendable {
    let schemaVersion: Int
    let workflow: WorkflowMeta
    let variables: [String: AnyCodableValue]?
    let failurePolicy: FailurePolicy?
    let scoring: ScoringConfig?
    let initialState: String
    let states: [String: WorkflowState]

    enum CodingKeys: String, CodingKey {
        case workflow, variables, scoring, states
        case schemaVersion = "schema_version"
        case failurePolicy = "failure_policy"
        case initialState = "initial_state"
    }
}

struct WorkflowMeta: Codable, Sendable {
    let id: String
    let name: String
    let usesAgentCatalog: String?
    let description: String
    let ideaInput: IdeaInputConfig?
    let execution: ExecutionConfig
    let requiredProviders: [String]

    enum CodingKeys: String, CodingKey {
        case id, name, description, execution
        case usesAgentCatalog = "uses_agent_catalog"
        case ideaInput = "idea_input"
        case requiredProviders = "required_providers"
    }
}

struct WorkflowState: Codable, Sendable {
    let label: String
    let type: String?
    let owner: String
    let approval: String?
    let approvalPolicy: String?
    let run: RunBlock?
    let runAfterApproval: RunBlock?
    let loop: LoopConfig?
    let transitions: [Transition]?

    enum CodingKeys: String, CodingKey {
        case label, type, owner, approval, run, loop, transitions
        case approvalPolicy = "approval_policy"
        case runAfterApproval = "run_after_approval"
    }

    init(
        label: String,
        type: String?,
        owner: String,
        approval: String?,
        approvalPolicy: String? = nil,
        run: RunBlock?,
        runAfterApproval: RunBlock?,
        loop: LoopConfig?,
        transitions: [Transition]?
    ) {
        self.label = label
        self.type = type
        self.owner = owner
        self.approval = approval
        self.approvalPolicy = approvalPolicy
        self.run = run
        self.runAfterApproval = runAfterApproval
        self.loop = loop
        self.transitions = transitions
    }
}

struct RunBlock: Codable, Sendable {
    let sequence: [AgentTask]?
    let parallel: [AgentTask]?
    let then: [AgentTask]?
}

struct AgentTask: Codable, Sendable {
    let agent: String
    let task: String
    let inputs: [String]?
    let outputs: [String]?
}

struct Transition: Codable, Sendable {
    let to: String
    let when: String
}

struct LoopConfig: Codable, Sendable {
    let counter: String
    let max: String
}

struct FailurePolicy: Codable, Sendable {
    let onError: String
    let onLoopBudgetExhausted: String
    let preserveArtifacts: Bool

    enum CodingKeys: String, CodingKey {
        case onError = "on_error"
        case onLoopBudgetExhausted = "on_loop_budget_exhausted"
        case preserveArtifacts = "preserve_artifacts"
    }
}

struct ExecutionConfig: Codable, Sendable {
    let singleActiveRunPerIdea: Bool
    let resumePolicy: String
    /// Proposal 011 (REQ-004): when true, the workflow requires a valid project directory.
    let requiresProjectAccess: Bool

    enum CodingKeys: String, CodingKey {
        case singleActiveRunPerIdea = "single_active_run_per_idea"
        case resumePolicy = "resume_policy"
        case requiresProjectAccess = "requires_project_access"
    }

    init(
        singleActiveRunPerIdea: Bool,
        resumePolicy: String,
        requiresProjectAccess: Bool = false
    ) {
        self.singleActiveRunPerIdea = singleActiveRunPerIdea
        self.resumePolicy = resumePolicy
        self.requiresProjectAccess = requiresProjectAccess
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        singleActiveRunPerIdea = try container.decode(Bool.self, forKey: .singleActiveRunPerIdea)
        resumePolicy = try container.decode(String.self, forKey: .resumePolicy)
        requiresProjectAccess = try container.decodeIfPresent(Bool.self, forKey: .requiresProjectAccess) ?? false
    }
}

struct IdeaInputConfig: Codable, Sendable {
    let mode: String
}

struct ScoringConfig: Codable, Sendable {
    let proposal: ProposalScoring?
    let implementation: ImplementationScoring?
}

struct ProposalScoring: Codable, Sendable {
    let aggregateFormula: String?
    let passWhen: [String]?

    enum CodingKeys: String, CodingKey {
        case aggregateFormula = "aggregate_formula"
        case passWhen = "pass_when"
    }
}

struct ImplementationScoring: Codable, Sendable {
    let implementedWhen: [String]?

    enum CodingKeys: String, CodingKey {
        case implementedWhen = "implemented_when"
    }
}

// MARK: - Type-erased Codable value

enum AnyCodableValue: Codable, Sendable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case array([AnyCodableValue])
    case dictionary([String: AnyCodableValue])
    case null

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(Int.self) {
            self = .int(value)
        } else if let value = try? container.decode(Double.self) {
            self = .double(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode([AnyCodableValue].self) {
            self = .array(value)
        } else if let value = try? container.decode([String: AnyCodableValue].self) {
            self = .dictionary(value)
        } else {
            throw DecodingError.typeMismatch(AnyCodableValue.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unsupported type"))
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let v): try container.encode(v)
        case .int(let v): try container.encode(v)
        case .double(let v): try container.encode(v)
        case .bool(let v): try container.encode(v)
        case .array(let v): try container.encode(v)
        case .dictionary(let v): try container.encode(v)
        case .null: try container.encodeNil()
        }
    }
}
