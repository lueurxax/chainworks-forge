import Testing
@testable import Chainworks_Forge

@Suite("Chainworks Forge App Bootstrap")
struct Chainworks_ForgeAppTests {
    @Test("Proposal 015 proof surface uses standalone UI path")
    func proposal015ProofUsesStandalonePath() {
        let environment = [
            "CHAINWORKS_UI_TEST_DIRECT_SURFACE": "proposal015_proof"
        ]

        let surface = Chainworks_ForgeApp.forcedUISurface(from: environment)

        #expect(surface == .proposal015Proof)
        #expect(Chainworks_ForgeApp.usesStandaloneUISurface(surface))
        #expect(!Chainworks_ForgeApp.requiresSharedModelContainer(for: surface))
        #expect(!Chainworks_ForgeApp.shouldCreateFallbackWindow(for: surface))
    }

    @Test("Non-proof surfaces keep normal bootstrap path")
    func regularSurfacesKeepBootstrappedPath() {
        let environment = [
            "CHAINWORKS_UI_TEST_DIRECT_SURFACE": "workflow_map"
        ]

        let surface = Chainworks_ForgeApp.forcedUISurface(from: environment)

        #expect(surface == .workflowMap)
        #expect(!Chainworks_ForgeApp.usesStandaloneUISurface(surface))
        #expect(Chainworks_ForgeApp.requiresSharedModelContainer(for: surface))
        #expect(Chainworks_ForgeApp.shouldCreateFallbackWindow(for: surface))
    }
}
