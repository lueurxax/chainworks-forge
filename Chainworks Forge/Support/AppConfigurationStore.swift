import Foundation
import Observation

@MainActor
@Observable
final class AppConfigurationStore {
    private let fileURL: URL
    private(set) var configuration: AppConfiguration

    init(fileURL: URL? = nil, initialConfiguration: AppConfiguration? = nil) {
        let resolvedURL = fileURL ?? Self.defaultFileURL()
        self.fileURL = resolvedURL

        if let initialConfiguration {
            self.configuration = initialConfiguration
            try? persist()
            return
        }

        if let loaded = try? Self.load(from: resolvedURL) {
            self.configuration = loaded
        } else {
            self.configuration = AppConfiguration.seededDefault()
            try? persist()
        }
    }

    func replace(with configuration: AppConfiguration) {
        self.configuration = configuration
        try? persist()
    }

    func update(_ mutate: (inout AppConfiguration) -> Void) {
        var copy = configuration
        mutate(&copy)
        configuration = copy
        try? persist()
    }

    func reload() {
        if let loaded = try? Self.load(from: fileURL) {
            configuration = loaded
        }
    }

    func hasPersistedConfiguration() -> Bool {
        FileManager.default.fileExists(atPath: fileURL.path)
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
