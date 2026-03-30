import Foundation

// MARK: - Agent Catalog (top-level)

nonisolated struct AgentCatalog: Codable, Sendable {
    let schemaVersion: Int
    let app: AppConfig
    let paths: [String: String]
    let artifacts: [String: String]
    let skills: [String: SkillRef]
    let contracts: [String: ArtifactContract]
    let backendProfiles: [String: BackendProfile]
    let permissionProfiles: [String: PermissionProfile]
    let agents: [AgentDefinition]

    enum CodingKeys: String, CodingKey {
        case app, paths, artifacts, skills, contracts, agents
        case schemaVersion = "schema_version"
        case backendProfiles = "backend_profiles"
        case permissionProfiles = "permission_profiles"
    }
}

nonisolated struct AppConfig: Codable, Sendable {
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

nonisolated struct AgentDefinition: Codable, Sendable, Identifiable {
    let id: String
    let title: String
    let mode: String
    let backendProfile: String
    let permissionProfile: String
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

    enum CodingKeys: String, CodingKey {
        case id, title, mode, prompt, notes, inputs, outputs
        case backendProfile = "backend_profile"
        case permissionProfile = "permission_profile"
        case skillRef = "skill_ref"
        case skillRole = "skill_role"
        case worktreePolicy = "worktree_policy"
        case requiredTools = "required_tools"
        case outputContract = "output_contract"
        case requiresHumanApproval = "requires_human_approval"
    }
}

nonisolated struct BackendProfile: Codable, Sendable {
    let provider: String
    let model: String
    let effort: String
    let temperature: Double
    let maxTurns: Int
    let structuredOutput: String

    enum CodingKeys: String, CodingKey {
        case provider, model, effort, temperature
        case maxTurns = "max_turns"
        case structuredOutput = "structured_output"
    }
}

nonisolated struct PermissionProfile: Codable, Sendable {
    let filesystem: FilesystemPermissions
    let git: GitPermissions
    let shell: ShellPermissions
    let network: NetworkPermissions
    let mcp: MCPPermissions
}

nonisolated struct ArtifactContract: Codable, Sendable {
    let format: String
    let requiredFields: [String]

    enum CodingKeys: String, CodingKey {
        case format
        case requiredFields = "required_fields"
    }
}

nonisolated struct SkillRef: Codable, Sendable {
    let type: String
    let path: String?
    let name: String?
    let description: String?
}

nonisolated struct WorktreePolicy: Codable, Sendable {
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

nonisolated struct FilesystemPermissions: Codable, Sendable {
    let read: [String]?
    let write: [String]?
    let deny: [String]?
}

nonisolated struct GitPermissions: Codable, Sendable {
    let status: Bool?
    let diff: Bool?
    let checkout: Bool?
    let commit: Bool?
    let push: Bool?
}

nonisolated struct ShellPermissions: Codable, Sendable {
    let allow: [String]?
    let deny: [String]?
}

nonisolated struct NetworkPermissions: Codable, Sendable {
    let allow: [String]?
}

nonisolated struct MCPPermissions: Codable, Sendable {
    let allow: [String]?
}
