import Foundation
import SwiftData

// MARK: - P005-OPS §8: Run Comparison Service

/// Deterministic structural comparison for compatible proposal-loop runs.
/// Limited to: same idea, same workflow family, current proposal-loop baseline.
/// Does NOT compare worktree paths, git receipts, or release artifacts (Proposal 007).
@MainActor
struct RunComparisonService {

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

    private enum StrategyTelemetryField {
        static let proofOwner = "shell_comparison_lane"
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
        let proposalLoopComparison = proposalLoopFeedbackComparison(for: runA, forAgainst: runB)

        let strategyComparison = strategyComparison(for: runA, against: runB)

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
            pinnedArtifactDiff: pinnedDiff,
            strategyComparison: strategyComparison.comparison,
            strategyRecommendation: strategyComparison.recommendation,
            proposalLoopComparison: proposalLoopComparison
        )
    }

    // MARK: - Helpers

    private func elapsedTime(for run: Run) -> Double {
        let end = run.completedAt ?? Date()
        return end.timeIntervalSince(run.startedAt)
    }

    private func extractBindings(from run: Run) -> [RunComparison.AgentBinding] {
        // Proposal 011 (REQ-008): Read from frozen binding snapshot first.
        let frozenBindings: [String: ResolvedProviderBinding]
        if let data = run.providerBindingSnapshotJSON,
           let decoded = try? JSONDecoder().decode([String: ResolvedProviderBinding].self, from: data) {
            frozenBindings = decoded
        } else {
            frozenBindings = [:]
        }

        // Proposal 011 (REQ-009): Read frozen provenance.
        let frozenProvenances: [String: FrozenBindingProvenance]
        if let data = run.bindingProvenanceJSON,
           let decoded = try? JSONDecoder().decode([String: FrozenBindingProvenance].self, from: data) {
            frozenProvenances = decoded
        } else {
            frozenProvenances = [:]
        }
        let resolvedSkills = decodeResolvedSkills(from: run)
        let catalogSkillRefs = decodeCatalogSkillRefs(from: run)
        let frozenMCPPolicies = decodeFrozenMCPPolicies(from: run)

        let allAgents = run.stageExecutions.flatMap { $0.agentExecutions }
        var seen = Set<String>()
        var bindings: [RunComparison.AgentBinding] = []
        for agent in allAgents {
            guard !seen.contains(agent.agentID) else { continue }
            seen.insert(agent.agentID)
            let frozenModel = frozenBindings[agent.agentID]?.model
            let provenanceSource = frozenProvenances[agent.agentID]?.source.rawValue
            let skillRef = agent.skillRef ?? catalogSkillRefs[agent.agentID]
            let resolvedSkill = skillRef.flatMap { resolvedSkills[$0] }
            let mcpResolution = frozenMCPPolicies[agent.agentID]
            bindings.append(RunComparison.AgentBinding(
                agentID: agent.agentID,
                provider: agent.provider,
                model: frozenModel ?? agent.resolvedModel ?? agent.resolvedBackendProfileID,
                effort: agent.effort,
                provenanceSource: provenanceSource,
                providerFamily: frozenBindings[agent.agentID]?.providerFamily,
                skillRef: skillRef,
                skillType: agent.skillType,
                skillRole: agent.skillRole,
                skillContentSummary: agent.skillContentSummary,
                skillSnapshotHash: agent.skillSnapshotHash,
                resolvedSkillContent: resolvedSkill?.resolvedContent,
                mcpProfileID: agent.mcpProfileID,
                requestedMCPExtensions: decodeStringArray(from: agent.requestedMCPExtensionsJSON),
                predictedMCPExtensions: mcpResolution?.predictedEffectiveRuntimeExtensionIDs ?? [],
                actualMCPExtensions: decodeStringArray(from: agent.effectiveMCPRuntimeExtensionIDsJSON),
                deniedMCPExtensions: decodeStringArray(from: agent.deniedMCPExtensionsJSON)
            ))
        }
        return bindings
    }

    private func decodeResolvedSkills(from run: Run) -> [String: ResolvedSkill] {
        guard let data = run.resolvedSkillsJSON else { return [:] }
        return (try? JSONDecoder().decode([String: ResolvedSkill].self, from: data)) ?? [:]
    }

    private func decodeCatalogSkillRefs(from run: Run) -> [String: String] {
        guard let catalog = try? JSONDecoder().decode(AgentCatalog.self, from: run.catalogSnapshotJSON) else {
            return [:]
        }
        return Dictionary(uniqueKeysWithValues: catalog.agents.map { ($0.id, $0.skillRef) })
    }

    private func decodeFrozenMCPPolicies(from run: Run) -> [String: MCPPolicyResolutionReport] {
        guard let data = run.resolvedMCPPoliciesJSON else { return [:] }
        return (try? JSONDecoder().decode([String: MCPPolicyResolutionReport].self, from: data)) ?? [:]
    }

    private func decodeStringArray(from data: Data?) -> [String] {
        guard let data, let decoded = try? JSONDecoder().decode([String].self, from: data) else {
            return []
        }
        return decoded
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

    // MARK: - Strategy Recommendation (§019)

    private func strategyComparison(for runA: Run, against runB: Run) -> (comparison: RunComparison.StrategyComparison, recommendation: StrategyRecommendation) {
        let profileA = strategyProfileID(for: runA)
        let profileB = strategyProfileID(for: runB)
        let modeA = strategyAssignmentMode(for: runA)
        let modeB = strategyAssignmentMode(for: runB)
        let evidenceA = strategyEvidenceSnapshot(for: runA)
        let evidenceB = strategyEvidenceSnapshot(for: runB)

        guard
            let profileA,
            let profileB,
            let modeA,
            let modeB,
            evidenceA.evidenceComplete,
            evidenceB.evidenceComplete
        else {
            let evaluationSet = evaluationSetSummary(for: runA, runB)
            return (
                comparison: RunComparison.StrategyComparison(
                    profileA: profileA,
                    profileB: profileB,
                    assignmentModeA: modeA,
                    assignmentModeB: modeB,
                    evidenceComplete: false,
                    comparisonSignal: nil,
                    qualityDeltaSummary: nil
                ),
                recommendation: StrategyRecommendation(
                    status: .insufficientEvidence,
                    proofOwner: StrategyTelemetryField.proofOwner,
                    evaluationSetComplete: false,
                    evaluationSetSummary: evaluationSet,
                    holdCriteria: [
                        "Canonical strategy profile IDs must be present for both runs.",
                        "Strategy telemetry set must include canonical session KPI export JSON.",
                        "Canonical assignment metadata must be complete."
                    ],
                    recommendedProfileID: nil,
                    rationale: "Insufficient evidence to emit a safe strategy recommendation."
                )
            )
        }

        if profileA == profileB {
            let evaluationSet = evaluationSetSummary(for: runA, runB)
            return (
                comparison: RunComparison.StrategyComparison(
                    profileA: profileA,
                    profileB: profileB,
                    assignmentModeA: modeA,
                    assignmentModeB: modeB,
                    evidenceComplete: true,
                    comparisonSignal: 0,
                    qualityDeltaSummary: "Same strategy profile was used for both runs."
                ),
                recommendation: StrategyRecommendation(
                    status: .notEvaluated,
                    proofOwner: StrategyTelemetryField.proofOwner,
                    evaluationSetComplete: false,
                    evaluationSetSummary: evaluationSet,
                    holdCriteria: [
                        "A comparison requires different strategy profiles."
                    ],
                    recommendedProfileID: nil,
                    rationale: "Runs used the same strategy profile; comparison is not actionable."
                )
            )
        }

        let scoreA = strategyScore(for: runA, profileID: profileA, evidence: evidenceA)
        let scoreB = strategyScore(for: runB, profileID: profileB, evidence: evidenceB)

        let comparisonSignal = scoreA - scoreB
        let recommendationProfileID = comparisonSignal > 0 ? profileA : profileB
        let winner = comparisonSignal > 0 ? "A" : comparisonSignal < 0 ? "B" : "tie"
        let status: StrategyRecommendationStatus = winner == "tie" ? .inconclusive : .candidateWinner
        let evaluationSet = evaluationSetSummary(for: runA, runB)

        let rationale = makeRecommendationRationale(
            winner: winner,
            profileA: profileA,
            profileB: profileB,
            modeA: modeA,
            modeB: modeB,
            scoreA: scoreA,
            scoreB: scoreB
        )

        return (
            comparison: RunComparison.StrategyComparison(
                profileA: profileA,
                profileB: profileB,
                assignmentModeA: modeA,
                assignmentModeB: modeB,
                evidenceComplete: true,
                comparisonSignal: comparisonSignal,
                qualityDeltaSummary: "ScoreA \(String(format: "%.2f", scoreA)) vs ScoreB \(String(format: "%.2f", scoreB))"
            ),
            recommendation: StrategyRecommendation(
                status: status,
                proofOwner: StrategyTelemetryField.proofOwner,
                evaluationSetComplete: true,
                evaluationSetSummary: evaluationSet,
                holdCriteria: [
                    "Canonical telemetry from session KPI export must be present for both runs.",
                    "Recommendation can only be made from a compatibility-checked pair."
                ],
                recommendedProfileID: winner == "tie" ? nil : recommendationProfileID,
                rationale: rationale
            )
        )
    }

    private func strategyProfileID(for run: Run) -> String? {
        let value = run.contextStrategyProfileID.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private func strategyAssignmentMode(for run: Run) -> String? {
        let value = run.strategyAssignmentMode.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private func strategyEvidenceSnapshot(for run: Run) -> (evidenceComplete: Bool, summary: SessionReuseKPIExporter.RunKPISummary?) {
        let recommendationState = run.strategyRecommendationState.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !recommendationState.isEmpty || run.sessionKPIExportJSON != nil else {
            return (false, nil)
        }

        let summary = SessionReuseKPIExporter.decodeSummary(from: run.sessionKPIExportJSON)
        return (SessionReuseKPIExporter.hasCanonicalStrategyTelemetry(summary), summary)
    }

    private func strategyScore(
        for run: Run,
        profileID: String,
        evidence: (evidenceComplete: Bool, summary: SessionReuseKPIExporter.RunKPISummary?)
    ) -> Double {
        let durationPenalty = elapsedTime(for: run) / 60_000.0
        let qualityComponent = qualityComponent(for: run)
        let reliabilityPenalty = (evidence.summary?.totalExecutions ?? 0) > 0 ? 0.0 : 1.0
        let costPenalty = Double(run.totalCostCents ?? 0) / 2_500.0
        let profilePenalty = profilePenalty(for: profileID)
        let telemetry = evidence.summary?.strategyTelemetry
        let reductionReward = Double(telemetry?.totalPayloadReductionBytes ?? 0) / 1_024.0
        let lazyHitReward = (telemetry?.averageLazyEvidenceHitRate ?? 0.0) * 10.0
        let cacheReward = (telemetry?.averageCacheEffectiveness ?? 0.0) * 25.0
        let escalationPenalty = Double(telemetry?.totalEscalationCount ?? 0) * 12.0
        let retryableEscalationPenalty = Double(telemetry?.totalRetryableEscalationCount ?? 0) * 8.0
        let contractPenalty = Double(telemetry?.totalContractFailureCount ?? 0) * 25.0
        let compactionPenalty = Double(telemetry?.totalCompactionChurn ?? 0) * 3.0
        let operatorPenalty = Double(telemetry?.totalPromotedArtifactUsages ?? 0) * 2.0

        return (qualityComponent * 100.0)
            + reductionReward
            + lazyHitReward
            + cacheReward
            - durationPenalty
            - costPenalty
            - reliabilityPenalty
            - escalationPenalty
            - retryableEscalationPenalty
            - contractPenalty
            - compactionPenalty
            - operatorPenalty
            - profilePenalty
    }

    private func evaluationSetSummary(for runA: Run, _ runB: Run) -> String {
        let ideaID = runA.idea?.id.uuidString ?? "unknown-idea"
        let family = runA.workflowFamily ?? runA.workflowID
        let pair = "\(runA.id.uuidString.prefix(8)) vs \(runB.id.uuidString.prefix(8))"
        return "paired canonical comparison (\(pair)) for idea \(ideaID), workflow family \(family)"
    }

    private func qualityComponent(for run: Run) -> Double {
        let approvals = run.approvals
        let requested = max(1, approvals.count)
        let granted = approvals.filter { $0.decision == .granted }.count
        let completion = run.status == .completed ? 1.0 : 0.0
        let failures = Double(completionFails(for: run))
        return completion + (Double(granted) / Double(requested)) - (0.25 * failures)
    }

    private func completionFails(for run: Run) -> Int {
        run.stageExecutions.filter { $0.status == .failed || $0.status == .blocked }.count
    }

    private func profilePenalty(for profileID: String) -> Double {
        max(0.0, 0.5 * Double(profileID.count - 10))
    }

    private func makeRecommendationRationale(
        winner: String,
        profileA: String,
        profileB: String,
        modeA: String,
        modeB: String,
        scoreA: Double,
        scoreB: Double
        ) -> String {
        switch winner {
        case "A":
            return "Shell recommendation: profile '\(profileA)' (mode '\(modeA)') outperformed '\(profileB)' (mode '\(modeB)') in this canonical paired comparison. ScoreA=\(String(format: "%.2f", scoreA)), ScoreB=\(String(format: "%.2f", scoreB))."
        case "B":
            return "Shell recommendation: profile '\(profileB)' (mode '\(modeB)') outperformed '\(profileA)' (mode '\(modeA)') in this canonical paired comparison. ScoreA=\(String(format: "%.2f", scoreA)), ScoreB=\(String(format: "%.2f", scoreB))."
        default:
            return "Shell comparison is currently inconclusive: scoreA=\(String(format: "%.2f", scoreA)), scoreB=\(String(format: "%.2f", scoreB))."
        }
    }

    private func proposalLoopFeedbackComparison(for runA: Run, forAgainst runB: Run) -> RunComparison.ProposalLoopComparison {
        let summaryA = proposalLoopFeedbackSummary(for: runA)
        let summaryB = proposalLoopFeedbackSummary(for: runB)

        guard summaryA != nil || summaryB != nil else {
            return RunComparison.ProposalLoopComparison(
                reviewCorpusBundlePresentA: nil,
                reviewCorpusBundlePresentB: nil,
                mergeProvenanceItemCountA: nil,
                mergeProvenanceItemCountB: nil,
                backlogItemCountA: nil,
                backlogItemCountB: nil,
                unresolvedItemCountA: nil,
                unresolvedItemCountB: nil,
                deferredItemCountA: nil,
                deferredItemCountB: nil,
                addressedItemCountA: nil,
                addressedItemCountB: nil,
                proposalGrowthRatioA: nil,
                proposalGrowthRatioB: nil,
                scoreDeltaA: nil,
                scoreDeltaB: nil,
                targetedRereviewRationaleA: nil,
                targetedRereviewRationaleB: nil,
                unresolvedDelta: nil,
                coverageDelta: nil,
                rationale: "Feedback fidelity summaries are not available for both runs."
            )
        }

        let unresolvedDelta = resolveDelta(summaryA?.unresolvedItemCount, summaryB?.unresolvedItemCount)
        let addressedDelta = resolveDelta(summaryA?.addressedItemCount, summaryB?.addressedItemCount)
        let rationale = proposalLoopRationale(
            summaryA: summaryA,
            summaryB: summaryB,
            unresolvedDelta: unresolvedDelta
        )

        return RunComparison.ProposalLoopComparison(
            reviewCorpusBundlePresentA: summaryA?.reviewCorpusBundlePresent,
            reviewCorpusBundlePresentB: summaryB?.reviewCorpusBundlePresent,
            mergeProvenanceItemCountA: summaryA?.mergeProvenanceItemCount,
            mergeProvenanceItemCountB: summaryB?.mergeProvenanceItemCount,
            backlogItemCountA: summaryA?.backlogItemCount,
            backlogItemCountB: summaryB?.backlogItemCount,
            unresolvedItemCountA: summaryA?.unresolvedItemCount,
            unresolvedItemCountB: summaryB?.unresolvedItemCount,
            deferredItemCountA: summaryA?.deferredItemCount,
            deferredItemCountB: summaryB?.deferredItemCount,
            addressedItemCountA: summaryA?.addressedItemCount,
            addressedItemCountB: summaryB?.addressedItemCount,
            proposalGrowthRatioA: summaryA?.proposalGrowthRatio,
            proposalGrowthRatioB: summaryB?.proposalGrowthRatio,
            scoreDeltaA: summaryA?.scoreDeltaSinceLastReview,
            scoreDeltaB: summaryB?.scoreDeltaSinceLastReview,
            targetedRereviewRationaleA: summaryA?.targetedReviewerSummary,
            targetedRereviewRationaleB: summaryB?.targetedReviewerSummary,
            unresolvedDelta: unresolvedDelta,
            coverageDelta: addressedDelta,
            rationale: rationale
        )
    }

    private func proposalLoopFeedbackSummary(for run: Run) -> ProposalLoopFeedbackSummary? {
        let allArtifacts = run.stageExecutions.flatMap { $0.agentExecutions }.flatMap { $0.artifacts }
        return ProposalLoopFeedbackParser.parseSummary(from: allArtifacts)
    }

    private func proposalLoopRationale(
        summaryA: ProposalLoopFeedbackSummary?,
        summaryB: ProposalLoopFeedbackSummary?,
        unresolvedDelta: Int?
    ) -> String {
        if summaryA == nil && summaryB == nil { return "No feedback fidelity summaries were produced for either run." }
        if summaryA == nil { return "Run A has no proposal-loop feedback summary; unable to compare carry-forward fidelity." }
        if summaryB == nil { return "Run B has no proposal-loop feedback summary; unable to compare carry-forward fidelity." }

        guard let unresolvedDelta else { return "Insufficient data to compute unresolved delta." }
        if unresolvedDelta == 0 {
            return "Unresolved score-limiting backlog did not increase between runs."
        }
        if unresolvedDelta > 0 {
            return "Run B has higher unresolved backlog and likely requires narrower rerun than before."
        }
        return "Run B reduced unresolved backlog versus Run A."
    }

    private func resolveDelta(_ valueA: Int?, _ valueB: Int?) -> Int? {
        guard let valueA, let valueB else { return nil }
        return valueB - valueA
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

    // Strategy comparison
    let strategyComparison: StrategyComparison
    let strategyRecommendation: StrategyRecommendation
    let proposalLoopComparison: ProposalLoopComparison

    struct StrategyComparison {
        let profileA: String?
        let profileB: String?
        let assignmentModeA: String?
        let assignmentModeB: String?
        let evidenceComplete: Bool
        let comparisonSignal: Double?
        let qualityDeltaSummary: String?
    }

    struct AgentBinding: Identifiable {
        let id = UUID()
        let agentID: String
        let provider: String
        let model: String?
        let effort: String
        /// Proposal 011 (REQ-009): Provenance source from frozen data.
        let provenanceSource: String?
        /// Proposal 011 (REQ-010): Provider family for cross-family mismatch detection.
        let providerFamily: String?
        let skillRef: String?
        let skillType: String?
        let skillRole: String?
        let skillContentSummary: String?
        let skillSnapshotHash: String?
        let resolvedSkillContent: String?
        let mcpProfileID: String?
        let requestedMCPExtensions: [String]
        let predictedMCPExtensions: [String]
        let actualMCPExtensions: [String]
        let deniedMCPExtensions: [String]

        /// Heuristic cross-family mismatch check, consistent with `ResolvedProviderBinding.hasCrossFamilyMismatch`.
        var hasCrossFamilyMismatch: Bool {
            guard let model, let providerFamily else { return false }
            let lowerModel = model.lowercased()
            let lowerFamily = providerFamily.lowercased()
            let familyModelPrefixes: [([String], [String])] = [
                (["claude"], ["claude", "anthropic"]),
                (["openai", "codex"], ["gpt", "o1", "o3", "chatgpt"]),
                (["gemini"], ["gemini", "palm"]),
            ]
            for (familyAliases, prefixes) in familyModelPrefixes {
                let modelBelongsToFamily = prefixes.contains(where: { lowerModel.hasPrefix($0) })
                let familyMatches = familyAliases.contains(where: { lowerFamily.contains($0) })
                if modelBelongsToFamily && !familyMatches {
                    return true
                }
            }
            return false
        }
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

    struct ProposalLoopComparison {
        let reviewCorpusBundlePresentA: Bool?
        let reviewCorpusBundlePresentB: Bool?
        let mergeProvenanceItemCountA: Int?
        let mergeProvenanceItemCountB: Int?
        let backlogItemCountA: Int?
        let backlogItemCountB: Int?
        let unresolvedItemCountA: Int?
        let unresolvedItemCountB: Int?
        let deferredItemCountA: Int?
        let deferredItemCountB: Int?
        let addressedItemCountA: Int?
        let addressedItemCountB: Int?
        let proposalGrowthRatioA: Double?
        let proposalGrowthRatioB: Double?
        let scoreDeltaA: Double?
        let scoreDeltaB: Double?
        let targetedRereviewRationaleA: String?
        let targetedRereviewRationaleB: String?
        let unresolvedDelta: Int?
        let coverageDelta: Int?
        let rationale: String
    }
}
