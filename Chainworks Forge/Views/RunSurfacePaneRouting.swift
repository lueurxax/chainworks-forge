import Foundation

enum RunSurfaceOpenIntent: String, Sendable {
    case neutral
    case approval
    case recovery
    case artifact
    case report
}

enum RunsHomePane: String, CaseIterable, Hashable, Sendable {
    case summary
    case flow
    case artifacts
    case diagnostics
}

enum IdeaRunPane: String, CaseIterable, Hashable, Sendable {
    case summary
    case progress
    case artifacts
}

func defaultRunsHomePane(
    for status: RunStatus,
    intent: RunSurfaceOpenIntent = .neutral
) -> RunsHomePane {
    switch intent {
    case .artifact:
        return .artifacts
    case .report, .recovery:
        return .diagnostics
    case .approval, .neutral:
        return .summary
    }
}

func defaultIdeaRunPane(
    for status: RunStatus,
    intent: RunSurfaceOpenIntent = .neutral
) -> IdeaRunPane {
    switch intent {
    case .approval:
        return .summary
    case .artifact:
        return .artifacts
    case .report, .recovery, .neutral:
        break
    }

    switch status {
    case .waitingApproval:
        return .summary
    case .blocked, .failed:
        return .summary
    case .pending, .ready, .running, .completed, .cancelled, .cancelling:
        return .summary
    }
}
