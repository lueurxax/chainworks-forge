import Foundation
import SwiftData
import Testing
@testable import Chainworks_Forge

@MainActor
@Suite("Active Execution Uniqueness Guard", .serialized, .tags(.fast))
struct ActiveExecutionUniquenessGuardTests {
    @Test("Claiming stage execution repairs older active sibling in same lineage")
    func claimingStageExecutionRepairsOlderActiveSiblingInSameLineage() throws {
        let context = try makeTestModelContext()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)
        let guardrail = ActiveExecutionUniquenessGuard(modelContext: context)

        let older = StageExecution(
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            startedAt: Date(timeIntervalSince1970: 10),
            status: .running,
            iteration: 1,
            attemptNumber: 1
        )
        older.lineageID = "state_2_proposal_drafted::iteration:1"
        older.activeOwnerToken = "stale-owner"
        older.run = run
        run.stageExecutions.append(older)
        context.insert(older)

        let claimed = guardrail.claimOrCreateStageExecution(
            run: run,
            stageID: "state_2_proposal_drafted",
            label: "Proposal drafted",
            iteration: 1
        )

        #expect(claimed.id == older.id)
        #expect(claimed.status == .running)
        #expect(claimed.activeOwnerToken != nil)
        #expect(claimed.activeOwnerToken != "stale-owner")
        #expect(run.stageExecutions.filter {
            $0.lineageID == "state_2_proposal_drafted::iteration:1" && $0.activeOwnerToken != nil
        }.count == 1)
    }

    @Test("Claiming requested approval expires older duplicate request in same lineage")
    func claimingRequestedApprovalExpiresOlderDuplicateRequestInSameLineage() throws {
        let context = try makeTestModelContext()
        let workspace = makeTestWorkspace()
        let run = makeTestRun(workspace: workspace, context: context)
        let guardrail = ActiveExecutionUniquenessGuard(modelContext: context)

        let older = Approval(stageID: "state_4_proposal_approval")
        older.decision = .requested
        older.requestedAt = Date(timeIntervalSince1970: 20)
        older.lineageID = "state_4_proposal_approval::iteration:1::approval"
        older.run = run
        run.approvals.append(older)
        context.insert(older)

        let newer = Approval(stageID: "state_4_proposal_approval")
        newer.decision = .requested
        newer.requestedAt = Date(timeIntervalSince1970: 30)
        newer.lineageID = "state_4_proposal_approval::iteration:1::approval"
        newer.run = run
        run.approvals.append(newer)
        context.insert(newer)

        let canonical = guardrail.claimOrCreateRequestedApproval(
            run: run,
            stageID: "state_4_proposal_approval",
            lineageID: "state_4_proposal_approval::iteration:1::approval"
        )

        #expect(canonical.id == newer.id)
        #expect(canonical.decision == .requested)
        #expect(older.decision == .expired)
        #expect(older.repairedAt != nil)
        #expect(run.approvals.filter {
            $0.lineageID == "state_4_proposal_approval::iteration:1::approval" && $0.decision == .requested
        }.count == 1)
    }
}
