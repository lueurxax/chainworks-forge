import Combine
import Foundation

/// EscalationReadAdapter is the governed UI source for escalation state.
///
/// Current implementation:
/// - Accepts already-decoded chain arrays via `applyChains(_:)`.
/// - Derives an `EscalationSnapshot` and publishes it on MainActor via @Published.
/// - Shared per run_id through `EscalationReadAdapterRegistry`.
/// - Feeds the governed read-only SwiftUI surface: status capsule, banner stack,
///   lineage, pause card, trace timeline, inspector, and DriftReviewSheet.
///
/// Authority boundary (per proposal p058-r14 — enforced in all phases):
/// - Reads GraphQL/subscription DTOs only; never reconstructs truth from local state.
/// - Must NOT call policy-drift acknowledgement, tier mutation, retry, resume, cancel,
///   or force-primary mutations.
/// - DriftReviewSheet is read-only; drift acknowledgement routes through MCP/operator workflow.
///
@MainActor
final class EscalationReadAdapter: ObservableObject {
    @Published private(set) var snapshot: EscalationSnapshot = .empty
    @Published private(set) var lastOperatorNotice: EscalationOperatorNotice?

    nonisolated let runId: String

    nonisolated init(runId: String) {
        self.runId = runId
    }

    /// Apply a freshly decoded chain array and publish the derived snapshot.
    /// Caller is responsible for decoding off-MainActor; this method publishes on MainActor.
    func applyChains(_ chains: [EscalationChainStateDTO]) {
        let snap = EscalationSnapshot.build(runId: runId, chains: chains, readPipelineState: .ready)
        snapshot = snap
        lastOperatorNotice = EscalationOperatorNotice.notice(for: snap)
    }

    func markSubscribing() {
        snapshot = EscalationSnapshot.build(
            runId: runId,
            chains: snapshot.activeChains,
            readPipelineState: .subscribing
        )
    }

    func markTransportDisconnected() {
        snapshot = EscalationSnapshot.build(
            runId: runId,
            chains: snapshot.activeChains,
            readPipelineState: .transportDisconnected
        )
    }

    func markStaleSnapshot() {
        snapshot = EscalationSnapshot.build(
            runId: runId,
            chains: snapshot.activeChains,
            readPipelineState: .stale
        )
    }

    func markDecodeFailed() {
        snapshot = EscalationSnapshot.build(
            runId: runId,
            chains: snapshot.activeChains,
            readPipelineState: .decodeFailed
        )
    }

    /// Reset to the empty snapshot (e.g. on transport disconnect / run cancel).
    func reset() {
        snapshot = .empty
    }

    var dockBadgeEscalationCount: Int {
        max(snapshot.pausedChainCount, snapshot.hasActiveEscalation ? 1 : 0)
    }

    func runbookURL(for anchor: String) -> URL {
        URL(fileURLWithPath: "docs/runbooks/\(anchor).md", relativeTo: nil)
    }
}

struct EscalationOperatorNotice: Equatable, Sendable {
    let title: String
    let body: String
    let requiresUserAttention: Bool

    static func notice(for snapshot: EscalationSnapshot) -> EscalationOperatorNotice? {
        if snapshot.isKillSwitchEngaged {
            return EscalationOperatorNotice(
                title: "Escalation disabled",
                body: "Escalation scheduling is paused by the kill switch.",
                requiresUserAttention: false
            )
        }
        if snapshot.isPolicyDrift {
            return EscalationOperatorNotice(
                title: "Escalation policy drift",
                body: "Review the frozen and current escalation policy before further escalation.",
                requiresUserAttention: true
            )
        }
        if let reason = snapshot.pauseReasonRaw, snapshot.pausedChainCount > 0 {
            return EscalationOperatorNotice(
                title: EscalationPresentationStyle.pauseTitle(for: reason),
                body: "Open the escalation runbook before resuming or cancelling.",
                requiresUserAttention: true
            )
        }
        return nil
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

    @discardableResult
    func applyChains(_ chains: [EscalationChainStateDTO], for runId: String) -> EscalationSnapshot {
        let adapter = adapter(for: runId)
        adapter.applyChains(chains)
        return adapter.snapshot
    }

    func reset(runId: String) {
        adapter(for: runId).reset()
    }

    var snapshots: [EscalationSnapshot] {
        adapters.values
            .map(\.snapshot)
            .filter { !$0.runId.isEmpty }
            .sorted { $0.runId < $1.runId }
    }

    var attentionSnapshots: [EscalationSnapshot] {
        snapshots.filter {
            $0.pausedChainCount > 0
                || $0.isPolicyDrift
                || $0.isKillSwitchEngaged
                || $0.hasActiveEscalation
        }
    }

    // Retained for Phase 1+ subscription teardown when a run window closes.
    func removeAdapter(for runId: String) {
        adapters.removeValue(forKey: runId)
    }
}
