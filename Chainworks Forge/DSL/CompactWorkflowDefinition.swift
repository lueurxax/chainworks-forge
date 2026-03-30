import Foundation

// MARK: - Compact Workflow (inspector-only, not executable)

nonisolated struct CompactWorkflowDefinition: Codable, Sendable {
    let version: Int
    let workflow: CompactWorkflowMeta
    /// Explicit alias map for non-mechanical agent ID resolution (ARCH-012).
    let agentAliases: [String: String]?

    enum CodingKeys: String, CodingKey {
        case version, workflow
        case agentAliases = "agent_aliases"
    }
}

nonisolated struct CompactWorkflowMeta: Codable, Sendable {
    let id: String
    let title: String
    let execution: ExecutionConfig
    let requiredProviders: [String]
    let stages: [CompactStage]

    enum CodingKeys: String, CodingKey {
        case id, title, execution, stages
        case requiredProviders = "required_providers"
    }
}

nonisolated struct CompactStage: Codable, Sendable, Identifiable {
    let id: String
    let type: String
    let agent: String?
    let agents: [String]?
    let approval: String?
    let needs: [String]?
    let gate: CompactGate?
}

nonisolated struct CompactGate: Codable, Sendable {
    let require: [String]
}
