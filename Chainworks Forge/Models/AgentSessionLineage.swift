import Foundation
import SwiftData

@Model final class AgentSessionLineage {
    @Attribute(.unique) var id: UUID
    var runID: UUID
    var agentID: String
    var lineageID: String
    var sessionReuseScope: SessionReuseScope
    var sessionFamilyID: String?
    var activeGenerationID: UUID?
    var createdAt: Date
    var closedAt: Date?

    @Relationship(deleteRule: .cascade, inverse: \AgentSessionGeneration.lineage)
    var generations: [AgentSessionGeneration] = []

    @Relationship(deleteRule: .cascade, inverse: \AgentSessionEvent.lineage)
    var events: [AgentSessionEvent] = []

    init(
        id: UUID = UUID(),
        runID: UUID,
        agentID: String,
        lineageID: String,
        sessionReuseScope: SessionReuseScope = .same_invocation_owner,
        sessionFamilyID: String? = nil,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.runID = runID
        self.agentID = agentID
        self.lineageID = lineageID
        self.sessionReuseScope = sessionReuseScope
        self.sessionFamilyID = sessionFamilyID
        self.createdAt = createdAt
    }
}

@Model final class AgentSessionGeneration {
    @Attribute(.unique) var id: UUID
    var generation: Int
    var invocationOwnerKey: String // Immutable ownership key for the owner that started/claimed this generation
    var providerSessionID: String?
    var rehydratedFromCheckpointArtifactID: UUID?
    var bindingFingerprint: String
    var workingDirectory: String
    var workspaceMode: String // "read_only" / "read_write"
    var runtimeProvider: String
    var runtimeModel: String
    var status: AgentSessionStatus
    var turnCount: Int
    var estimatedInputTokens: Int64
    var cumulativePromptTokens: Int64
    var cumulativeCostCents: Int64
    var lastCheckpointArtifactID: UUID?
    var createdAt: Date
    var endedAt: Date?
    var endReason: String?

    var lineage: AgentSessionLineage?

    init(
        id: UUID = UUID(),
        generation: Int,
        invocationOwnerKey: String,
        providerSessionID: String? = nil,
        bindingFingerprint: String,
        workingDirectory: String,
        workspaceMode: String,
        runtimeProvider: String,
        runtimeModel: String,
        status: AgentSessionStatus = .active,
        createdAt: Date = Date()
    ) {
        self.id = id
        self.generation = generation
        self.invocationOwnerKey = invocationOwnerKey
        self.providerSessionID = providerSessionID
        self.bindingFingerprint = bindingFingerprint
        self.workingDirectory = workingDirectory
        self.workspaceMode = workspaceMode
        self.runtimeProvider = runtimeProvider
        self.runtimeModel = runtimeModel
        self.status = status
        self.turnCount = 0
        self.estimatedInputTokens = 0
        self.cumulativePromptTokens = 0
        self.cumulativeCostCents = 0
        self.createdAt = createdAt
    }
}

@Model final class AgentSessionEvent {
    @Attribute(.unique) var id: UUID
    var generationID: UUID
    var eventType: AgentSessionEventType
    var recordedAt: Date
    var detailsJSON: Data?

    var lineage: AgentSessionLineage?

    init(
        id: UUID = UUID(),
        generationID: UUID,
        eventType: AgentSessionEventType,
        recordedAt: Date = Date(),
        detailsJSON: Data? = nil
    ) {
        self.id = id
        self.generationID = generationID
        self.eventType = eventType
        self.recordedAt = recordedAt
        self.detailsJSON = detailsJSON
    }
}

enum SessionReuseScope: String, Codable, Sendable {
    case none
    case same_invocation_owner
    case same_agent_family_within_run
}

enum AgentSessionStatus: String, Codable, Sendable {
    case active
    case invalidated
    case closed
    case reset
}

enum AgentSessionEventType: String, Codable, Sendable {
    case created
    case reused
    case invalidated
    case closed
    case operator_reset
    case resume_reused
    case resume_rejected
    case checkpoint_created
    case budget_exceeded
    case compacted
}

enum SessionReuseDisposition: String, Codable, Sendable {
    case fresh
    case reused
    case reused_after_resume
    case fresh_after_reset
    case fresh_after_invalidation
    case fresh_after_budget
    case fresh_after_compaction
    case fresh_after_transport_error
    case fresh_after_timeout
    case fresh_session_required
    case unverifiable_session_history
}
