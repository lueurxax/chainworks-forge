import Foundation
import Observation
#if os(macOS)
import Darwin
import AppKit
#endif

enum GooseServerLaunchState: Equatable, Sendable {
    case idle
    case starting
    case running
    case external
    case failed(String)
}

struct GooseManagedServerLaunchPlan: Equatable, Sendable {
    let executablePath: String
    let arguments: [String]
    let environment: [String: String]
    let baseURL: URL
    let secretKey: String?
}

protocol GooseServerProcessHandle: AnyObject {
    var isRunning: Bool { get }
    func terminate()
}

final class ProcessGooseServerHandle: GooseServerProcessHandle {
    private let process: Process

    init(process: Process) {
        self.process = process
    }

    var isRunning: Bool {
        process.isRunning
    }

    func terminate() {
        if process.isRunning {
            process.terminate()
        }
    }
}

final class PIDGooseServerHandle: GooseServerProcessHandle {
    private let pid: Int32

    init(pid: Int32) {
        self.pid = pid
    }

    var isRunning: Bool {
#if os(macOS)
        kill(pid, 0) == 0
#else
        false
#endif
    }

    func terminate() {
#if os(macOS)
        guard isRunning else { return }
        kill(pid, SIGTERM)
#endif
    }
}

@MainActor
@Observable
final class GooseServerManager: ManagedGooseServerControlling {
    typealias ReachabilityProbe = @Sendable (URL) async -> GooseServerReachability
    typealias Launcher = @MainActor @Sendable (GooseManagedServerLaunchPlan) throws -> GooseServerProcessHandle
    typealias ManagedProcessPIDResolver = @Sendable (AppConfiguration) -> Int32?
    typealias AdoptedHandleFactory = @Sendable (Int32) -> GooseServerProcessHandle

    private let appConfigurationStore: AppConfigurationStore
    @ObservationIgnored private let probe: ReachabilityProbe
    @ObservationIgnored private let launcher: Launcher
    @ObservationIgnored private let managedProcessPIDResolver: ManagedProcessPIDResolver
    @ObservationIgnored private let adoptedHandleFactory: AdoptedHandleFactory
    @ObservationIgnored private var processHandle: GooseServerProcessHandle?
    @ObservationIgnored private var terminationObserver: NSObjectProtocol?

    private(set) var launchState: GooseServerLaunchState = .idle
    private(set) var lastCheckedAt: Date?
    private(set) var lastStartedAt: Date?

    init(
        appConfigurationStore: AppConfigurationStore,
        probe: @escaping ReachabilityProbe = ProviderAdapterSupport.probeGooseServerStatus,
        launcher: @escaping Launcher = GooseServerManager.defaultLauncher(plan:),
        managedProcessPIDResolver: @escaping ManagedProcessPIDResolver = GooseServerManager.findManagedGooseServerPID(configuration:),
        adoptedHandleFactory: @escaping AdoptedHandleFactory = { PIDGooseServerHandle(pid: $0) }
    ) {
        self.appConfigurationStore = appConfigurationStore
        self.probe = probe
        self.launcher = launcher
        self.managedProcessPIDResolver = managedProcessPIDResolver
        self.adoptedHandleFactory = adoptedHandleFactory
        registerTerminationObserverIfNeeded()
    }

    deinit {
#if os(macOS)
        if let terminationObserver {
            NotificationCenter.default.removeObserver(terminationObserver)
        }
#endif
    }

    var configuration: AppConfiguration {
        appConfigurationStore.configuration
    }

    var baseURL: URL? {
        configuration.gooseServerBaseURL
    }

    var liveRuntimeConfiguration: LiveRuntimeConfiguration? {
        guard let baseURL else { return nil }
        return LiveRuntimeConfiguration(
            baseURL: baseURL,
            apiKey: configuration.gooseServerSecretKey,
            override: nil,
            transportMode: .network,
            transportAPI: .gooseServer
        )
    }

    var statusSummary: String {
        switch launchState {
        case .idle:
            return "Goose server has not been checked yet"
        case .starting:
            return "Starting managed Goose server"
        case .running:
            return "Managed Goose server is running"
        case .external:
            return "Using externally managed Goose server"
        case .failed(let reason):
            return reason
        }
    }

    func bootstrap() async {
        ensureSecretKeyIfNeeded()
        if configuration.gooseServerAutostart {
            await ensureRunning()
        } else {
            await refreshStatus()
        }
    }

    func refreshStatus() async {
        guard let baseURL else {
            launchState = .failed("Managed Goose server base URL is invalid")
            lastCheckedAt = Date()
            return
        }

        switch await probe(baseURL) {
        case .reachable:
            attachManagedHandleIfNeeded()
            launchState = configuration.gooseServerAutostart ? .running : .external
        case .unreachable(let reason):
            launchState = configuration.gooseServerAutostart ? .failed(reason) : .idle
        }

        lastCheckedAt = Date()
    }

    func ensureRunning() async {
        guard configuration.gooseServerAutostart else {
            await refreshStatus()
            return
        }
        guard let baseURL else {
            launchState = .failed("Managed Goose server base URL is invalid")
            return
        }

        switch await probe(baseURL) {
        case .reachable:
            attachManagedHandleIfNeeded()
            launchState = .running
            lastCheckedAt = Date()
            return
        case .unreachable:
            break
        }

        guard let plan = makeLaunchPlan() else {
            return
        }

        launchState = .starting

        do {
            processHandle = try launcher(plan)
            lastStartedAt = Date()
        } catch {
            launchState = .failed("Failed to launch Goose server: \(error.localizedDescription)")
            return
        }

        let deadline = Date().addingTimeInterval(8)
        while Date() < deadline {
            switch await probe(baseURL) {
            case .reachable:
                launchState = .running
                lastCheckedAt = Date()
                return
            case .unreachable:
                try? await Task.sleep(for: .milliseconds(250))
            }
        }

        let failedReason: String
        switch await probe(baseURL) {
        case .reachable:
            attachManagedHandleIfNeeded()
            launchState = .running
            lastCheckedAt = Date()
            return
        case .unreachable(let reason):
            failedReason = reason
        }

        launchState = .failed("Managed Goose server did not become reachable: \(failedReason)")
    }

    func stopManagedServer() {
        processHandle?.terminate()
        processHandle = nil
        launchState = .idle
        lastCheckedAt = Date()
    }

    func prepareForSystemSleep() {
        processHandle = nil
        launchState = .idle
        lastCheckedAt = Date()
    }

    func reconcileAfterSystemWake() async {
        processHandle = nil
        if configuration.gooseServerAutostart {
            await ensureRunning()
        } else {
            await refreshStatus()
        }
    }

    private func registerTerminationObserverIfNeeded() {
#if os(macOS)
        terminationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification,
            object: nil,
            queue: nil
        ) { [weak self] _ in
            MainActor.assumeIsolated { [weak self] in
                self?.stopManagedServer()
            }
        }
#endif
    }

    private func attachManagedHandleIfNeeded() {
        guard configuration.gooseServerAutostart, processHandle == nil else { return }
        guard let pid = managedProcessPIDResolver(configuration) else { return }
        processHandle = adoptedHandleFactory(pid)
    }

    private func makeLaunchPlan() -> GooseManagedServerLaunchPlan? {
        guard let baseURL else {
            launchState = .failed("Managed Goose server base URL is invalid")
            return nil
        }

        let binaryPath = configuration.gooseServerBinaryPath ?? AppConfiguration.defaultGooseServerBinaryPath()
        guard let binaryPath, FileManager.default.isExecutableFile(atPath: binaryPath) else {
            launchState = .failed("Managed Goose server binary was not found. Expected Goose.app to provide goosed.")
            return nil
        }

        var environment = ProcessInfo.processInfo.environment
        environment["PATH"] = Self.managedPATH(base: environment["PATH"])
        environment["GOOSE_HOST"] = configuration.gooseServerHost
        environment["GOOSE_PORT"] = String(configuration.gooseServerPort)
        environment["GOOSE_TLS"] = configuration.gooseServerTLS ? "true" : "false"
        if let secret = configuration.gooseServerSecretKey, !secret.isEmpty {
            environment["GOOSE_SERVER__SECRET_KEY"] = secret
        }

        return GooseManagedServerLaunchPlan(
            executablePath: binaryPath,
            arguments: ["agent"],
            environment: environment,
            baseURL: baseURL,
            secretKey: configuration.gooseServerSecretKey
        )
    }

    private func ensureSecretKeyIfNeeded() {
        guard configuration.gooseServerAutostart,
              (configuration.gooseServerSecretKey ?? "").isEmpty else {
            return
        }

        appConfigurationStore.update {
            $0.gooseServerSecretKey = Self.generateSecretKey()
        }
    }

    nonisolated private static func generateSecretKey() -> String {
        let bytes = (0..<32).map { _ in UInt8.random(in: 0...255) }
        return bytes.map { String(format: "%02x", $0) }.joined()
    }

    nonisolated private static func managedPATH(base: String?) -> String {
        let preferred = [
            "\(NSHomeDirectory())/.local/bin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin"
        ]
        let existing = (base ?? "")
            .split(separator: ":")
            .map(String.init)
        let merged = preferred + existing
        var unique: [String] = []
        for path in merged where !path.isEmpty && !unique.contains(path) {
            unique.append(path)
        }
        return unique.joined(separator: ":")
    }

    nonisolated private static func defaultLauncher(plan: GooseManagedServerLaunchPlan) throws -> GooseServerProcessHandle {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: plan.executablePath)
        process.arguments = plan.arguments
        process.environment = plan.environment
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        try process.run()
        return ProcessGooseServerHandle(process: process)
    }

    nonisolated private static func findManagedGooseServerPID(configuration: AppConfiguration) -> Int32? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/sbin/lsof")
        process.arguments = [
            "-nP",
            "-iTCP:\(configuration.gooseServerPort)",
            "-sTCP:LISTEN",
            "-Fpc"
        ]

        let output = Pipe()
        process.standardOutput = output
        process.standardError = Pipe()

        do {
            try process.run()
        } catch {
            return nil
        }

        process.waitUntilExit()
        guard process.terminationStatus == 0 else { return nil }

        guard let data = try? output.fileHandleForReading.readToEnd(),
              let text = String(data: data, encoding: .utf8) else {
            return nil
        }

        var candidatePID: Int32?
        var candidateCommand: String?
        for line in text.split(whereSeparator: { $0.isNewline }) {
            guard let prefix = line.first else { continue }
            let value = String(line.dropFirst())
            switch prefix {
            case "p":
                if let pid = Int32(value) {
                    candidatePID = pid
                }
            case "c":
                candidateCommand = value
            default:
                break
            }
        }

        guard let pid = candidatePID else { return nil }

        if let configuredBinaryPath = configuration.gooseServerBinaryPath,
           !configuredBinaryPath.isEmpty,
           let command = candidateCommand,
           command != URL(fileURLWithPath: configuredBinaryPath).lastPathComponent {
            return nil
        }

        return pid
    }
}
