import SwiftUI
#if os(macOS)
import AppKit
import Darwin
import ServiceManagement
#endif

#if os(macOS)
private struct LaunchctlKickstartError: LocalizedError {
    let status: Int32

    var errorDescription: String? {
        "launchctl kickstart exited \(status)"
    }
}
#endif

@main
struct Chainworks_ForgeApp: App {
    static let processEnvironment = ProcessInfo.processInfo.environment
    static let isTestHost = isTestHost(for: processEnvironment)
    static let isUIAutomationHost = isUIAutomationHost(for: processEnvironment)

    @NSApplicationDelegateAdaptor(AutomationFallbackAppDelegate.self) private var automationFallbackAppDelegate

    init() {
        if Self.shouldDisableWindowRestoration {
            Self.applyWindowRestorationDefaults()
            Self.clearSavedWindowState()
        }
        if Self.isUIAutomationHost {
            ProcessInfo.processInfo.disableAutomaticTermination("Chainworks Forge UI automation session")
        }
        if !Self.isTestHost {
            Self.registerPackagedDaemonAgentIfAvailable()
        }
    }

    var body: some Scene {
        WindowGroup {
            if Self.isTestHost {
                EmptyView()
            } else {
                ContentView()
            }
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

    static func isTestHost(for environment: [String: String]) -> Bool {
        environment.keys.contains("XCTestConfigurationFilePath")
    }

    static func isUIAutomationHost(for environment: [String: String]) -> Bool {
        environment.keys.contains { $0.hasPrefix("CHAINWORKS_UI_TEST") }
    }

    private static func clearSavedWindowState() {
        #if os(macOS)
        NSApplication.shared.disableRelaunchOnLogin()
        clearSavedWindowState(
            bundleIdentifier: Bundle.main.bundleIdentifier ?? "xax.Chainworks-Forge"
        )
        #endif
    }

    static func applyWindowRestorationDefaults(
        userDefaults: UserDefaults = .standard
    ) {
        userDefaults.set(false, forKey: "NSQuitAlwaysKeepsWindows")
        userDefaults.set(true, forKey: "ApplePersistenceIgnoreState")
    }

    static func savedApplicationStateURL(
        bundleIdentifier: String,
        homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser
    ) -> URL {
        homeDirectory
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("Saved Application State", isDirectory: true)
            .appendingPathComponent("\(bundleIdentifier).savedState", isDirectory: true)
    }

    static func clearSavedWindowState(
        bundleIdentifier: String,
        fileManager: FileManager = .default,
        homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser
    ) {
        let url = savedApplicationStateURL(
            bundleIdentifier: bundleIdentifier,
            homeDirectory: homeDirectory
        )
        guard fileManager.fileExists(atPath: url.path) else { return }
        try? fileManager.removeItem(at: url)
    }

    #if os(macOS)
    @MainActor
    static func applyWindowRestorationPolicy(to window: NSWindow) {
        window.isRestorable = false
        window.restorationClass = nil
    }

    @MainActor
    static func applyWindowRestorationPolicyToOpenWindows() {
        for window in NSApp.windows {
            applyWindowRestorationPolicy(to: window)
        }
    }
    #endif

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
        #if DEBUG
        Task.detached {
            try? await SMAppService.agent(plistName: plistName).unregister()
            do {
                try await DebugPackagedDaemonProcess.shared.ensureStarted(
                    bundleURL: Bundle.main.bundleURL
                )
            } catch {
                ForgeLogger.app.error("Failed to start debug packaged daemon process: \(error.localizedDescription)")
            }
        }
        #else
        do {
            try SMAppService.agent(plistName: plistName).register()
        } catch {
            ForgeLogger.app.error("Failed to register packaged daemon LaunchAgent: \(error.localizedDescription)")
        }
        let label = "com.chainworks.forge.daemon"
        kickstartPackagedDaemonAgentIfStopped(label: label)
        #endif
        #endif
    }

    #if os(macOS)
    static func launchctlKickstartArguments(
        label: String,
        uid: uid_t = getuid(),
        force: Bool = false
    ) -> [String] {
        force
            ? ["kickstart", "-k", "gui/\(uid)/\(label)"]
            : ["kickstart", "gui/\(uid)/\(label)"]
    }

    static func restartPackagedDaemonAgent() async throws {
        let plistName = "com.chainworks.forge.daemon.plist"
        #if DEBUG
        try? await SMAppService.agent(plistName: plistName).unregister()
        try await DebugPackagedDaemonProcess.shared.restart(bundleURL: Bundle.main.bundleURL)
        #else
        let label = "com.chainworks.forge.daemon"
        let service = SMAppService.agent(plistName: plistName)
        try? await service.unregister()
        try service.register()
        try await runLaunchctlKickstart(label: label, force: true)
        #endif
    }

    private static func kickstartPackagedDaemonAgentIfStopped(label: String) {
        DispatchQueue.global(qos: .utility).async {
            do {
                try runLaunchctlKickstartSync(label: label, force: false)
            } catch {
                ForgeLogger.app.error("Failed to kickstart packaged daemon LaunchAgent: \(error.localizedDescription)")
            }
        }
    }

    private static func runLaunchctlKickstart(label: String, force: Bool) async throws {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .utility).async {
                do {
                    try runLaunchctlKickstartSync(label: label, force: force)
                    continuation.resume()
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private static func runLaunchctlKickstartSync(label: String, force: Bool) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
        process.arguments = launchctlKickstartArguments(label: label, force: force)
        try process.run()
        process.waitUntilExit()
        if process.terminationStatus != 0 {
            throw LaunchctlKickstartError(status: process.terminationStatus)
        }
    }
    #endif
}

#if os(macOS) && DEBUG
private actor DebugPackagedDaemonProcess {
    static let shared = DebugPackagedDaemonProcess()

    private var process: Process?

    func ensureStarted(bundleURL: URL) throws {
        if process?.isRunning == true {
            return
        }
        if existingPackagedDaemonIsRunning() {
            return
        }
        process = nil
        process = try startProcess(bundleURL: bundleURL)
    }

    func restart(bundleURL: URL) async throws {
        if let process, process.isRunning {
            process.terminate()
            let deadline = Date().addingTimeInterval(2)
            while process.isRunning && Date() < deadline {
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
            if process.isRunning {
                kill(process.processIdentifier, SIGKILL)
            }
        } else if let pid = existingPackagedDaemonPID() {
            kill(pid, SIGTERM)
            let deadline = Date().addingTimeInterval(2)
            while packagedDaemonPIDIsRunning(pid) && Date() < deadline {
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
            if packagedDaemonPIDIsRunning(pid) {
                kill(pid, SIGKILL)
            }
        }
        process = nil
        process = try startProcess(bundleURL: bundleURL)
    }

    private func startProcess(bundleURL: URL) throws -> Process {
        let executableURL = bundleURL
            .appendingPathComponent("Contents", isDirectory: true)
            .appendingPathComponent("MacOS", isDirectory: true)
            .appendingPathComponent("chainworks-forge-daemon", isDirectory: false)
        guard FileManager.default.isExecutableFile(atPath: executableURL.path) else {
            throw DebugPackagedDaemonError.missingExecutable(executableURL.path)
        }

        var environment = ProcessInfo.processInfo.environment
        environment["MODE"] = "packaged-app"
        environment["PATH"] = DebugPackagedDaemonProcess.providerPath(
            existing: environment["PATH"]
        )

        let process = Process()
        process.executableURL = executableURL
        process.environment = environment
        process.currentDirectoryURL = bundleURL.deletingLastPathComponent()
        process.terminationHandler = { process in
            Task { @MainActor in
                DaemonProcessSupervisor.shared.record(
                    status: process.terminationStatus,
                    reason: process.terminationReason
                )
            }
        }
        try process.run()
        return process
    }

    private func existingPackagedDaemonIsRunning() -> Bool {
        guard let pid = existingPackagedDaemonPID() else { return false }
        return packagedDaemonPIDIsRunning(pid)
    }

    private func existingPackagedDaemonPID() -> Int32? {
        let pidURL = FileManager.default
            .homeDirectoryForCurrentUser
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("Application Support", isDirectory: true)
            .appendingPathComponent("Chainworks Forge", isDirectory: true)
            .appendingPathComponent("daemon.pid", isDirectory: false)
        guard let raw = try? String(contentsOf: pidURL, encoding: .utf8),
              let pid = Int32(raw.trimmingCharacters(in: .whitespacesAndNewlines))
        else {
            return nil
        }
        return pid
    }

    private func packagedDaemonPIDIsRunning(_ pid: Int32) -> Bool {
        guard kill(pid, 0) == 0 else { return false }
        var buffer = [CChar](repeating: 0, count: 4096)
        let length = proc_pidpath(pid, &buffer, UInt32(buffer.count))
        guard length > 0 else { return false }
        let path = String(cString: buffer)
        return path.hasSuffix("/chainworks-forge-daemon")
    }

    private static func providerPath(existing: String?) -> String {
        var components: [String] = []
        if let existing, !existing.isEmpty {
            components.append(contentsOf: existing.split(separator: ":").map(String.init))
        }
        components.append(contentsOf: [
            "\(NSHomeDirectory())/.local/bin",
            "\(NSHomeDirectory())/.cargo/bin",
            "/opt/homebrew/bin",
            "/opt/homebrew/sbin",
            "/usr/local/bin",
            "/usr/local/sbin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
        ])

        var seen = Set<String>()
        return components.filter { path in
            guard !path.isEmpty, !seen.contains(path) else { return false }
            seen.insert(path)
            return true
        }
        .joined(separator: ":")
    }
}

private enum DebugPackagedDaemonError: LocalizedError {
    case missingExecutable(String)

    var errorDescription: String? {
        switch self {
        case .missingExecutable(let path):
            return "Debug packaged daemon executable is missing or not executable at \(path)"
        }
    }
}
#endif
