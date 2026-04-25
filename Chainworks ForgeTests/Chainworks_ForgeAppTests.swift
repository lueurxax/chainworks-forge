import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("Chainworks Forge App Bootstrap")
struct Chainworks_ForgeAppTests {
    @Test("Window restoration is disabled unless explicitly enabled")
    func windowRestorationDefaultsToDisabled() {
        #expect(Chainworks_ForgeApp.shouldDisableWindowRestoration(for: [:]))
        #expect(!Chainworks_ForgeApp.shouldDisableWindowRestoration(for: [
            "CHAINWORKS_ENABLE_WINDOW_RESTORATION": "1"
        ]))
    }

    @Test("UI automation host detection stays environment scoped")
    func uiAutomationHostDetectionIsEnvironmentScoped() {
        #expect(Chainworks_ForgeApp.isUIAutomationHost(for: [
            "CHAINWORKS_UI_TEST_MODE": "1"
        ]))
        #expect(!Chainworks_ForgeApp.isUIAutomationHost(for: [
            "CHAINWORKS_ENABLE_WINDOW_RESTORATION": "1"
        ]))
    }

    @Test("Packaged daemon LaunchAgent lookup uses bundle root, not Resources")
    func packagedDaemonAgentLookupUsesBundleRoot() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("ChainworksForgeAppTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let bundleURL = root.appendingPathComponent("Chainworks Forge.app", isDirectory: true)
        let launchAgents = bundleURL
            .appendingPathComponent("Contents", isDirectory: true)
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("LaunchAgents", isDirectory: true)
        try FileManager.default.createDirectory(at: launchAgents, withIntermediateDirectories: true)

        let plistURL = launchAgents.appendingPathComponent("com.chainworks.forge.daemon.plist", isDirectory: false)
        try Data("<plist/>".utf8).write(to: plistURL)

        #expect(Chainworks_ForgeApp.packagedDaemonAgentPlistURL(in: bundleURL) == plistURL)
    }
}
