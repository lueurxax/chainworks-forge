import Testing
@testable import Chainworks_Forge

@Suite("Steward Config Defaults")
struct StewardConfigDefaultsTests {
    @Test("Steward analyzes every completed run by default")
    @MainActor
    func postRunHookDefaultsToEveryRun() {
        #expect(StewardConfig.defaultConfig.triggers.postRunHook.enabled)
        #expect(StewardConfig.defaultConfig.triggers.postRunHook.runInterval == 1)
    }
}
