import Testing
@testable import Chainworks_Forge

@Suite("RunStartModePresentation", .tags(.fast))
struct RunStartModePresentationTests {
    @Test("Live is ordered before simulated when live execution is supported")
    func liveComesFirstWhenAvailable() {
        #expect(RunStartModePresentationPolicy.orderedModes(supportsLiveExecution: true) == [.live, .simulated])
    }

    @Test("Default mode prefers live when live execution is supported")
    func defaultModePrefersLive() {
        #expect(
            RunStartModePresentationPolicy.defaultMode(
                supportsLiveExecution: true,
                shouldDefaultToDeliveryFlow: false,
                currentSelection: .simulated
            ) == .live
        )
    }

    @Test("Simulated mode stays selected when live execution is unavailable")
    func defaultModeFallsBackToSimulatedWithoutLiveSupport() {
        #expect(
            RunStartModePresentationPolicy.defaultMode(
                supportsLiveExecution: false,
                shouldDefaultToDeliveryFlow: false,
                currentSelection: .simulated
            ) == .simulated
        )
    }

    @Test("Live mode keeps recommended copy while simulated is secondary")
    func presentationCopyMatchesPriority() {
        let live = RunStartModePresentationPolicy.presentation(for: .live)
        let simulated = RunStartModePresentationPolicy.presentation(for: .simulated)

        #expect(live.badge == "Recommended")
        #expect(simulated.badge == "Secondary")
    }
}
