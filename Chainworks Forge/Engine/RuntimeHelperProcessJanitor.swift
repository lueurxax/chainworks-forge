import Foundation

protocol RuntimeHelperProcessJanitorProtocol: Sendable {
    nonisolated func sweepStaleHelpers()
}

struct RuntimeHelperProcessJanitor: RuntimeHelperProcessJanitorProtocol, Sendable {
    struct ProcessSnapshot: Equatable, Sendable {
        let pid: Int32
        let ppid: Int32
        let elapsedTime: String
        let command: String
    }

    static let live = RuntimeHelperProcessJanitor()
    nonisolated static let staleThreshold: TimeInterval = 300

    private let listProcesses: @Sendable () throws -> [ProcessSnapshot]
    private let terminateProcess: @Sendable (Int32) -> Void
    private let recordFailure: @Sendable (String) -> Void

    init(
        listProcesses: @escaping @Sendable () throws -> [ProcessSnapshot] = Self.loadProcessSnapshots,
        terminateProcess: @escaping @Sendable (Int32) -> Void = Self.terminateProcess,
        recordFailure: @escaping @Sendable (String) -> Void = { message in
            Task { @MainActor in
                ForgeLogger.execution.error(message)
            }
        }
    ) {
        self.listProcesses = listProcesses
        self.terminateProcess = terminateProcess
        self.recordFailure = recordFailure
    }

    nonisolated func sweepStaleHelpers() {
        let processes: [ProcessSnapshot]
        do {
            processes = try listProcesses()
        } catch {
            recordFailure("Runtime helper janitor failed to enumerate helper processes: \(error.localizedDescription)")
            return
        }

        let stalePIDs = processes
            .filter(Self.isStaleOrphanedHelper)
            .map(\.pid)
            .sorted()

        guard stalePIDs.isEmpty == false else { return }

        for pid in stalePIDs {
            terminateProcess(pid)
        }
    }

    nonisolated static func isStaleOrphanedHelper(_ snapshot: ProcessSnapshot) -> Bool {
        snapshot.ppid == 1
            && isHelperCommand(snapshot.command)
            && (elapsedSeconds(from: snapshot.elapsedTime) ?? 0) >= staleThreshold
    }

    nonisolated static func isHelperCommand(_ command: String) -> Bool {
        command.contains("claude-agent-acp")
            || command.contains("codex-acp")
            || command.contains("gemini --acp")
            || command.contains("/usr/bin/mcpbridge")
    }

    nonisolated static func elapsedSeconds(from rawValue: String) -> TimeInterval? {
        let trimmed = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.isEmpty == false else { return nil }

        let dayParts = trimmed.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        let days: Int
        let timePart: Substring
        if dayParts.count == 2 {
            guard let parsedDays = Int(dayParts[0]) else { return nil }
            days = parsedDays
            timePart = dayParts[1]
        } else {
            days = 0
            timePart = Substring(trimmed)
        }

        let components = timePart.split(separator: ":")
        let values = components.compactMap { Int($0) }
        guard values.count == components.count else { return nil }

        let seconds: Int
        switch values.count {
        case 3:
            seconds = values[0] * 3600 + values[1] * 60 + values[2]
        case 2:
            seconds = values[0] * 60 + values[1]
        default:
            return nil
        }

        return TimeInterval(days * 86_400 + seconds)
    }

    nonisolated static func runProcessAndCaptureStdout(
        executableURL: URL,
        arguments: [String]
    ) throws -> (terminationStatus: Int32, stdout: String) {
        let process = Process()
        process.executableURL = executableURL
        process.arguments = arguments

        let outputPipe = Pipe()
        process.standardOutput = outputPipe
        process.standardError = FileHandle.nullDevice

        try process.run()
        let data = outputPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        let output = String(data: data, encoding: .utf8) ?? ""
        return (process.terminationStatus, output)
    }

    private nonisolated static func loadProcessSnapshots() throws -> [ProcessSnapshot] {
        let result = try runProcessAndCaptureStdout(
            executableURL: URL(fileURLWithPath: "/bin/ps"),
            arguments: ["-axo", "pid=,ppid=,etime=,command="]
        )

        guard result.terminationStatus == 0 else {
            return []
        }

        let output = result.stdout
        guard output.isEmpty == false else { return [] }

        return output
            .split(separator: "\n")
            .compactMap(parseProcessSnapshot)
    }

    private nonisolated static func parseProcessSnapshot(_ line: Substring) -> ProcessSnapshot? {
        let pattern = #"^\s*(\d+)\s+(\d+)\s+(\S+)\s+(.+)$"#
        guard
            let regex = try? NSRegularExpression(pattern: pattern),
            let match = regex.firstMatch(
                in: String(line),
                range: NSRange(location: 0, length: line.utf16.count)
            )
        else {
            return nil
        }

        func capture(_ index: Int) -> String? {
            let range = match.range(at: index)
            guard
                let swiftRange = Range(range, in: line)
            else {
                return nil
            }
            return String(line[swiftRange])
        }

        guard
            let pidValue = capture(1).flatMap(Int32.init),
            let ppidValue = capture(2).flatMap(Int32.init),
            let elapsedTime = capture(3),
            let command = capture(4)
        else {
            return nil
        }

        return ProcessSnapshot(
            pid: pidValue,
            ppid: ppidValue,
            elapsedTime: elapsedTime,
            command: command
        )
    }

    private nonisolated static func terminateProcess(_ pid: Int32) {
        kill(pid, SIGTERM)
    }
}
