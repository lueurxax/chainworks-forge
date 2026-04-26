import Foundation

enum RunStartExecutionMode: String, CaseIterable, Identifiable {
    case simulated
    case live

    var id: String { rawValue }

    var title: String {
        switch self {
        case .simulated: return "Simulated"
        case .live: return "Live"
        }
    }
}

struct RunStartModePresentation: Equatable {
    let subtitle: String
    let badge: String?
}

enum RunStartModePresentationPolicy {
    static func orderedModes(supportsLiveExecution: Bool) -> [RunStartExecutionMode] {
        supportsLiveExecution ? [.live, .simulated] : [.simulated]
    }

    static func defaultMode(
        supportsLiveExecution: Bool,
        shouldDefaultToDeliveryFlow: Bool,
        currentSelection: RunStartExecutionMode
    ) -> RunStartExecutionMode {
        if shouldDefaultToDeliveryFlow {
            return .live
        }
        let ordered = orderedModes(supportsLiveExecution: supportsLiveExecution)
        if supportsLiveExecution {
            return .live
        }
        return ordered.contains(currentSelection) ? currentSelection : .simulated
    }

    static func presentation(for mode: RunStartExecutionMode) -> RunStartModePresentation {
        switch mode {
        case .live:
            return RunStartModePresentation(
                subtitle: "Uses configured live runtime execution.",
                badge: "Recommended"
            )
        case .simulated:
            return RunStartModePresentation(
                subtitle: "Local diagnostic mode without live runtime execution.",
                badge: "Secondary"
            )
        }
    }
}
