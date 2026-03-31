import Foundation
import SwiftData

@Model final class Artifact {
    @Attribute(.unique) var id: UUID
    var name: String
    var contractID: String
    var format: ArtifactFormat
    var filePath: String
    var checksumSHA256: String?
    var createdAt: Date
    var sizeBytes: Int64?
    var runID: UUID
    var stageID: String
    var agentID: String
    var provider: String
    var model: String?
    var effort: String?
    var attemptNumber: Int

    // P005-OPS §6.5: Report and inspector additions
    var isPinned: Bool = false
    var displayRole: String?
    var reportKind: String?           // "immutable_history" | "latest_summary"
    var reportVersion: Int?
    var supersedesArtifactID: UUID?

    // Proposal 013 — Layer N: Agent-Retry Lineage (§5.4)
    var agentAttemptNumber: Int?               // Agent-level attempt number (for agent-only retries)
    var supersedesAgentArtifactID: UUID?       // Which agent artifact this supersedes
    var artifactLineageKind: String?           // "stage_attempt_primary" | "agent_retry_delta" | "reused_sibling_reference"

    @Relationship(inverse: \AgentExecution.artifacts)
    var agentExecution: AgentExecution?

    init(id: UUID = UUID(), name: String, contractID: String, format: ArtifactFormat, filePath: String, createdAt: Date = Date(), runID: UUID, stageID: String, agentID: String, provider: String, attemptNumber: Int = 1) {
        self.id = id
        self.name = name
        self.contractID = contractID
        self.format = format
        self.filePath = filePath
        self.createdAt = createdAt
        self.runID = runID
        self.stageID = stageID
        self.agentID = agentID
        self.provider = provider
        self.attemptNumber = attemptNumber
    }
}

enum ArtifactFormat: String, Codable {
    case json, markdown, diff, report
}

// MARK: - ArtifactFormat.detect (§7.3 — strict priority order)

extension ArtifactFormat {
    /// Detect format. Priority: explicit extension > contract.format > fallback.
    /// `contract` is the resolved ArtifactContract from the agent catalog (if the agent has one).
    static func detect(from name: String, contract: ArtifactContract?) -> ArtifactFormat {
        // 1. File extension takes precedence
        if name.hasSuffix(".json") { return .json }
        if name.hasSuffix(".md") { return .markdown }
        if name.hasSuffix(".diff") || name.hasSuffix(".patch") { return .diff }

        // 2. If an output contract exists, use its declared format
        if let contract {
            return ArtifactFormat(rawValue: contract.machineFormat ?? contract.format) ?? .json
        }

        // 3. Fallback: treat as report (generic structured output)
        return .report
    }
}
