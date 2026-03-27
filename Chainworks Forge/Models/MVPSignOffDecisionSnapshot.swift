import Foundation
import SwiftData

@Model final class MVPSignOffDecisionSnapshot {
    @Attribute(.unique) var id: UUID
    private(set) var evaluatorVersion: String
    private(set) var cohortID: UUID
    var evaluatedAt: Date
    var decision: SignOffDecision

    var medianManualOrchestrationSeconds: Double?
    var medianAppOrchestrationSeconds: Double?
    var medianImprovementPercent: Double?
    var medianProposalApprovalSeconds: Double?
    var medianImplementationApprovalSeconds: Double?
    var medianReleaseDecisionSeconds: Double?

    private(set) var failingGateReasonsJSON: Data
    private(set) var decisionPayloadJSON: Data
    private(set) var payloadChecksum: String

    var pairCount: Int
    var happyPathCount: Int
    var recoveredCount: Int

    // Computed accessor for failingGateReasons
    var failingGateReasons: [String] {
        get {
            (try? JSONDecoder().decode([String].self, from: failingGateReasonsJSON)) ?? []
        }
        set {
            failingGateReasonsJSON = (try? JSONEncoder().encode(newValue)) ?? Data()
        }
    }

    // Computed accessor for decisionPayload
    var decisionPayload: [String: String] {
        get {
            (try? JSONDecoder().decode([String: String].self, from: decisionPayloadJSON)) ?? [:]
        }
        set {
            decisionPayloadJSON = (try? JSONEncoder().encode(newValue)) ?? Data()
        }
    }

    init(
        id: UUID = UUID(),
        evaluatorVersion: String,
        cohortID: UUID,
        evaluatedAt: Date = Date(),
        decision: SignOffDecision,
        payloadChecksum: String,
        pairCount: Int,
        happyPathCount: Int,
        recoveredCount: Int,
        failingGateReasons: [String] = [],
        decisionPayloadJSON: Data = Data()
    ) {
        self.id = id
        self.evaluatorVersion = evaluatorVersion
        self.cohortID = cohortID
        self.evaluatedAt = evaluatedAt
        self.decision = decision
        self.payloadChecksum = payloadChecksum
        self.pairCount = pairCount
        self.happyPathCount = happyPathCount
        self.recoveredCount = recoveredCount
        self.failingGateReasonsJSON = (try? JSONEncoder().encode(failingGateReasons)) ?? Data()
        self.decisionPayloadJSON = decisionPayloadJSON
    }
}

enum SignOffDecision: String, Codable {
    case go
    case hold
}
