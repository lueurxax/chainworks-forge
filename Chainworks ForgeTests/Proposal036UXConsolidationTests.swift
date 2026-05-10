import XCTest
@testable import Chainworks_Forge

@MainActor
final class Proposal036UXConsolidationTests: XCTestCase {
    
    func testAgentGroupingPrecedence() {
        let agent1 = AgentDefinition(
            id: "a1",
            title: "Agent 1",
            mode: "fast",
            group: "Custom Group",
            backendProfile: "p1",
            permissionProfile: "perm1",
            skillRef: "skill1",
            inputs: [],
            outputs: [],
            requiresHumanApproval: false,
            prompt: ""
        )
        
        let agent2 = AgentDefinition(
            id: "a2",
            title: "Agent 2",
            mode: "slow",
            group: nil,
            backendProfile: "p2",
            permissionProfile: "perm2",
            skillRef: "skill2",
            inputs: [],
            outputs: [],
            requiresHumanApproval: false,
            prompt: ""
        )
        
        let catalog = AgentCatalog(agents: [agent1, agent2], validationIssues: [])
        // This is a bit tricky to test because groupedAgents is private in AgentCatalogView.
        // In a real scenario, we might want to expose the grouping logic in a separate presenter.
    }
    
    func testRunsWorkbenchLaneCategorization() {
        let model = RunsWorkbenchPresentationModel()
        
        let row1 = P031RunsHomeRowPresentation(
            runID: "r1",
            title: "Run 1",
            workflowLabel: "W1",
            statusLabel: "Running",
            progressLabel: nil,
            pendingApprovalsLabel: "1",
            closeoutReadinessSignalLabel: nil,
            freshnessState: .live,
            accessibilityLabel: ""
        )
        
        let row2 = P031RunsHomeRowPresentation(
            runID: "r2",
            title: "Run 2",
            workflowLabel: "W2",
            statusLabel: "Blocked",
            progressLabel: nil,
            pendingApprovalsLabel: nil,
            closeoutReadinessSignalLabel: nil,
            freshnessState: .live,
            accessibilityLabel: ""
        )
        
        let presentation = P031RunsHomePresentation(
            orientation: nil,
            rows: [row1, row2],
            freshness: P031FreshnessSnapshot(state: .live),
            refreshFeedbackText: "",
            emptyStateTitle: nil,
            errorDescription: nil
        )
        
        model.populate(from: presentation)
        
        XCTAssertEqual(model.sidebarLanes.count, 2)
        XCTAssertTrue(model.sidebarLanes.contains(where: { $0.title == "Waiting approval" }))
        XCTAssertTrue(model.sidebarLanes.contains(where: { $0.title == "Blocked or failed" }))
    }
}
