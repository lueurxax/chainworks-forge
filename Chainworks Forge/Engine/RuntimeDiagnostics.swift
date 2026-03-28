import Foundation

enum RuntimeDiagnostics {
    private static let environment = ProcessInfo.processInfo.environment
    private static let isEnabled = environment["CHAINWORKS_IN_MEMORY_STORE"] == "1"
        || environment.keys.contains(where: { $0.hasPrefix("CHAINWORKS_UI_TEST") })
    private static let logURL = URL(fileURLWithPath: "/tmp/chainworks-runtime.log")

    static func log(_ message: String) {
        guard isEnabled else { return }

        let formatter = ISO8601DateFormatter()
        let line = "[\(formatter.string(from: Date()))] \(message)\n"
        guard let data = line.data(using: .utf8) else { return }

        if FileManager.default.fileExists(atPath: logURL.path) == false {
            try? data.write(to: logURL, options: .atomic)
            return
        }

        guard let handle = try? FileHandle(forWritingTo: logURL) else { return }
        defer { try? handle.close() }
        do {
            try handle.seekToEnd()
            try handle.write(contentsOf: data)
        } catch {
            // Ignore diagnostics failures.
        }
    }
}
