import Foundation
import SwiftData

// MARK: - BenchmarkRunRecorder (Proposal 008 — §5.2-5.4)

/// Captures checkpoint timestamps and outcome metadata for app-driven benchmark runs.
/// Observes the operational Run and extracts timing from approval decisions and
/// stage completion timestamps. Writes ONLY to BenchmarkExecutionRecord.
@MainActor
struct BenchmarkRunRecorder {

    let modelContext: ModelContext

    // MARK: - Record App-Driven Execution (§5.2)

    /// Create a BenchmarkExecutionRecord from a completed Run and attach it to the pair.
    /// The Run must be in a terminal state (completed, failed, or cancelled).
    /// - Parameters:
    ///   - pair: The benchmark pair to attach the app-driven record to
    ///   - run: The completed operational Run to extract timings from
    func recordAppDrivenExecution(pair: BenchmarkPair, run: Run) throws {
        guard pair.appDrivenRecord == nil else {
            throw BenchmarkRunRecorderError.recordAlreadyExists(pairID: pair.id)
        }

        guard run.status == .completed || run.status == .failed || run.status == .cancelled else {
            throw BenchmarkRunRecorderError.runNotTerminal(runID: run.id, status: run.status.rawValue)
        }

        let timings = computeCheckpointTimings(run: run)
        let outcome = resolveOutcome(run: run)

        let totalOrchestrationTime: Double? = {
            guard let completedAt = run.completedAt else { return nil }
            return completedAt.timeIntervalSince(run.startedAt)
        }()

        let record = BenchmarkExecutionRecord(
            executionMode: .appDriven,
            linkedRunID: run.id,
            startedAt: run.startedAt,
            completedAt: run.completedAt,
            terminalOutcome: outcome
        )
        record.totalOrchestrationTimeSeconds = totalOrchestrationTime
        record.timeToProposalApprovalSeconds = timings.proposalApprovalSeconds
        record.timeToImplementationApprovalSeconds = timings.implementationApprovalSeconds
        record.timeToFinalReleaseDecisionSeconds = timings.releaseDecisionSeconds

        // Link relevant artifact IDs for evidence traceability
        let artifactLinks = collectArtifactLinks(run: run)
        record.artifactLinks = artifactLinks

        modelContext.insert(record)
        pair.appDrivenRecord = record

        try modelContext.save()
    }

    // MARK: - Checkpoint Timings (§5.4)

    /// Extract proposal/implementation/release approval checkpoint times from the Run.
    /// Timing semantics (§5.4):
    ///   - proposalApproval: time from run start to first proposal-stage approval decision
    ///   - implementationApproval: time from run start to first implementation-review approval decision
    ///   - releaseDecision: time from run start to final release-gate approval decision
    func computeCheckpointTimings(run: Run) -> CheckpointTimings {
        let runStart = run.startedAt
        let sortedApprovals = run.approvals.sorted { $0.requestedAt < $1.requestedAt }

        // Proposal approval: first approval on a stage whose ID contains "proposal"
        let proposalApprovalSeconds = sortedApprovals
            .first(where: { approval in
                approval.decidedAt != nil &&
                approval.decision == .granted &&
                approval.stageID.localizedCaseInsensitiveContains("proposal")
            })
            .flatMap { $0.decidedAt.map { $0.timeIntervalSince(runStart) } }

        // Implementation approval: first approval on a stage whose ID contains "implementation" or "review"
        let implementationApprovalSeconds = sortedApprovals
            .first(where: { approval in
                approval.decidedAt != nil &&
                approval.decision == .granted &&
                (approval.stageID.localizedCaseInsensitiveContains("implementation") ||
                 approval.stageID.localizedCaseInsensitiveContains("review"))
            })
            .flatMap { $0.decidedAt.map { $0.timeIntervalSince(runStart) } }

        // Release decision: last approval on a stage whose ID contains "release"
        let releaseDecisionSeconds = sortedApprovals
            .last(where: { approval in
                approval.decidedAt != nil &&
                (approval.decision == .granted || approval.decision == .rejected) &&
                approval.stageID.localizedCaseInsensitiveContains("release")
            })
            .flatMap { $0.decidedAt.map { $0.timeIntervalSince(runStart) } }

        return CheckpointTimings(
            proposalApprovalSeconds: proposalApprovalSeconds,
            implementationApprovalSeconds: implementationApprovalSeconds,
            releaseDecisionSeconds: releaseDecisionSeconds
        )
    }

    // MARK: - Outcome Resolution

    /// Map operational Run terminal state to benchmark outcome.
    private func resolveOutcome(run: Run) -> BenchmarkExecutionOutcome {
        switch run.status {
        case .completed:
            // Check if any retries or recovery actions occurred
            let hasRetries = run.stageExecutions.contains { $0.attemptNumber > 1 }
            let hasRecoveryActions = run.stageExecutions
                .flatMap(\.agentExecutions)
                .contains { $0.retryReason != nil }
            if hasRetries || hasRecoveryActions {
                return .recoveredNonHappyPathCompleted
            }
            return .happyPathCompleted

        case .failed, .cancelled:
            return .failedUnrecovered

        default:
            return .pending
        }
    }

    // MARK: - Artifact Link Collection

    /// Collect evidence-relevant artifact links from the Run for benchmark traceability.
    private func collectArtifactLinks(run: Run) -> [BenchmarkArtifactLink] {
        let allArtifacts = run.stageExecutions
            .flatMap(\.agentExecutions)
            .flatMap(\.artifacts)

        // Only link pinned artifacts and report artifacts for benchmark evidence
        let evidenceArtifacts = allArtifacts.filter { artifact in
            artifact.isPinned ||
            artifact.reportKind != nil ||
            artifact.contractID == "run_report" ||
            artifact.contractID == "run_summary"
        }

        return evidenceArtifacts.map { artifact in
            BenchmarkArtifactLink(
                artifactID: artifact.id,
                name: artifact.name,
                role: artifact.isPinned ? "pinned" : (artifact.reportKind ?? "evidence")
            )
        }
    }
}

// MARK: - CheckpointTimings

struct CheckpointTimings: Sendable {
    let proposalApprovalSeconds: Double?
    let implementationApprovalSeconds: Double?
    let releaseDecisionSeconds: Double?

    /// Whether all three checkpoint timings are present.
    var isComplete: Bool {
        proposalApprovalSeconds != nil &&
        implementationApprovalSeconds != nil &&
        releaseDecisionSeconds != nil
    }
}

// MARK: - BenchmarkRunRecorderError

enum BenchmarkRunRecorderError: Error, LocalizedError {
    case recordAlreadyExists(pairID: UUID)
    case runNotTerminal(runID: UUID, status: String)

    var errorDescription: String? {
        switch self {
        case .recordAlreadyExists(let id):
            return "App-driven benchmark record already exists for pair \(id)"
        case .runNotTerminal(let id, let status):
            return "Run \(id) is not in a terminal state (current: \(status))"
        }
    }
}
