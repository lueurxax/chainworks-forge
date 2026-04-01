import SwiftUI

// MARK: - SessionReuseBadge (Proposal 018, Layer C)

/// Shows whether the latest execution used a `fresh`, `reused`, `reused_after_resume`,
/// or `fresh_after_reset` session disposition.
///
/// Intended for use in run progress views, stage detail views, and report surfaces
/// to make session reuse truth visible beyond the blocked-run inspector.
struct SessionReuseBadge: View {
    let disposition: SessionReuseDisposition?

    var body: some View {
        if let disposition {
            StatusCapsule(
                text: displayText(disposition),
                color: displayColor(disposition),
                size: .small
            )
            .help(helpText(disposition))
        }
    }

    // MARK: - Display Logic

    private func displayText(_ d: SessionReuseDisposition) -> String {
        switch d {
        case .fresh:                        return "Fresh"
        case .reused:                       return "Reused"
        case .reused_after_resume:          return "Reused (Resume)"
        case .fresh_after_reset:            return "Fresh (Reset)"
        case .fresh_after_invalidation:     return "Fresh (Invalidated)"
        case .fresh_after_budget:           return "Fresh (Budget)"
        case .fresh_after_compaction:       return "Fresh (Compacted)"
        case .fresh_after_transport_error:  return "Fresh (Error)"
        case .fresh_after_timeout:          return "Fresh (Timeout)"
        case .fresh_session_required:       return "Fresh (Required)"
        case .unverifiable_session_history: return "Unverifiable"
        }
    }

    private func displayColor(_ d: SessionReuseDisposition) -> Color {
        switch d {
        case .reused, .reused_after_resume:
            return .green
        case .fresh:
            return .blue
        case .fresh_after_reset:
            return .orange
        case .fresh_after_invalidation, .fresh_after_budget, .fresh_after_compaction:
            return .yellow
        case .fresh_after_transport_error, .fresh_after_timeout:
            return .red
        case .fresh_session_required:
            return .purple
        case .unverifiable_session_history:
            return .gray
        }
    }

    private func helpText(_ d: SessionReuseDisposition) -> String {
        switch d {
        case .fresh:
            return "This execution used a brand-new provider session."
        case .reused:
            return "This execution reused an existing provider session from the same lineage."
        case .reused_after_resume:
            return "This execution reused a session that survived a run resume."
        case .fresh_after_reset:
            return "This execution used a fresh session because the operator explicitly reset the previous one."
        case .fresh_after_invalidation:
            return "This execution used a fresh session because the previous one was invalidated."
        case .fresh_after_budget:
            return "This execution used a fresh session because the previous one exceeded budget thresholds."
        case .fresh_after_compaction:
            return "This execution used a fresh session after the previous one was compacted."
        case .fresh_after_transport_error:
            return "This execution used a fresh session because the previous one had a transport error."
        case .fresh_after_timeout:
            return "This execution used a fresh session because the previous one timed out."
        case .fresh_session_required:
            return "A fresh session was required due to binding or scope changes."
        case .unverifiable_session_history:
            return "Session history could not be verified; a fresh session was created."
        }
    }
}
