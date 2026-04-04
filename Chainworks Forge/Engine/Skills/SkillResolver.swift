import Foundation

struct SkillResolverContext: Sendable {
    let catalogBaseURL: URL?
    let currentDirectoryPath: String
    let environment: [String: String]

    init(
        catalogBaseURL: URL? = nil,
        currentDirectoryPath: String = FileManager.default.currentDirectoryPath,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) {
        self.catalogBaseURL = catalogBaseURL
        self.currentDirectoryPath = currentDirectoryPath
        self.environment = environment
    }

    func resolveSkillPath(_ rawPath: String, skillID: String) throws -> URL {
        let expanded = try expandEnvironmentPlaceholders(in: rawPath, skillID: skillID)
        let tildeExpanded = NSString(string: expanded).expandingTildeInPath
        if tildeExpanded.hasPrefix("/") {
            return URL(fileURLWithPath: tildeExpanded, isDirectory: true).standardizedFileURL
        }

        if let catalogBaseURL {
            return catalogBaseURL
                .deletingLastPathComponent()
                .appendingPathComponent(tildeExpanded, isDirectory: true)
                .standardizedFileURL
        }

        return URL(fileURLWithPath: currentDirectoryPath, isDirectory: true)
            .appendingPathComponent(tildeExpanded, isDirectory: true)
            .standardizedFileURL
    }

    private func expandEnvironmentPlaceholders(in rawPath: String, skillID: String) throws -> String {
        let pattern = #/\$\{([^}:]+)(?:(:-)([^}]*))?\}/#
        var result = rawPath

        for match in rawPath.matches(of: pattern).reversed() {
            let variable = String(match.1)
            let defaultValue = match.3.map(String.init)
            let replacement: String
            if let value = environment[variable], !value.isEmpty {
                replacement = value
            } else if let defaultValue {
                replacement = defaultValue
            } else {
                throw SkillResolutionError.unresolvedEnvironmentVariable(variable, path: rawPath, skillID: skillID)
            }
            result.replaceSubrange(match.range, with: replacement)
        }

        return result
    }
}

enum SkillResolver {
    static func resolve(
        skillID: String,
        skillRef: SkillRef,
        skillRole: String?,
        context: SkillResolverContext
    ) throws -> ResolvedSkill {
        guard let type = SkillType(catalogType: skillRef.type) else {
            throw SkillResolutionError.unsupportedSkillType(skillRef.type, skillID: skillID)
        }

        let baseContent: String
        let sourcePath: String?
        let sourceDescription: String?
        let bundleManifest: SkillBundleManifest?
        let bundleRoot: URL?

        switch type {
        case .external:
            guard let rawPath = skillRef.path?.trimmingCharacters(in: .whitespacesAndNewlines), !rawPath.isEmpty else {
                throw SkillResolutionError.externalPathMissing(skillID: skillID)
            }
            let loaded = try ExternalSkillLoader().loadBundle(skillID: skillID, from: rawPath, context: context)
            baseContent = loaded.content
            sourcePath = loaded.rootURL.path
            sourceDescription = nil
            bundleManifest = loaded.manifest
            bundleRoot = loaded.rootURL

        case .inline:
            guard let description = skillRef.description?.trimmingCharacters(in: .whitespacesAndNewlines), !description.isEmpty else {
                throw SkillResolutionError.emptyInlineDescription(skillID: skillID)
            }
            baseContent = description
            sourcePath = nil
            sourceDescription = description
            bundleManifest = nil
            bundleRoot = nil

        case .builtin:
            let builtinName = skillRef.name?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            baseContent = try BuiltinSkillRegistry.instructionSet(for: builtinName, skillID: skillID)
            sourcePath = nil
            sourceDescription = builtinName
            bundleManifest = nil
            bundleRoot = nil
        }

        let specialized = try SkillRoleCustomizer.specialization(
            skillID: skillID,
            role: skillRole,
            baseContent: baseContent,
            bundleRoot: bundleRoot
        )
        let injectedContent = SkillInjector.injectedContent(
            skillID: skillID,
            type: type,
            content: specialized.content
        )

        return ResolvedSkill(
            id: skillID,
            type: type,
            resolvedContent: baseContent,
            contentHash: DefinitionHasher.hashString(baseContent),
            injectedContent: injectedContent,
            injectedContentHash: DefinitionHasher.hashString(injectedContent),
            sourcePath: sourcePath,
            sourceDescription: sourceDescription,
            bundleManifest: bundleManifest,
            role: skillRole,
            specializationSummary: specialized.summary,
            injectionPolicy: .prependToSystemPrompt
        )
    }
}
