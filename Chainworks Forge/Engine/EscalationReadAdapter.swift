import Combine
import Foundation

/// EscalationReadAdapter is the governed UI source for escalation state.
///
/// Authority boundary (per proposal p058-r14):
/// - Reads GraphQL/subscription DTOs only; never reconstructs truth from local state.
/// - May request readback refreshes and copy redacted traces.
/// - Must NOT call policy-drift acknowledgement, tier mutation, retry, resume, cancel,
///   or force-primary mutations.
/// - DriftReviewSheet is read-only; drift acknowledgement routes through MCP/operator workflow.
///
/// Concurrency contract:
/// - Decode/normalization happens off-MainActor (nonisolated helpers + async context).
/// - Snapshot publication occurs on MainActor via @Published.
/// - All windows for the same run_id share one instance.
@MainActor
final class EscalationReadAdapter: ObservableObject {
    @Published private(set) var snapshot: EscalationSnapshot = .empty

    nonisolated let runId: String

    nonisolated init(runId: String) {
        self.runId = runId
    }

    /// Apply a freshly decoded chain array and publish the derived snapshot.
    /// Safe to call from any actor; switches to MainActor for publication.
    func applyChains(_ chains: [EscalationChainStateDTO]) {
        let snap = EscalationSnapshot.build(runId: runId, chains: chains)
        snapshot = snap
    }

    /// Reset to the empty snapshot (e.g. on transport disconnect / run cancel).
    func reset() {
        snapshot = .empty
    }
}

// MARK: - Shared registry

/// Thread-safe registry keyed by run_id so all windows for the same run share one adapter.
/// Access from MainActor only.
@MainActor
final class EscalationReadAdapterRegistry {
    static let shared = EscalationReadAdapterRegistry()

    private var adapters: [String: EscalationReadAdapter] = [:]

    private init() {}

    func adapter(for runId: String) -> EscalationReadAdapter {
        if let existing = adapters[runId] {
            return existing
        }
        let adapter = EscalationReadAdapter(runId: runId)
        adapters[runId] = adapter
        return adapter
    }

    func removeAdapter(for runId: String) {
        adapters.removeValue(forKey: runId)
    }
}
