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
        defaultRepositoryRoot(
            currentDirectoryPath: FileManager.default.currentDirectoryPath,
            bundleURL: Bundle.main.bundleURL,
            allowsDocumentsFallback: allowsDocumentsFallbackForCurrentProcess,
            sourceFilePath: #filePath
        )
    }

    static func defaultRepositoryRoot(
        currentDirectoryPath: String,
        bundleURL: URL?,
        allowsDocumentsFallback: Bool,
        sourceFilePath: String
    ) -> URL {
        let fileManager = FileManager.default
        let candidates = candidateRepositoryRoots(
            currentDirectoryPath: currentDirectoryPath,
            bundleURL: bundleURL,
            allowsDocumentsFallback: allowsDocumentsFallback,
            sourceFilePath: sourceFilePath
        )

        if let repoRoot = candidates.first(where: {
            fileManager.fileExists(atPath: $0.appendingPathComponent("examples/agents/agents.yaml").path)
        }) {
            return repoRoot
        }

        return candidates[0]
    }

    static func candidateRepositoryRoots(
        currentDirectoryPath: String = FileManager.default.currentDirectoryPath,
        bundleURL: URL? = Bundle.main.bundleURL,
        allowsDocumentsFallback: Bool = AppConfiguration.allowsDocumentsFallbackForCurrentProcess,
        sourceFilePath: String = #filePath
    ) -> [URL] {
        let fileManager = FileManager.default
        var candidates: [URL] = []

        func append(_ url: URL?) {
            guard let url else { return }
            let standardized = url.standardizedFileURL
            if candidates.contains(where: { $0.standardizedFileURL == standardized }) {
                return
            }
            candidates.append(standardized)
        }

        if let override = ProcessInfo.processInfo.environment["CHAINWORKS_REPOSITORY_ROOT"],
           !override.isEmpty {
            append(URL(fileURLWithPath: override, isDirectory: true))
        }

        let currentDirectoryURL = URL(fileURLWithPath: currentDirectoryPath, isDirectory: true)
        let sourceRepositoryRoot = repositoryRootDerivedFromSourcePath(sourceFilePath)
        let authorizedRoots = SecurityScopedAccess.authorizedRepositoryRoots()

        if authorizedRoots.contains(where: {
            currentDirectoryURL.standardizedFileURL.path == $0.path
                || currentDirectoryURL.standardizedFileURL.path.hasPrefix($0.path + "/")
        }) || currentDirectoryURL.standardizedFileURL.path == sourceRepositoryRoot.standardizedFileURL.path
            || currentDirectoryURL.standardizedFileURL.path.hasPrefix(sourceRepositoryRoot.standardizedFileURL.path + "/") {
            append(currentDirectoryURL)
        }

        append(sourceRepositoryRoot)
        authorizedRoots.forEach(append)

        if let bundleURL {
            append(bundleURL)
            append(bundleURL.deletingLastPathComponent())
            append(bundleURL.deletingLastPathComponent().deletingLastPathComponent())
        }

        if allowsDocumentsFallback {
            let documentsFallback = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
                .appendingPathComponent("Documents/Chainworks Forge", isDirectory: true)
            if SecurityScopedAccess.hasBookmark(for: documentsFallback) {
                append(documentsFallback)
            }
        }

        if candidates.isEmpty {
            append(URL(fileURLWithPath: fileManager.currentDirectoryPath, isDirectory: true))
        }

        return candidates
    }

    static func repositoryRootDerivedFromSourcePath(_ sourceFilePath: String = #filePath) -> URL {
        URL(fileURLWithPath: sourceFilePath, isDirectory: false)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    static func preferredExampleURL(
        configuredURL: URL? = nil,
        repoRelativePath: String,
        bundledURL: URL? = nil,
        currentDirectoryPath: String = FileManager.default.currentDirectoryPath,
        allowsDocumentsFallback: Bool = AppConfiguration.allowsDocumentsFallbackForCurrentProcess,
        sourceFilePath: String = #filePath
    ) -> URL? {
        let trimmedRelativePath = repoRelativePath.trimmingCharacters(in: CharacterSet(charactersIn: "/"))

        var candidates: [URL?] = [configuredURL]
        let sourceRepositoryRoot = repositoryRootDerivedFromSourcePath(sourceFilePath)
        let currentDirectoryURL = URL(fileURLWithPath: currentDirectoryPath, isDirectory: true)

        if currentDirectoryURL.standardizedFileURL.path == sourceRepositoryRoot.standardizedFileURL.path
            || currentDirectoryURL.standardizedFileURL.path.hasPrefix(sourceRepositoryRoot.standardizedFileURL.path + "/")
            || SecurityScopedAccess.hasBookmark(for: currentDirectoryURL) {
            candidates.append(currentDirectoryURL.appendingPathComponent(trimmedRelativePath))
        }

        candidates.append(sourceRepositoryRoot.appendingPathComponent(trimmedRelativePath))
        candidates.append(contentsOf: SecurityScopedAccess.authorizedRepositoryRoots().map {
            $0.appendingPathComponent(trimmedRelativePath)
        })
        candidates.append(defaultRepositoryRoot(
                currentDirectoryPath: currentDirectoryPath,
                bundleURL: nil,
                allowsDocumentsFallback: allowsDocumentsFallback,
                sourceFilePath: sourceFilePath
        ).appendingPathComponent(trimmedRelativePath))

        if allowsDocumentsFallback {
            let documentsFallback = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
                .appendingPathComponent("Documents/Chainworks Forge", isDirectory: true)
            if SecurityScopedAccess.hasBookmark(for: documentsFallback) {
                candidates.append(documentsFallback.appendingPathComponent(trimmedRelativePath))
            }
        }

        candidates.append(bundledURL)

        return candidates.first { candidate in
            guard let candidate else { return false }
            return SecurityScopedAccess.fileExists(at: candidate)
        } ?? nil
    }

    static func defaultSupportRoot() -> URL {
        let fileManager = FileManager.default

        if ProcessInfo.processInfo.environment["CHAINWORKS_IN_MEMORY_STORE"] == "1" {
            let base = fileManager.temporaryDirectory
                .appendingPathComponent("ChainworksForgeSupport", isDirectory: true)
            if let sessionID = ProcessInfo.processInfo.environment["CHAINWORKS_UI_TEST_SESSION_ID"],
               !sessionID.isEmpty {
                return base.appendingPathComponent(sessionID, isDirectory: true)
            }
            return base
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

    static var allowsDocumentsFallbackForCurrentProcess: Bool {
        let environment = ProcessInfo.processInfo.environment
        return environment["CHAINWORKS_IN_MEMORY_STORE"] != "1"
            && environment["CHAINWORKS_UI_TEST_SESSION_ID"] == nil
            && environment["CHAINWORKS_UI_TEST_INITIAL_TAB"] == nil
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
