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

    @Test("Window restoration defaults disable AppKit persistent UI snapshots")
    func windowRestorationDefaultsDisablePersistentUISnapshots() {
        let suiteName = "ChainworksForgeAppTests-\(UUID().uuidString)"
        let defaults = try! #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }

        Chainworks_ForgeApp.applyWindowRestorationDefaults(userDefaults: defaults)

        #expect(defaults.bool(forKey: "NSQuitAlwaysKeepsWindows") == false)
        #expect(defaults.bool(forKey: "ApplePersistenceIgnoreState") == true)
    }

    @Test("Saved application state cleanup removes only the app snapshot directory")
    func savedApplicationStateCleanupRemovesOnlyAppSnapshotDirectory() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("ChainworksForgeAppTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let appState = Chainworks_ForgeApp.savedApplicationStateURL(
            bundleIdentifier: "xax.Chainworks-Forge",
            homeDirectory: root
        )
        let otherState = Chainworks_ForgeApp.savedApplicationStateURL(
            bundleIdentifier: "xax.Other-App",
            homeDirectory: root
        )
        try FileManager.default.createDirectory(at: appState, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: otherState, withIntermediateDirectories: true)
        try Data("stale".utf8).write(to: appState.appendingPathComponent("windows.plist"))

        Chainworks_ForgeApp.clearSavedWindowState(
            bundleIdentifier: "xax.Chainworks-Forge",
            homeDirectory: root
        )

        #expect(!FileManager.default.fileExists(atPath: appState.path))
        #expect(FileManager.default.fileExists(atPath: otherState.path))
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

    @Test("Unit test host detection treats an empty XCTest configuration path as active")
    func testHostDetectionUsesEnvironmentKeyPresence() {
        #expect(Chainworks_ForgeApp.isTestHost(for: [
            "XCTestConfigurationFilePath": ""
        ]))
        #expect(!Chainworks_ForgeApp.isTestHost(for: [
            "CHAINWORKS_UI_TEST_MODE": "1"
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

    @Test("Packaged app bootstrap prefers bundled write-path guide over source checkout")
    @MainActor
    func packagedBootstrapPrefersBundledWritePathGuide() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("ChainworksForgeAppTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let bundledURL = root
            .appendingPathComponent("Bundle", isDirectory: true)
            .appendingPathComponent("p031-operator-write-path-guide.json", isDirectory: false)
        try FileManager.default.createDirectory(
            at: bundledURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data("bundled".utf8).write(to: bundledURL)

        let repoGuideURL = root
            .appendingPathComponent("Repo", isDirectory: true)
            .appendingPathComponent("docs/reference/p031-operator-write-path-guide.json", isDirectory: false)
        try FileManager.default.createDirectory(
            at: repoGuideURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data("source".utf8).write(to: repoGuideURL)

        let sourceFilePath = root
            .appendingPathComponent("Repo", isDirectory: true)
            .appendingPathComponent("Chainworks Forge/Views/RunsHomeView.swift", isDirectory: false)
            .path

        let resource = P031OperatorWritePathGuideBootstrap.load(
            currentDirectoryPath: root.path,
            bundledURL: bundledURL,
            sourceFilePath: sourceFilePath
        )

        #expect(resource.url == bundledURL)
        #expect(resource.data == Data("bundled".utf8))
    }

    @Test("Packaged app bootstrap does not read source guide without explicit fallback")
    @MainActor
    func packagedBootstrapDoesNotFallbackToSourceGuideByDefault() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("ChainworksForgeAppTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let repoGuideURL = root
            .appendingPathComponent("Repo", isDirectory: true)
            .appendingPathComponent("docs/reference/p031-operator-write-path-guide.json", isDirectory: false)
        try FileManager.default.createDirectory(
            at: repoGuideURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try Data("source".utf8).write(to: repoGuideURL)

        let sourceFilePath = root
            .appendingPathComponent("Repo", isDirectory: true)
            .appendingPathComponent("Chainworks Forge/Views/RunsHomeView.swift", isDirectory: false)
            .path

        let resource = P031OperatorWritePathGuideBootstrap.load(
            currentDirectoryPath: root.path,
            bundledURL: nil,
            sourceFilePath: sourceFilePath,
            environment: [:]
        )

        #expect(resource.url == nil)
        #expect(resource.data == nil)
    }

    @Test("LaunchAgent kickstart targets the submitted GUI service without forcing restart")
    func launchAgentKickstartArgumentsDoNotForceRestart() {
        #if os(macOS)
        #expect(
            Chainworks_ForgeApp.launchctlKickstartArguments(
                label: "com.chainworks.forge.daemon",
                uid: 501
            ) == [
                "kickstart",
                "gui/501/com.chainworks.forge.daemon",
            ])
        #expect(
            Chainworks_ForgeApp.launchctlKickstartArguments(
                label: "com.chainworks.forge.daemon",
                uid: 501,
                force: true
            ) == [
                "kickstart",
                "-k",
                "gui/501/com.chainworks.forge.daemon",
            ])
        #endif
    }
}
