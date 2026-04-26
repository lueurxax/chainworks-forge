import Foundation

nonisolated struct ImplementationHandoffStatus: Codable, Equatable, Sendable {
    static let schemaVersion = "p017_implementation_handoff_status_v1"

    let schemaVersion: String
    let runID: String
    let currentStateID: String
    let taskName: String
    let requiredInputArtifacts: [String]
    let availableInputArtifacts: [String]
    let missingInputArtifacts: [String]
    let approvedProposalPresent: Bool
    let approvedProposalArtifactID: String?
    let approvedProposalDigest: String?
    let worktreeRoot: String?
    let workspaceRoot: String
    let artifactRoot: String
    let codeWriterStartStatus: String
    let status: String
    let missingHandoffOutputs: [String]
    let lastHandoffAgentExecutionID: String?
    let retryableFrom: String?
    let blockedBeforeCodeReason: String?
    let updatedAt: String

    init(
        schemaVersion: String = ImplementationHandoffStatus.schemaVersion,
        runID: String,
        currentStateID: String,
        taskName: String,
        requiredInputArtifacts: [String],
        availableInputArtifacts: [String],
        missingInputArtifacts: [String],
        approvedProposalPresent: Bool,
        approvedProposalArtifactID: String? = nil,
        approvedProposalDigest: String? = nil,
        worktreeRoot: String?,
        workspaceRoot: String,
        artifactRoot: String,
        codeWriterStartStatus: String,
        status: String,
        missingHandoffOutputs: [String] = [],
        lastHandoffAgentExecutionID: String? = nil,
        retryableFrom: String? = nil,
        blockedBeforeCodeReason: String? = nil,
        updatedAt: String
    ) {
        self.schemaVersion = schemaVersion
        self.runID = runID
        self.currentStateID = currentStateID
        self.taskName = taskName
        self.requiredInputArtifacts = requiredInputArtifacts
        self.availableInputArtifacts = availableInputArtifacts
        self.missingInputArtifacts = missingInputArtifacts
        self.approvedProposalPresent = approvedProposalPresent
        self.approvedProposalArtifactID = approvedProposalArtifactID
        self.approvedProposalDigest = approvedProposalDigest
        self.worktreeRoot = worktreeRoot
        self.workspaceRoot = workspaceRoot
        self.artifactRoot = artifactRoot
        self.codeWriterStartStatus = codeWriterStartStatus
        self.status = status
        self.missingHandoffOutputs = missingHandoffOutputs
        self.lastHandoffAgentExecutionID = lastHandoffAgentExecutionID
        self.retryableFrom = retryableFrom
        self.blockedBeforeCodeReason = blockedBeforeCodeReason
        self.updatedAt = updatedAt
    }

    enum CodingKeys: String, CodingKey {
        case schemaVersion
        case runID
        case currentStateID
        case taskName
        case requiredInputArtifacts
        case availableInputArtifacts
        case missingInputArtifacts
        case approvedProposalPresent
        case approvedProposalArtifactID
        case approvedProposalDigest
        case worktreeRoot
        case workspaceRoot
        case artifactRoot
        case codeWriterStartStatus
        case status
        case missingHandoffOutputs
        case lastHandoffAgentExecutionID
        case retryableFrom
        case blockedBeforeCodeReason
        case updatedAt
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try container.decode(String.self, forKey: .schemaVersion)
        runID = try container.decode(String.self, forKey: .runID)
        currentStateID = try container.decode(String.self, forKey: .currentStateID)
        taskName = try container.decode(String.self, forKey: .taskName)
        requiredInputArtifacts = try container.decode([String].self, forKey: .requiredInputArtifacts)
        availableInputArtifacts = try container.decode([String].self, forKey: .availableInputArtifacts)
        missingInputArtifacts = try container.decode([String].self, forKey: .missingInputArtifacts)
        approvedProposalPresent = try container.decode(Bool.self, forKey: .approvedProposalPresent)
        approvedProposalArtifactID = try container.decodeIfPresent(
            String.self, forKey: .approvedProposalArtifactID)
        approvedProposalDigest = try container.decodeIfPresent(
            String.self, forKey: .approvedProposalDigest)
        worktreeRoot = try container.decodeIfPresent(String.self, forKey: .worktreeRoot)
        workspaceRoot = try container.decode(String.self, forKey: .workspaceRoot)
        artifactRoot = try container.decode(String.self, forKey: .artifactRoot)
        codeWriterStartStatus = try container.decode(String.self, forKey: .codeWriterStartStatus)
        status = try container.decode(String.self, forKey: .status)
        missingHandoffOutputs = try container.decodeIfPresent(
            [String].self, forKey: .missingHandoffOutputs) ?? missingInputArtifacts
        lastHandoffAgentExecutionID = try container.decodeIfPresent(
            String.self, forKey: .lastHandoffAgentExecutionID)
        retryableFrom = try container.decodeIfPresent(String.self, forKey: .retryableFrom)
        blockedBeforeCodeReason = try container.decodeIfPresent(
            String.self, forKey: .blockedBeforeCodeReason)
        updatedAt = try container.decode(String.self, forKey: .updatedAt)
    }
}

enum WorkflowConflictReason: String, Codable, Sendable {
    case invalidNextStageHint = "invalid_next_stage_hint"
    case noDeclarativeTransitionMatched = "no_declarative_transition_matched"
    case multipleDeclarativeTransitionsMatchedWithoutTieBreak = "multiple_declarative_transitions_matched_without_tie_break"
    case requiredArtifactOrFieldMissingForTransition = "required_artifact_or_field_missing_for_transition"
    case aggregateTransitionTruthConflicted = "aggregate_transition_truth_conflicted"
    case workflowConflictUnverifiable = "workflow_conflict_unverifiable"
    case implementationHandoffUnavailable = "implementation_handoff_unavailable"
}

enum WorkflowConflictStatus: String, Codable, Sendable {
    case unresolved
    case leadMediationPending = "lead_mediation_pending"
    case operatorConfirmationRequired = "operator_confirmation_required"
    case resolved
    case superseded
    case terminalUnverifiable = "terminal_unverifiable"

    var isCurrentBlocking: Bool {
        switch self {
        case .unresolved, .leadMediationPending, .operatorConfirmationRequired:
            return true
        case .resolved, .superseded, .terminalUnverifiable:
            return false
        }
    }
}

enum CandidateTransitionResult: String, Codable, Sendable {
    case matched
    case notMatched = "not_matched"
    case missingInput = "missing_input"
    case invalidExpression = "invalid_expression"
    case evaluationError = "evaluation_error"
}

struct CandidateTransitionEvaluation: Codable, Equatable, Sendable {
    let transitionID: String
    let fromStateID: String
    let toStateID: String
    let conditionExpressionID: String?
    let result: CandidateTransitionResult
    let requiredArtifacts: [String]
    let missingArtifacts: [String]
    let missingFields: [String]
    let sourceArtifactIDs: [String]
    let sourceAgentExecutionID: String?
    let sanitizedDiagnostic: String?
}

struct WorkflowConflictRecord: Codable, Sendable {
    static let bridgeSchemaVersion = "p017_conflict_record_v1"

    let schemaVersion: String
    let conflictID: String
    let conflictFingerprint: String
    let runID: String
    let stageExecutionID: String?
    let lineageID: String?
    let currentStateID: String
    let reason: WorkflowConflictReason
    let operatorLabel: String
    let status: WorkflowConflictStatus
    let candidateTransitions: [CandidateTransitionEvaluation]
    let candidateTransitionHash: String
    let advisoryEvidenceRefs: [String]
    let leadAgentID: String?
    let mediationRecordID: String?
    let createdAt: String
    let updatedAt: String
    let resolvedAt: String?
    let supersededByConflictID: String?
    let resolutionRecordJSON: AnyCodableValue?
    let terminalFailureReason: String?
    let diagnosticRedactionTier: String

    init(
        schemaVersion: String = WorkflowConflictRecord.bridgeSchemaVersion,
        conflictID: String,
        conflictFingerprint: String,
        runID: String,
        stageExecutionID: String? = nil,
        lineageID: String? = nil,
        currentStateID: String,
        reason: WorkflowConflictReason,
        operatorLabel: String,
        status: WorkflowConflictStatus,
        candidateTransitions: [CandidateTransitionEvaluation],
        candidateTransitionHash: String,
        advisoryEvidenceRefs: [String] = [],
        leadAgentID: String? = nil,
        mediationRecordID: String? = nil,
        createdAt: String,
        updatedAt: String,
        resolvedAt: String? = nil,
        supersededByConflictID: String? = nil,
        resolutionRecordJSON: AnyCodableValue? = nil,
        terminalFailureReason: String? = nil,
        diagnosticRedactionTier: String = "operator_safe"
    ) {
        self.schemaVersion = schemaVersion
        self.conflictID = conflictID
        self.conflictFingerprint = conflictFingerprint
        self.runID = runID
        self.stageExecutionID = stageExecutionID
        self.lineageID = lineageID
        self.currentStateID = currentStateID
        self.reason = reason
        self.operatorLabel = operatorLabel
        self.status = status
        self.candidateTransitions = candidateTransitions
        self.candidateTransitionHash = candidateTransitionHash
        self.advisoryEvidenceRefs = advisoryEvidenceRefs
        self.leadAgentID = leadAgentID
        self.mediationRecordID = mediationRecordID
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.resolvedAt = resolvedAt
        self.supersededByConflictID = supersededByConflictID
        self.resolutionRecordJSON = resolutionRecordJSON
        self.terminalFailureReason = terminalFailureReason
        self.diagnosticRedactionTier = diagnosticRedactionTier
    }
}

struct WorkflowAdvisoryRejectionRecord: Codable, Equatable, Sendable {
    let rejectionID: String
    let runID: String
    let stageExecutionID: String?
    let lineageID: String?
    let currentStateID: String
    let selectedTransitionID: String
    let selectedNextStateID: String
    let advisoryNextStageHint: String?
    let advisoryNextAction: String?
    let advisoryHintHash: String
    let advisoryHintProvenance: [AdvisoryHintExtraction]
    let graphMembershipResult: String
    let createdAt: String
}

struct AdvisoryHintExtraction: Codable, Equatable, Sendable {
    let sourceArtifactID: String
    let sourceAgentExecutionID: String?
    let advisoryPath: String
    let rawValueHash: String
    let redactedValue: String?
    let graphMembershipResult: String
    let supersededByProjection: Bool
    let includedInCandidateTransitionHash: Bool
}

nonisolated struct WorkflowConflictBridgeV1: Codable, Sendable {
    var conflicts: [WorkflowConflictRecord] = []
    var advisoryRejections: [WorkflowAdvisoryRejectionRecord] = []
}

struct WorkflowConflictReportSnapshot: Codable, Sendable {
    let current: WorkflowConflictRecord?
    let history: [WorkflowConflictRecord]
    let advisoryRejections: [WorkflowAdvisoryRejectionRecord]
    let blockedReason: String?
    let leadOwner: String?
    let validNextActionClass: String?
    let candidateTransitionMatrix: [CandidateTransitionEvaluation]
    let resolutionRecordJSON: AnyCodableValue?

    @MainActor
    static func make(from run: Run) -> WorkflowConflictReportSnapshot? {
        let bridge = run.workflowConflictBridgeV1
        guard !bridge.conflicts.isEmpty || !bridge.advisoryRejections.isEmpty else {
            return nil
        }
        let current = run.currentWorkflowConflictRecord
        return WorkflowConflictReportSnapshot(
            current: current,
            history: bridge.conflicts,
            advisoryRejections: bridge.advisoryRejections,
            blockedReason: current?.operatorLabel,
            leadOwner: current?.leadAgentID,
            validNextActionClass: current?.status.isCurrentBlocking == true
                ? "await_conflict_resolution"
                : nil,
            candidateTransitionMatrix: current?.candidateTransitions ?? [],
            resolutionRecordJSON: current?.resolutionRecordJSON
        )
    }
}

extension WorkflowConflictRecord {
    func updatingStatus(
        _ status: WorkflowConflictStatus,
        updatedAt: String,
        resolvedAt: String? = nil,
        supersededByConflictID: String? = nil,
        resolutionRecordJSON: AnyCodableValue? = nil,
        terminalFailureReason: String? = nil
    ) -> WorkflowConflictRecord {
        WorkflowConflictRecord(
            schemaVersion: schemaVersion,
            conflictID: conflictID,
            conflictFingerprint: conflictFingerprint,
            runID: runID,
            stageExecutionID: stageExecutionID,
            lineageID: lineageID,
            currentStateID: currentStateID,
            reason: reason,
            operatorLabel: operatorLabel,
            status: status,
            candidateTransitions: candidateTransitions,
            candidateTransitionHash: candidateTransitionHash,
            advisoryEvidenceRefs: advisoryEvidenceRefs,
            leadAgentID: leadAgentID,
            mediationRecordID: mediationRecordID,
            createdAt: createdAt,
            updatedAt: updatedAt,
            resolvedAt: resolvedAt ?? self.resolvedAt,
            supersededByConflictID: supersededByConflictID ?? self.supersededByConflictID,
            resolutionRecordJSON: resolutionRecordJSON ?? self.resolutionRecordJSON,
            terminalFailureReason: terminalFailureReason ?? self.terminalFailureReason,
            diagnosticRedactionTier: diagnosticRedactionTier
        )
    }

    static func graphResolutionRecord(
        selectedTransitionID: String,
        selectedNextStateID: String,
        stageExecutionID: UUID?
    ) -> AnyCodableValue {
        var record: [String: AnyCodableValue] = [
            "resolution_owner": .string("compiled_workflow_graph"),
            "selected_transition_id": .string(selectedTransitionID),
            "selected_next_state_id": .string(selectedNextStateID)
        ]
        if let stageExecutionID {
            record["stage_execution_id"] = .string(stageExecutionID.uuidString)
        }
        return .dictionary(record)
    }
}
