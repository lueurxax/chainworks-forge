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
    @State private var notificationService = NotificationService.shared

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
                UnitTestHostRootView()
            } else {
                ContentView(notificationService: notificationService)
            }
        }
        .commands {
            CommandGroup(replacing: .newItem) { }
            CommandMenu("Navigation") {
                Button("Runs") {
                    NotificationCenter.default.post(
                        name: .chainworksSelectTab,
                        object: "Runs",
                        userInfo: ["tab": "Runs"]
                    )
                }
                .keyboardShortcut("1", modifiers: .command)
                Button("Ideas") {
                    NotificationCenter.default.post(
                        name: .chainworksSelectTab,
                        object: "Ideas",
                        userInfo: ["tab": "Ideas"]
                    )
                }
                .keyboardShortcut("2", modifiers: .command)
                Button("Definitions") {
                    NotificationCenter.default.post(
                        name: .chainworksSelectTab,
                        object: "Definitions",
                        userInfo: ["tab": "Definitions"]
                    )
                }
                .keyboardShortcut("3", modifiers: .command)
                Button("Settings") {
                    NotificationCenter.default.post(
                        name: .chainworksSelectTab,
                        object: "Settings",
                        userInfo: ["tab": "Settings"]
                    )
                }
                .keyboardShortcut("4", modifiers: .command)
            }
            P083RunCommands()
        }
        MenuBarExtra {
            EscalationMenuBarList(snapshots: notificationService.p058EscalationSnapshots) { runID in
                NotificationCenter.default.post(
                    name: .chainworksOpenRunInRunsHome,
                    object: runID,
                    userInfo: ["runID": runID]
                )
            } onShowAllPausedRuns: {
                NotificationCenter.default.post(
                    name: .chainworksFocusEscalationAttentionRuns,
                    object: nil
                )
            }
        } label: {
            HStack(spacing: 4) {
                Label(
                    "Escalation attention",
                    systemImage: notificationService.p058EscalationAttentionCount > 0
                        ? "clock.badge.exclamationmark"
                        : "circle"
                )
                if notificationService.p058EscalationAttentionCount > 0 {
                    Text("\(notificationService.p058EscalationAttentionCount)")
                        .font(.caption.monospacedDigit().weight(.semibold))
                }
            }
        }
        .menuBarExtraStyle(.menu)
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
        for key in environment.keys {
            if key.hasPrefix("CHAINWORKS_UI_TEST") {
                return true
            }
        }
        return false
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

    nonisolated static func bundledDaemonBuildSHA(
        in bundleURL: URL,
        fileManager: FileManager = .default
    ) -> String? {
        let url = bundleURL
            .appendingPathComponent("Contents", isDirectory: true)
            .appendingPathComponent("Resources", isDirectory: true)
            .appendingPathComponent("bundled-daemon-build-sha.txt", isDirectory: false)
        guard fileManager.fileExists(atPath: url.path),
              let raw = try? String(contentsOf: url, encoding: .utf8)
        else {
            return nil
        }
        let value = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private static func registerPackagedDaemonAgentIfAvailable() {
        #if os(macOS)
        let plistName = "com.chainworks.forge.daemon.plist"
        cleanupStaleDaemonLaunchAgents()
        guard packagedDaemonAgentPlistURL(in: Bundle.main.bundleURL) != nil else {
            ForgeLogger.app.error("Packaged daemon LaunchAgent plist is missing from Contents/Library/LaunchAgents")
            return
        }
        #if DEBUG
        let label = "com.chainworks.forge.daemon"
        if launchdServiceIsRegistered(label: label) {
            kickstartPackagedDaemonAgentIfStopped(label: label)
            return
        }
        Task.detached {
            do {
                try await DebugPackagedDaemonProcess.shared.ensureStarted(
                    bundleURL: Bundle.main.bundleURL
                )
            } catch {
                ForgeLogger.app.error("Failed to start debug packaged daemon process: \(error.localizedDescription)")
            }
            try? await SMAppService.agent(plistName: plistName).unregister()
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
    nonisolated static func launchctlKickstartArguments(
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
        cleanupStaleDaemonLaunchAgents()
        let label = "com.chainworks.forge.daemon"
        #if DEBUG
        if launchdServiceIsRegistered(label: label) {
            try await runLaunchctlKickstart(label: label, force: true)
            return
        }
        try await DebugPackagedDaemonProcess.shared.restart(bundleURL: Bundle.main.bundleURL)
        Task.detached {
            try? await SMAppService.agent(plistName: plistName).unregister()
        }
        #else
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

    private nonisolated static func runLaunchctlKickstartSync(label: String, force: Bool) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
        process.arguments = launchctlKickstartArguments(label: label, force: force)
        try process.run()
        process.waitUntilExit()
        if process.terminationStatus != 0 {
            throw LaunchctlKickstartError(status: process.terminationStatus)
        }
    }

    nonisolated static func staleDaemonLaunchAgentLabels() -> [String] {
        [
            "com.chainworks.forge.daemon.local-restart",
        ]
    }

    private static func cleanupStaleDaemonLaunchAgents() {
        for label in staleDaemonLaunchAgentLabels() {
            do {
                try runLaunchctlBootoutSync(label: label)
            } catch {
                ForgeLogger.app.debug("Stale daemon LaunchAgent bootout skipped for \(label): \(error.localizedDescription)")
            }
            removeStaleDaemonLaunchAgentPlist(label: label)
        }
    }

    nonisolated static func launchctlBootoutArguments(
        label: String,
        uid: uid_t = getuid()
    ) -> [String] {
        ["bootout", "gui/\(uid)/\(label)"]
    }

    private nonisolated static func runLaunchctlBootoutSync(label: String) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
        process.arguments = launchctlBootoutArguments(label: label)
        try process.run()
        process.waitUntilExit()
        if process.terminationStatus != 0 {
            throw LaunchctlKickstartError(status: process.terminationStatus)
        }
    }

    private static func removeStaleDaemonLaunchAgentPlist(label: String) {
        let url = FileManager.default
            .homeDirectoryForCurrentUser
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("LaunchAgents", isDirectory: true)
            .appendingPathComponent("\(label).plist", isDirectory: false)
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        do {
            try FileManager.default.removeItem(at: url)
        } catch {
            ForgeLogger.app.debug("Stale daemon LaunchAgent plist removal skipped for \(label): \(error.localizedDescription)")
        }
    }

    private nonisolated static func launchdServiceIsRegistered(label: String) -> Bool {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
        process.arguments = ["print", "gui/\(getuid())/\(label)"]
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        do {
            try process.run()
            process.waitUntilExit()
            return process.terminationStatus == 0
        } catch {
            return false
        }
    }
    #endif
}

private struct UnitTestHostRootView: View {
    var body: some View {
        Color.clear
            .frame(width: 1, height: 1)
            .accessibilityIdentifier("unit-test-host-root")
    }
}

private extension Dictionary where Key == String, Value == String {
    nonisolated func mergingBundledDaemonBuildSHA(from bundleURL: URL) -> [String: String] {
        var result = self
        if result["GIT_SHA"]?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty != false,
           let buildSHA = Chainworks_ForgeApp.bundledDaemonBuildSHA(in: bundleURL) {
            result["GIT_SHA"] = buildSHA
        }
        return result
    }
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
        } else if let pid = existingPackagedDaemonPID(), packagedDaemonPIDIsRunning(pid) {
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
        for (key, value) in Self.bundledDaemonEnvironment(bundleURL: bundleURL) {
            environment[key] = value
        }
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
        ForgeLogger.app.info("Started debug packaged daemon process pid=\(process.processIdentifier)")
        return process
    }

    private nonisolated static func bundledDaemonEnvironment(bundleURL: URL) -> [String: String] {
        let contentsURL = bundleURL.appendingPathComponent("Contents", isDirectory: true)
        let plistCandidates = [
            contentsURL
                .appendingPathComponent("Resources", isDirectory: true)
                .appendingPathComponent("com.chainworks.forge.daemon.plist", isDirectory: false),
            contentsURL
                .appendingPathComponent("Library", isDirectory: true)
                .appendingPathComponent("LaunchAgents", isDirectory: true)
                .appendingPathComponent("com.chainworks.forge.daemon.plist", isDirectory: false)
        ]

        for plistURL in plistCandidates {
            guard let data = try? Data(contentsOf: plistURL),
                  let plist = try? PropertyListSerialization.propertyList(
                    from: data,
                    options: [],
                    format: nil
                  ) as? [String: Any],
                  let environment = plist["EnvironmentVariables"] as? [String: Any] else {
                continue
            }
            return environment.reduce(into: [String: String]()) { result, entry in
                if let value = entry.value as? String {
                    result[entry.key] = value
                }
            }.mergingBundledDaemonBuildSHA(from: bundleURL)
        }

        return [:].mergingBundledDaemonBuildSHA(from: bundleURL)
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
        let bytes = buffer.prefix(Int(length)).prefix { $0 != 0 }.map { UInt8(bitPattern: $0) }
        let path = String(decoding: bytes, as: UTF8.self)
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

struct P083RunCommandState: Equatable {
    let hasSelectedRun: Bool
    let hasPendingApproval: Bool
    let hasIdentityHold: Bool

    var canUseSelectedRunCommand: Bool { hasSelectedRun }
    var canResolveApproval: Bool { hasPendingApproval }
    var canRetryIdentityCheck: Bool { hasIdentityHold }
    var canCopyDiagnostic: Bool { hasIdentityHold }
    var canExportText: Bool { hasSelectedRun }
}

struct P083RunCommandActions {
    var cancelRun: () -> Void = {}
    var retryRun: () -> Void = {}
    var retryStage: () -> Void = {}
    var resolveApproval: () -> Void = {}
    var shutdownProviderSession: () -> Void = {}
    var retryIdentityCheck: () -> Void = {}
    var copyDiagnostic: () -> Void = {}
    var exportText: () -> Void = {}
}

private struct P083RunCommandStateKey: FocusedValueKey {
    typealias Value = P083RunCommandState
}

private struct P083RunCommandActionsKey: FocusedValueKey {
    typealias Value = P083RunCommandActions
}

extension FocusedValues {
    var p083RunCommandState: P083RunCommandState? {
        get { self[P083RunCommandStateKey.self] }
        set { self[P083RunCommandStateKey.self] = newValue }
    }

    var p083RunCommandActions: P083RunCommandActions? {
        get { self[P083RunCommandActionsKey.self] }
        set { self[P083RunCommandActionsKey.self] = newValue }
    }
}

struct P083RunCommands: Commands {
    @FocusedValue(\.p083RunCommandState) private var state
    @FocusedValue(\.p083RunCommandActions) private var actions

    var body: some Commands {
        CommandMenu("Run") {
            Menu("Lifecycle") {
                Button("Cancel Run") { actions?.cancelRun() }
                    .keyboardShortcut(".", modifiers: .command)
                    .disabled(!(state?.canUseSelectedRunCommand ?? false))
                    .help("Cancel the selected run through the backend lifecycle authority")
                Button("Retry Run") { actions?.retryRun() }
                    .keyboardShortcut("r", modifiers: .command)
                    .disabled(!(state?.canUseSelectedRunCommand ?? false))
                    .help("Retry the selected run through the backend lifecycle authority")
                Button("Retry Stage") { actions?.retryStage() }
                    .keyboardShortcut("r", modifiers: [.command, .shift])
                    .disabled(!(state?.canUseSelectedRunCommand ?? false))
                    .help("Retry the focused stage through the backend lifecycle authority")
                Button("Resolve Approval") { actions?.resolveApproval() }
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(!(state?.canResolveApproval ?? false))
                    .help("Open the approval lane for the focused run")
                Button("Shutdown Provider Session") { actions?.shutdownProviderSession() }
                    .keyboardShortcut("k", modifiers: [.command, .shift])
                    .disabled(!(state?.canUseSelectedRunCommand ?? false))
                    .help("Request provider session shutdown through the backend lifecycle authority")
            }

            Menu("Recovery") {
                Button("Retry Identity Check") { actions?.retryIdentityCheck() }
                    .keyboardShortcut("i", modifiers: .command)
                    .disabled(!(state?.canRetryIdentityCheck ?? false))
                    .help("Refresh process identity evidence for held provider sessions")
                Button("Copy Diagnostic") { actions?.copyDiagnostic() }
                    .keyboardShortcut("c", modifiers: [.command, .shift])
                    .disabled(!(state?.canCopyDiagnostic ?? false))
                    .help("Copy the focused provider-session diagnostic")
                Button("Export Text") { actions?.exportText() }
                    .keyboardShortcut("e", modifiers: [.command, .shift])
                    .disabled(!(state?.canExportText ?? false))
                    .help("Copy a text export command for the selected run")
            }
        }
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
