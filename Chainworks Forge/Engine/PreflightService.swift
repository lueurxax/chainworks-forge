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
        idea: Idea? = nil,
        effectiveProjectRootPath: String? = nil
    ) async -> PreflightReport {
        var checks: [PreflightCheck] = []
        var warnings: [String] = []
        var blockingIssues: [String] = []
        var loadedCatalog: AgentCatalog?

        let config = appConfigurationStore.configuration
        checks.append(checkFile(category: "Catalog", title: "Workflow YAML", url: workflowURL))
        checks.append(checkFile(category: "Catalog", title: "Agent Catalog", url: catalogURL))

        do {
            let workflow = try YAMLParser.loadWorkflow(from: workflowURL)
            let catalog = try YAMLParser.loadAgentCatalog(from: catalogURL)
            loadedCatalog = catalog
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
            checks.append(checkIdeaWorkspaceReadiness(
                idea: idea,
                effectiveProjectRootPath: effectiveProjectRootPath
            ))
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
                providerBindings = try resolver.resolveBindings(plan: plan, startOptions: startOptions, runtimeProfiles: loadedCatalog?.runtimeProfiles ?? [:])
                checks.append(PreflightCheck(
                    category: "Providers",
                    title: "Provider Binding Resolution",
                    status: .pass,
                    message: "Resolved \(providerBindings.count) provider binding(s)"
                ))
            } catch let resolverError as BackendProfileResolverError {
                providerBindings = [:]
                if case .providerNotEnabled(let family) = resolverError {
                    let message = "Provider family \(family.displayName) is configured but not enabled. Enable it in Settings to use this runtime profile."
                    checks.append(PreflightCheck(
                        category: "Rollout",
                        title: "Provider Not Enabled — \(family.displayName)",
                        status: .fail,
                        message: message
                    ))
                    blockingIssues.append(message)
                } else {
                    let message = resolverError.localizedDescription
                    checks.append(PreflightCheck(
                        category: "Providers",
                        title: "Provider Binding Resolution",
                        status: .fail,
                        message: message
                    ))
                    blockingIssues.append(message)
                }
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

        // Proposal 029: Validate RuntimeProfile.requires against ProviderCapabilities
        if let plan, let loadedCatalog {
            appendCapabilityChecks(
                plan: plan,
                catalog: loadedCatalog,
                bindings: providerBindings,
                checks: &checks,
                warnings: &warnings,
                blockingIssues: &blockingIssues
            )
        }

        // Proposal 029: Validate adapter family registration (fail-closed)
        for (agentID, binding) in providerBindings {
            let family = binding.adapterFamily ?? "goose"
            let knownFamilies: Set<String> = ["goose", "claude_agent_acp", "gemini_cli_acp", "codex_acp", "auggie_cli_acp", "junie_cli_acp"]
            if !knownFamilies.contains(family) {
                let msg = "Agent '\(agentID)' uses unregistered adapter family '\(family)'. Register the adapter before adding its runtime profile."
                checks.append(PreflightCheck(category: "Runtime", title: "Adapter Registration", status: .fail, message: msg))
                blockingIssues.append(msg)
            }
        }

        if let plan, let loadedCatalog {
            appendMCPChecks(
                plan: plan,
                catalog: loadedCatalog,
                bindings: providerBindings,
                checks: &checks,
                warnings: &warnings,
                blockingIssues: &blockingIssues
            )
        }

        if let loadedCatalog {
            appendSkillChecks(
                catalog: loadedCatalog,
                catalogURL: catalogURL,
                checks: &checks,
                warnings: &warnings,
                blockingIssues: &blockingIssues
            )
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

    private func appendSkillChecks(
        catalog: AgentCatalog,
        catalogURL: URL,
        checks: inout [PreflightCheck],
        warnings: inout [String],
        blockingIssues: inout [String]
    ) {
        let resolverContext = SkillResolverContext(catalogBaseURL: catalogURL)
        let uniqueAgentSkills = Dictionary(uniqueKeysWithValues: catalog.agents.map { ($0.id, $0) })
        var successfulResolutions = 0

        for (_, agent) in uniqueAgentSkills.sorted(by: { $0.key < $1.key }) {
            guard let skillRef = catalog.skills[agent.skillRef] else {
                let message = "Agent '\(agent.id)' references non-existent skill '\(agent.skillRef)'."
                checks.append(PreflightCheck(
                    category: "Skills",
                    title: "Skill resolution: \(agent.id)",
                    status: .fail,
                    message: message
                ))
                blockingIssues.append(message)
                continue
            }

            do {
                let resolved = try SkillResolver.resolve(
                    skillID: agent.skillRef,
                    skillRef: skillRef,
                    skillRole: agent.skillRole,
                    context: resolverContext
                )
                successfulResolutions += 1

                let sourceLabel = resolved.sourcePath ?? resolved.sourceDescription ?? resolved.type.catalogType
                checks.append(PreflightCheck(
                    category: "Skills",
                    title: "Skill resolution: \(agent.id)",
                    status: .pass,
                    message: "Resolved \(resolved.type.catalogType) '\(agent.skillRef)' from \(sourceLabel)"
                ))
            } catch {
                let message = "Agent '\(agent.id)' skill '\(agent.skillRef)' failed resolution: \(error.localizedDescription)"
                checks.append(PreflightCheck(
                    category: "Skills",
                    title: "Skill resolution: \(agent.id)",
                    status: .fail,
                    message: message
                ))
                blockingIssues.append(message)
            }
        }

        checks.append(PreflightCheck(
            category: "Skills",
            title: "Skill summary",
            status: blockingIssues.contains(where: { $0.contains("skill '") || $0.contains("references non-existent skill") }) ? .fail : .pass,
            message: "Resolved \(successfulResolutions) skill binding(s) across \(uniqueAgentSkills.count) agent(s)"
        ))
    }

    func runReport(
        workflowURL: URL,
        catalogURL: URL,
        plan: RunPlan?,
        idea: Idea? = nil,
        effectiveProjectRootPath: String? = nil
    ) async -> PreflightReport {
        await runReport(
            workflowURL: workflowURL,
            catalogURL: catalogURL,
            plan: plan,
            startOptions: RunStartOptions(),
            idea: idea,
            effectiveProjectRootPath: effectiveProjectRootPath
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
    private func checkIdeaWorkspaceReadiness(
        idea: Idea?,
        effectiveProjectRootPath: String?
    ) -> PreflightCheck {
        let workspaceRootPath = ProjectRootPolicy.effectiveProjectRoot(
            workspaceRootPath: idea?.workspaceRootPath,
            deliveryRepoRootPath: effectiveProjectRootPath
        )

        guard let workspaceRootPath else {
            if idea == nil {
                return PreflightCheck(
                    category: "Workspace",
                    title: "Project Root",
                    status: .fail,
                    message: "Workflow requires project access but no idea was provided to preflight"
                )
            }

            return PreflightCheck(
                category: "Workspace",
                title: "Project Root",
                status: .fail,
                message: "Workflow requires project access but no effective project root is configured"
            )
        }

        let status = SecurityScopedAccess.itemStatus(atPath: workspaceRootPath)
        guard status.exists, status.isDirectory else {
            return PreflightCheck(
                category: "Workspace",
                title: "Project Root",
                status: .fail,
                message: "Project root path is not a valid accessible directory: \(workspaceRootPath)"
            )
        }

        return PreflightCheck(
            category: "Workspace",
            title: "Project Root",
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
        let effectiveBindings = bindings.values
            .filter { $0.configuredProviderID == provider.id }
        let usesConfiguredTransport = effectiveBindings.contains { $0.transport == provider.transport.rawValue }
        let runtimeSummary = Array(Set(
            effectiveBindings.map { binding in
                binding.runtimeProfileID ?? binding.adapterFamily ?? binding.transport
            }
        )).sorted().joined(separator: ", ")

        let snapshot = providerRegistry.healthSnapshot(for: provider.id)
        if provider.transport == .gooseServer {
            let title = "\(provider.displayName) Reachability"
            if usesConfiguredTransport {
                let reachabilityIssue = snapshot.flatMap { ProviderAdapterSupport.gooseServerReachabilityIssue(from: $0.blockingIssues) }
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
            } else if !effectiveBindings.isEmpty {
                checks.append(PreflightCheck(
                    category: "Providers",
                    title: title,
                    status: .pass,
                    message: "Effective bindings use runtime-managed transport (\(runtimeSummary)); configured Goose reachability is not on the execution path."
                ))
            }
        }
        let healthStatus: PreflightCheckStatus
        let healthMessage: String
        if usesConfiguredTransport || effectiveBindings.isEmpty {
            healthStatus = mapProviderStatus(snapshot)
            healthMessage = snapshot?.summary ?? "Health not yet verified"
        } else {
            healthStatus = .pass
            healthMessage = "Effective bindings use runtime-managed transport (\(runtimeSummary)); configured provider transport health is informational only."
        }
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
        let normalizedAvailableModels = Set(
            availableModels.map { model in
                model.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            }
        )
        for model in boundModels.sorted() {
            if !usesConfiguredTransport && !effectiveBindings.isEmpty {
                checks.append(PreflightCheck(
                    category: "Providers",
                    title: "\(provider.displayName) Model",
                    status: .pass,
                    message: "Model \(model) is bound through runtime-managed transport (\(runtimeSummary)); configured provider inventory is not authoritative on this path."
                ))
                continue
            }
            let normalizedModel = model.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            let isAvailable = normalizedAvailableModels.isEmpty || normalizedAvailableModels.contains(normalizedModel)
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

    // MARK: - Capability Enforcement (Proposal 029)

    private func appendCapabilityChecks(
        plan: RunPlan,
        catalog: AgentCatalog,
        bindings: [String: ResolvedProviderBinding],
        checks: inout [PreflightCheck],
        warnings: inout [String],
        blockingIssues: inout [String]
    ) {
        let runtimeProfiles = catalog.runtimeProfiles ?? [:]
        for (agentID, binding) in bindings {
            guard let profileID = binding.runtimeProfileID,
                  let profile = runtimeProfiles[profileID] else { continue }

            let provider = providerRegistry.configuredProviders.first { $0.id == binding.configuredProviderID }
            let capabilities = provider?.capabilities ?? ProviderCapabilities.default(for: provider?.family ?? .codex)

            let unsatisfied = profile.requires.filter { !capabilities.satisfies($0) }
            if unsatisfied.isEmpty {
                checks.append(PreflightCheck(
                    category: "Runtime",
                    title: "Capability Check: \(agentID)",
                    status: .pass,
                    message: "Runtime profile '\(profileID)' requirements satisfied"
                ))
            } else {
                let msg = "Agent '\(agentID)' runtime profile '\(profileID)' requires [\(unsatisfied.joined(separator: ", "))] but provider does not support them"
                checks.append(PreflightCheck(
                    category: "Runtime",
                    title: "Capability Check: \(agentID)",
                    status: .fail,
                    message: msg
                ))
                blockingIssues.append(msg)
            }
        }
    }

    private func appendMCPChecks(
        plan: RunPlan,
        catalog: AgentCatalog,
        bindings: [String: ResolvedProviderBinding],
        checks: inout [PreflightCheck],
        warnings: inout [String],
        blockingIssues: inout [String]
    ) {
        let gooseRegistry = try? GooseExtensionRegistryReader().snapshot()
        let resolver = MCPPolicyResolver()
        let activeAgents = plan.agentBindings.values.sorted { $0.id < $1.id }

        let anyRequestedMCP = activeAgents.contains { agent in
            let resolution = resolver.resolve(
                agent: agent,
                catalog: catalog,
                providerBinding: bindings[agent.id],
                gooseRegistry: gooseRegistry
            )
            return !resolution.requestedExtensions.isEmpty
        }

        let registryStatus: PreflightCheckStatus
        let registryMessage: String
        if let gooseRegistry {
            registryStatus = .pass
            registryMessage = "Loaded Goose extension registry from \(gooseRegistry.configURL.path) (\(gooseRegistry.installedExtensionIDs.count) installed)"
        } else if anyRequestedMCP {
            registryStatus = .fail
            registryMessage = "Goose extension registry is unavailable, but one or more agents request MCP extensions."
            blockingIssues.append(registryMessage)
        } else {
            registryStatus = .warn
            registryMessage = "Goose extension registry is unavailable; zero-MCP sessions remain valid."
            warnings.append(registryMessage)
        }

        checks.append(PreflightCheck(
            category: "MCP",
            title: "Goose Extension Registry",
            status: registryStatus,
            message: registryMessage
        ))

        for agent in activeAgents {
            let resolution = resolver.resolve(
                agent: agent,
                catalog: catalog,
                providerBinding: bindings[agent.id],
                gooseRegistry: gooseRegistry
            )
            let summary = [
                "profile=\(resolution.profileID)",
                "requested=\(resolution.requestedExtensions.joined(separator: ","))",
                "effective=\(resolution.predictedEffectiveExtensions.joined(separator: ","))",
                "denied=\(resolution.deniedExtensions.joined(separator: ","))"
            ].joined(separator: " • ")

            let status: PreflightCheckStatus
            let message: String
            if !resolution.blockingIssues.isEmpty {
                status = .fail
                message = resolution.blockingIssues.joined(separator: "; ")
                blockingIssues.append(contentsOf: resolution.blockingIssues)
            } else if !resolution.warnings.isEmpty {
                status = .warn
                message = "\(summary); \(resolution.warnings.joined(separator: "; "))"
                warnings.append(contentsOf: resolution.warnings)
            } else {
                status = .pass
                message = summary
            }

            checks.append(PreflightCheck(
                category: "MCP",
                title: "MCP Profile — \(agent.id)",
                status: status,
                message: message
            ))
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
