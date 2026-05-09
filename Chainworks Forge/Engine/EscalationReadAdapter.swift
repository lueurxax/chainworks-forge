import Combine
import Foundation

/// EscalationReadAdapter is the governed UI source for escalation state.
///
/// Current implementation (Phase 0-1 foundation):
/// - Accepts already-decoded chain arrays via `applyChains(_:)`.
/// - Derives an `EscalationSnapshot` and publishes it on MainActor via @Published.
/// - Shared per run_id through `EscalationReadAdapterRegistry`.
///
/// Authority boundary (per proposal p058-r14 — enforced in all phases):
/// - Reads GraphQL/subscription DTOs only; never reconstructs truth from local state.
/// - Must NOT call policy-drift acknowledgement, tier mutation, retry, resume, cancel,
///   or force-primary mutations.
/// - DriftReviewSheet is read-only; drift acknowledgement routes through MCP/operator workflow.
///
/// Not yet implemented (Phase 1+):
/// - GraphQL subscription and transport-stale handling.
/// - Readback refresh requests.
/// - Redacted trace copy to pasteboard.
/// - Runbook URL opening.
/// - AppKit attention requests and dock badge updates.
/// - Notification presentation.
@MainActor
final class EscalationReadAdapter: ObservableObject {
    @Published private(set) var snapshot: EscalationSnapshot = .empty

    nonisolated let runId: String

    nonisolated init(runId: String) {
        self.runId = runId
    }

    /// Apply a freshly decoded chain array and publish the derived snapshot.
    /// Caller is responsible for decoding off-MainActor; this method publishes on MainActor.
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

    // Retained for Phase 1+ subscription teardown when a run window closes.
    func removeAdapter(for runId: String) {
        adapters.removeValue(forKey: runId)
    }
}
