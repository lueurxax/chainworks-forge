import SwiftUI

enum ForgeToolbarVisibilityPriority {
    case low
    case high
}

extension ToolbarContent {
    @ToolbarContentBuilder
    func forgeVisibilityPriority(_ priority: ForgeToolbarVisibilityPriority) -> some ToolbarContent {
        #if compiler(>=6.4)
        switch priority {
        case .low:
            visibilityPriority(.low)
        case .high:
            visibilityPriority(.high)
        }
        #else
        self
        #endif
    }

    @ToolbarContentBuilder
    func forgeContentMarginsRemovedWhenAvailable() -> some ToolbarContent {
        #if compiler(>=6.4)
        if #available(macOS 27.0, *) {
            contentMarginsRemoved()
        } else {
            self
        }
        #else
        self
        #endif
    }
}

extension View {
    @ViewBuilder
    func forgeSwipeActionsContainerWhenAvailable() -> some View {
        #if compiler(>=6.4)
        if #available(macOS 27.0, *) {
            swipeActionsContainer()
        } else {
            self
        }
        #else
        self
        #endif
    }
}

extension DynamicViewContent {
    @ViewBuilder
    func forgeReorderableWhenAvailable() -> some View {
        #if compiler(>=6.4)
        if #available(macOS 27.0, *) {
            reorderable()
        } else {
            self
        }
        #else
        self
        #endif
    }
}
