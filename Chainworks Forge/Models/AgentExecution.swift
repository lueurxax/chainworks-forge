import Foundation
import SwiftData

@Model final class AgentExecution {
    @Attribute(.unique) var id: UUID
    var agentID: String
    var agentTitle: String
    var taskName: String
    var startedAt: Date
    var completedAt: Date?
    var status: AgentStatus
    var provider: String
    var effort: String
    var costCents: Int64?
    var logSnippet: String?
    @Attribute(originalName: "gooseSessionID") var runtimeSessionID: String?

    // Proposal 004: Live provider fields (Section 11.1)
    var providerSessionID: String?
    var providerRequestID: String?
    var transcriptArtifactPath: String?
    var resolvedBackendProfileID: String?
    var consumedInputArtifactNamesJSON: Data?
    var providerReceiptJSON: Data?
    var resolvedModel: String?
    var configuredProviderID: UUID?
    var adapterVersion: String?

    // P005-OPS §9.3: Structured input bindings for traceability
    // Stores [InputBinding] — maps declared input names to source artifact names and producing agents.
    var inputBindingsJSON: Data?

    // Steward data model additions (Proposal 003 — optional, lightweight migration)
    var agentConfigHash: String?
    var skillRef: String?
    var skillSnapshotHash: String?
    var skillType: String?
    var skillRole: String?
    var skillContentSummary: String?
    var transcriptPath: String?
    var toolTracePath: String?
    var retryReason: String?

    // Proposal 013 — Layer N: Agent Attempt Lineage (§5.3, §5.4)
    var agentAttemptNumber: Int?              // Agent-level attempt within the same stage attempt
    var supersedesAgentExecutionID: UUID?     // Which agent execution this supersedes
    var reusedSiblingExecutionIDsJSON: Data?  // [UUID] — sibling executions reused for this retry

    // Proposal 013 — Layer O: Validation Evidence
    var validationFailureJSON: Data?          // Serialized ValidationFailureRecord for this agent
    var outputEnvelopesJSON: Data?            // Serialized [StructuredOutputEnvelope]
    var compactionMetadataJSON: Data?         // Serialized CompactionMetadata (if output was compacted)
    var canonicalOutcome: AgentCanonicalOutcome?
    var supervisionClassification: SupervisionClassification?
    var transportErrorKind: TransportErrorKind?
    var providerStopReason: String?
    var outputPresence: OutputPresence?
    var settledAt: Date?
    var runtimeProvider: String?
    var runtimeModel: String?
    var outcomeEnvelopeJSON: Data?
    var mcpProfileID: String?
    var requestedMCPExtensionsJSON: Data?
    var effectiveMCPRuntimeExtensionIDsJSON: Data?
    var deniedMCPExtensionsJSON: Data?
    var mcpSessionStartupLatencyMilliseconds: Int?
    var mcpServerTelemetryJSON: Data?

    /// Proposal 026: Actual runtime profile used for this execution attempt.
    var runtimeProfileID: String?
    /// Proposal 026: Actual adapter family (e.g. "claude_agent_acp", "gemini_cli_acp", "codex_acp").
    var actualAdapterFamily: String?
    /// Proposal 026: Actual capability class of the runtime used.
    var actualCapabilityClass: String?

    // Proposal 007: Repo-backed execution tracking
    var repoRevisionBefore: String?
    var repoRevisionAfter: String?

    // Proposal 018: Session lineage and reuse fields
    var sessionLineageID: UUID?
    var sessionGenerationID: UUID?
    var rehydratedFromCheckpointArtifactID: UUID?
    var invocationOwnerKey: String? // Exact owner tuple for reuse decisions
    var sessionReuseScope: SessionReuseScope?
    var sessionFamilyID: String?
    var sessionReuseDisposition: SessionReuseDisposition?
    var sessionResetReason: String?

    // Proposal 019: Strategy telemetry fields persisted on the canonical execution row.
    var inputPayloadBytes: Int = 0
    var handoffMode: String?
    var limitPressureSignalsJSON: Data?
    var modelTierUsed: String?
    var promotedArtifactNamesJSON: Data?

    @Relationship(inverse: \StageExecution.agentExecutions)
    var stageExecution: StageExecution?

    @Relationship(deleteRule: .cascade)
    var artifacts: [Artifact] = []

    init(id: UUID = UUID(), agentID: String, agentTitle: String, taskName: String, startedAt: Date = Date(), status: AgentStatus = .pending, provider: String, effort: String) {
        self.id = id
        self.agentID = agentID
        self.agentTitle = agentTitle
        self.taskName = taskName
        self.startedAt = startedAt
        self.status = status
        self.provider = provider
        self.effort = effort
    }
}

/// P005-OPS §9.3: Structured input binding for traceability.
struct InputBinding: Codable, Sendable {
    let inputName: String
    let artifactName: String
    let producingAgentID: String?
}

enum AgentStatus: String, Codable {
    case pending
    case ready
    case running
    case completed
    case failed
    case cancelled
    case skipped
}
