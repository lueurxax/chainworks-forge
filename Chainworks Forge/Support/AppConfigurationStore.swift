import Foundation
import Observation

@Observable
final class AppConfigurationStore {
    private let fileURL: URL
    private let loadedPersistedConfiguration: Bool
    @MainActor private(set) var configuration: AppConfiguration
    @MainActor private(set) var diagnosticsMessage: String?

    @MainActor
    init(fileURL: URL? = nil, initialConfiguration: AppConfiguration? = nil) {
        let resolvedURL = fileURL ?? Self.defaultFileURL()
        self.fileURL = resolvedURL
        self.diagnosticsMessage = nil

        if let initialConfiguration {
            self.loadedPersistedConfiguration = true
            self.configuration = initialConfiguration
            persistOrRecordFailure(summary: "Failed to persist app configuration during initialization")
            return
        }

        let fileExists = FileManager.default.fileExists(atPath: resolvedURL.path)
        if let loaded = try? Self.load(from: resolvedURL) {
            self.loadedPersistedConfiguration = true
            self.configuration = loaded
            clearDiagnostics()
        } else {
            self.loadedPersistedConfiguration = false
            self.configuration = AppConfiguration.seededDefault()
            if fileExists {
                recordFailure(
                    summary: "Failed to load persisted app configuration; using defaults instead",
                    error: AppConfigurationStoreError.loadFailed(resolvedURL.path)
                )
                do {
                    try persist()
                } catch {
                    recordFailure(summary: "Failed to persist fallback app configuration", error: error)
                }
                return
            }
            persistOrRecordFailure(summary: "Failed to create default app configuration store")
        }
    }

    @MainActor
    func replace(with configuration: AppConfiguration) {
        self.configuration = configuration
        persistOrRecordFailure(summary: "Failed to save app configuration")
    }

    @MainActor
    func update(_ mutate: (inout AppConfiguration) -> Void) {
        var copy = configuration
        mutate(&copy)
        configuration = copy
        persistOrRecordFailure(summary: "Failed to save app configuration")
    }

    @MainActor
    func reload() {
        if let loaded = try? Self.load(from: fileURL) {
            configuration = loaded
            clearDiagnostics()
        } else {
            recordFailure(
                summary: "Failed to reload persisted app configuration",
                error: AppConfigurationStoreError.loadFailed(fileURL.path)
            )
        }
    }

    @MainActor
    func hasPersistedConfiguration() -> Bool {
        loadedPersistedConfiguration
    }

    private func persist() throws {
        let directory = fileURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        let data = try encoder.encode(configuration)
        try data.write(to: fileURL, options: .atomic)
    }

    private static func load(from fileURL: URL) throws -> AppConfiguration {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try decoder.decode(AppConfiguration.self, from: Data(contentsOf: fileURL))
    }

    @MainActor
    private func persistOrRecordFailure(summary: String) {
        do {
            try persist()
            clearDiagnostics()
        } catch {
            recordFailure(summary: summary, error: error)
        }
    }

    @MainActor
    private func recordFailure(summary: String, error: Error) {
        let message = "\(summary): \(error.localizedDescription)"
        diagnosticsMessage = message
        ForgeLogger.app.error(message)
    }

    @MainActor
    private func clearDiagnostics() {
        diagnosticsMessage = nil
    }

    static func defaultFileURL() -> URL {
        AppConfiguration.defaultSupportRoot()
            .appendingPathComponent("app-configuration.json")
    }
}

private enum AppConfigurationStoreError: LocalizedError {
    case loadFailed(String)

    var errorDescription: String? {
        switch self {
        case .loadFailed(let path):
            "Could not read app configuration at \(path)"
        }
    }
}
