import Foundation

struct PreflightCheck: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    let category: String
    let title: String
    let status: PreflightCheckStatus
    let message: String

    init(id: UUID = UUID(), category: String, title: String, status: PreflightCheckStatus, message: String) {
        self.id = id
        self.category = category
        self.title = title
        self.status = status
        self.message = message
    }
}

enum PreflightCheckStatus: String, Codable, Equatable, Sendable {
    case pass
    case warn
    case fail
}

struct PreflightReport: Codable, Equatable, Sendable {
    let status: PreflightStatus
    let configurationSource: ConfigurationSource
    let checks: [PreflightCheck]
    let warnings: [String]
    let blockingIssues: [String]
}

enum PreflightStatus: String, Codable, Equatable, Sendable {
    case pass
    case warn
    case fail
}

@MainActor
struct PreflightService {
    let appConfigurationStore: AppConfigurationStore
    let providerRegistry: ProviderRegistry

    func runReport(
        workflowURL: URL,
        catalogURL: URL,
        plan: RunPlan?,
        startOptions: RunStartOptions,
        idea: Idea? = nil
    ) async -> PreflightReport {
        var checks: [PreflightCheck] = []
        var warnings: [String] = []
        var blockingIssues: [String] = []

        let config = appConfigurationStore.configuration
        checks.append(checkFile(category: "Catalog", title: "Workflow YAML", url: workflowURL))
        checks.append(checkFile(category: "Catalog", title: "Agent Catalog", url: catalogURL))

        do {
            let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
            let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
            let issues = YAMLValidator.validateAll(workflow: workflow, catalog: catalog)
            let errors = issues.filter { $0.severity == .error }
            checks.append(PreflightCheck(
                category: "Catalog",
                title: "YAML Validation",
                status: errors.isEmpty ? .pass : .fail,
                message: errors.isEmpty ? "Workflow and catalog parsed successfully" : errors.map(\.message).joined(separator: "; ")
            ))

            // Proposal 013 Layer Q: Structured Output Schema Gate
            // Ensures backend_profiles.*.structured_output reaches transport or triggers preflight failure.
            let gateResults = StructuredOutputSchemaGate.validate(catalog: catalog)
            let blockingGates = gateResults.filter { $0.isBlocking }
            if blockingGates.isEmpty {
                checks.append(PreflightCheck(
                    category: "Contracts",
                    title: "Structured Output Gate",
                    status: .pass,
                    message: "All structured_output declarations are supported by their transport"
                ))
            } else {
                let msg = blockingGates.map { $0.explanation }.joined(separator: "; ")
                checks.append(PreflightCheck(
                    category: "Contracts",
                    title: "Structured Output Gate",
                    status: .fail,
                    message: msg
                ))
                blockingIssues.append("Structured output gate: \(msg)")
            }

            let warningGates = gateResults.filter { !$0.isBlocking && !$0.transportSupportsStructured }
            if !warningGates.isEmpty {
                let warnMsg = warningGates.map { $0.explanation }.joined(separator: "; ")
                warnings.append("Structured output: \(warnMsg)")
            }
        } catch {
            checks.append(PreflightCheck(
                category: "Catalog",
                title: "YAML Validation",
                status: .fail,
                message: error.localizedDescription
            ))
        }

        let runStorageURL = config.runStorageBaseURL
        checks.append(checkDirectory(category: "Workspace", title: "Run Storage", url: runStorageURL))
        checks.append(checkDerivedWorkspace(runStorageURL: runStorageURL))
        checks.append(checkDerivedArtifactRoot(runStorageURL: runStorageURL))

        if let worktreeBasePath = config.worktreeBasePath {
            checks.append(checkDirectory(category: "Workspace", title: "Worktree Base", url: URL(fileURLWithPath: worktreeBasePath, isDirectory: true)))
        }
        checks.append(checkWorkspaceIsolation(runStorageURL: runStorageURL, worktreeBasePath: config.worktreeBasePath))

        // Proposal 011 (REQ-005, REQ-006): Workspace readiness — fail-closed when requiresProjectAccess is true.
        if let plan, plan.requiresProjectAccess {
            checks.append(checkIdeaWorkspaceReadiness(idea: idea))
        }

        let hasProviders = !providerRegistry.configuredProviders.isEmpty
        checks.append(PreflightCheck(
            category: "Providers",
            title: "Configured Providers",
            status: hasProviders ? .pass : .fail,
            message: hasProviders ? "\(providerRegistry.configuredProviders.count) configured provider(s)" : "No configured providers"
        ))

        await providerRegistry.refreshHealth()
        let resolver = BackendProfileResolverV2(providerRegistry: providerRegistry)
        let providerBindings: [String: ResolvedProviderBinding]
        if let plan {
            do {
                providerBindings = try resolver.resolveBindings(plan: plan, startOptions: startOptions)
                checks.append(PreflightCheck(
                    category: "Providers",
                    title: "Provider Binding Resolution",
                    status: .pass,
                    message: "Resolved \(providerBindings.count) provider binding(s)"
                ))
            } catch {
                providerBindings = [:]
                let message = error.localizedDescription
                checks.append(PreflightCheck(
                    category: "Providers",
                    title: "Provider Binding Resolution",
                    status: .fail,
                    message: message
                ))
                blockingIssues.append(message)
            }
        } else {
            providerBindings = [:]
        }

        // Proposal 011 (REQ-010): Cross-family coherence check.
        for (agentID, binding) in providerBindings {
            let coherenceResult = checkBindingCoherence(agentID: agentID, binding: binding)
            if let warning = coherenceResult {
                checks.append(warning)
                warnings.append(warning.message)
            }
        }

        let requiredFamilies = Set(plan?.agentBindings.values.compactMap { ProviderFamily.from(runtimeIdentifier: $0.provider) } ?? [])
        for family in requiredFamilies.sorted(by: { $0.rawValue < $1.rawValue }) {
            guard let provider = providerRegistry.preferredProvider(for: family) else {
                let issue = "No provider configured for \(family.displayName)"
                checks.append(PreflightCheck(category: "Providers", title: "\(family.displayName) Availability", status: .fail, message: issue))
                blockingIssues.append(issue)
                continue
            }
            await appendProviderChecks(
                for: provider,
                bindings: providerBindings,
                checks: &checks,
                warnings: &warnings,
                blockingIssues: &blockingIssues
            )
        }

        let keychainAccessible = providerRegistry.secretStore.isAccessible()
        checks.append(PreflightCheck(
            category: "Environment",
            title: "Keychain",
            status: keychainAccessible ? .pass : .warn,
            message: keychainAccessible ? "Secrets store accessible" : "Keychain access requires attention"
        ))
        checks.append(checkDirectory(
            category: "Environment",
            title: "Support Bundle Export",
            url: supportBundleExportURL(for: config)
        ))

        let finalStatus: PreflightStatus
        if checks.contains(where: { $0.status == .fail }) {
            finalStatus = .fail
        } else if checks.contains(where: { $0.status == .warn }) {
            finalStatus = .warn
        } else {
            finalStatus = .pass
        }

        warnings.append(contentsOf: checks.filter { $0.status == .warn }.map(\.message))
        blockingIssues.append(contentsOf: checks.filter { $0.status == .fail }.map(\.message))

        return PreflightReport(
            status: finalStatus,
            configurationSource: config.activeConfigurationSource,
            checks: checks,
            warnings: Array(Set(warnings)),
            blockingIssues: Array(Set(blockingIssues))
        )
    }

    func runReport(
        workflowURL: URL,
        catalogURL: URL,
        plan: RunPlan?,
        idea: Idea? = nil
    ) async -> PreflightReport {
        await runReport(
            workflowURL: workflowURL,
            catalogURL: catalogURL,
            plan: plan,
            startOptions: RunStartOptions(),
            idea: idea
        )
    }

    private func checkFile(category: String, title: String, url: URL) -> PreflightCheck {
        let exists = FileManager.default.isReadableFile(atPath: url.path)
        return PreflightCheck(
            category: category,
            title: title,
            status: exists ? .pass : .fail,
            message: exists ? url.path : "File missing at \(url.path)"
        )
    }

    private func checkDirectory(category: String, title: String, url: URL) -> PreflightCheck {
        let fileManager = FileManager.default
        do {
            try fileManager.createDirectory(at: url, withIntermediateDirectories: true)
            return PreflightCheck(category: category, title: title, status: .pass, message: url.path)
        } catch {
            return PreflightCheck(category: category, title: title, status: .fail, message: error.localizedDescription)
        }
    }

    private func checkDerivedWorkspace(runStorageURL: URL) -> PreflightCheck {
        let workspaceURL = runStorageURL.appendingPathComponent(".preflight-workspace-\(UUID().uuidString)", isDirectory: true)
        let fileManager = FileManager.default
        do {
            try fileManager.createDirectory(at: workspaceURL, withIntermediateDirectories: true)
            try? fileManager.removeItem(at: workspaceURL)
            return PreflightCheck(
                category: "Workspace",
                title: "Derived Workspace Root",
                status: .pass,
                message: workspaceURL.path
            )
        } catch {
            return PreflightCheck(
                category: "Workspace",
                title: "Derived Workspace Root",
                status: .fail,
                message: error.localizedDescription
            )
        }
    }

    private func checkDerivedArtifactRoot(runStorageURL: URL) -> PreflightCheck {
        let workspaceURL = runStorageURL.appendingPathComponent(".preflight-artifact-\(UUID().uuidString)", isDirectory: true)
        let artifactURL = workspaceURL.appendingPathComponent("artifacts", isDirectory: true)
        let fileManager = FileManager.default
        do {
            try fileManager.createDirectory(at: artifactURL, withIntermediateDirectories: true)
            let probeURL = artifactURL.appendingPathComponent("write-probe.txt")
            try Data("ok".utf8).write(to: probeURL, options: .atomic)
            try? fileManager.removeItem(at: workspaceURL)
            return PreflightCheck(
                category: "Workspace",
                title: "Derived Artifact Root",
                status: .pass,
                message: artifactURL.path
            )
        } catch {
            return PreflightCheck(
                category: "Workspace",
                title: "Derived Artifact Root",
                status: .fail,
                message: error.localizedDescription
            )
        }
    }

    private func checkWorkspaceIsolation(runStorageURL: URL, worktreeBasePath: String?) -> PreflightCheck {
        guard let worktreeBasePath, !worktreeBasePath.isEmpty else {
            return PreflightCheck(
                category: "Permissions",
                title: "Workspace Isolation",
                status: .pass,
                message: "No worktree base configured for this provider-only slice"
            )
        }

        let runStoragePath = runStorageURL.standardizedFileURL.path
        let worktreePath = URL(fileURLWithPath: worktreeBasePath, isDirectory: true).standardizedFileURL.path
        let overlaps = worktreePath.hasPrefix(runStoragePath + "/")
            || runStoragePath.hasPrefix(worktreePath + "/")

        return PreflightCheck(
            category: "Permissions",
            title: "Workspace Isolation",
            status: overlaps ? .fail : .pass,
            message: overlaps
                ? "Worktree base overlaps run storage and violates workspace isolation"
                : "Worktree base is isolated from run storage"
        )
    }

    // Proposal 011 (REQ-010): Detect cross-family provider/model mismatches.
    // Uses the shared `hasCrossFamilyMismatch` heuristic on ResolvedProviderBinding.
    private func checkBindingCoherence(agentID: String, binding: ResolvedProviderBinding) -> PreflightCheck? {
        guard binding.hasCrossFamilyMismatch else { return nil }
        return PreflightCheck(
            category: "Providers",
            title: "Binding Coherence — \(agentID)",
            status: .warn,
            message: "Agent '\(agentID)' uses provider family '\(binding.providerFamily)' but resolved model '\(binding.model)' appears to belong to a different family. Verify this is intentional."
        )
    }

    // Proposal 011 (REQ-005, REQ-006): Validate idea has a valid, accessible workspace root.
    private func checkIdeaWorkspaceReadiness(idea: Idea?) -> PreflightCheck {
        guard let idea else {
            return PreflightCheck(
                category: "Workspace",
                title: "Idea Workspace Root",
                status: .fail,
                message: "Workflow requires project access but no idea was provided to preflight"
            )
        }

        guard let workspaceRootPath = idea.workspaceRootPath,
              !workspaceRootPath.trimmingCharacters(in: .whitespaces).isEmpty else {
            return PreflightCheck(
                category: "Workspace",
                title: "Idea Workspace Root",
                status: .fail,
                message: "Workflow requires project access but idea has no workspace root path set"
            )
        }

        var isDirectory: ObjCBool = false
        let exists = FileManager.default.fileExists(atPath: workspaceRootPath, isDirectory: &isDirectory)
        guard exists, isDirectory.boolValue else {
            return PreflightCheck(
                category: "Workspace",
                title: "Idea Workspace Root",
                status: .fail,
                message: "Workspace root path is not a valid accessible directory: \(workspaceRootPath)"
            )
        }

        return PreflightCheck(
            category: "Workspace",
            title: "Idea Workspace Root",
            status: .pass,
            message: workspaceRootPath
        )
    }

    private func supportBundleExportURL(for configuration: AppConfiguration) -> URL {
        if let supportBundleExportPath = configuration.supportBundleExportPath, !supportBundleExportPath.isEmpty {
            return URL(fileURLWithPath: supportBundleExportPath, isDirectory: true)
        }
        return AppConfiguration.defaultSupportRoot().appendingPathComponent("exports", isDirectory: true)
    }

    private func appendProviderChecks(
        for provider: ConfiguredProvider,
        bindings: [String: ResolvedProviderBinding],
        checks: inout [PreflightCheck],
        warnings: inout [String],
        blockingIssues: inout [String]
    ) async {
        let snapshot = providerRegistry.healthSnapshot(for: provider.id)
        if provider.transport == .gooseServer {
            let reachabilityIssue = snapshot.flatMap { ProviderAdapterSupport.gooseServerReachabilityIssue(from: $0.blockingIssues) }
            let title = "\(provider.displayName) Reachability"
            if let reachabilityIssue {
                checks.append(PreflightCheck(
                    category: "Providers",
                    title: title,
                    status: .fail,
                    message: reachabilityIssue
                ))
                blockingIssues.append(reachabilityIssue)
            } else if let endpoint = provider.endpoint?.trimmingCharacters(in: .whitespacesAndNewlines), !endpoint.isEmpty {
                checks.append(PreflightCheck(
                    category: "Providers",
                    title: title,
                    status: snapshot == nil ? .warn : .pass,
                    message: snapshot == nil
                        ? "Reachability has not been checked yet"
                        : "Goose server is reachable at \(ProviderAdapterSupport.gooseStatusURLString(for: endpoint))"
                ))
            }
        }
        let healthStatus = mapProviderStatus(snapshot)
        let healthMessage = snapshot?.summary ?? "Health not yet verified"
        checks.append(PreflightCheck(
            category: "Providers",
            title: "\(provider.displayName) Health",
            status: healthStatus,
            message: healthMessage
        ))
        if healthStatus == .warn {
            warnings.append(healthMessage)
        } else if healthStatus == .fail {
            blockingIssues.append(contentsOf: snapshot?.blockingIssues ?? [healthMessage])
        }

        let boundModels = bindings.values
            .filter { $0.configuredProviderID == provider.id }
            .map(\.model)
        guard !boundModels.isEmpty else { return }

        let availableModels = await providerRegistry.availableModels(for: provider)
        for model in boundModels.sorted() {
            let isAvailable = availableModels.isEmpty || availableModels.contains(model)
            let message = isAvailable
                ? "Model \(model) is available for \(provider.displayName)"
                : "Model \(model) is not available for \(provider.displayName)"
            let status: PreflightCheckStatus = isAvailable ? .pass : .fail
            checks.append(PreflightCheck(
                category: "Providers",
                title: "\(provider.displayName) Model",
                status: status,
                message: message
            ))
            if status == .fail {
                blockingIssues.append(message)
            }
        }
    }

    private func mapProviderStatus(_ snapshot: ProviderHealthSnapshot?) -> PreflightCheckStatus {
        guard let snapshot else { return .warn }
        if snapshot.status == .unavailable { return .fail }
        if !snapshot.blockingIssues.isEmpty { return .fail }
        switch snapshot.status {
        case .healthy:
            return .pass
        case .unknown, .degraded:
            return .warn
        case .unavailable:
            return .fail
        }
    }
}
