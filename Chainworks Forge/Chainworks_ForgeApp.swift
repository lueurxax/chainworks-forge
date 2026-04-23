import SwiftUI
#if os(macOS)
import AppKit
import ServiceManagement
#endif

@main
struct Chainworks_ForgeApp: App {
    static let processEnvironment = ProcessInfo.processInfo.environment
    static let isTestHost = processEnvironment["XCTestConfigurationFilePath"] != nil
    static let isUIAutomationHost = processEnvironment.keys.contains { $0.hasPrefix("CHAINWORKS_UI_TEST") }

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

    private static var shouldDisableWindowRestoration: Bool {
        processEnvironment["CHAINWORKS_ENABLE_WINDOW_RESTORATION"] != "1"
    }

    private static func clearSavedWindowState() {
        #if os(macOS)
        NSApp.disableRelaunchOnLogin()
        #endif
    }

    private static func registerPackagedDaemonAgentIfAvailable() {
        #if os(macOS)
        let plistName = "com.chainworks.forge.daemon.plist"
        guard Bundle.main.url(forResource: plistName, withExtension: nil, subdirectory: "Contents/Library/LaunchAgents") != nil else {
            return
        }
        try? SMAppService.agent(plistName: plistName).register()
        #endif
    }
}
