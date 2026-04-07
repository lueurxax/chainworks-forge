import Foundation

// MARK: - Proposal 026: Runtime Profiles

/// Proposal 026: Runtime capability classification for transport adapters.
enum RuntimeCapabilityClass: String, Codable, Sendable {
    case lifecycleCapable = "lifecycle_capable"
    case controlCapable = "control_capable"
    case operatorGrade = "operator_grade"
    /// Legacy Goose REST/SSE runtime, operator-grade equivalent.
    case legacyOperatorGrade = "legacy_operator_grade"
}

/// Proposal 026: Declares a runtime transport adapter's identity and capabilities.
struct RuntimeProfile: Codable, Sendable {
    let capabilityClass: RuntimeCapabilityClass
    /// Adapter family identifier (e.g. "goose", "claude_agent_acp", "gemini_cli_acp").
    let adapterFamily: String
    /// Required runtime capabilities (e.g. ["streaming", "tools", "permission_callbacks"]).
    let requires: [String]
    /// Transport mechanism (e.g. "goose_server", "acp_stdio", "acp_http").
    let transportKind: String
    /// MCP realization strategy (e.g. "goose_extension", "acp_native", nil).
    let mcpRealizationPath: String?

    enum CodingKeys: String, CodingKey {
        case capabilityClass = "capability_class"
        case adapterFamily = "adapter_family"
        case requires
        case transportKind = "transport_kind"
        case mcpRealizationPath = "mcp_realization_path"
    }
}

// MARK: - Agent Catalog (top-level)

struct AgentCatalog: Codable, Sendable {
    let schemaVersion: Int
    let app: AppConfig
    let paths: [String: String]
    let artifacts: [String: String]
    let skills: [String: SkillRef]
    let mcpPolicy: MCPPolicyConfig
    let mcpServerRegistry: [String: MCPServerRegistryEntry]
    let mcpProfiles: [String: MCPProfile]
    let contracts: [String: ArtifactContract]
    let backendProfiles: [String: BackendProfile]
    let permissionProfiles: [String: PermissionProfile]
    /// Proposal 026: Catalog-owned runtime profile declarations.
    let runtimeProfiles: [String: RuntimeProfile]
    let agents: [AgentDefinition]

    enum CodingKeys: String, CodingKey {
        case app, paths, artifacts, skills, contracts, agents
        case schemaVersion = "schema_version"
        case mcpPolicy = "mcp_policy"
        case mcpServerRegistry = "mcp_server_registry"
        case mcpProfiles = "mcp_profiles"
        case backendProfiles = "backend_profiles"
        case permissionProfiles = "permission_profiles"
        case runtimeProfiles = "runtime_profiles"
    }

    init(
        schemaVersion: Int,
        app: AppConfig,
        paths: [String: String],
        artifacts: [String: String],
        skills: [String: SkillRef],
        mcpPolicy: MCPPolicyConfig = .defaultDeny,
        mcpServerRegistry: [String: MCPServerRegistryEntry] = [:],
        mcpProfiles: [String: MCPProfile] = [:],
        contracts: [String: ArtifactContract],
        backendProfiles: [String: BackendProfile],
        permissionProfiles: [String: PermissionProfile],
        runtimeProfiles: [String: RuntimeProfile] = [:],
        agents: [AgentDefinition]
    ) {
        self.schemaVersion = schemaVersion
        self.app = app
        self.paths = paths
        self.artifacts = artifacts
        self.skills = skills
        self.mcpPolicy = mcpPolicy
        self.mcpServerRegistry = mcpServerRegistry
        self.mcpProfiles = mcpProfiles
        self.contracts = contracts
        self.backendProfiles = backendProfiles
        self.permissionProfiles = permissionProfiles
        self.runtimeProfiles = runtimeProfiles
        self.agents = agents
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try container.decode(Int.self, forKey: .schemaVersion)
        app = try container.decode(AppConfig.self, forKey: .app)
        paths = try container.decode([String: String].self, forKey: .paths)
        artifacts = try container.decode([String: String].self, forKey: .artifacts)
        skills = try container.decode([String: SkillRef].self, forKey: .skills)
        mcpPolicy = try container.decodeIfPresent(MCPPolicyConfig.self, forKey: .mcpPolicy) ?? .defaultDeny
        mcpServerRegistry = try container.decodeIfPresent([String: MCPServerRegistryEntry].self, forKey: .mcpServerRegistry) ?? [:]
        mcpProfiles = try container.decodeIfPresent([String: MCPProfile].self, forKey: .mcpProfiles) ?? [:]
        contracts = try container.decode([String: ArtifactContract].self, forKey: .contracts)
        backendProfiles = try container.decode([String: BackendProfile].self, forKey: .backendProfiles)
        permissionProfiles = try container.decode([String: PermissionProfile].self, forKey: .permissionProfiles)
        runtimeProfiles = try container.decodeIfPresent([String: RuntimeProfile].self, forKey: .runtimeProfiles) ?? [:]
        agents = try container.decode([AgentDefinition].self, forKey: .agents)
    }
}

struct AppConfig: Codable, Sendable {
    let name: String
    let runtime: String
    let transport: String
    let description: String
    let ideaInputMode: String
    let singleActiveRunPerIdea: Bool
    let runResumePolicy: String
    let requiredProviders: [String]

    enum CodingKeys: String, CodingKey {
        case name, runtime, transport, description
        case ideaInputMode = "idea_input_mode"
        case singleActiveRunPerIdea = "single_active_run_per_idea"
        case runResumePolicy = "run_resume_policy"
        case requiredProviders = "required_providers"
    }
}

struct AgentDefinition: Codable, Sendable, Identifiable {
    let id: String
    let title: String
    let mode: String
    let backendProfile: String
    let permissionProfile: String
    let mcpProfile: String?
    let skillRef: String
    let skillRole: String?
    let worktreePolicy: WorktreePolicy?
    let requiredTools: [String]?
    let inputs: [String]
    let outputs: [String]
    let outputContract: String?
    let requiresHumanApproval: Bool
    let prompt: String
    let notes: String?
    
    // Proposal 018: Session reuse policy
    let sessionReuseScope: String?
    let sessionFamilyID: String?

    enum CodingKeys: String, CodingKey {
        case id, title, mode, prompt, notes, inputs, outputs
        case backendProfile = "backend_profile"
        case permissionProfile = "permission_profile"
        case mcpProfile = "mcp_profile"
        case skillRef = "skill_ref"
        case skillRole = "skill_role"
        case worktreePolicy = "worktree_policy"
        case requiredTools = "required_tools"
        case outputContract = "output_contract"
        case requiresHumanApproval = "requires_human_approval"
        case sessionReuseScope = "session_reuse_scope"
        case sessionFamilyID = "session_family_id"
    }

    init(
        id: String,
        title: String,
        mode: String,
        backendProfile: String,
        permissionProfile: String,
        mcpProfile: String? = nil,
        skillRef: String,
        skillRole: String? = nil,
        worktreePolicy: WorktreePolicy? = nil,
        requiredTools: [String]? = nil,
        inputs: [String],
        outputs: [String],
        outputContract: String? = nil,
        requiresHumanApproval: Bool,
        prompt: String,
        notes: String? = nil,
        sessionReuseScope: String? = nil,
        sessionFamilyID: String? = nil
    ) {
        self.id = id
        self.title = title
        self.mode = mode
        self.backendProfile = backendProfile
        self.permissionProfile = permissionProfile
        self.mcpProfile = mcpProfile
        self.skillRef = skillRef
        self.skillRole = skillRole
        self.worktreePolicy = worktreePolicy
        self.requiredTools = requiredTools
        self.inputs = inputs
        self.outputs = outputs
        self.outputContract = outputContract
        self.requiresHumanApproval = requiresHumanApproval
        self.prompt = prompt
        self.notes = notes
        self.sessionReuseScope = sessionReuseScope
        self.sessionFamilyID = sessionFamilyID
    }
}

struct BackendProfile: Codable, Sendable {
    let provider: String
    let model: String
    let effort: String
    let temperature: Double
    let maxTurns: Int
    let structuredOutput: String
    /// Proposal 026: References a key in AgentCatalog.runtimeProfiles.
    let runtimeProfile: String?

    enum CodingKeys: String, CodingKey {
        case provider, model, effort, temperature
        case maxTurns = "max_turns"
        case structuredOutput = "structured_output"
        case runtimeProfile = "runtime_profile"
    }

    init(
        provider: String,
        model: String,
        effort: String,
        temperature: Double,
        maxTurns: Int,
        structuredOutput: String,
        runtimeProfile: String? = nil
    ) {
        self.provider = provider
        self.model = model
        self.effort = effort
        self.temperature = temperature
        self.maxTurns = maxTurns
        self.structuredOutput = structuredOutput
        self.runtimeProfile = runtimeProfile
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        provider = try container.decode(String.self, forKey: .provider)
        model = try container.decode(String.self, forKey: .model)
        effort = try container.decode(String.self, forKey: .effort)
        temperature = try container.decode(Double.self, forKey: .temperature)
        maxTurns = try container.decode(Int.self, forKey: .maxTurns)
        structuredOutput = try container.decode(String.self, forKey: .structuredOutput)
        runtimeProfile = try container.decodeIfPresent(String.self, forKey: .runtimeProfile)
    }
}

struct PermissionProfile: Codable, Sendable {
    let filesystem: FilesystemPermissions
    let git: GitPermissions
    let shell: ShellPermissions
    let network: NetworkPermissions
    let mcp: MCPPermissions

    init(
        filesystem: FilesystemPermissions,
        git: GitPermissions,
        shell: ShellPermissions,
        network: NetworkPermissions,
        mcp: MCPPermissions
    ) {
        self.filesystem = filesystem
        self.git = git
        self.shell = shell
        self.network = network
        self.mcp = mcp
    }
}

struct ArtifactContract: Codable, Sendable {
    let format: String
    let requiredFields: [String]
    let machineFormat: String?
    let humanFormat: String?
    let validationMode: String?
    let rawArtifactName: String?
    let normalizedArtifactName: String?

    init(
        format: String,
        requiredFields: [String],
        machineFormat: String? = nil,
        humanFormat: String? = nil,
        validationMode: String? = nil,
        rawArtifactName: String? = nil,
        normalizedArtifactName: String? = nil
    ) {
        self.format = format
        self.requiredFields = requiredFields
        self.machineFormat = machineFormat
        self.humanFormat = humanFormat
        self.validationMode = validationMode
        self.rawArtifactName = rawArtifactName
        self.normalizedArtifactName = normalizedArtifactName
    }

    enum CodingKeys: String, CodingKey {
        case format
        case requiredFields = "required_fields"
        case machineFormat = "machine_format"
        case humanFormat = "human_format"
        case validationMode = "validation_mode"
        case rawArtifactName = "raw_artifact_name"
        case normalizedArtifactName = "normalized_artifact_name"
    }
}

struct SkillRef: Codable, Sendable {
    let type: String
    let path: String?
    let name: String?
    let description: String?
}

struct WorktreePolicy: Codable, Sendable {
    let strategy: String
    let path: String
    let baseBranch: String?
    let writeEnabled: Bool

    enum CodingKeys: String, CodingKey {
        case strategy, path
        case baseBranch = "base_branch"
        case writeEnabled = "write_enabled"
    }
}

// MARK: - Supporting Permission Types

struct FilesystemPermissions: Codable, Sendable {
    let read: [String]?
    let write: [String]?
    let deny: [String]?
}

struct GitPermissions: Codable, Sendable {
    let status: Bool?
    let diff: Bool?
    let checkout: Bool?
    let commit: Bool?
    let push: Bool?
}

struct ShellPermissions: Codable, Sendable {
    let allow: [String]?
    let deny: [String]?
}

struct NetworkPermissions: Codable, Sendable {
    let allow: [String]?
}

struct MCPPermissions: Codable, Sendable {
    let legacyAllow: [String]?
    let runtimeAuthority: Bool

    enum CodingKeys: String, CodingKey {
        case allow
        case legacyAllow = "legacy_allow"
        case runtimeAuthority = "runtime_authority"
    }

    init(
        allow: [String]? = nil,
        legacyAllow: [String]? = nil,
        runtimeAuthority: Bool = false
    ) {
        self.legacyAllow = legacyAllow ?? allow
        self.runtimeAuthority = runtimeAuthority
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let legacyAllow = try container.decodeIfPresent([String].self, forKey: .legacyAllow)
        let allow = try container.decodeIfPresent([String].self, forKey: .allow)
        self.legacyAllow = legacyAllow ?? allow
        self.runtimeAuthority = try container.decodeIfPresent(Bool.self, forKey: .runtimeAuthority) ?? false
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encodeIfPresent(legacyAllow, forKey: .legacyAllow)
        try container.encode(runtimeAuthority, forKey: .runtimeAuthority)
    }
}

struct MCPPolicyConfig: Codable, Sendable, Equatable {
    let defaultProfile: String
    let runtimeAuthority: String
    let permissionProfileMCPMode: String
    let zeroMCPIsPreferredBaseline: Bool
    let unsupportedRequiredExtensionBehavior: String
    let unsupportedOptionalExtensionBehavior: String

    enum CodingKeys: String, CodingKey {
        case defaultProfile = "default_profile"
        case runtimeAuthority = "runtime_authority"
        case permissionProfileMCPMode = "permission_profile_mcp_mode"
        case zeroMCPIsPreferredBaseline = "zero_mcp_is_preferred_baseline"
        case unsupportedRequiredExtensionBehavior = "unsupported_required_extension_behavior"
        case unsupportedOptionalExtensionBehavior = "unsupported_optional_extension_behavior"
    }

    static let defaultDeny = MCPPolicyConfig(
        defaultProfile: "none",
        runtimeAuthority: "agent.mcp_profile",
        permissionProfileMCPMode: "legacy_ceiling_only",
        zeroMCPIsPreferredBaseline: true,
        unsupportedRequiredExtensionBehavior: "fail_preflight",
        unsupportedOptionalExtensionBehavior: "drop_and_record"
    )
}

struct MCPServerRegistryEntry: Codable, Sendable, Equatable {
    let runtimeIDs: [String: String]
    let sessionScoped: Bool
    let assignmentPolicy: String
    let riskClass: String
    let notes: String?

    enum CodingKeys: String, CodingKey {
        case runtimeIDs = "runtime_ids"
        case sessionScoped = "session_scoped"
        case assignmentPolicy = "assignment_policy"
        case riskClass = "risk_class"
        case notes
    }
}

struct MCPProfile: Codable, Sendable, Equatable {
    let requiredExtensions: [String]
    let optionalExtensions: [String]
    let fallbackPolicy: String

    enum CodingKeys: String, CodingKey {
        case requiredExtensions = "required_extensions"
        case optionalExtensions = "optional_extensions"
        case fallbackPolicy = "fallback_policy"
    }

    var allRequestedExtensions: [String] {
        Array(Set(requiredExtensions + optionalExtensions)).sorted()
    }
}

struct ResolvedMCPProfile: Codable, Sendable, Equatable {
    let profileID: String
    let requiredExtensions: [String]
    let optionalExtensions: [String]
    let requestedExtensions: [String]
    let fallbackPolicy: String

    static let none = ResolvedMCPProfile(
        profileID: "none",
        requiredExtensions: [],
        optionalExtensions: [],
        requestedExtensions: [],
        fallbackPolicy: "allow_without_extensions"
    )
}
