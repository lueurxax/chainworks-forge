import Foundation

struct RolloutDecisionSummary: Codable, Equatable, Sendable {
    let schemaVersion: String
    let authoritativeRecordID: String?
    let runID: String?
    let proposalID: String?
    let proposalRevisionID: String?
    let status: String
    let backendDecision: String
    let failureReasons: [String]
    let waiverState: String
    let waiverExpiresAt: String?
    let enforcementMode: String
    let enforcementModeReason: String?
    let holdConditions: [String]
    let rollbackDisposition: RollbackDisposition
    let enabledState: String
    let disabledReasonCode: String?
    let actionID: String?
    let operatorMessage: String
    let sourceLane: String
    let projectionIntegrity: String
    let cutoverPolicyRevision: String?
    let diagnosticRedaction: String
    let nextSteps: [String]
    let updatedAt: String?

    struct RollbackDisposition: Codable, Equatable, Sendable {
        let mode: String
        let dataLossRisk: String
        let steps: [String]

        init(mode: String, dataLossRisk: String, steps: [String]) {
            self.mode = mode
            self.dataLossRisk = dataLossRisk
            self.steps = steps
        }

        enum CodingKeys: String, CodingKey {
            case mode
            case dataLossRisk = "data_loss_risk"
            case steps
        }

        enum GraphQLCodingKeys: String, CodingKey {
            case mode
            case dataLossRisk
            case steps
        }

        init(from decoder: Decoder) throws {
            if let container = try? decoder.container(keyedBy: CodingKeys.self),
               container.contains(.dataLossRisk) {
                self.mode = try container.decode(String.self, forKey: .mode)
                self.dataLossRisk = try container.decode(String.self, forKey: .dataLossRisk)
                self.steps = try container.decodeIfPresent([String].self, forKey: .steps) ?? []
                return
            }
            let container = try decoder.container(keyedBy: GraphQLCodingKeys.self)
            self.mode = try container.decode(String.self, forKey: .mode)
            self.dataLossRisk = try container.decode(String.self, forKey: .dataLossRisk)
            self.steps = try container.decodeIfPresent([String].self, forKey: .steps) ?? []
        }
    }

    init(
        schemaVersion: String,
        authoritativeRecordID: String?,
        runID: String?,
        proposalID: String?,
        proposalRevisionID: String?,
        status: String,
        backendDecision: String,
        failureReasons: [String],
        waiverState: String,
        waiverExpiresAt: String?,
        enforcementMode: String,
        enforcementModeReason: String?,
        holdConditions: [String],
        rollbackDisposition: RollbackDisposition,
        enabledState: String,
        disabledReasonCode: String?,
        actionID: String?,
        operatorMessage: String,
        sourceLane: String,
        projectionIntegrity: String,
        cutoverPolicyRevision: String?,
        diagnosticRedaction: String,
        nextSteps: [String],
        updatedAt: String?
    ) {
        self.schemaVersion = schemaVersion
        self.authoritativeRecordID = authoritativeRecordID
        self.runID = runID
        self.proposalID = proposalID
        self.proposalRevisionID = proposalRevisionID
        self.status = status
        self.backendDecision = backendDecision
        self.failureReasons = failureReasons
        self.waiverState = waiverState
        self.waiverExpiresAt = waiverExpiresAt
        self.enforcementMode = enforcementMode
        self.enforcementModeReason = enforcementModeReason
        self.holdConditions = holdConditions
        self.rollbackDisposition = rollbackDisposition
        self.enabledState = enabledState
        self.disabledReasonCode = disabledReasonCode
        self.actionID = actionID
        self.operatorMessage = operatorMessage
        self.sourceLane = sourceLane
        self.projectionIntegrity = projectionIntegrity
        self.cutoverPolicyRevision = cutoverPolicyRevision
        self.diagnosticRedaction = diagnosticRedaction
        self.nextSteps = nextSteps
        self.updatedAt = updatedAt
    }

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case authoritativeRecordID = "authoritative_record_id"
        case runID = "run_id"
        case proposalID = "proposal_id"
        case proposalRevisionID = "proposal_revision_id"
        case status
        case backendDecision = "backend_decision"
        case failureReasons = "failure_reasons"
        case waiverState = "waiver_state"
        case waiverExpiresAt = "waiver_expires_at"
        case enforcementMode = "enforcement_mode"
        case enforcementModeReason = "enforcement_mode_reason"
        case holdConditions = "hold_conditions"
        case rollbackDisposition = "rollback_disposition"
        case enabledState = "enabled_state"
        case disabledReasonCode = "disabled_reason_code"
        case actionID = "action_id"
        case operatorMessage = "operator_message"
        case sourceLane = "source_lane"
        case projectionIntegrity = "projection_integrity"
        case cutoverPolicyRevision = "cutover_policy_revision"
        case diagnosticRedaction = "diagnostic_redaction"
        case nextSteps = "next_steps"
        case updatedAt = "updated_at"
    }

    enum GraphQLCodingKeys: String, CodingKey {
        case schemaVersion
        case authoritativeRecordID = "authoritativeRecordId"
        case runID = "runId"
        case proposalID = "proposalId"
        case proposalRevisionID = "proposalRevisionId"
        case status
        case backendDecision
        case failureReasons
        case waiverState
        case waiverExpiresAt
        case enforcementMode
        case enforcementModeReason
        case holdConditions
        case rollbackDisposition
        case enabledState
        case disabledReasonCode
        case actionID = "actionId"
        case operatorMessage
        case sourceLane
        case projectionIntegrity
        case cutoverPolicyRevision
        case diagnosticRedaction
        case nextSteps
        case updatedAt
    }

    init(from decoder: Decoder) throws {
        if let container = try? decoder.container(keyedBy: CodingKeys.self),
           container.contains(.schemaVersion) {
            self.schemaVersion = try container.decode(String.self, forKey: .schemaVersion)
            self.authoritativeRecordID = try container.decodeIfPresent(String.self, forKey: .authoritativeRecordID)
            self.runID = try container.decodeIfPresent(String.self, forKey: .runID)
            self.proposalID = try container.decodeIfPresent(String.self, forKey: .proposalID)
            self.proposalRevisionID = try container.decodeIfPresent(String.self, forKey: .proposalRevisionID)
            self.status = try container.decode(String.self, forKey: .status)
            self.backendDecision = try container.decode(String.self, forKey: .backendDecision)
            self.failureReasons = try container.decode([String].self, forKey: .failureReasons)
            self.waiverState = try container.decode(String.self, forKey: .waiverState)
            self.waiverExpiresAt = try container.decodeIfPresent(String.self, forKey: .waiverExpiresAt)
            self.enforcementMode = try container.decode(String.self, forKey: .enforcementMode)
            self.enforcementModeReason = try container.decodeIfPresent(String.self, forKey: .enforcementModeReason)
            self.holdConditions = try container.decode([String].self, forKey: .holdConditions)
            self.rollbackDisposition = try container.decode(RollbackDisposition.self, forKey: .rollbackDisposition)
            self.enabledState = try container.decode(String.self, forKey: .enabledState)
            self.disabledReasonCode = try container.decodeIfPresent(String.self, forKey: .disabledReasonCode)
            self.actionID = try container.decodeIfPresent(String.self, forKey: .actionID)
            self.operatorMessage = try container.decode(String.self, forKey: .operatorMessage)
            self.sourceLane = try container.decode(String.self, forKey: .sourceLane)
            self.projectionIntegrity = try container.decode(String.self, forKey: .projectionIntegrity)
            self.cutoverPolicyRevision = try container.decodeIfPresent(String.self, forKey: .cutoverPolicyRevision)
            self.diagnosticRedaction = try container.decode(String.self, forKey: .diagnosticRedaction)
            self.nextSteps = try container.decode([String].self, forKey: .nextSteps)
            self.updatedAt = try container.decodeIfPresent(String.self, forKey: .updatedAt)
            return
        }

        let container = try decoder.container(keyedBy: GraphQLCodingKeys.self)
        self.schemaVersion = try container.decode(String.self, forKey: .schemaVersion)
        self.authoritativeRecordID = try container.decodeIfPresent(String.self, forKey: .authoritativeRecordID)
        self.runID = try container.decodeIfPresent(String.self, forKey: .runID)
        self.proposalID = try container.decodeIfPresent(String.self, forKey: .proposalID)
        self.proposalRevisionID = try container.decodeIfPresent(String.self, forKey: .proposalRevisionID)
        self.status = try container.decode(String.self, forKey: .status)
        self.backendDecision = try container.decode(String.self, forKey: .backendDecision)
        self.failureReasons = try container.decode([String].self, forKey: .failureReasons)
        self.waiverState = try container.decode(String.self, forKey: .waiverState)
        self.waiverExpiresAt = try container.decodeIfPresent(String.self, forKey: .waiverExpiresAt)
        self.enforcementMode = try container.decode(String.self, forKey: .enforcementMode)
        self.enforcementModeReason = try container.decodeIfPresent(String.self, forKey: .enforcementModeReason)
        self.holdConditions = try container.decode([String].self, forKey: .holdConditions)
        self.rollbackDisposition = try container.decode(RollbackDisposition.self, forKey: .rollbackDisposition)
        self.enabledState = try container.decode(String.self, forKey: .enabledState)
        self.disabledReasonCode = try container.decodeIfPresent(String.self, forKey: .disabledReasonCode)
        self.actionID = try container.decodeIfPresent(String.self, forKey: .actionID)
        self.operatorMessage = try container.decode(String.self, forKey: .operatorMessage)
        self.sourceLane = try container.decode(String.self, forKey: .sourceLane)
        self.projectionIntegrity = try container.decode(String.self, forKey: .projectionIntegrity)
        self.cutoverPolicyRevision = try container.decodeIfPresent(String.self, forKey: .cutoverPolicyRevision)
        self.diagnosticRedaction = try container.decode(String.self, forKey: .diagnosticRedaction)
        self.nextSteps = try container.decode([String].self, forKey: .nextSteps)
        self.updatedAt = try container.decodeIfPresent(String.self, forKey: .updatedAt)
    }
}
