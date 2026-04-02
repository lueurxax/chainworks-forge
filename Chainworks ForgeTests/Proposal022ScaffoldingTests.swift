import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("Proposal 022 Scaffolding")
struct Proposal022ScaffoldingTests {

    @Test("Output templates generate review corpus bundle contract payload")
    func outputTemplatesGenerateReviewCorpusBundlePayload() throws {
        let generated = OutputContractTemplates.generate(
            contractID: "review_corpus_bundle_v1",
            agentID: "lead_orchestrator",
            stageID: "state_3_proposal_reviewed"
        )

        #expect(generated.format == .json)
        let object = try #require(try JSONSerialization.jsonObject(with: generated.data) as? [String: Any])
        #expect(object["review_pass_id"] is String)
        #expect(object["review_iteration_id"] is String)
        #expect(object["source_proposal_artifact"] is String)
        #expect(object["raw_review_artifacts"] is [String])
        #expect(object["aggregate_summary_artifact"] is String)
    }

    @Test("Output templates generate score lift backlog contract payload")
    func outputTemplatesGenerateScoreLiftBacklogPayload() throws {
        let generated = OutputContractTemplates.generate(
            contractID: "score_lift_backlog_v1",
            agentID: "lead_orchestrator",
            stageID: "state_3_proposal_reviewed"
        )

        #expect(generated.format == .json)
        let object = try #require(try JSONSerialization.jsonObject(with: generated.data) as? [String: Any])
        #expect(object["review_pass_id"] is String)
        #expect(object["source_proposal_artifact"] is String)
        #expect(object["proposal_growth_ratio"] is NSNumber)
        #expect(object["score_delta_since_last_review"] is NSNumber)
        #expect(object["growth_guard_recommendation"] is String)
        #expect(object["bounded_next_action"] is String)
        let items = try #require(object["items"] as? [[String: Any]])
        #expect(items.isEmpty == false)
        #expect(items.contains { item in
            guard let merge = item["merge_provenance"] as? [String: Any] else { return false }
            return (merge["merged_issue_refs"] as? [String])?.isEmpty == false
                && merge["rationale"] is String
        })
    }

    @Test("Output templates generate proposal feedback coverage contract payload")
    func outputTemplatesGenerateProposalFeedbackCoveragePayload() throws {
        let generated = OutputContractTemplates.generate(
            contractID: "proposal_feedback_coverage_v1",
            agentID: "proposal_writer",
            stageID: "state_4_proposal_refined"
        )

        #expect(generated.format == .json)
        let object = try #require(try JSONSerialization.jsonObject(with: generated.data) as? [String: Any])
        #expect(object["proposal_revision_id"] is String)
        #expect(object["source_review_pass_id"] is String)
        #expect(object["backlog_items_addressed"] is [[String: Any]])
        #expect(object["sections_changed"] is [String])
    }

    @Test("Review corpus bundle round-trips through Codable")
    func reviewCorpusBundleRoundTrips() throws {
        let bundle = ReviewCorpusBundle(
            reviewPassID: "review-pass-2",
            reviewIterationID: "state_3_proposal_reviewed.2",
            sourceProposalArtifact: "proposal_current",
            rawReviewArtifacts: [
                "proposal_review_po",
                "proposal_review_ux",
                "proposal_review_ui",
                "proposal_review_architect"
            ],
            aggregateSummaryArtifact: "proposal_review_summary"
        )

        let data = try JSONEncoder().encode(bundle)
        let decoded = try JSONDecoder().decode(ReviewCorpusBundle.self, from: data)

        #expect(decoded.reviewPassID == "review-pass-2")
        #expect(decoded.rawReviewArtifacts.count == 4)
        #expect(decoded.aggregateSummaryArtifact == "proposal_review_summary")
    }
}
