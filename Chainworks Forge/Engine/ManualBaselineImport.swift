import Foundation
import SwiftData

// MARK: - ManualBaselineImport (Proposal 008 — Layer K)

/// Records manual-orchestration baselines into persisted benchmark state.
/// Writes only BenchmarkExecutionRecord; reads nothing from operational Run aggregate.
@MainActor
struct ManualBaselineImport {

    let modelContext: ModelContext

    /// Import a manual baseline measurement for a benchmark pair.
    /// - Parameters:
    ///   - pair: The benchmark pair to attach the baseline to
    ///   - totalOrchestrationTime: Total manual orchestration time in seconds
    ///   - proposalApprovalTime: Time to proposal approval in seconds (optional)
    ///   - implementationApprovalTime: Time to implementation approval in seconds (optional)
    ///   - releaseDecisionTime: Time to final release decision in seconds (optional)
    ///   - outcome: Terminal outcome of the manual execution
    ///   - notes: Optional operator notes about the manual baseline
    func importBaseline(
        pair: BenchmarkPair,
        totalOrchestrationTime: Double,
        proposalApprovalTime: Double? = nil,
        implementationApprovalTime: Double? = nil,
        releaseDecisionTime: Double? = nil,
        outcome: BenchmarkExecutionOutcome = .happyPathCompleted,
        notes: String? = nil
    ) throws {
        guard pair.manualRecord == nil else {
            throw ManualBaselineError.baselineAlreadyRecorded(pairID: pair.id)
        }

        let record = BenchmarkExecutionRecord(
            executionMode: .manualBaseline,
            completedAt: Date(),
            terminalOutcome: outcome
        )
        record.totalOrchestrationTimeSeconds = totalOrchestrationTime
        record.timeToProposalApprovalSeconds = proposalApprovalTime
        record.timeToImplementationApprovalSeconds = implementationApprovalTime
        record.timeToFinalReleaseDecisionSeconds = releaseDecisionTime

        if let notes {
            record.notesJSON = try? JSONEncoder().encode([notes])
        }

        modelContext.insert(record)
        pair.manualRecord = record

        try modelContext.save()
    }
}

enum ManualBaselineError: Error, LocalizedError {
    case baselineAlreadyRecorded(pairID: UUID)

    var errorDescription: String? {
        switch self {
        case .baselineAlreadyRecorded(let id):
            return "Manual baseline already recorded for pair \(id)"
        }
    }
}
