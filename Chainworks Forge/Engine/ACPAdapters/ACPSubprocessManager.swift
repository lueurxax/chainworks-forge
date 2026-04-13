import Darwin
import Foundation

// MARK: - ACPSubprocessManager (Proposal 026, Phase 3 — Step 3.4)

/// Manages subprocess lifecycle for ACP-compliant runtimes communicating via stdin/stdout JSON-RPC.
/// Used by both `ClaudeAgentACPTransport` and `GeminiCLIACPTransport`.
///
/// Transport framing: newline-delimited JSON (ndjson) over stdio, consistent with
/// the live-probed behavior of both `claude-agent-acp` and `gemini --acp`.
///
/// Thread-safety: internal state is guarded by `NSLock`; the class is `@unchecked Sendable`
/// because `Process` and `Pipe` are not themselves `Sendable`.
final class ACPSubprocessManager: @unchecked Sendable {

    // MARK: - Configuration

    let executablePath: String
    let arguments: [String]
    let environment: [String: String]
    let workingDirectory: String?

    // MARK: - Internal State

    private var process: Process?
    private var stdinPipe: Pipe?
    private var stdoutPipe: Pipe?
    private var stderrPipe: Pipe?
    private let lock = NSLock()

    /// Shared line stream — created once per subprocess, reused by all callers.
    /// Prevents multiple tasks from racing on the same stdout FileHandle.
    private var sharedLineStream: AsyncThrowingStream<String, Error>?
    private var sharedStreamContinuation: AsyncThrowingStream<String, Error>.Continuation?
    private var sharedStderrLineStream: AsyncThrowingStream<String, Error>?
    private var sharedStderrStreamContinuation: AsyncThrowingStream<String, Error>.Continuation?

    // MARK: - Init

    init(
        executablePath: String,
        arguments: [String] = [],
        environment: [String: String] = [:],
        workingDirectory: String? = nil
    ) {
        self.executablePath = executablePath
        self.arguments = arguments
        self.environment = environment
        self.workingDirectory = workingDirectory
    }

    // MARK: - Lifecycle

    /// Launch the subprocess with stdin/stdout/stderr pipes.
    /// Inherits the current process environment and merges custom environment variables.
    func launch() throws {
        lock.lock()
        defer { lock.unlock() }

        guard process == nil || process?.isRunning == false else {
            throw ACPSubprocessError.alreadyRunning
        }

        let proc = Process()
        let resolvedPath = Self.resolveExecutablePath(executablePath)
        proc.executableURL = URL(fileURLWithPath: resolvedPath)
        proc.arguments = arguments

        // Inherit current environment and merge custom entries
        var mergedEnv = ProcessInfo.processInfo.environment
        for (key, value) in environment {
            mergedEnv[key] = value
        }
        // Ensure common tool paths are available
        mergedEnv["PATH"] = Self.enrichedPATH(base: mergedEnv["PATH"])
        proc.environment = mergedEnv

        if let workingDirectory {
            proc.currentDirectoryURL = URL(fileURLWithPath: workingDirectory, isDirectory: true)
        }

        let stdin = Pipe()
        let stdout = Pipe()
        let stderr = Pipe()

        proc.standardInput = stdin
        proc.standardOutput = stdout
        proc.standardError = stderr

        ForgeLogger.acpSubprocess.debug("Launching: \(resolvedPath) args=\(arguments) PATH=\(mergedEnv["PATH"]?.prefix(200) ?? "nil")")
        try proc.run()
        ForgeLogger.acpSubprocess.debug("Launched pid=\(proc.processIdentifier)")

        self.process = proc
        self.stdinPipe = stdin
        self.stdoutPipe = stdout
        self.stderrPipe = stderr

        // Avoid process-wide crashes on EPIPE when a runtime closes stdin before Forge sends
        // a best-effort shutdown message.
        let stdinFD = stdin.fileHandleForWriting.fileDescriptor
        _ = fcntl(stdinFD, F_SETNOSIGPIPE, 1)
    }

    /// Send a JSON-RPC message (dictionary) to the subprocess stdin.
    /// Messages are newline-delimited (ndjson framing).
    func sendJSON(_ message: [String: Any]) throws {
        lock.lock()
        let running = process?.isRunning ?? false
        let pipe = stdinPipe
        lock.unlock()

        guard running else {
            throw ACPSubprocessError.notRunning
        }
        guard let pipe else {
            throw ACPSubprocessError.notRunning
        }

        let data = try JSONSerialization.data(withJSONObject: message, options: [.sortedKeys])
        guard var payload = String(data: data, encoding: .utf8) else {
            throw ACPSubprocessError.serializationFailed
        }
        payload.append("\n")

        guard let payloadData = payload.data(using: .utf8) else {
            throw ACPSubprocessError.serializationFailed
        }

        let fd = pipe.fileHandleForWriting.fileDescriptor
        guard fd >= 0 else {
            throw ACPSubprocessError.brokenPipe
        }

        try payloadData.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress?.assumingMemoryBound(to: UInt8.self) else {
                throw ACPSubprocessError.serializationFailed
            }

            var written = 0
            while written < payloadData.count {
                let result = Darwin.write(fd, baseAddress.advanced(by: written), payloadData.count - written)
                if result > 0 {
                    written += result
                    continue
                }
                if result == 0 {
                    throw ACPSubprocessError.brokenPipe
                }

                switch errno {
                case EINTR:
                    continue
                case EPIPE, EBADF:
                    throw ACPSubprocessError.brokenPipe
                default:
                    throw ACPSubprocessError.writeFailed(errno: errno)
                }
            }
        }
        ForgeLogger.acpSubprocess.debug("Sent \(payloadData.count) bytes to stdin: \(payload.prefix(200))")
    }

    /// Returns a **shared** `AsyncThrowingStream` that reads stdout line by line.
    /// The reader task is started once on first call; subsequent calls return the same stream.
    /// This prevents multiple tasks from racing on the same stdout FileHandle.
    func readLines() -> AsyncThrowingStream<String, Error> {
        lock.lock()
        if let existing = sharedLineStream {
            lock.unlock()
            return existing
        }
        let pipe = stdoutPipe
        let stream = AsyncThrowingStream<String, Error> { continuation in
            self.sharedStreamContinuation = continuation

            guard let pipe else {
                continuation.finish(throwing: ACPSubprocessError.notRunning)
                return
            }

            let readerTask = Task.detached { [weak self] in
                let handle = pipe.fileHandleForReading
                var buffer = Data()

                while !Task.isCancelled {
                    let chunk = handle.availableData

                    if !chunk.isEmpty {
                        ForgeLogger.acpSubprocess.debug("Read \(chunk.count) bytes from stdout")
                    }
                    guard !chunk.isEmpty else {
                        // EOF — subprocess closed stdout
                        if !buffer.isEmpty, let line = String(data: buffer, encoding: .utf8) {
                            continuation.yield(line)
                        }
                        continuation.finish()
                        return
                    }

                    buffer.append(chunk)

                    while let newlineRange = buffer.range(of: Data([0x0A])) {
                        let lineData = buffer.subdata(in: buffer.startIndex..<newlineRange.lowerBound)
                        buffer.removeSubrange(buffer.startIndex...newlineRange.lowerBound)

                        if let line = String(data: lineData, encoding: .utf8), !line.isEmpty {
                            continuation.yield(line)
                        }
                    }

                    let running = self?.isRunning ?? false
                    if !running && buffer.isEmpty {
                        continuation.finish()
                        return
                    }
                }
                continuation.finish()
            }

            continuation.onTermination = { @Sendable _ in
                readerTask.cancel()
            }
        }
        sharedLineStream = stream
        lock.unlock()
        return stream
    }

    /// Returns a shared stderr line stream for runtime diagnostics.
    /// Transports can drain this to surface provider-side errors that do not appear on stdout.
    func readStderrLines() -> AsyncThrowingStream<String, Error> {
        lock.lock()
        if let existing = sharedStderrLineStream {
            lock.unlock()
            return existing
        }
        let pipe = stderrPipe
        let stream = AsyncThrowingStream<String, Error> { continuation in
            self.sharedStderrStreamContinuation = continuation

            guard let pipe else {
                continuation.finish(throwing: ACPSubprocessError.notRunning)
                return
            }

            let readerTask = Task.detached {
                let handle = pipe.fileHandleForReading
                var buffer = Data()

                while !Task.isCancelled {
                    let chunk = handle.availableData

                    guard !chunk.isEmpty else {
                        if !buffer.isEmpty, let line = String(data: buffer, encoding: .utf8) {
                            continuation.yield(line)
                        }
                        continuation.finish()
                        return
                    }

                    buffer.append(chunk)

                    while let newlineRange = buffer.range(of: Data([0x0A])) {
                        let lineData = buffer.subdata(in: buffer.startIndex..<newlineRange.lowerBound)
                        buffer.removeSubrange(buffer.startIndex...newlineRange.lowerBound)

                        if let line = String(data: lineData, encoding: .utf8), !line.isEmpty {
                            continuation.yield(line)
                        }
                    }
                }
                continuation.finish()
            }

            continuation.onTermination = { @Sendable _ in
                readerTask.cancel()
            }
        }
        sharedStderrLineStream = stream
        lock.unlock()
        return stream
    }

    /// Terminate the subprocess gracefully (SIGTERM), then forcefully (SIGKILL) if needed.
    func terminate() {
        lock.lock()
        let proc = process
        lock.unlock()

        guard let proc, proc.isRunning else { return }

        // Close stdin to signal the subprocess
        lock.lock()
        try? stdinPipe?.fileHandleForWriting.close()
        lock.unlock()

        proc.terminate() // SIGTERM

        // Wait briefly for graceful shutdown
        let deadline = Date().addingTimeInterval(2.0)
        while proc.isRunning && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.05)
        }

        // Force kill if still running
        if proc.isRunning {
            #if os(macOS)
            kill(proc.processIdentifier, SIGKILL)
            #endif
        }

        proc.waitUntilExit()

        lock.lock()
        process = nil
        stdinPipe = nil
        stdoutPipe = nil
        stderrPipe = nil
        sharedLineStream = nil
        sharedStreamContinuation = nil
        sharedStderrLineStream = nil
        sharedStderrStreamContinuation = nil
        lock.unlock()
    }

    /// Whether the subprocess is currently running.
    var isRunning: Bool {
        lock.lock()
        defer { lock.unlock() }
        return process?.isRunning ?? false
    }

    // MARK: - Private: PATH enrichment

    /// Ensure common tool directories are on PATH so that ACP binaries installed
    /// via npm/Homebrew are discoverable.
    /// Resolve a command name to an absolute path by searching the enriched PATH.
    /// If the input is already absolute, returns it as-is.
    private static func resolveExecutablePath(_ name: String) -> String {
        if name.hasPrefix("/") { return name }
        let searchDirs = [
            "\(NSHomeDirectory())/.local/bin",
            "\(NSHomeDirectory())/.npm-global/bin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            "/bin"
        ]
        for dir in searchDirs {
            let candidate = "\(dir)/\(name)"
            if FileManager.default.isExecutableFile(atPath: candidate) {
                return candidate
            }
        }
        return name // Fallback: let Process try and fail with a clear error
    }

    private static func enrichedPATH(base: String?) -> String {
        let preferred = [
            "\(NSHomeDirectory())/.local/bin",
            "\(NSHomeDirectory())/.npm-global/bin",
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
        // Preserve caller-prepared PATH precedence. Runtime-specific shims such as the
        // isolated Codex `swift` wrapper must stay ahead of generic discovery paths.
        let merged = existing + preferred
        var unique: [String] = []
        for path in merged where !path.isEmpty && !unique.contains(path) {
            unique.append(path)
        }
        return unique.joined(separator: ":")
    }
}

// MARK: - ACPSubprocessError

enum ACPSubprocessError: Error, LocalizedError {
    case alreadyRunning
    case notRunning
    case serializationFailed
    case brokenPipe
    case writeFailed(errno: Int32)
    case unexpectedExit(terminationStatus: Int32)

    var errorDescription: String? {
        switch self {
        case .alreadyRunning:
            return "ACP subprocess is already running"
        case .notRunning:
            return "ACP subprocess is not running"
        case .serializationFailed:
            return "Failed to serialize JSON-RPC message"
        case .brokenPipe:
            return "ACP subprocess stdin is closed"
        case .writeFailed(let errno):
            return "Failed to write to ACP subprocess stdin (errno \(errno))"
        case .unexpectedExit(let status):
            return "ACP subprocess exited unexpectedly with status \(status)"
        }
    }
}
