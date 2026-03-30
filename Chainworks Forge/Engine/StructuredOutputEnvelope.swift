import Foundation

// MARK: - Proposal 013 Layer M: Structured Output Envelope

/// Persisted wrapper for structured outputs that captures raw payload,
/// parsed payload, validation result, and origin metadata.
/// This ensures validation never becomes the point where all downstream evidence disappears.
nonisolated struct StructuredOutputEnvelope: Codable, Sendable, Identifiable, Equatable {
    let id: UUID
    let timestamp: Date

    /// The output name (artifact key).
    let outputName: String
    /// The agent that produced this output.
    let agentID: String
    /// The stage where this output was produced.
    let stageID: String
    /// The run ID.
    let runID: UUID

    /// Raw payload size in bytes.
    let rawPayloadSize: Int
    /// SHA-256 checksum of the raw payload.
    let rawPayloadChecksum: String?
    /// Whether the raw payload was persisted to disk before validation.
    let rawPayloadPersisted: Bool

    /// The contract ID used for validation (nil if no contract).
    let contractID: String?
    /// Validation result for this output.
    let validationResult: OutputValidationResult?

    /// Whether a normalized (post-validation) artifact was produced.
    let normalizedArtifactProduced: Bool

    /// Origin metadata.
    let provider: String
    let model: String?
    let effort: String?
    let sessionID: String?
    let durationSeconds: Double?

    init(
        id: UUID = UUID(),
        timestamp: Date = Date(),
        outputName: String,
        agentID: String,
        stageID: String,
        runID: UUID,
        rawPayloadSize: Int,
        rawPayloadChecksum: String? = nil,
        rawPayloadPersisted: Bool,
        contractID: String? = nil,
        validationResult: OutputValidationResult? = nil,
        normalizedArtifactProduced: Bool = false,
        provider: String,
        model: String? = nil,
        effort: String? = nil,
        sessionID: String? = nil,
        durationSeconds: Double? = nil
    ) {
        self.id = id
        self.timestamp = timestamp
        self.outputName = outputName
        self.agentID = agentID
        self.stageID = stageID
        self.runID = runID
        self.rawPayloadSize = rawPayloadSize
        self.rawPayloadChecksum = rawPayloadChecksum
        self.rawPayloadPersisted = rawPayloadPersisted
        self.contractID = contractID
        self.validationResult = validationResult
        self.normalizedArtifactProduced = normalizedArtifactProduced
        self.provider = provider
        self.model = model
        self.effort = effort
        self.sessionID = sessionID
        self.durationSeconds = durationSeconds
    }
}
