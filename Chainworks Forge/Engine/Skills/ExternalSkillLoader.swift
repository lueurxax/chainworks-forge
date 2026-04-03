import Foundation

struct ExternalSkillLoader: Sendable {
    func loadBundle(
        skillID: String,
        from rawPath: String,
        context: SkillResolverContext
    ) throws -> LoadedExternalSkill {
        let resolvedPath = try context.resolveSkillPath(rawPath, skillID: skillID)
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: resolvedPath.path, isDirectory: &isDirectory), isDirectory.boolValue else {
            throw SkillResolutionError.skillBundleNotFound(path: resolvedPath.path, skillID: skillID)
        }

        let entrypoint = resolvedPath.appendingPathComponent("SKILL.md", isDirectory: false)
        guard FileManager.default.fileExists(atPath: entrypoint.path) else {
            throw SkillResolutionError.skillEntryPointMissing(path: resolvedPath.path, skillID: skillID)
        }

        let content = try String(contentsOf: entrypoint, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !content.isEmpty else {
            throw SkillResolutionError.emptyExternalSkill(path: entrypoint.path, skillID: skillID)
        }

        return LoadedExternalSkill(
            rootURL: resolvedPath,
            content: content,
            manifest: SkillBundleManifest(
                references: bundleFiles(in: resolvedPath.appendingPathComponent("references", isDirectory: true)),
                assets: bundleFiles(in: resolvedPath.appendingPathComponent("assets", isDirectory: true)),
                evals: bundleFiles(in: resolvedPath.appendingPathComponent("evals", isDirectory: true)),
                agents: bundleFiles(in: resolvedPath.appendingPathComponent("agents", isDirectory: true))
            )
        )
    }

    private func bundleFiles(in directory: URL) -> [String] {
        guard FileManager.default.fileExists(atPath: directory.path) else { return [] }
        let enumerator = FileManager.default.enumerator(
            at: directory,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        )

        var files: [String] = []
        while let url = enumerator?.nextObject() as? URL {
            let values = try? url.resourceValues(forKeys: [.isDirectoryKey])
            guard values?.isDirectory != true else { continue }
            files.append(url.lastPathComponent)
        }
        return files.sorted()
    }
}

struct LoadedExternalSkill: Sendable {
    let rootURL: URL
    let content: String
    let manifest: SkillBundleManifest
}
