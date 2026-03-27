import Foundation
import SwiftData
import CryptoKit

// MARK: - MVPSignOffEvaluator (Proposal 008 — §5.5-5.6)

/// Computes GO/HOLD sign-off decision from persisted benchmark records ONLY.
/// Never reads live operational state — all inputs come from BenchmarkExecutionRecord
/// and BenchmarkCohort aggregates.
@MainActor
struct MVPSignOffEvaluator {

    static let evaluatorVersion = "008-v1"

    /// Minimum median improvement required for GO (50%).
    static let minimumMedianImprovementPercent: Double = 50.0

    let modelContext: ModelContext

    // MARK: - Evaluate (§5.5)

    /// Main entry point: evaluate a cohort and produce a persisted decision snapshot.
    /// - Parameter cohort: The benchmark cohort to evaluate
    /// - Returns: The persisted MVPSignOffDecisionSnapshot
    func evaluate(cohort: BenchmarkCohort) throws -> MVPSignOffDecisionSnapshot {
        let pairs = cohort.pairs
        let failingReasons = checkGateRequirements(pairs: pairs)
        let medians = computeMedians(pairs: pairs)

        let decision: SignOffDecision = failingReasons.isEmpty ? .go : .hold

        // Build the decision payload for auditability
        let payloadDict = buildDecisionPayload(
            cohort: cohort,
            pairs: pairs,
            medians: medians,
            failingReasons: failingReasons
        )
        let payloadData = (try? JSONEncoder().encode(payloadDict)) ?? Data()
        let checksum = sha256Hex(payloadData)

        // Count outcome categories
        let happyPathCount = pairs.compactMap(\.appDrivenRecord).filter {
            $0.terminalOutcome == .happyPathCompleted
        }.count
        let recoveredCount = pairs.compactMap(\.appDrivenRecord).filter {
            $0.terminalOutcome == .recoveredNonHappyPathCompleted
        }.count

        let snapshot = MVPSignOffDecisionSnapshot(
            evaluatorVersion: Self.evaluatorVersion,
            cohortID: cohort.id,
            decision: decision,
            payloadChecksum: checksum,
            pairCount: pairs.count,
            happyPathCount: happyPathCount,
            recoveredCount: recoveredCount,
            failingGateReasons: failingReasons,
            decisionPayloadJSON: payloadData
        )
        snapshot.medianManualOrchestrationSeconds = medians.medianManualSeconds
        snapshot.medianAppOrchestrationSeconds = medians.medianAppSeconds
        snapshot.medianImprovementPercent = medians.medianImprovementPercent
        snapshot.medianProposalApprovalSeconds = medians.medianProposalApprovalSeconds
        snapshot.medianImplementationApprovalSeconds = medians.medianImplementationApprovalSeconds
        snapshot.medianReleaseDecisionSeconds = medians.medianReleaseDecisionSeconds

        modelContext.insert(snapshot)
        try modelContext.save()

        return snapshot
    }

    // MARK: - Gate Requirements (§5.6)

    /// Check all GO gate requirements and return failing reasons.
    /// Empty array means all gates pass.
    ///
    /// GO requires ALL of:
    /// 1. Median total_orchestration_time improves by >= 50% vs manual baseline
    /// 2. All three checkpoint timings present for every benchmark run
    /// 3. At least one happy-path and one recovered non-happy-path evidence pack
    /// 4. No benchmark run requires raw-log archaeology
    /// 5. Evidence packs exportable for all sign-off runs
    func checkGateRequirements(pairs: [BenchmarkPair]) -> [String] {
        var reasons: [String] = []

        // Gate 0: Must have at least one complete pair
        let completePairs = pairs.filter { $0.manualRecord != nil && $0.appDrivenRecord != nil }
        if completePairs.isEmpty {
            reasons.append("No complete benchmark pairs (both manual and app-driven records required)")
            return reasons  // Cannot evaluate further gates without complete pairs
        }

        // Gate 1: Median improvement >= 50%
        let medians = computeMedians(pairs: pairs)
        if let improvementPercent = medians.medianImprovementPercent {
            if improvementPercent < Self.minimumMedianImprovementPercent {
                reasons.append(
                    "Median improvement \(String(format: "%.1f", improvementPercent))% " +
                    "is below required \(String(format: "%.0f", Self.minimumMedianImprovementPercent))%"
                )
            }
        } else {
            reasons.append("Cannot compute median improvement (insufficient data)")
        }

        // Gate 2: All three checkpoint timings present for every app-driven record
        for pair in completePairs {
            if let appRecord = pair.appDrivenRecord {
                let missingCheckpoints = checkMissingCheckpoints(record: appRecord)
                if !missingCheckpoints.isEmpty {
                    reasons.append(
                        "Pair \(pair.id.uuidString.prefix(8)): missing checkpoints — " +
                        missingCheckpoints.joined(separator: ", ")
                    )
                }
            }
        }

        // Gate 3: At least one happy-path and one recovered non-happy-path
        let appDrivenRecords = completePairs.compactMap(\.appDrivenRecord)
        let hasHappyPath = appDrivenRecords.contains { $0.terminalOutcome == .happyPathCompleted }
        let hasRecovered = appDrivenRecords.contains { $0.terminalOutcome == .recoveredNonHappyPathCompleted }
        if !hasHappyPath {
            reasons.append("No happy-path completion evidence pack present")
        }
        if !hasRecovered {
            reasons.append("No recovered non-happy-path evidence pack present")
        }

        // Gate 4: No benchmark run requires raw-log archaeology
        // A record requires raw-log archaeology if it has no artifact links and no notes
        for pair in completePairs {
            if let appRecord = pair.appDrivenRecord {
                if appRecord.artifactLinks.isEmpty && appRecord.terminalOutcome != .pending {
                    reasons.append(
                        "Pair \(pair.id.uuidString.prefix(8)): app-driven record has no artifact links " +
                        "(raw-log archaeology would be required)"
                    )
                }
            }
        }

        // Gate 5: Evidence packs exportable for all sign-off runs
        // Verify that all app-driven records reference existing linked runs
        for pair in completePairs {
            if let appRecord = pair.appDrivenRecord, appRecord.linkedRunID == nil {
                reasons.append(
                    "Pair \(pair.id.uuidString.prefix(8)): app-driven record has no linked run ID " +
                    "(evidence pack not exportable)"
                )
            }
        }

        return reasons
    }

    // MARK: - Median Computation (§5.5)

    /// Compute median orchestration times and improvement percentage from complete pairs.
    func computeMedians(pairs: [BenchmarkPair]) -> MedianComputationResult {
        let completePairs = pairs.filter { $0.manualRecord != nil && $0.appDrivenRecord != nil }

        let manualTimes = completePairs.compactMap { $0.manualRecord?.totalOrchestrationTimeSeconds }
        let appTimes = completePairs.compactMap { $0.appDrivenRecord?.totalOrchestrationTimeSeconds }

        let medianManual = median(manualTimes)
        let medianApp = median(appTimes)

        let medianImprovement: Double? = {
            guard let manual = medianManual, let app = medianApp, manual > 0 else { return nil }
            return ((manual - app) / manual) * 100.0
        }()

        // Checkpoint medians across all app-driven records
        let proposalTimes = completePairs.compactMap {
            $0.appDrivenRecord?.timeToProposalApprovalSeconds
        }
        let implementationTimes = completePairs.compactMap {
            $0.appDrivenRecord?.timeToImplementationApprovalSeconds
        }
        let releaseTimes = completePairs.compactMap {
            $0.appDrivenRecord?.timeToFinalReleaseDecisionSeconds
        }

        return MedianComputationResult(
            medianManualSeconds: medianManual,
            medianAppSeconds: medianApp,
            medianImprovementPercent: medianImprovement,
            medianProposalApprovalSeconds: median(proposalTimes),
            medianImplementationApprovalSeconds: median(implementationTimes),
            medianReleaseDecisionSeconds: median(releaseTimes),
            completePairCount: completePairs.count,
            manualSampleCount: manualTimes.count,
            appSampleCount: appTimes.count
        )
    }

    // MARK: - Helpers

    private func checkMissingCheckpoints(record: BenchmarkExecutionRecord) -> [String] {
        var missing: [String] = []
        if record.timeToProposalApprovalSeconds == nil {
            missing.append("proposal_approval")
        }
        if record.timeToImplementationApprovalSeconds == nil {
            missing.append("implementation_approval")
        }
        if record.timeToFinalReleaseDecisionSeconds == nil {
            missing.append("release_decision")
        }
        return missing
    }

    private func median(_ values: [Double]) -> Double? {
        guard !values.isEmpty else { return nil }
        let sorted = values.sorted()
        let count = sorted.count
        if count % 2 == 0 {
            return (sorted[count / 2 - 1] + sorted[count / 2]) / 2.0
        } else {
            return sorted[count / 2]
        }
    }

    private func sha256Hex(_ data: Data) -> String {
        let digest = SHA256.hash(data: data)
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private func buildDecisionPayload(
        cohort: BenchmarkCohort,
        pairs: [BenchmarkPair],
        medians: MedianComputationResult,
        failingReasons: [String]
    ) -> [String: String] {
        var payload: [String: String] = [:]
        payload["evaluator_version"] = Self.evaluatorVersion
        payload["cohort_id"] = cohort.id.uuidString
        payload["cohort_label"] = cohort.label
        payload["evaluated_at"] = ISO8601DateFormatter().string(from: Date())
        payload["pair_count"] = "\(pairs.count)"
        payload["complete_pair_count"] = "\(medians.completePairCount)"
        payload["manual_sample_count"] = "\(medians.manualSampleCount)"
        payload["app_sample_count"] = "\(medians.appSampleCount)"

        if let medianManual = medians.medianManualSeconds {
            payload["median_manual_orchestration_seconds"] = String(format: "%.2f", medianManual)
        }
        if let medianApp = medians.medianAppSeconds {
            payload["median_app_orchestration_seconds"] = String(format: "%.2f", medianApp)
        }
        if let improvement = medians.medianImprovementPercent {
            payload["median_improvement_percent"] = String(format: "%.2f", improvement)
        }
        if let proposalMedian = medians.medianProposalApprovalSeconds {
            payload["median_proposal_approval_seconds"] = String(format: "%.2f", proposalMedian)
        }
        if let implMedian = medians.medianImplementationApprovalSeconds {
            payload["median_implementation_approval_seconds"] = String(format: "%.2f", implMedian)
        }
        if let releaseMedian = medians.medianReleaseDecisionSeconds {
            payload["median_release_decision_seconds"] = String(format: "%.2f", releaseMedian)
        }

        payload["failing_gate_count"] = "\(failingReasons.count)"
        for (index, reason) in failingReasons.enumerated() {
            payload["failing_gate_\(index)"] = reason
        }

        return payload
    }
}

// MARK: - MedianComputationResult

struct MedianComputationResult: Sendable {
    let medianManualSeconds: Double?
    let medianAppSeconds: Double?
    let medianImprovementPercent: Double?
    let medianProposalApprovalSeconds: Double?
    let medianImplementationApprovalSeconds: Double?
    let medianReleaseDecisionSeconds: Double?
    let completePairCount: Int
    let manualSampleCount: Int
    let appSampleCount: Int
}
