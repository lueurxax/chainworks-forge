import Foundation

struct AppConfiguration: Codable, Equatable, Sendable {
    var runStorageBasePath: String
    var worktreeBasePath: String?
    var workflowSourcePath: String
    var agentCatalogSourcePath: String
    var supportBundleExportPath: String?
    var gooseServerHost: String
    var gooseServerPort: Int
    var gooseServerTLS: Bool
    var gooseServerAutostart: Bool
    var gooseServerBinaryPath: String?
    var gooseServerSecretKey: String?
    var activeConfigurationSource: ConfigurationSource

    init(
        runStorageBasePath: String,
        worktreeBasePath: String?,
        workflowSourcePath: String,
        agentCatalogSourcePath: String,
        supportBundleExportPath: String?,
        gooseServerHost: String = "127.0.0.1",
        gooseServerPort: Int = 51200,
        gooseServerTLS: Bool = true,
        gooseServerAutostart: Bool = true,
        gooseServerBinaryPath: String? = AppConfiguration.defaultGooseServerBinaryPath(),
        gooseServerSecretKey: String? = nil,
        activeConfigurationSource: ConfigurationSource
    ) {
        self.runStorageBasePath = runStorageBasePath
        self.worktreeBasePath = worktreeBasePath
        self.workflowSourcePath = workflowSourcePath
        self.agentCatalogSourcePath = agentCatalogSourcePath
        self.supportBundleExportPath = supportBundleExportPath
        self.gooseServerHost = gooseServerHost
        self.gooseServerPort = gooseServerPort
        self.gooseServerTLS = gooseServerTLS
        self.gooseServerAutostart = gooseServerAutostart
        self.gooseServerBinaryPath = gooseServerBinaryPath
        self.gooseServerSecretKey = gooseServerSecretKey
        self.activeConfigurationSource = activeConfigurationSource
    }

    var runStorageBaseURL: URL {
        URL(fileURLWithPath: runStorageBasePath, isDirectory: true)
    }

    var workflowSourceURL: URL {
        URL(fileURLWithPath: workflowSourcePath)
    }

    var agentCatalogSourceURL: URL {
        URL(fileURLWithPath: agentCatalogSourcePath)
    }

    var gooseServerBaseURL: URL? {
        var components = URLComponents()
        components.scheme = gooseServerTLS ? "https" : "http"
        components.host = gooseServerHost
        components.port = gooseServerPort
        return components.url
    }

    static func seededDefault() -> AppConfiguration {
        let repoRoot = defaultRepositoryRoot()
        let supportRoot = defaultSupportRoot()
        let runStorage = supportRoot.appendingPathComponent("runs", isDirectory: true)
        let exportRoot = supportRoot.appendingPathComponent("exports", isDirectory: true)

        return AppConfiguration(
            runStorageBasePath: runStorage.path,
            worktreeBasePath: supportRoot.appendingPathComponent("worktrees", isDirectory: true).path,
            workflowSourcePath: repoRoot.appendingPathComponent("examples/workflows/workflow.yaml").path,
            agentCatalogSourcePath: repoRoot.appendingPathComponent("examples/agents/agents.yaml").path,
            supportBundleExportPath: exportRoot.path,
            gooseServerHost: "127.0.0.1",
            gooseServerPort: 51200,
            gooseServerTLS: true,
            gooseServerAutostart: true,
            gooseServerBinaryPath: defaultGooseServerBinaryPath(),
            gooseServerSecretKey: nil,
            activeConfigurationSource: .persistedSettings
        )
    }

    static func defaultRepositoryRoot() -> URL {
        let fileManager = FileManager.default
        let candidates = [
            URL(fileURLWithPath: fileManager.currentDirectoryPath, isDirectory: true),
            URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
                .appendingPathComponent("Documents/Chainworks Forge", isDirectory: true)
        ]

        if let repoRoot = candidates.first(where: {
            fileManager.fileExists(atPath: $0.appendingPathComponent("examples/agents/agents.yaml").path)
        }) {
            return repoRoot
        }

        return candidates[0]
    }

    static func defaultSupportRoot() -> URL {
        let fileManager = FileManager.default

        if ProcessInfo.processInfo.environment["CHAINWORKS_IN_MEMORY_STORE"] == "1" {
            return fileManager.temporaryDirectory
                .appendingPathComponent("ChainworksForgeSupport", isDirectory: true)
        }

        let appSupport = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support", isDirectory: true)

        return appSupport.appendingPathComponent("Chainworks Forge", isDirectory: true)
    }

    static func defaultGooseServerBinaryPath() -> String? {
        let candidates = [
            "/Applications/Goose.app/Contents/Resources/bin/goosed",
            NSHomeDirectory() + "/Applications/Goose.app/Contents/Resources/bin/goosed"
        ]

        return candidates.first {
            FileManager.default.isExecutableFile(atPath: $0)
        }
    }
}

enum ConfigurationSource: String, Codable, CaseIterable, Sendable {
    case persistedSettings = "persisted_settings"
    case seededFromEnv = "seeded_from_env"
    case developmentEnvOverride = "development_env_override"

    var displayName: String {
        switch self {
        case .persistedSettings:
            return "Persisted Settings"
        case .seededFromEnv:
            return "Seeded from Environment"
        case .developmentEnvOverride:
            return "Development Env Override"
        }
    }
}
