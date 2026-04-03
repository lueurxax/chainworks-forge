import Testing
@testable import Chainworks_Forge

@Suite("Proposal 024 Run Surface Routing", .tags(.fast, .provider))
struct Proposal024RunSurfaceTests {
    @Test("Idea pane routing prioritizes approvals and recovery-critical states")
    func ideaPaneRoutingPrioritizesCriticalStates() {
        #expect(defaultIdeaRunPane(for: .running, intent: .neutral) == .summary)
        #expect(defaultIdeaRunPane(for: .waitingApproval, intent: .neutral) == .approvals)
        #expect(defaultIdeaRunPane(for: .blocked, intent: .neutral) == .summary)
        #expect(defaultIdeaRunPane(for: .failed, intent: .neutral) == .summary)
    }

    @Test("Idea pane routing respects approval and recovery deep-link intents")
    func ideaPaneRoutingRespectsOpenIntent() {
        #expect(defaultIdeaRunPane(for: .running, intent: .approval) == .approvals)
        #expect(defaultIdeaRunPane(for: .waitingApproval, intent: .approval) == .approvals)
        #expect(defaultIdeaRunPane(for: .blocked, intent: .recovery) == .summary)
        #expect(defaultIdeaRunPane(for: .failed, intent: .recovery) == .summary)
    }

    @Test("Runs Home defaults to summary and allows focused deep links")
    func runsHomePaneRoutingDefaultsToSummary() {
        #expect(defaultRunsHomePane(for: .running, intent: .neutral) == .summary)
        #expect(defaultRunsHomePane(for: .waitingApproval, intent: .neutral) == .summary)
        #expect(defaultRunsHomePane(for: .blocked, intent: .neutral) == .summary)
        #expect(defaultRunsHomePane(for: .failed, intent: .neutral) == .summary)
        #expect(defaultRunsHomePane(for: .running, intent: .report) == .diagnostics)
        #expect(defaultRunsHomePane(for: .running, intent: .artifact) == .artifacts)
    }
}
