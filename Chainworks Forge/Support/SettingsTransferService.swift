import Foundation

struct ExportableSettingsPackage: Codable, Equatable {
    var transferSchemaVersion: Int
    var appConfiguration: AppConfiguration
    var providerSettings: ProviderSettings
    var exportedAt: Date
    var appVersion: String
    var secretPlaceholders: [String]
}

enum SettingsTransferError: Error, LocalizedError {
    case unsupportedSchema(Int)
    case missingPlaceholders([String])
    case invalidConfiguration(String)
    case unsupportedProviderFamily(String)

    var errorDescription: String? {
        switch self {
        case .unsupportedSchema(let version):
            return "Settings schema version \(version) is not supported"
        case .missingPlaceholders(let placeholders):
            return "Imported settings require credentials: \(placeholders.joined(separator: ", "))"
        case .invalidConfiguration(let reason):
            return "Imported settings are invalid: \(reason)"
        case .unsupportedProviderFamily(let family):
            return "Imported settings reference unsupported provider family '\(family)'"
        }
    }
}

@MainActor
struct SettingsTransferService {
    static let currentSchemaVersion = 1

    let appConfigurationStore: AppConfigurationStore
    let providerSettingsStore: ProviderSettingsStore
    let secretStore: KeychainSecretStore

    func exportSettings(to directory: URL? = nil) throws -> URL {
        let outputDirectory = directory
            ?? (appConfigurationStore.configuration.supportBundleExportPath.map {
                URL(fileURLWithPath: $0, isDirectory: true)
            } ?? AppConfiguration.defaultSupportRoot().appendingPathComponent("exports", isDirectory: true))
        SecurityScopedAccess.remember(url: outputDirectory, kind: .supportBundleRoot)

        let placeholders: [String] = providerSettingsStore.settings.configuredProviders.compactMap { provider in
            switch provider.authMode {
            case .none:
                return nil
            case .apiKey, .sessionToken:
                return ProviderAdapterSupport.secretKey(for: provider)
            }
        }

        let package = ExportableSettingsPackage(
            transferSchemaVersion: Self.currentSchemaVersion,
            appConfiguration: appConfigurationStore.configuration,
            providerSettings: providerSettingsStore.settings,
            exportedAt: Date(),
            appVersion: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "dev",
            secretPlaceholders: placeholders
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        let payload = try encoder.encode(package)
        return try SecurityScopedAccess.withAccess(to: outputDirectory) { securedDirectory in
            try FileManager.default.createDirectory(at: securedDirectory, withIntermediateDirectories: true)
            let fileURL = securedDirectory.appendingPathComponent("chainworks-settings.json")
            try payload.write(to: fileURL, options: .atomic)
            return fileURL
        }
    }

    @discardableResult
    func importSettings(from fileURL: URL) throws -> [String] {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        SecurityScopedAccess.remember(url: fileURL, kind: .settingsFile)
        let package = try decoder.decode(ExportableSettingsPackage.self, from: SecurityScopedAccess.loadData(from: fileURL))

        guard package.transferSchemaVersion == Self.currentSchemaVersion else {
            throw SettingsTransferError.unsupportedSchema(package.transferSchemaVersion)
        }

        try validate(package: package)

        let missingSecrets = package.secretPlaceholders.filter {
            ((try? secretStore.secret(for: $0)) ?? "").isEmpty
        }

        if !missingSecrets.isEmpty {
            throw SettingsTransferError.missingPlaceholders(missingSecrets)
        }

        appConfigurationStore.replace(with: package.appConfiguration)
        providerSettingsStore.replace(with: package.providerSettings)

        return []
    }

    private func validate(package: ExportableSettingsPackage) throws {
        let appConfiguration = package.appConfiguration
        guard !appConfiguration.runStorageBasePath.isEmpty else {
            throw SettingsTransferError.invalidConfiguration("runStorageBasePath is empty")
        }
        guard !appConfiguration.workflowSourcePath.isEmpty else {
            throw SettingsTransferError.invalidConfiguration("workflowSourcePath is empty")
        }
        guard !appConfiguration.agentCatalogSourcePath.isEmpty else {
            throw SettingsTransferError.invalidConfiguration("agentCatalogSourcePath is empty")
        }

        let configuredProviders = package.providerSettings.configuredProviders
        for provider in configuredProviders {
            if ProviderFamily(rawValue: provider.family.rawValue) == nil {
                throw SettingsTransferError.unsupportedProviderFamily(provider.family.rawValue)
            }
        }
    }
}
