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

    @State private var selectedSessionID: String?

    var body: some View {
        let visibleSessions = deduplicatedSessions
        if let focusedSession = focusedSession(from: visibleSessions) {
            VStack(alignment: .leading, spacing: 8) {
                if visibleSessions.count > 1 {
                    Picker("Provider session", selection: selectedSessionBinding(sessions: visibleSessions)) {
                        ForEach(visibleSessions) { session in
                            Text("\(session.providerName) · \(session.id)")
                                .tag(session.id)
                        }
                    }
                    .pickerStyle(.menu)
                    .accessibilityLabel("Provider session requiring identity review")
                }

                ManualProcessIdentityCheckBanner(
                    providerName: focusedSession.providerName,
                    sessionId: focusedSession.id,
                    cancellationEpoch: focusedSession.cancellationEpoch,
                    lastSeenPid: focusedSession.lastSeenPid,
                    processStartIdentityHash: focusedSession.processStartIdentityHash,
                    latestReceiptId: focusedSession.latestReceiptId,
                    reasonDetail: focusedSession.reasonDetail,
                    onRetryIdentityCheck: { onRetryIdentityCheck(focusedSession.id) },
                    onMarkProcessAbsent: { onMarkProcessAbsent(focusedSession.id) },
                    onOpenProviderSessionEvidence: { onOpenProviderSessionEvidence(focusedSession.id) }
                )
            }
            .accessibilityElement(children: .contain)
            .accessibilityLabel("Provider session identity holds: \(visibleSessions.count) session\(visibleSessions.count == 1 ? "" : "s") require attention")
            .onChange(of: visibleSessions.map(\.id)) { _, sessionIDs in
                guard let selectedSessionID, !sessionIDs.contains(selectedSessionID) else { return }
                self.selectedSessionID = sessionIDs.first
            }
        }
    }

    private var deduplicatedSessions: [IdentityAmbiguousSession] {
        var seen = Set<String>()
        return sessions.filter { session in
            seen.insert(session.id).inserted
        }
    }

    private func focusedSession(from sessions: [IdentityAmbiguousSession]) -> IdentityAmbiguousSession? {
        if let selectedSessionID,
           let selected = sessions.first(where: { $0.id == selectedSessionID }) {
            return selected
        }
        return sessions.first
    }

    private func selectedSessionBinding(sessions: [IdentityAmbiguousSession]) -> Binding<String> {
        Binding(
            get: { selectedSessionID ?? sessions.first?.id ?? "" },
            set: { selectedSessionID = $0 }
        )
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
