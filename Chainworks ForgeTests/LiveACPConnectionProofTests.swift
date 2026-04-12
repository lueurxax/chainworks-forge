import Testing
import Foundation
@testable import Chainworks_Forge

@MainActor
@Suite("Live ACP Connection", .tags(.live))
struct LiveACPConnectionProofTests {
    @Test("Live workflow examples declare canonical ACP provider identifiers")
    func liveWorkflowExamplesDeclareCanonicalACPProviderIdentifiers() throws {
        let proposalLoop = try loadTestLiveWorkflow()
        let fullMVPLive = try loadTestFullMVPLiveWorkflow()

        for workflow in [proposalLoop, fullMVPLive] {
            #expect(!workflow.workflow.requiredProviders.contains("codex"))
            #expect(!workflow.workflow.requiredProviders.contains("claude_code"))
            #expect(!workflow.workflow.requiredProviders.contains("gemini"))

            for provider in workflow.workflow.requiredProviders {
                let family = try #require(ProviderFamily.from(runtimeIdentifier: provider))
                #expect(provider == family.runtimeProviderIdentifier)
            }
        }
    }

    @Test("Live workflow agents resolve only to ACP-backed provider families")
    func liveWorkflowAgentsResolveOnlyToACPProviderFamilies() throws {
        let catalog = try loadTestCanonicalCatalog()
        let workflow = try loadTestLiveWorkflow()

        var referencedAgents = Set<String>()
        for state in workflow.states.values {
            referencedAgents.insert(state.owner)
            for task in (state.run?.sequence ?? []) + (state.run?.parallel ?? []) + (state.run?.then ?? []) {
                referencedAgents.insert(task.agent)
            }
        }

        for agentID in referencedAgents {
            let agent = try #require(catalog.agents.first(where: { $0.id == agentID }))
            let backendProfile = try #require(catalog.backendProfiles[agent.backendProfile])
            let family = try #require(ProviderFamily.from(runtimeIdentifier: backendProfile.provider))
            #expect(backendProfile.provider == family.runtimeProviderIdentifier)
            #expect(backendProfile.runtimeProfile?.contains("acp") == true)
        }
    }
}
