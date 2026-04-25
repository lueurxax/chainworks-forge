import SwiftUI
#if os(macOS)
import AppKit
import ServiceManagement
#endif

@main
struct Chainworks_ForgeApp: App {
    static let processEnvironment = ProcessInfo.processInfo.environment
    static let isTestHost = processEnvironment["XCTestConfigurationFilePath"] != nil
    static let isUIAutomationHost = isUIAutomationHost(for: processEnvironment)

    @NSApplicationDelegateAdaptor(AutomationFallbackAppDelegate.self) private var automationFallbackAppDelegate

    init() {
        if Self.shouldDisableWindowRestoration {
            UserDefaults.standard.set(false, forKey: "NSQuitAlwaysKeepsWindows")
            UserDefaults.standard.set(true, forKey: "ApplePersistenceIgnoreState")
            Self.clearSavedWindowState()
        }
        if Self.isUIAutomationHost {
            ProcessInfo.processInfo.disableAutomaticTermination("Chainworks Forge UI automation session")
        }
        Self.registerPackagedDaemonAgentIfAvailable()
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .commands {
            CommandGroup(replacing: .newItem) { }
        }
    }

    static var shouldDisableWindowRestoration: Bool {
        shouldDisableWindowRestoration(for: processEnvironment)
    }

    static func shouldDisableWindowRestoration(for environment: [String: String]) -> Bool {
        environment["CHAINWORKS_ENABLE_WINDOW_RESTORATION"] != "1"
    }

    static func isUIAutomationHost(for environment: [String: String]) -> Bool {
        environment.keys.contains { $0.hasPrefix("CHAINWORKS_UI_TEST") }
    }

    private static func clearSavedWindowState() {
        #if os(macOS)
        NSApplication.shared.disableRelaunchOnLogin()
        #endif
    }

    static func packagedDaemonAgentPlistURL(
        in bundleURL: URL,
        fileManager: FileManager = .default
    ) -> URL? {
        let url = bundleURL
            .appendingPathComponent("Contents", isDirectory: true)
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("LaunchAgents", isDirectory: true)
            .appendingPathComponent("com.chainworks.forge.daemon.plist", isDirectory: false)
        return fileManager.fileExists(atPath: url.path) ? url : nil
    }

    private static func registerPackagedDaemonAgentIfAvailable() {
        #if os(macOS)
        let plistName = "com.chainworks.forge.daemon.plist"
        guard packagedDaemonAgentPlistURL(in: Bundle.main.bundleURL) != nil else {
            ForgeLogger.app.error("Packaged daemon LaunchAgent plist is missing from Contents/Library/LaunchAgents")
            return
        }
        do {
            try SMAppService.agent(plistName: plistName).register()
        } catch {
            ForgeLogger.app.error("Failed to register packaged daemon LaunchAgent: \(error.localizedDescription)")
        }
        #endif
    }
}
