import Foundation

nonisolated struct SideEffectReadbackSummary: Codable, Sendable, Equatable {
    let schemaVersion: String
    let runID: String
    let unresolvedCount: Int
    let blocked: Bool
    let readbackSource: String
    let effects: [SideEffectReadbackItem]

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case runID = "run_id"
        case unresolvedCount = "unresolved_count"
        case blocked
        case readbackSource = "readback_source"
        case effects
    }
}

nonisolated struct SideEffectReadbackItem: Codable, Sendable, Equatable {
    let id: String
    let runID: String
    let stageExecutionID: String
    let agentExecutionID: String?
    let effectKind: String
    let status: String
    let targetKey: String
    let externalWriteAttempted: Bool
    let evidenceRoot: String?
    let readbackSource: String
    let reportPath: String?
    let blockedReason: String
    let operatorNextAction: String
    let recommendedMCPTool: String
    let retryForbidden: Bool
    let lastErrorKind: String?
    let updatedAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case runID = "run_id"
        case stageExecutionID = "stage_execution_id"
        case agentExecutionID = "agent_execution_id"
        case effectKind = "effect_kind"
        case status
        case targetKey = "target_key"
        case externalWriteAttempted = "external_write_attempted"
        case evidenceRoot = "evidence_root"
        case readbackSource = "readback_source"
        case reportPath = "report_path"
        case blockedReason = "blocked_reason"
        case operatorNextAction = "operator_next_action"
        case recommendedMCPTool = "recommended_mcp_tool"
        case retryForbidden = "retry_forbidden"
        case lastErrorKind = "last_error_kind"
        case updatedAt = "updated_at"
    }
}
