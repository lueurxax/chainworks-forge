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
        proc.executableURL = URL(fileURLWithPath: executablePath)
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

        try proc.run()

        self.process = proc
        self.stdinPipe = stdin
        self.stdoutPipe = stdout
        self.stderrPipe = stderr
    }

    /// Send a JSON-RPC message (dictionary) to the subprocess stdin.
    /// Messages are newline-delimited (ndjson framing).
    func sendJSON(_ message: [String: Any]) throws {
        lock.lock()
        let pipe = stdinPipe
        lock.unlock()

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

        try pipe.fileHandleForWriting.write(contentsOf: payloadData)
    }

    /// Returns an `AsyncThrowingStream` that reads stdout line by line.
    /// Each yielded string is a single ndjson line (without the trailing newline).
    func readLines() -> AsyncThrowingStream<String, Error> {
        lock.lock()
        let pipe = stdoutPipe
        lock.unlock()

        return AsyncThrowingStream { continuation in
            guard let pipe else {
                continuation.finish(throwing: ACPSubprocessError.notRunning)
                return
            }

            let task = Task.detached { [weak self] in
                let handle = pipe.fileHandleForReading
                var buffer = Data()

                while true {
                    do {
                        try Task.checkCancellation()
                    } catch {
                        continuation.finish()
                        return
                    }

                    let chunk: Data?
                    do {
                        chunk = try handle.availableData
                    } catch {
                        continuation.finish(throwing: error)
                        return
                    }

                    guard let chunk, !chunk.isEmpty else {
                        // EOF — subprocess closed stdout
                        // Flush any remaining partial line
                        if !buffer.isEmpty, let line = String(data: buffer, encoding: .utf8) {
                            continuation.yield(line)
                        }
                        continuation.finish()
                        return
                    }

                    buffer.append(chunk)

                    // Extract complete lines from buffer
                    while let newlineRange = buffer.range(of: Data([0x0A])) {
                        let lineData = buffer.subdata(in: buffer.startIndex..<newlineRange.lowerBound)
                        buffer.removeSubrange(buffer.startIndex...newlineRange.lowerBound)

                        if let line = String(data: lineData, encoding: .utf8), !line.isEmpty {
                            continuation.yield(line)
                        }
                    }

                    // Check if process is still alive
                    let running = self?.isRunning ?? false
                    if !running && buffer.isEmpty {
                        continuation.finish()
                        return
                    }
                }
            }

            continuation.onTermination = { @Sendable _ in
                task.cancel()
            }
        }
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
        let merged = preferred + existing
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
    case unexpectedExit(terminationStatus: Int32)

    var errorDescription: String? {
        switch self {
        case .alreadyRunning:
            return "ACP subprocess is already running"
        case .notRunning:
            return "ACP subprocess is not running"
        case .serializationFailed:
            return "Failed to serialize JSON-RPC message"
        case .unexpectedExit(let status):
            return "ACP subprocess exited unexpectedly with status \(status)"
        }
    }
}
