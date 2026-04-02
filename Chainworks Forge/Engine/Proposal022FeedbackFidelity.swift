import Foundation
import SwiftData

// MARK: - Proposal 022 Feedback Fidelity Scaffolding

struct ReviewCorpusBundle: Codable, Hashable, Sendable {
    let reviewPassID: String
    let reviewIterationID: String
    let sourceProposalArtifact: String
    let rawReviewArtifacts: [String]
    let aggregateSummaryArtifact: String
}

struct ScoreLiftMergeProvenance: Codable, Hashable, Sendable {
    let mergedIssueRefs: [String]
    let rationale: String
}

enum ScoreImpactClass: String, Codable, Hashable, Sendable {
    case ceilingBlocker = "ceiling_blocker"
    case highLift = "high_lift"
    case mediumLift = "medium_lift"
    case lowLift = "low_lift"
}

enum ScoreLiftBacklogStatus: String, Codable, Hashable, Sendable {
    case open
    case resolved
    case partiallyResolved = "partially_resolved"
    case deferred
    case disputed
    case reopened
}

struct ScoreLiftBacklogItem: Codable, Hashable, Sendable {
    let id: String
    let reviewPassID: String
    let sourceReviewer: String
    let severity: String
    let blocker: Bool
    let category: String
    let scoreImpactClass: ScoreImpactClass
    let description: String
    let evidenceRefs: [String]
    let status: ScoreLiftBacklogStatus
    let mergeProvenance: ScoreLiftMergeProvenance?
    let writerCoverageRef: String?
    let lastTouchedIteration: Int
}

struct ProposalFeedbackCoverageRecord: Codable, Hashable, Sendable {
    let proposalRevisionID: String
    let sourceReviewPassID: String
    let backlogItemsAddressed: [String]
    let backlogItemsUnresolved: [String]
    let backlogItemsDeferred: [String]
    let backlogItemsDisputed: [String]
    let sectionsChanged: [String]
    let factualClaimsAddedOrCorrected: [String]
    let notes: String
}

struct ProposalFactDigest: Codable, Hashable, Sendable {
    struct Claim: Codable, Hashable, Sendable {
        let claimID: String
        let statement: String
        let evidenceRefs: [String]
        let verificationState: String
    }

    let proposalRevisionID: String
    let claims: [Claim]
}

// MARK: - Proposal 022 Report Surface Summary

struct ProposalLoopFeedbackSummary: Codable, Hashable, Sendable {
    let reviewPassID: String?
    let reviewIterationID: String?
    let sourceProposalArtifact: String?
    let reviewCorpusBundlePresent: Bool
    let reviewCorpusRawArtifactCount: Int?
    let backlogItemCount: Int
    let unresolvedItemCount: Int
    let deferredItemCount: Int
    let disputedItemCount: Int
    let partiallyResolvedItemCount: Int
    let addressedItemCount: Int
    let mergeProvenanceItemCount: Int
    let unresolvedHighLiftCount: Int?
    let targetedReviewerSummary: String?
    let coverageStatusSummary: String
    let proposalByteSize: Int?
    let previousProposalByteSize: Int?
    let proposalGrowthRatio: Double?
    let scoreDeltaSinceLastReview: Double?
    let backlogItemsClosedCount: Int?
    let reopenedItemCount: Int?
    let growthGuardRecommendation: String?
    let boundedNextAction: String?
}

enum ProposalLoopFeedbackArtifactName {
    nonisolated static let reviewCorpusBundle = "review_corpus_bundle"
    nonisolated static let proposalReviewArchitect = "proposal_review_architect"
    nonisolated static let proposalReviewPO = "proposal_review_po"
    nonisolated static let proposalReviewUI = "proposal_review_ui"
    nonisolated static let proposalReviewUX = "proposal_review_ux"
    nonisolated static let scoreLiftBacklog = "score_lift_backlog"
    nonisolated static let proposalFeedbackCoverage = "proposal_feedback_coverage"
    nonisolated static let proposalReviewSummary = "proposal_review_summary"
    nonisolated static let proposalFactDigest = "proposal_fact_digest"
    nonisolated static let reviewerScopePlan = "reviewer_scope_plan"
}

enum ProposalLoopFeedbackParser {
    static func parseSummary(from artifacts: [Artifact]) -> ProposalLoopFeedbackSummary? {
        let bundleData = newestArtifactData(for: ProposalLoopFeedbackArtifactName.reviewCorpusBundle, from: artifacts)
        let backlogData = newestArtifactData(for: ProposalLoopFeedbackArtifactName.scoreLiftBacklog, from: artifacts)
        let coverageData = newestArtifactData(for: ProposalLoopFeedbackArtifactName.proposalFeedbackCoverage, from: artifacts)
        guard bundleData != nil || backlogData != nil || coverageData != nil else { return nil }

        var reviewPassID: String?
        var reviewIterationID: String?
        var sourceProposalArtifact: String?
        var reviewCorpusBundlePresent = false
        var reviewCorpusRawArtifactCount: Int?
        var backlogItemCount = 0
        var unresolvedItemCount = 0
        var deferredItemCount = 0
        var disputedItemCount = 0
        var partiallyResolvedItemCount = 0
        var mergeProvenanceItemCount = 0
        var unresolvedHighLiftCount: Int?
        var proposalByteSize: Int?
        var previousProposalByteSize: Int?
        var proposalGrowthRatio: Double?
        var scoreDeltaSinceLastReview: Double?
        var backlogItemsClosedCount: Int?
        var reopenedItemCount: Int?
        var growthGuardRecommendation: String?
        var boundedNextAction: String?

        var targetedSummary: String?

        if let bundleData {
            let bundle = parseReviewCorpusBundlePayload(from: bundleData)
            reviewPassID = bundle.reviewPassID ?? reviewPassID
            reviewIterationID = bundle.reviewIterationID ?? reviewIterationID
            sourceProposalArtifact = bundle.sourceProposalArtifact ?? sourceProposalArtifact
            reviewCorpusBundlePresent = bundle.isPresent
            reviewCorpusRawArtifactCount = bundle.rawReviewArtifactCount ?? reviewCorpusRawArtifactCount
        }

        if let backlogData {
            let payload = parseScoreLiftPayload(from: backlogData)
            reviewPassID = payload.reviewPassID ?? reviewPassID
            reviewIterationID = payload.reviewIterationID ?? reviewIterationID
            sourceProposalArtifact = payload.sourceProposalArtifact ?? sourceProposalArtifact
            backlogItemCount = payload.items.count
            unresolvedItemCount += payload.unresolvedCount
            deferredItemCount += payload.deferredCount
            disputedItemCount += payload.disputedCount
            partiallyResolvedItemCount += payload.partiallyResolvedCount
            mergeProvenanceItemCount += payload.mergeProvenanceItemCount
            unresolvedHighLiftCount = payload.unresolvedHighLiftCount ?? unresolvedHighLiftCount
            proposalByteSize = payload.proposalByteSize ?? proposalByteSize
            previousProposalByteSize = payload.previousProposalByteSize ?? previousProposalByteSize
            proposalGrowthRatio = payload.proposalGrowthRatio ?? proposalGrowthRatio
            scoreDeltaSinceLastReview = payload.scoreDeltaSinceLastReview ?? scoreDeltaSinceLastReview
            backlogItemsClosedCount = payload.backlogItemsClosedCount ?? backlogItemsClosedCount
            reopenedItemCount = payload.reopenedItemCount ?? reopenedItemCount
            growthGuardRecommendation = payload.growthGuardRecommendation ?? growthGuardRecommendation
            boundedNextAction = payload.boundedNextAction ?? boundedNextAction
            targetedSummary = payload.targetedReviewerSummary
        }

        var addressedItemCount = 0
        var unresolvedCoverageCount = 0
        var deferredCoverageCount = 0
        var disputedCoverageCount = 0

        if let coverageData {
            let coverage = parseCoveragePayload(from: coverageData)
            addressedItemCount += coverage.addressedCount
            unresolvedCoverageCount += coverage.unresolvedCount
            deferredCoverageCount += coverage.deferredCount
            disputedCoverageCount += coverage.disputedCount
        }

        let unresolvedItemDelta = max(0, unresolvedCoverageCount)
        let effectiveUnresolved = unresolvedItemCount > 0 ? unresolvedItemCount : unresolvedItemDelta
        let effectiveDeferred = deferredItemCount > 0 ? deferredItemCount : deferredCoverageCount
        let effectiveDisputed = disputedItemCount > 0 ? disputedItemCount : disputedCoverageCount

        let coverageSummary = [
            addressedItemCount > 0 ? "addressed \(addressedItemCount)" : nil,
            unresolvedCoverageCount > 0 ? "unresolved \(unresolvedCoverageCount)" : nil,
            deferredCoverageCount > 0 ? "deferred \(deferredCoverageCount)" : nil,
            disputedCoverageCount > 0 ? "disputed \(disputedCoverageCount)" : nil
        ]
        .compactMap { $0 }
        .joined(separator: ", ")

        return ProposalLoopFeedbackSummary(
            reviewPassID: reviewPassID,
            reviewIterationID: reviewIterationID,
            sourceProposalArtifact: sourceProposalArtifact,
            reviewCorpusBundlePresent: reviewCorpusBundlePresent,
            reviewCorpusRawArtifactCount: reviewCorpusRawArtifactCount,
            backlogItemCount: backlogItemCount,
            unresolvedItemCount: effectiveUnresolved,
            deferredItemCount: effectiveDeferred,
            disputedItemCount: effectiveDisputed,
            partiallyResolvedItemCount: partiallyResolvedItemCount,
            addressedItemCount: addressedItemCount,
            mergeProvenanceItemCount: mergeProvenanceItemCount,
            unresolvedHighLiftCount: unresolvedHighLiftCount,
            targetedReviewerSummary: targetedSummary,
            coverageStatusSummary: coverageSummary.isEmpty ? "coverage unavailable" : coverageSummary,
            proposalByteSize: proposalByteSize,
            previousProposalByteSize: previousProposalByteSize,
            proposalGrowthRatio: proposalGrowthRatio,
            scoreDeltaSinceLastReview: scoreDeltaSinceLastReview,
            backlogItemsClosedCount: backlogItemsClosedCount,
            reopenedItemCount: reopenedItemCount,
            growthGuardRecommendation: growthGuardRecommendation,
            boundedNextAction: boundedNextAction
        )
    }

    nonisolated private static func newestArtifactData(for name: String, from artifacts: [Artifact]) -> Data? {
        let candidate = artifacts
            .filter { $0.name == name }
            .max { $0.createdAt < $1.createdAt }
        guard let artifact = candidate else { return nil }
        return try? Data(contentsOf: URL(fileURLWithPath: artifact.filePath))
    }

    private static func parseReviewCorpusBundlePayload(from data: Data) -> (
        reviewPassID: String?,
        reviewIterationID: String?,
        sourceProposalArtifact: String?,
        rawReviewArtifactCount: Int?,
        isPresent: Bool
    ) {
        guard let object = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) else {
            return (nil, nil, nil, nil, false)
        }

        return (
            reviewPassID: object["review_pass_id"] as? String,
            reviewIterationID: object["review_iteration_id"] as? String,
            sourceProposalArtifact: object["source_proposal_artifact"] as? String,
            rawReviewArtifactCount: parseStringArray(object["raw_review_artifacts"]).count,
            isPresent: true
        )
    }

    private static func parseScoreLiftPayload(from data: Data) -> (
        reviewPassID: String?,
        reviewIterationID: String?,
        sourceProposalArtifact: String?,
        unresolvedHighLiftCount: Int?,
        proposalByteSize: Int?,
        previousProposalByteSize: Int?,
        proposalGrowthRatio: Double?,
        scoreDeltaSinceLastReview: Double?,
        backlogItemsClosedCount: Int?,
        reopenedItemCount: Int?,
        growthGuardRecommendation: String?,
        boundedNextAction: String?,
        items: [String],
        unresolvedCount: Int,
        deferredCount: Int,
        disputedCount: Int,
        partiallyResolvedCount: Int,
        mergeProvenanceItemCount: Int,
        targetedReviewerSummary: String?
    ) {
        guard let object = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) else {
            return (
                reviewPassID: nil,
                reviewIterationID: nil,
                sourceProposalArtifact: nil,
                unresolvedHighLiftCount: nil,
                proposalByteSize: nil,
                previousProposalByteSize: nil,
                proposalGrowthRatio: nil,
                scoreDeltaSinceLastReview: nil,
                backlogItemsClosedCount: nil,
                reopenedItemCount: nil,
                growthGuardRecommendation: nil,
                boundedNextAction: nil,
                items: [],
                unresolvedCount: 0,
                deferredCount: 0,
                disputedCount: 0,
                partiallyResolvedCount: 0,
                mergeProvenanceItemCount: 0,
                targetedReviewerSummary: nil
            )
        }

        let reviewPassID = object["review_pass_id"] as? String
        let reviewIterationID = object["review_iteration_id"] as? String
        let sourceProposalArtifact = object["source_proposal_artifact"] as? String
        let unresolvedHighLiftCount = parseInt(object["unresolved_high_lift_count"])
        let proposalByteSize = parseInt(object["proposal_byte_size"])
        let previousProposalByteSize = parseInt(object["previous_proposal_byte_size"])
        let proposalGrowthRatio = parseDouble(object["proposal_growth_ratio"])
        let scoreDeltaSinceLastReview = parseDouble(object["score_delta_since_last_review"])
        let backlogItemsClosedCount = parseInt(object["backlog_items_closed_count"])
        let reopenedItemCount = parseInt(object["reopened_item_count"])
        let growthGuardRecommendation = object["growth_guard_recommendation"] as? String
        let boundedNextAction = object["bounded_next_action"] as? String

        var unresolvedCount = 0
        var deferredCount = 0
        var disputedCount = 0
        var partiallyResolvedCount = 0
        var mergeProvenanceItemCount = 0
        var itemIDs: [String] = []

        let items = (object["items"] as? [[String: Any]]) ?? []
        for item in items {
            if let id = item["id"] as? String { itemIDs.append(id) }
            if hasMergeProvenance(item) {
                mergeProvenanceItemCount += 1
            }
            let status = (item["status"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            switch status {
            case "open":
                unresolvedCount += 1
            case "deferred":
                deferredCount += 1
            case "disputed":
                disputedCount += 1
            case "partially_resolved", "partial", "partially-resolved":
                partiallyResolvedCount += 1
            default:
                break
            }
        }

        let targetedSummary = parseReviewerScopeSummary(from: object["reviewer_scope_summary"] as? [String: Any])
        return (
            reviewPassID: reviewPassID,
            reviewIterationID: reviewIterationID,
            sourceProposalArtifact: sourceProposalArtifact,
            unresolvedHighLiftCount: unresolvedHighLiftCount,
            proposalByteSize: proposalByteSize,
            previousProposalByteSize: previousProposalByteSize,
            proposalGrowthRatio: proposalGrowthRatio,
            scoreDeltaSinceLastReview: scoreDeltaSinceLastReview,
            backlogItemsClosedCount: backlogItemsClosedCount,
            reopenedItemCount: reopenedItemCount,
            growthGuardRecommendation: growthGuardRecommendation,
            boundedNextAction: boundedNextAction,
            items: itemIDs,
            unresolvedCount: unresolvedCount,
            deferredCount: deferredCount,
            disputedCount: disputedCount,
            partiallyResolvedCount: partiallyResolvedCount,
            mergeProvenanceItemCount: mergeProvenanceItemCount,
            targetedReviewerSummary: targetedSummary
        )
    }

    private static func parseCoveragePayload(from data: Data) -> (
        addressedCount: Int,
        unresolvedCount: Int,
        deferredCount: Int,
        disputedCount: Int
    ) {
        guard let object = (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) else {
            return (0, 0, 0, 0)
        }

        let unresolvedItems = parseStringArray(object["backlog_items_unresolved"])
        let deferredItems = parseStringArray(object["backlog_items_deferred"])
        let disputedItems = parseStringArray(object["backlog_items_disputed"])
        let resolvedItems = parseStringArray(object["backlog_items_addressed"])

        return (
            addressedCount: resolvedItems.count,
            unresolvedCount: unresolvedItems.count,
            deferredCount: deferredItems.count,
            disputedCount: disputedItems.count
        )
    }

    private static func parseReviewerScopeSummary(from object: [String: Any]?) -> String? {
        guard let object else { return nil }

        let full = parseStringArray(object["full_rerun"]).sorted()
        let delta = parseStringArray(object["delta_rerun"]).sorted()
        let verification = parseStringArray(object["verification"] ?? object["verification_only"]).sorted()

        guard !full.isEmpty || !delta.isEmpty || !verification.isEmpty else { return nil }

        let fullLabel = "full: \(full.isEmpty ? "none" : full.joined(separator: ", "))"
        let deltaLabel = "delta: \(delta.isEmpty ? "none" : delta.joined(separator: ", "))"
        let verificationLabel = "verification: \(verification.isEmpty ? "none" : verification.joined(separator: ", "))"

        return [fullLabel, deltaLabel, verificationLabel].joined(separator: "; ")
    }

    private static func parseInt(_ value: Any?) -> Int? {
        switch value {
        case let number as NSNumber:
            return number.intValue
        case let text as String:
            return Int(text)
        default:
            return nil
        }
    }

    private static func parseDouble(_ value: Any?) -> Double? {
        switch value {
        case let number as NSNumber:
            return number.doubleValue
        case let text as String:
            return Double(text)
        default:
            return nil
        }
    }

    nonisolated private static func parseStringArray(_ value: Any?) -> [String] {
        guard let array = value as? [Any] else { return [] }
        return array.compactMap { item in
            if let text = item as? String {
                return text
            }
            if let object = item as? [String: Any], let id = object["id"] as? String {
                return id
            }
            return nil
        }
    }

    nonisolated static func backlogContainsMergeProvenance(from artifacts: [Artifact]) -> Bool {
        guard let data = newestArtifactData(for: ProposalLoopFeedbackArtifactName.scoreLiftBacklog, from: artifacts),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return false
        }

        let items = (object["items"] as? [[String: Any]]) ?? []
        return items.contains(where: hasMergeProvenance)
    }

    nonisolated private static func hasMergeProvenance(_ item: [String: Any]) -> Bool {
        guard let merge = item["merge_provenance"] as? [String: Any] else { return false }
        let refs = parseStringArray(merge["merged_issue_refs"])
        let rationale = (merge["rationale"] as? String)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return !refs.isEmpty && !rationale.isEmpty
    }
}

struct Proposal022AppProofResult: Codable, Sendable {
    let runID: UUID
    let refineReviewCorpusInputs: [String]
    let refineCorpusInputCount: Int
    let reviewCorpusBundleExists: Bool
    let reviewCorpusBundleConsumed: Bool
    let scoreLiftBacklogExists: Bool
    let scoreLiftBacklogMergeProvenanceExists: Bool
    let proposalFeedbackCoverageExists: Bool
    let unresolvedBacklogItemIDs: [String]
    let targetedRerunRationale: String
    let terminalStatus: String
    let proofStatus: String
}

struct Proposal022AppProofExport: Codable, Sendable {
    let result: Proposal022AppProofResult
    let summary: ProposalLoopFeedbackSummary
}

enum Proposal022AppProofHarnessError: LocalizedError {
    case missingWorkflow
    case missingCatalog
    case runDisappeared
    case unableToComputeSummary
    case missingWriterStage
    case missingWriterAgent
    case missingRefineInputBindings
    case missingSecondReviewStage
    case terminalFailure(String)
    case timedOut

    var errorDescription: String? {
        switch self {
        case .missingWorkflow:
            return "Could not locate proposal-loop-live workflow for Proposal 022 proof."
        case .missingCatalog:
            return "Could not locate agents catalog for Proposal 022 proof."
        case .runDisappeared:
            return "Proposal 022 proof run disappeared before reaching the required checkpoint."
        case .unableToComputeSummary:
            return "Proposal 022 proof could not derive review-feedback summary from proof artifacts."
        case .missingWriterStage:
            return "Proposal 022 proof did not reach the refine stage."
        case .missingWriterAgent:
            return "Proposal 022 proof did not produce the proposal_writer execution."
        case .missingRefineInputBindings:
            return "Proposal 022 proof could not read persisted refine input bindings."
        case .missingSecondReviewStage:
            return "Proposal 022 proof did not persist the second review pass."
        case .terminalFailure(let message):
            return message
        case .timedOut:
            return "Proposal 022 proof timed out before reaching the review-to-approval checkpoint."
        }
    }
}

@MainActor
final class Proposal022AppProofHarness {
    private let modelContext: ModelContext
    private let executionService: ExecutionService

    init(modelContext: ModelContext, executionService: ExecutionService) {
        self.modelContext = modelContext
        self.executionService = executionService
    }

    func run() async throws -> (Run, ProposalLoopFeedbackSummary, Proposal022AppProofResult) {
        let compiler = RunPlanCompiler(modelContext: modelContext)
        let workflowURL = try resolveWorkflowURL()
        let catalogURL = try resolveCatalogURL()
        let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
        let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
        let plan = try compiler.previewCompile(workflow: workflow, catalog: catalog)

        let idea = Idea(
            title: "Proposal 022 App Proof",
            body: "Fixture-backed review-corpus fidelity proof for Proposal 022.",
            workspaceRootPath: AppConfiguration.defaultRepositoryRoot().path
        )
        modelContext.insert(idea)

        let (run, workspace) = try compiler.createRun(
            for: idea,
            plan: plan,
            workflowSourcePath: workflowURL.path,
            catalogSourcePath: catalogURL.path
        )

        executionService.startRun(run: run, plan: plan, workspace: workspace)
        let proofRun = try await waitForProofCheckpoint(runID: run.id)

        let allProofArtifacts = allArtifacts(for: proofRun)
        guard let summary = ProposalLoopFeedbackParser.parseSummary(from: allProofArtifacts) else {
            throw Proposal022AppProofHarnessError.unableToComputeSummary
        }

        guard let writerStage = proofRun.stageExecutions
            .filter({ $0.stageID == "state_4_proposal_refined" && $0.status == .completed })
            .sorted(by: { lhs, rhs in
                if lhs.iteration != rhs.iteration { return lhs.iteration < rhs.iteration }
                return lhs.startedAt < rhs.startedAt
            })
            .last else {
            throw Proposal022AppProofHarnessError.missingWriterStage
        }
        guard let writerAgent = writerStage.agentExecutions.first(where: { $0.agentID == "proposal_writer" }) else {
            throw Proposal022AppProofHarnessError.missingWriterAgent
        }
        guard let consumedData = writerAgent.consumedInputArtifactNamesJSON,
              let consumedInputs = try? JSONDecoder().decode([String].self, from: consumedData) else {
            throw Proposal022AppProofHarnessError.missingRefineInputBindings
        }

        let refineCorpusInputs = [
            ProposalLoopFeedbackArtifactName.proposalReviewArchitect,
            ProposalLoopFeedbackArtifactName.proposalReviewPO,
            ProposalLoopFeedbackArtifactName.proposalReviewUI,
            ProposalLoopFeedbackArtifactName.proposalReviewUX,
            ProposalLoopFeedbackArtifactName.proposalReviewSummary
        ]
        .filter(Set(consumedInputs).contains)
        .sorted()

        let reviewCorpusBundleExists = allProofArtifacts.contains(where: { $0.name == ProposalLoopFeedbackArtifactName.reviewCorpusBundle })
        let reviewCorpusBundleConsumed = Set(consumedInputs).contains(ProposalLoopFeedbackArtifactName.reviewCorpusBundle)
        let backlogExists = Set(consumedInputs).contains(ProposalLoopFeedbackArtifactName.scoreLiftBacklog)
            && allProofArtifacts.contains(where: { $0.name == ProposalLoopFeedbackArtifactName.scoreLiftBacklog })
        let mergeProvenanceExists = ProposalLoopFeedbackParser.backlogContainsMergeProvenance(from: allProofArtifacts)
        let coverageExists = writerAgent.artifacts.contains(where: { $0.name == ProposalLoopFeedbackArtifactName.proposalFeedbackCoverage })
        let unresolvedIDs = unresolvedBacklogItemIDs(from: allProofArtifacts)
        let targetedRerunRationale = summary.targetedReviewerSummary ?? "missing targeted rerun rationale"

        let proofPassed = Self.isCanonicalPass(
            terminalStatus: proofRun.status,
            refineCorpusInputCount: refineCorpusInputs.count,
            reviewCorpusBundleExists: reviewCorpusBundleExists,
            reviewCorpusBundleConsumed: reviewCorpusBundleConsumed,
            scoreLiftBacklogExists: backlogExists,
            scoreLiftBacklogMergeProvenanceExists: mergeProvenanceExists,
            proposalFeedbackCoverageExists: coverageExists,
            unresolvedBacklogItemCount: unresolvedIDs.count,
            targetedRerunRationale: targetedRerunRationale
        )

        let result = Proposal022AppProofResult(
            runID: proofRun.id,
            refineReviewCorpusInputs: refineCorpusInputs,
            refineCorpusInputCount: refineCorpusInputs.count,
            reviewCorpusBundleExists: reviewCorpusBundleExists,
            reviewCorpusBundleConsumed: reviewCorpusBundleConsumed,
            scoreLiftBacklogExists: backlogExists,
            scoreLiftBacklogMergeProvenanceExists: mergeProvenanceExists,
            proposalFeedbackCoverageExists: coverageExists,
            unresolvedBacklogItemIDs: unresolvedIDs,
            targetedRerunRationale: targetedRerunRationale,
            terminalStatus: proofRun.status.rawValue,
            proofStatus: proofPassed
                ? "PASS — Proposal 022 app proof verified"
                : "FAIL — Proposal 022 app proof did not expose bundle/backlog/coverage/merge/rerun truth"
        )

        return (proofRun, summary, result)
    }

    func runAndPersist(to url: URL) async throws -> Proposal022AppProofExport {
        let (_, summary, result) = try await run()
        let export = Proposal022AppProofExport(result: result, summary: summary)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let directory = url.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try encoder.encode(export).write(to: url, options: .atomic)
        return export
    }

    private func allArtifacts(for run: Run) -> [Artifact] {
        run.stageExecutions
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)
            .sorted { $0.createdAt < $1.createdAt }
    }

    private func unresolvedBacklogItemIDs(from artifacts: [Artifact]) -> [String] {
        guard let data = artifacts
            .filter({ $0.name == ProposalLoopFeedbackArtifactName.scoreLiftBacklog })
            .max(by: { $0.createdAt < $1.createdAt })
            .flatMap({ try? Data(contentsOf: URL(fileURLWithPath: $0.filePath)) }),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return []
        }

        let items = (object["items"] as? [[String: Any]]) ?? []
        return items.compactMap { item in
            let status = (item["status"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            guard status == "open" || status == "reopened" || status == "partially_resolved" else {
                return nil
            }
            return item["id"] as? String
        }
    }

    private func resolveWorkflowURL() throws -> URL {
        if let bundled = Bundle.main.url(forResource: "proposal-loop-live", withExtension: "yaml") {
            return bundled
        }
        let fallback = AppConfiguration.defaultRepositoryRoot()
            .appendingPathComponent("examples/workflows/proposal-loop-live.yaml")
        guard FileManager.default.fileExists(atPath: fallback.path) else {
            throw Proposal022AppProofHarnessError.missingWorkflow
        }
        return fallback
    }

    private func resolveCatalogURL() throws -> URL {
        if let bundled = Bundle.main.url(forResource: "agents", withExtension: "yaml") {
            return bundled
        }
        let fallback = AppConfiguration.defaultRepositoryRoot()
            .appendingPathComponent("examples/agents/agents.yaml")
        guard FileManager.default.fileExists(atPath: fallback.path) else {
            throw Proposal022AppProofHarnessError.missingCatalog
        }
        return fallback
    }

    private func waitForProofCheckpoint(runID: UUID) async throws -> Run {
        let deadline = Date().addingTimeInterval(25)
        while Date() < deadline {
            let descriptor = FetchDescriptor<Run>()
            guard let run = try modelContext.fetch(descriptor).first(where: { $0.id == runID }) else {
                throw Proposal022AppProofHarnessError.runDisappeared
            }

            let reviewStages = run.stageExecutions.filter { $0.stageID == "state_3_proposal_reviewed" && $0.status == .completed }
            let writerStages = run.stageExecutions.filter { $0.stageID == "state_4_proposal_refined" && $0.status == .completed }
            if reviewStages.count >= 2, !writerStages.isEmpty {
                return run
            }

            switch run.status {
            case .failed, .cancelled:
                throw Proposal022AppProofHarnessError.terminalFailure(
                    "Proposal 022 proof run ended as \(run.status.rawValue) before persisting the required review/refine checkpoint."
                )
            default:
                break
            }

            try await Task.sleep(for: .milliseconds(100))
        }

        throw Proposal022AppProofHarnessError.timedOut
    }

    nonisolated static func isCanonicalPass(
        terminalStatus: RunStatus,
        refineCorpusInputCount: Int,
        reviewCorpusBundleExists: Bool,
        reviewCorpusBundleConsumed: Bool,
        scoreLiftBacklogExists: Bool,
        scoreLiftBacklogMergeProvenanceExists: Bool,
        proposalFeedbackCoverageExists: Bool,
        unresolvedBacklogItemCount: Int,
        targetedRerunRationale: String
    ) -> Bool {
        _ = terminalStatus
        return refineCorpusInputCount == 5 &&
            reviewCorpusBundleExists &&
            reviewCorpusBundleConsumed &&
            scoreLiftBacklogExists &&
            scoreLiftBacklogMergeProvenanceExists &&
            proposalFeedbackCoverageExists &&
            unresolvedBacklogItemCount > 0 &&
            !targetedRerunRationale.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
            targetedRerunRationale != "missing targeted rerun rationale"
    }
}
