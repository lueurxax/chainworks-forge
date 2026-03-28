import Foundation
import Observation

@Observable
final class AppConfigurationStore {
    private let fileURL: URL
    private let loadedPersistedConfiguration: Bool
    @MainActor private(set) var configuration: AppConfiguration

    @MainActor
    init(fileURL: URL? = nil, initialConfiguration: AppConfiguration? = nil) {
        let resolvedURL = fileURL ?? Self.defaultFileURL()
        self.fileURL = resolvedURL

        if let initialConfiguration {
            self.loadedPersistedConfiguration = true
            self.configuration = initialConfiguration
            try? persist()
            return
        }

        if let loaded = try? Self.load(from: resolvedURL) {
            self.loadedPersistedConfiguration = true
            self.configuration = loaded
        } else {
            self.loadedPersistedConfiguration = false
            self.configuration = AppConfiguration.seededDefault()
        }
    }

    @MainActor
    func replace(with configuration: AppConfiguration) {
        self.configuration = configuration
        try? persist()
    }

    @MainActor
    func update(_ mutate: (inout AppConfiguration) -> Void) {
        var copy = configuration
        mutate(&copy)
        configuration = copy
        try? persist()
    }

    @MainActor
    func reload() {
        if let loaded = try? Self.load(from: fileURL) {
            configuration = loaded
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

    static func defaultFileURL() -> URL {
        AppConfiguration.defaultSupportRoot()
            .appendingPathComponent("app-configuration.json")
    }
}
