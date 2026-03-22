import Foundation
import SwiftData

/// Deterministic cohorting logic for Steward cross-run analysis.
/// Implements the cohorting contract from Proposal 003, section 6.
struct CohortClassifier: Sendable {

    /// Primary grouping key: (workflowFamily, riskClass).
    /// Runs with different primary keys are NEVER compared directly.
    struct CohortKey: Hashable, Sendable {
        let workflowFamily: String
        let riskClass: RiskClass
    }

    /// Filter runs to a primary cohort.
    static func primaryCohort(from runs: [Run], workflowFamily: String, riskClass: RiskClass) -> [Run] {
        runs.filter { run in
            run.workflowFamily == workflowFamily
                && (run.riskClass ?? .standard) == riskClass
                && run.workflowFamily != nil
        }
    }

    /// Classify cohort quality based on the cohorting contract rules.
    static func classifyQuality(runs: [Run]) -> CohortQuality {
        guard !runs.isEmpty else { return .weak }

        let hasUntaggedProject = runs.contains { $0.projectKey == nil || $0.projectKey == "untagged" }
        let hasUnknownStack = runs.contains { $0.stack == nil || $0.stack == "unknown" }

        if hasUntaggedProject || runs.count < 5 {
            return .weak
        }
        if hasUnknownStack || (runs.count >= 5 && runs.count < 10) {
            return .acceptable
        }
        return .strong
    }

    /// Split runs into observation and baseline windows.
    static func splitWindows(
        runs: [Run],
        observationSize: Int,
        baselineSize: Int,
        maximumAgeDays: Int
    ) -> (observation: [Run], baseline: [Run]) {
        let now = Date()
        let cutoff = Calendar.current.date(byAdding: .day, value: -maximumAgeDays, to: now) ?? now

        let eligible = runs
            .filter { $0.status == .completed && $0.startedAt >= cutoff }
            .sorted { $0.completedAt ?? $0.startedAt > $1.completedAt ?? $1.startedAt }

        let observationEnd = min(observationSize, eligible.count)
        let observation = Array(eligible.prefix(observationEnd))

        let baselineStart = observationEnd
        let baselineEnd = min(baselineStart + baselineSize, eligible.count)
        let baseline = Array(eligible[baselineStart..<baselineEnd])

        return (observation, baseline)
    }

    /// Determine the confidence level based on cohort quality.
    static func confidenceForQuality(_ quality: CohortQuality) -> ConfidenceLevel {
        switch quality {
        case .strong: return .high
        case .acceptable: return .medium
        case .weak: return .low
        }
    }
}
