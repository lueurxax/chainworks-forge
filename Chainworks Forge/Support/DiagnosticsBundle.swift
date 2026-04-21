// P042 §9.4 / §9.5 diagnostics bundle exporter.
//
// When the daemon is Failed, unreachable, or on a non-default port, the
// operator needs a local artifact bundle they can attach to a support
// ticket. This module gathers:
//
//   • the last known `DaemonStatus` snapshot (optional — may be nil if
//     the daemon never came up)
//   • `build-sha.txt` (written by the daemon after Ready)
//   • `daemon.log` (most recent rolling-appender file, if any)
//   • `daemon.port` (so the user can include the port discovery state)
//   • `crash-budget.json` (so the user can include the crash counter)
//
// The output is a single zip written to the caller-supplied location.
// Crucially: the export path is *file-system only*. No `URLSession`
// calls, no GraphQL, no MCP — the whole point is that a failed daemon
// or an offline laptop can still produce evidence (§9.4).
//
// `DiagnosticsBundleBuilder` bundles these via the system-provided
// `NSFileCoordinator`-equivalent approach: a temporary staging
// directory + `Foundation.Process`-driven `zip` invocation. We avoid
// SwiftZip / third-party libs because the macOS /usr/bin/zip is
// guaranteed-present and zero-dependency.

import Foundation

/// Result of an export. `sizeBytes` is the final zip's size; fields
/// starting with `has…` let the UI render which components are present.
struct DiagnosticsExportResult: Equatable, Sendable {
    let bundleURL: URL
    let sizeBytes: Int64
    let hasStatusSnapshot: Bool
    let hasBuildSha: Bool
    let hasDaemonLog: Bool
    let hasPortFile: Bool
    let hasCrashBudget: Bool
    /// `principals.redacted.json` — the principals file with token values
    /// scrubbed (§9.4: support-ticket attachments must never leak bearer
    /// tokens). `false` when no principals file was found at the configured
    /// auth path.
    let hasPrincipalsRedacted: Bool
    /// `system_info.txt` — OS version, hardware model, app version, and
    /// locale (§9.4). Always produced unless the platform APIs fail.
    let hasSystemInfo: Bool
    /// `build_sha.txt` value that landed in the zip, or `"unknown"` when
    /// `build-sha.txt` is absent (§9.4: the bundle must never crash on
    /// missing SHA; it reports `unknown` instead).
    let buildShaReported: String
}

enum DiagnosticsBundleError: Error, CustomStringConvertible {
    case stagingFailed(Error)
    case zipFailed(status: Int32, stderr: String)
    case noComponentsFound

    var description: String {
        switch self {
        case .stagingFailed(let e): return "diagnostics staging failed: \(e)"
        case .zipFailed(let status, let stderr):
            return "zip exited \(status): \(stderr)"
        case .noComponentsFound:
            return "diagnostics bundle has no components — daemon paths and status snapshot were all missing"
        }
    }
}

/// Built-in inputs that the app ships today. Callers override any of
/// these for testing (`DaemonPortFile.appSupportDirectory` can point
/// into a tmpdir).
struct DiagnosticsBundleInputs {
    /// The last status snapshot observed by `DaemonStatusViewModel`, if any.
    var status: DaemonStatus?
    /// Directory containing `daemon.port`, `build-sha.txt`,
    /// `crash-budget.json`. Defaults to `DaemonPortFile.appSupportDirectory()`.
    var appSupportDirectory: URL
    /// Directory containing `daemon.log` (packaged rolling-appender
    /// output). Defaults to `$HOME/Library/Logs/Chainworks Forge/`.
    var logsDirectory: URL
    /// Location of `principals.json` (P042 §9.4: the bundle ships a
    /// `principals.redacted.json` derived from this file with every
    /// bearer token replaced by `<BEARER_REDACTED>`). Defaults to
    /// `$HOME/.chainworks/auth/principals.json`.
    var principalsPath: URL
    /// Closure that produces a `system_info.txt` payload. Injectable so
    /// tests can pin deterministic contents without touching the real
    /// host's APIs.
    var systemInfoProducer: () -> String
}

extension DiagnosticsBundleInputs {
    static func defaults(status: DaemonStatus?) -> DiagnosticsBundleInputs {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let logs = home
            .appendingPathComponent("Library", isDirectory: true)
            .appendingPathComponent("Logs", isDirectory: true)
            .appendingPathComponent("Chainworks Forge", isDirectory: true)
        let principals = home
            .appendingPathComponent(".chainworks", isDirectory: true)
            .appendingPathComponent("auth", isDirectory: true)
            .appendingPathComponent("principals.json", isDirectory: false)
        return DiagnosticsBundleInputs(
            status: status,
            appSupportDirectory: DaemonPortFile.appSupportDirectory(),
            logsDirectory: logs,
            principalsPath: principals,
            systemInfoProducer: DiagnosticsBundleBuilder.defaultSystemInfo
        )
    }
}

struct DiagnosticsBundleBuilder {
    /// Write a zip containing whichever inputs exist. The caller chooses
    /// `destination` (e.g. `~/Desktop/chainworks-diagnostics-2026-04-18.zip`
    /// from an `NSSavePanel`). Returns the manifest so the UI can show
    /// which components made it into the zip.
    static func export(
        inputs: DiagnosticsBundleInputs,
        to destination: URL,
        exportedAt: Date = Date()
    ) throws -> DiagnosticsExportResult {
        let fm = FileManager.default
        let staging = fm.temporaryDirectory.appendingPathComponent(
            "chainworks-diagnostics-\(UUID().uuidString)",
            isDirectory: true
        )
        do {
            try fm.createDirectory(at: staging, withIntermediateDirectories: true)
        } catch {
            throw DiagnosticsBundleError.stagingFailed(error)
        }
        defer { try? fm.removeItem(at: staging) }

        var result = DiagnosticsExportResult(
            bundleURL: destination,
            sizeBytes: 0,
            hasStatusSnapshot: false,
            hasBuildSha: false,
            hasDaemonLog: false,
            hasPortFile: false,
            hasCrashBudget: false,
            hasPrincipalsRedacted: false,
            hasSystemInfo: false,
            buildShaReported: "unknown"
        )

        // 1. Status snapshot as JSON.
        if let status = inputs.status {
            let enc = JSONEncoder()
            enc.outputFormatting = [.prettyPrinted, .sortedKeys]
            enc.dateEncodingStrategy = .iso8601
            let data = try enc.encode(status)
            let out = staging.appendingPathComponent("status.json")
            try data.write(to: out)
            result = result.with(hasStatusSnapshot: true)
        }

        // 2. App-support passthroughs. The daemon writes `build-sha.txt`
        //    (hyphenated) on every Ready transition (§9.4); P042's
        //    diagnostics bundle contract names it `build_sha.txt`
        //    (underscored) inside the zip. Rename on copy so the
        //    support-ticket consumer sees the canonical underscored
        //    artifact. When the source is absent, still materialize a
        //    `build_sha.txt` containing `"unknown"` so the zip always
        //    exposes the slot and tooling never has to special-case
        //    missing files.
        let buildShaURL = inputs.appSupportDirectory
            .appendingPathComponent("build-sha.txt", isDirectory: false)
        if fm.fileExists(atPath: buildShaURL.path) {
            let sha = (try? String(contentsOf: buildShaURL, encoding: .utf8))?
                .trimmingCharacters(in: .whitespacesAndNewlines)
            let canonical = (sha?.isEmpty == false ? sha! : "unknown")
            try Data(canonical.utf8)
                .write(to: staging.appendingPathComponent("build_sha.txt"))
            result = result.with(hasBuildSha: true, buildShaReported: canonical)
        } else {
            try Data("unknown".utf8)
                .write(to: staging.appendingPathComponent("build_sha.txt"))
            // `hasBuildSha` stays false because the source daemon file
            // was missing — the slot is populated via fallback, not
            // passthrough. Support engineers reading the manifest can
            // distinguish the two via `build_sha=="unknown"`.
            result = result.with(buildShaReported: "unknown")
        }

        for (src, present) in [
            (inputs.appSupportDirectory.appendingPathComponent("daemon.port"), "hasPortFile"),
            (inputs.appSupportDirectory.appendingPathComponent("crash-budget.json"), "hasCrashBudget"),
        ] {
            if fm.fileExists(atPath: src.path) {
                let dst = staging.appendingPathComponent(src.lastPathComponent)
                try fm.copyItem(at: src, to: dst)
                switch present {
                case "hasPortFile":    result = result.with(hasPortFile: true)
                case "hasCrashBudget": result = result.with(hasCrashBudget: true)
                default: break
                }
            }
        }

        // 3. Most recent `daemon.log.*` from the rolling-appender dir,
        //    compressed inline. P042 §9.4 names the zip artifact
        //    `daemon.log.gz` — a fixed, datestamp-less filename so
        //    support tooling doesn't need to pattern-match on dates.
        //    We preserve the source date inside the manifest if
        //    needed; the zip entry name itself is canonical.
        if let log = Self.mostRecentLog(in: inputs.logsDirectory) {
            let gzPath = staging.appendingPathComponent(
                "daemon.log.gz",
                isDirectory: false
            )
            try Self.gzipFile(from: log, to: gzPath)
            result = result.with(hasDaemonLog: true)
        }

        // 4. §9.4: principals.redacted.json. Copy the principals file but
        //    replace every token value with `<BEARER_REDACTED>`.
        if fm.fileExists(atPath: inputs.principalsPath.path),
           let data = try? Data(contentsOf: inputs.principalsPath)
        {
            let redacted = redactPrincipals(from: data)
            try redacted.write(
                to: staging.appendingPathComponent("principals.redacted.json")
            )
            result = result.with(hasPrincipalsRedacted: true)
        }

        // 5. §9.4: system_info.txt — OS version, hardware, app version,
        //    locale. Produced by an injectable closure so tests stay
        //    deterministic. An empty string means the producer opted
        //    out; we skip the file rather than write a zero-byte one.
        let systemInfo = inputs.systemInfoProducer()
        if !systemInfo.isEmpty, let data = systemInfo.data(using: .utf8) {
            try data.write(to: staging.appendingPathComponent("system_info.txt"))
            result = result.with(hasSystemInfo: true)
        }

        // 6. Manifest stamping the export time. `build_sha` carries the
        //    scalar so a support engineer can grep the zip's manifest
        //    for the SHA without unpacking `build-sha.txt`.
        let manifest: [String: Any] = [
            "exported_at": ISO8601DateFormatter().string(from: exportedAt),
            "app_version": Bundle.main.infoDictionary?["CFBundleShortVersionString"] ?? "dev",
            "build_sha": result.buildShaReported,
            "components": [
                "status_snapshot":     result.hasStatusSnapshot,
                "build_sha":           result.hasBuildSha,
                "daemon_log":          result.hasDaemonLog,
                "port_file":           result.hasPortFile,
                "crash_budget":        result.hasCrashBudget,
                "principals_redacted": result.hasPrincipalsRedacted,
                "system_info":         result.hasSystemInfo,
            ],
        ]
        let manifestData = try JSONSerialization.data(
            withJSONObject: manifest,
            options: [.prettyPrinted, .sortedKeys]
        )
        try manifestData.write(to: staging.appendingPathComponent("manifest.json"))

        // If nothing but the manifest landed, fail loudly — the caller
        // likely had misconfigured input paths.
        guard result.hasAnyComponent else {
            throw DiagnosticsBundleError.noComponentsFound
        }

        // 7. Zip the staging directory into `destination`. Uses
        //    /usr/bin/zip which is part of macOS base system.
        try runZip(staging: staging, output: destination)
        let size = (try? fm.attributesOfItem(atPath: destination.path)[.size] as? Int64) ?? 0
        result = result.with(sizeBytes: size)
        return result
    }

    /// Default `system_info.txt` producer — platform + locale + app
    /// version. Kept public so tests can swap in a stub.
    static func defaultSystemInfo() -> String {
        let pi = ProcessInfo.processInfo
        let osv = pi.operatingSystemVersion
        let osString = "macOS \(osv.majorVersion).\(osv.minorVersion).\(osv.patchVersion)"
        let app = Bundle.main.infoDictionary?["CFBundleShortVersionString"] ?? "dev"
        let bundleId = Bundle.main.bundleIdentifier ?? "unknown"
        let locale = Locale.current.identifier
        return """
        system_info.txt
        os: \(osString)
        app: \(bundleId) \(app)
        locale: \(locale)
        processor_count: \(pi.activeProcessorCount)
        memory_mb: \(pi.physicalMemory / 1024 / 1024)
        """
    }

    /// Produce a redacted copy of `principals.json`. The file is
    /// JSON-shaped: `{"principals":[{"token":"…","id":"…","class":"…"}…]}`.
    /// P042 §9.4 mandates every `token` value be replaced with the
    /// literal placeholder `"[REDACTED]"` (not the `<BEARER_REDACTED>`
    /// form the daemon's stream redactor uses — that one is for
    /// log-line `Bearer <token>` substrings, principals get the
    /// shorter placeholder because they carry no `Bearer ` prefix in
    /// the source JSON).
    static func redactPrincipals(from data: Data) -> Data {
        let placeholder = "[REDACTED]"
        guard
            var root = (try? JSONSerialization.jsonObject(with: data))
                as? [String: Any],
            var principals = root["principals"] as? [[String: Any]]
        else {
            // Unparseable principals file — ship a manifest-only note
            // that says so; never leak the raw file.
            let fallback: [String: Any] = [
                "error": "principals file unparseable; not included raw"
            ]
            return (try? JSONSerialization.data(
                withJSONObject: fallback,
                options: [.prettyPrinted, .sortedKeys]
            )) ?? Data()
        }
        for (i, entry) in principals.enumerated() {
            var redacted = entry
            if redacted["token"] != nil {
                redacted["token"] = placeholder
            }
            principals[i] = redacted
        }
        root["principals"] = principals
        return (try? JSONSerialization.data(
            withJSONObject: root,
            options: [.prettyPrinted, .sortedKeys]
        )) ?? Data()
    }

    /// Gzip `source` into `destination` via /usr/bin/gzip. The file is
    /// kept in the zip staging directory so the final bundle contains
    /// `daemon.log.yyyy-mm-dd.gz` entries and stays small even when the
    /// log spans hundreds of MB (§9.4).
    static func gzipFile(from source: URL, to destination: URL) throws {
        let fm = FileManager.default
        // /usr/bin/gzip emits to `source.gz`, so we copy first to a
        // temp file under the destination name without the .gz suffix,
        // gzip it in place, and the .gz output lands where we want.
        let tempInput = destination.deletingPathExtension()
        // `deletingPathExtension` on "path/log.gz" yields "path/log".
        try fm.copyItem(at: source, to: tempInput)
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/bin/gzip")
        proc.arguments = ["-f", tempInput.path]
        let stderr = Pipe()
        proc.standardError = stderr
        do {
            try proc.run()
        } catch {
            throw DiagnosticsBundleError.stagingFailed(error)
        }
        proc.waitUntilExit()
        if proc.terminationStatus != 0 {
            let errData = try? stderr.fileHandleForReading.readToEnd()
            let errStr = errData.flatMap { String(data: $0, encoding: .utf8) } ?? ""
            throw DiagnosticsBundleError.zipFailed(
                status: proc.terminationStatus,
                stderr: "gzip: \(errStr)"
            )
        }
    }

    /// Find the lexically-newest file matching `daemon.log*` in the
    /// rolling-appender directory. The daily rotator names files like
    /// `daemon.log.2026-04-18`, so lex-sort equals date-sort.
    private static func mostRecentLog(in dir: URL) -> URL? {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(
            at: dir,
            includingPropertiesForKeys: nil
        ) else {
            return nil
        }
        let candidates = entries.filter { $0.lastPathComponent.hasPrefix("daemon.log") }
        return candidates.sorted { $0.lastPathComponent > $1.lastPathComponent }.first
    }

    /// Runs `/usr/bin/zip -r destination.zip .` from inside `staging` so
    /// the archive is flat (no absolute paths leak into the zip).
    private static func runZip(staging: URL, output: URL) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/zip")
        process.arguments = ["-r", output.path, "."]
        process.currentDirectoryURL = staging
        let stderr = Pipe()
        process.standardError = stderr
        do {
            try process.run()
        } catch {
            throw DiagnosticsBundleError.zipFailed(status: -1, stderr: "\(error)")
        }
        process.waitUntilExit()
        if process.terminationStatus != 0 {
            let errData = try? stderr.fileHandleForReading.readToEnd()
            let errStr = errData.flatMap { String(data: $0, encoding: .utf8) } ?? ""
            throw DiagnosticsBundleError.zipFailed(
                status: process.terminationStatus,
                stderr: errStr
            )
        }
    }
}

private extension DiagnosticsExportResult {
    var hasAnyComponent: Bool {
        hasStatusSnapshot
            || hasBuildSha
            || hasDaemonLog
            || hasPortFile
            || hasCrashBudget
            || hasPrincipalsRedacted
            || hasSystemInfo
    }

    func with(
        bundleURL: URL? = nil,
        sizeBytes: Int64? = nil,
        hasStatusSnapshot: Bool? = nil,
        hasBuildSha: Bool? = nil,
        hasDaemonLog: Bool? = nil,
        hasPortFile: Bool? = nil,
        hasCrashBudget: Bool? = nil,
        hasPrincipalsRedacted: Bool? = nil,
        hasSystemInfo: Bool? = nil,
        buildShaReported: String? = nil
    ) -> DiagnosticsExportResult {
        DiagnosticsExportResult(
            bundleURL: bundleURL ?? self.bundleURL,
            sizeBytes: sizeBytes ?? self.sizeBytes,
            hasStatusSnapshot: hasStatusSnapshot ?? self.hasStatusSnapshot,
            hasBuildSha: hasBuildSha ?? self.hasBuildSha,
            hasDaemonLog: hasDaemonLog ?? self.hasDaemonLog,
            hasPortFile: hasPortFile ?? self.hasPortFile,
            hasCrashBudget: hasCrashBudget ?? self.hasCrashBudget,
            hasPrincipalsRedacted: hasPrincipalsRedacted ?? self.hasPrincipalsRedacted,
            hasSystemInfo: hasSystemInfo ?? self.hasSystemInfo,
            buildShaReported: buildShaReported ?? self.buildShaReported
        )
    }
}
