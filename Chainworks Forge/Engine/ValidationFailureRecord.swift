import Foundation

// MARK: - Proposal 013 Layer M: Validation Failure Record

/// First-class persisted record describing why output validation failed
/// after agent execution completed. This is the canonical reference target
/// for recovery UI, immutable run reports, and exported evidence.
nonisolated struct ValidationFailureRecord: Codable, Sendable, Identifiable, Equatable {
    let id: UUID
    let timestamp: Date

    /// The agent that produced the output.
    let agentID: String
    /// The stage where validation failed.
    let stageID: String
    /// The run ID for this validation.
    let runID: UUID

    /// Per-output validation results.
    let outputResults: [OutputValidationResult]

    /// Summary of the failure for operator display.
    let failureSummary: String

    /// The failure class for categorization.
    let failureClass: ValidationFailureClass

    /// Contract metadata that was used for validation.
    let contractMetadata: [ContractValidationMetadata]

    /// Whether the agent produced any raw output before validation failed.
    let rawOutputExists: Bool
    /// Whether a receipt was persisted before validation failed.
    let receiptExists: Bool
    /// Whether a transcript was persisted before validation failed.
    let transcriptExists: Bool

    /// Recovery recommendation based on this failure.
    let recoveryRecommendation: RecoveryRecommendation

    init(
        id: UUID = UUID(),
        timestamp: Date = Date(),
        agentID: String,
        stageID: String,
        runID: UUID,
        outputResults: [OutputValidationResult],
        failureSummary: String,
        failureClass: ValidationFailureClass,
        contractMetadata: [ContractValidationMetadata],
        rawOutputExists: Bool,
        receiptExists: Bool,
        transcriptExists: Bool,
        recoveryRecommendation: RecoveryRecommendation
    ) {
        self.id = id
        self.timestamp = timestamp
        self.agentID = agentID
        self.stageID = stageID
        self.runID = runID
        self.outputResults = outputResults
        self.failureSummary = failureSummary
        self.failureClass = failureClass
        self.contractMetadata = contractMetadata
        self.rawOutputExists = rawOutputExists
        self.receiptExists = receiptExists
        self.transcriptExists = transcriptExists
        self.recoveryRecommendation = recoveryRecommendation
    }
}

// MARK: - Failure Class (§1.1)

/// Categorizes the type of validation failure.
nonisolated enum ValidationFailureClass: String, Codable, Sendable, Equatable {
    /// Agent output does not match declared contract format.
    case outputContractMismatch = "output_contract_mismatch"
    /// Agent produced no output at all.
    case noOutputProduced = "no_output_produced"
    /// Agent produced output but it was empty.
    case emptyOutput = "empty_output"
    /// Agent execution succeeded but persistence failed.
    case persistenceFailure = "persistence_failure"
    /// Transport or connectivity failure.
    case transportFailure = "transport_failure"
    /// Agent explicitly reported failure.
    case agentReportedFailure = "agent_reported_failure"
}

// MARK: - Contract Validation Metadata

/// Metadata about the contract used for validation.
nonisolated struct ContractValidationMetadata: Codable, Sendable, Equatable {
    let outputName: String
    let contractID: String
    let machineFormat: String
    let validationMode: String
    let requiredFieldCount: Int
}

// MARK: - Recovery Recommendation

/// Suggested recovery action derived from the validation failure.
nonisolated struct RecoveryRecommendation: Codable, Sendable, Equatable {
    let action: RecommendedAction
    let explanation: String
    let source: RecommendationSource
}

nonisolated enum RecommendedAction: String, Codable, Sendable, Equatable {
    case retryFailedAgent = "retry_failed_agent"
    case retryFailedStage = "retry_failed_stage"
    case cloneRun = "clone_run"
    case operatorInspection = "operator_inspection"
}

nonisolated enum RecommendationSource: String, Codable, Sendable, Equatable {
    case runtimePolicy = "runtime_policy"
    case operatorOverride = "operator_override"
}
