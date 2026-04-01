import Foundation

// MARK: - SessionReuseReceiptFields (Proposal 018, Layer D)

/// Extends receipts and execution metadata with session lineage provenance.
///
/// These fields are included in `AgentExecution` persisted metadata and
/// can be exported via the receipt/report pipeline.
struct SessionReuseReceiptFields: Codable, Sendable {
    /// Session lineage ID for this execution.
    let sessionLineageID: UUID?
    /// Which immutable generation this execution used.
    let sessionGenerationID: UUID?
    /// The checkpoint artifact used for rehydration (if any).
    let rehydratedFromCheckpointArtifactID: UUID?
    /// The exact owner tuple that authorized reuse or forced freshness.
    let invocationOwnerKey: String?
    /// Effective scope used for this invocation.
    let sessionReuseScope: String?
    /// Optional family key used when scope widens beyond one invocation owner.
    let sessionFamilyID: String?
    /// How this session was created or reused.
    let sessionReuseDisposition: String?
    /// Optional reason when a fresh session was forced.
    let sessionResetReason: String?

    /// Build from an `AgentExecution` record.
    static func from(execution: AgentExecution) -> SessionReuseReceiptFields {
        SessionReuseReceiptFields(
            sessionLineageID: execution.sessionLineageID,
            sessionGenerationID: execution.sessionGenerationID,
            rehydratedFromCheckpointArtifactID: execution.rehydratedFromCheckpointArtifactID,
            invocationOwnerKey: execution.invocationOwnerKey,
            sessionReuseScope: execution.sessionReuseScope?.rawValue,
            sessionFamilyID: execution.sessionFamilyID,
            sessionReuseDisposition: execution.sessionReuseDisposition?.rawValue,
            sessionResetReason: execution.sessionResetReason
        )
    }

    /// Build from an `AgentResult`.
    static func from(result: AgentResult, scope: SessionReuseScope?, familyID: String?) -> SessionReuseReceiptFields {
        SessionReuseReceiptFields(
            sessionLineageID: result.sessionLineageID,
            sessionGenerationID: result.sessionGenerationID,
            rehydratedFromCheckpointArtifactID: nil,
            invocationOwnerKey: nil,
            sessionReuseScope: scope?.rawValue,
            sessionFamilyID: familyID,
            sessionReuseDisposition: result.sessionReuseDisposition?.rawValue,
            sessionResetReason: nil
        )
    }
}
