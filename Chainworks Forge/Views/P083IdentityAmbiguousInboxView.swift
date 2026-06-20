import SwiftUI

/// P083 SCORE-LIFT-UI-P083-R66-001: Host view for identity-ambiguous provider sessions.
///
/// Reads provider_session identity_ambiguous state from the P083 rollout contract readback
/// (backend truth only — no local state) and renders ManualProcessIdentityCheckBanner for
/// each held session. The banner clears only when the backend moves intent_state away from
/// held or process_fate away from identity_ambiguous.
///
/// Surfaces: run detail provider-session section, stage detail provider-session row,
/// recovery inbox item — per manual_process_identity_check_ui_v1.
struct P083IdentityAmbiguousInboxView: View {
    /// A value-type snapshot of a single identity-ambiguous provider session.
    struct IdentityAmbiguousSession: Identifiable {
        let id: String  // provider_session_id
        let providerName: String
        let cancellationEpoch: Int?
        let lastSeenPid: Int?
        let processStartIdentityHash: String?
        let latestReceiptId: String?
        let reasonDetail: String?
    }

    let sessions: [IdentityAmbiguousSession]
    var onRetryIdentityCheck: (String) -> Void = { _ in }
    var onMarkProcessAbsent: (String) -> Void = { _ in }
    var onOpenProviderSessionEvidence: (String) -> Void = { _ in }

    var body: some View {
        if !sessions.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(sessions) { session in
                    ManualProcessIdentityCheckBanner(
                        providerName: session.providerName,
                        sessionId: session.id,
                        cancellationEpoch: session.cancellationEpoch,
                        lastSeenPid: session.lastSeenPid,
                        processStartIdentityHash: session.processStartIdentityHash,
                        latestReceiptId: session.latestReceiptId,
                        reasonDetail: session.reasonDetail,
                        onRetryIdentityCheck: { onRetryIdentityCheck(session.id) },
                        onMarkProcessAbsent: { onMarkProcessAbsent(session.id) },
                        onOpenProviderSessionEvidence: { onOpenProviderSessionEvidence(session.id) }
                    )
                }
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Provider session identity holds: \(sessions.count) session\(sessions.count == 1 ? "" : "s") require attention")
        }
    }
}

#if DEBUG
struct P083IdentityAmbiguousInboxView_Previews: PreviewProvider {
    static var previews: some View {
        P083IdentityAmbiguousInboxView(
            sessions: [
                .init(
                    id: "psess-abc",
                    providerName: "codex",
                    cancellationEpoch: 2,
                    lastSeenPid: 99001,
                    processStartIdentityHash: "deadbeef1234",
                    latestReceiptId: "rcpt-001",
                    reasonDetail: "Process identity could not be confirmed after restart recovery."
                ),
                .init(
                    id: "psess-def",
                    providerName: "claude",
                    cancellationEpoch: nil,
                    lastSeenPid: nil,
                    processStartIdentityHash: nil,
                    latestReceiptId: nil,
                    reasonDetail: nil
                ),
            ]
        )
        .padding()
        .frame(width: 500)
    }
}
#endif
