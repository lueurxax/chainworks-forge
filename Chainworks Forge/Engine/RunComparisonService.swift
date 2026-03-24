import Foundation
import SwiftData

// MARK: - P005-OPS §8: Run Comparison Service

/// Deterministic structural comparison for compatible proposal-loop runs.
/// Limited to: same idea, same workflow family, current proposal-loop baseline.
/// Does NOT compare worktree paths, git receipts, or release artifacts (Proposal 007).
@MainActor
final class RunComparisonService {

    private let modelContext: ModelContext

    init(modelContext: ModelContext) {
        self.modelContext = modelContext
    }

    // MARK: - Compatibility Check (§8.1)

    /// Whether two runs can be compared.
    func areCompatible(_ runA: Run, _ runB: Run) -> Bool {
        // Same idea
        guard runA.idea?.id == runB.idea?.id else { return false }
        // Same workflow family
        guard (runA.workflowFamily ?? runA.workflowID) == (runB.workflowFamily ?? runB.workflowID) else { return false }
        // Both must be from current proposal-loop baseline
        return true
    }

    /// Find all compatible comparison targets for a given run.
    func compatibleTargets(for run: Run) -> [Run] {
        guard let idea = run.idea else { return [] }
        return idea.runs.filter { other in
            other.id != run.id && areCompatible(run, other)
        }
    }

    // MARK: - Comparison (§8.2)

    /// Produce a deterministic comparison between two compatible runs.
    func compare(_ runA: Run, _ runB: Run) -> RunComparison? {
        guard areCompatible(runA, runB) else { return nil }

        let workflowHashMatch = runA.workflowSnapshotHash == runB.workflowSnapshotHash
        let catalogHashMatch = runA.catalogSnapshotHash == runB.catalogSnapshotHash

        // Drift metadata
        let driftA = runA.driftDetails
        let driftB = runB.driftDetails

        // Runtime trust level
        let trustA = runA.runtimeTrustLevel ?? "unknown"
        let trustB = runB.runtimeTrustLevel ?? "unknown"

        // Provider / model / effort bindings
        let bindingsA = extractBindings(from: runA)
        let bindingsB = extractBindings(from: runB)

        // Stage status delta
        let stageDelta = computeStageDelta(runA: runA, runB: runB)

        // Duration delta
        let durationA = elapsedTime(for: runA)
        let durationB = elapsedTime(for: runB)

        // Cost delta
        let costA = runA.totalCostCents ?? 0
        let costB = runB.totalCostCents ?? 0

        // Loop delta
        let loopsA = runA.loopCounters.values.reduce(0, +)
        let loopsB = runB.loopCounters.values.reduce(0, +)

        // Approval delta
        let approvalDelta = computeApprovalDelta(runA: runA, runB: runB)

        // Pinned artifact diff
        let pinnedDiff = computePinnedArtifactDiff(runA: runA, runB: runB)

        return RunComparison(
            runA_ID: runA.id,
            runB_ID: runB.id,
            ideaTitle: runA.idea?.title ?? "Unknown",
            workflowHashMatch: workflowHashMatch,
            catalogHashMatch: catalogHashMatch,
            driftA: driftA,
            driftB: driftB,
            trustLevelA: trustA,
            trustLevelB: trustB,
            bindingsA: bindingsA,
            bindingsB: bindingsB,
            stageDelta: stageDelta,
            durationA: durationA,
            durationB: durationB,
            durationDelta: durationB - durationA,
            costA: costA,
            costB: costB,
            costDelta: costB - costA,
            loopsA: loopsA,
            loopsB: loopsB,
            loopDelta: loopsB - loopsA,
            approvalDelta: approvalDelta,
            pinnedArtifactDiff: pinnedDiff
        )
    }

    // MARK: - Helpers

    private func elapsedTime(for run: Run) -> Double {
        let end = run.completedAt ?? Date()
        return end.timeIntervalSince(run.startedAt)
    }

    private func extractBindings(from run: Run) -> [RunComparison.AgentBinding] {
        let allAgents = run.stageExecutions.flatMap { $0.agentExecutions }
        var seen = Set<String>()
        var bindings: [RunComparison.AgentBinding] = []
        for agent in allAgents {
            guard !seen.contains(agent.agentID) else { continue }
            seen.insert(agent.agentID)
            bindings.append(RunComparison.AgentBinding(
                agentID: agent.agentID,
                provider: agent.provider,
                model: agent.resolvedBackendProfileID,
                effort: agent.effort
            ))
        }
        return bindings
    }

    private func computeStageDelta(runA: Run, runB: Run) -> [RunComparison.StageDelta] {
        let stagesA = Dictionary(grouping: runA.stageExecutions, by: \.stageID)
        let stagesB = Dictionary(grouping: runB.stageExecutions, by: \.stageID)
        let allStageIDs = Set(stagesA.keys).union(stagesB.keys).sorted()

        return allStageIDs.map { stageID in
            let statusA = stagesA[stageID]?.last?.status.rawValue
            let statusB = stagesB[stageID]?.last?.status.rawValue
            return RunComparison.StageDelta(
                stageID: stageID,
                statusA: statusA,
                statusB: statusB,
                changed: statusA != statusB
            )
        }
    }

    private func computeApprovalDelta(runA: Run, runB: Run) -> RunComparison.ApprovalDelta {
        let approvalsA = runA.approvals
        let approvalsB = runB.approvals
        return RunComparison.ApprovalDelta(
            requestedA: approvalsA.count,
            requestedB: approvalsB.count,
            grantedA: approvalsA.filter { $0.decision == .granted }.count,
            grantedB: approvalsB.filter { $0.decision == .granted }.count,
            rejectedA: approvalsA.filter { $0.decision == .rejected }.count,
            rejectedB: approvalsB.filter { $0.decision == .rejected }.count
        )
    }

    private func computePinnedArtifactDiff(runA: Run, runB: Run) -> [RunComparison.PinnedArtifactDelta] {
        let pinnedA = runA.stageExecutions
            .flatMap { $0.agentExecutions }
            .flatMap { $0.artifacts }
            .filter { $0.isPinned }
        let pinnedB = runB.stageExecutions
            .flatMap { $0.agentExecutions }
            .flatMap { $0.artifacts }
            .filter { $0.isPinned }

        let namesA = Set(pinnedA.map(\.name))
        let namesB = Set(pinnedB.map(\.name))
        let allNames = namesA.union(namesB).sorted()

        return allNames.map { name in
            let inA = namesA.contains(name)
            let inB = namesB.contains(name)
            let checksumA = pinnedA.first(where: { $0.name == name })?.checksumSHA256
            let checksumB = pinnedB.first(where: { $0.name == name })?.checksumSHA256
            let contentMatch: Bool? = (inA && inB && checksumA != nil && checksumB != nil) ? (checksumA == checksumB) : nil
            return RunComparison.PinnedArtifactDelta(
                name: name,
                presentInA: inA,
                presentInB: inB,
                contentMatch: contentMatch
            )
        }
    }
}

// MARK: - Comparison Types

struct RunComparison: Identifiable {
    let id = UUID()
    let runA_ID: UUID
    let runB_ID: UUID
    let ideaTitle: String

    // Snapshot
    let workflowHashMatch: Bool
    let catalogHashMatch: Bool
    let driftA: String?
    let driftB: String?

    // Trust
    let trustLevelA: String
    let trustLevelB: String

    // Bindings
    let bindingsA: [AgentBinding]
    let bindingsB: [AgentBinding]

    // Stage delta
    let stageDelta: [StageDelta]

    // Duration
    let durationA: Double
    let durationB: Double
    let durationDelta: Double

    // Cost
    let costA: Int64
    let costB: Int64
    let costDelta: Int64

    // Loops
    let loopsA: Int
    let loopsB: Int
    let loopDelta: Int

    // Approvals
    let approvalDelta: ApprovalDelta

    // Pinned artifacts
    let pinnedArtifactDiff: [PinnedArtifactDelta]

    struct AgentBinding: Identifiable {
        let id = UUID()
        let agentID: String
        let provider: String
        let model: String?
        let effort: String
    }

    struct StageDelta: Identifiable {
        let id = UUID()
        let stageID: String
        let statusA: String?
        let statusB: String?
        let changed: Bool
    }

    struct ApprovalDelta {
        let requestedA: Int
        let requestedB: Int
        let grantedA: Int
        let grantedB: Int
        let rejectedA: Int
        let rejectedB: Int
    }

    struct PinnedArtifactDelta: Identifiable {
        let id = UUID()
        let name: String
        let presentInA: Bool
        let presentInB: Bool
        let contentMatch: Bool?
    }
}
