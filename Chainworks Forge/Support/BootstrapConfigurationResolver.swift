import Foundation

enum BootstrapConfigurationResolver {
    static func resolve(
        store: AppConfigurationStore,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> AppConfiguration {
        let allowOverride = environment["CHAINWORKS_ALLOW_ENV_OVERRIDE"] == "1"
        let persisted = store.configuration
        let envSeed = buildEnvironmentSeed(environment: environment, fallback: persisted)

        if persisted.activeConfigurationSource == .persistedSettings && !allowOverride {
            return persisted
        }

        if allowOverride {
            var overridden = envSeed
            overridden.activeConfigurationSource = .developmentEnvOverride
            store.replace(with: overridden)
            return overridden
        }

        if !store.hasPersistedConfiguration() {
            var seeded = envSeed
            seeded.activeConfigurationSource = .seededFromEnv
            store.replace(with: seeded)
            return seeded
        }

        return persisted
    }

    private static func buildEnvironmentSeed(
        environment: [String: String],
        fallback: AppConfiguration
    ) -> AppConfiguration {
        return AppConfiguration(
            runStorageBasePath: environment["CHAINWORKS_RUN_STORAGE_BASE_PATH"] ?? fallback.runStorageBasePath,
            worktreeBasePath: environment["CHAINWORKS_WORKTREE_BASE_PATH"] ?? fallback.worktreeBasePath,
            workflowSourcePath: environment["CHAINWORKS_WORKFLOW_SOURCE_PATH"] ?? fallback.workflowSourcePath,
            agentCatalogSourcePath: environment["CHAINWORKS_AGENT_CATALOG_SOURCE_PATH"] ?? fallback.agentCatalogSourcePath,
            supportBundleExportPath: environment["CHAINWORKS_SUPPORT_BUNDLE_EXPORT_PATH"] ?? fallback.supportBundleExportPath,
            activeConfigurationSource: fallback.activeConfigurationSource
        )
    }
}
