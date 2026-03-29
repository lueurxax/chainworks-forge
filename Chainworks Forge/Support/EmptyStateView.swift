import SwiftUI

// MARK: - Proposal 012 §4 / L-01: Standardised Empty State

/// Wrapper around `ContentUnavailableView` that applies
/// consistent icon sizing, multicolor rendering, and optional
/// call-to-action.
///
/// Usage:
/// ```
/// StyledEmptyState(
///     title: "No Runs",
///     systemImage: "tray",
///     description: "Start a run from the Ideas tab.",
///     actionTitle: "Go to Ideas"
/// ) { /* action */ }
/// ```
struct StyledEmptyState: View {
    let title: String
    let systemImage: String
    var description: String?
    var actionTitle: String?
    var action: (() -> Void)?

    var body: some View {
        ContentUnavailableView {
            Label(title, systemImage: systemImage)
                .symbolRenderingMode(.multicolor)
                .font(.system(size: 48))
        } description: {
            if let description {
                Text(description)
            }
        } actions: {
            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .buttonStyle(.borderedProminent)
                    .controlSize(.regular)
            }
        }
    }
}

// MARK: - Preview

#Preview("StyledEmptyState — with action") {
    StyledEmptyState(
        title: "No Runs",
        systemImage: "tray",
        description: "Start a run from the Ideas tab to see it here.",
        actionTitle: "Go to Ideas"
    ) {
        // action
    }
    .frame(width: 400, height: 300)
}

#Preview("StyledEmptyState — no action") {
    StyledEmptyState(
        title: "Select a Run",
        systemImage: "sidebar.left",
        description: "Choose a run from the sidebar to view details."
    )
    .frame(width: 400, height: 300)
}
