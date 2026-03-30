import SwiftUI

// MARK: - Proposal 014 / Compatibility Facade
//
// `ForgeEmptyState` is the canonical shared primitive. This alias keeps the
// current call sites working while the codebase migrates onto the Forge layer.
typealias StyledEmptyState = ForgeEmptyState

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
