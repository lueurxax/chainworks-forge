import Foundation

enum SkillType: String, Codable, Sendable {
    case external
    case inline
    case builtin

    init?(catalogType: String) {
        switch catalogType {
        case "external_skill":
            self = .external
        case "inline_skill":
            self = .inline
        case "builtin_agent":
            self = .builtin
        default:
            return nil
        }
    }

    var catalogType: String {
        switch self {
        case .external:
            return "external_skill"
        case .inline:
            return "inline_skill"
        case .builtin:
            return "builtin_agent"
        }
    }
}

enum SkillInjectionPolicy: String, Codable, Sendable {
    case prependToSystemPrompt = "prepend_to_system_prompt"
}

struct SkillBundleManifest: Codable, Sendable, Hashable {
    let references: [String]
    let assets: [String]
    let evals: [String]
    let agents: [String]

    var hasCompanions: Bool {
        !references.isEmpty || !assets.isEmpty || !evals.isEmpty || !agents.isEmpty
    }
}

struct ResolvedSkill: Codable, Sendable, Hashable {
    let id: String
    let type: SkillType
    let resolvedContent: String
    let contentHash: String
    let injectedContent: String
    let injectedContentHash: String
    let sourcePath: String?
    let sourceDescription: String?
    let bundleManifest: SkillBundleManifest?
    let role: String?
    let specializationSummary: String?
    let injectionPolicy: SkillInjectionPolicy

    var contentSummary: String {
        let normalized = resolvedContent
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\n\n", with: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard normalized.count > 200 else { return normalized }
        return String(normalized.prefix(200)) + "..."
    }
}

enum SkillResolutionError: LocalizedError, Equatable {
    case skillNotFound(String)
    case unsupportedSkillType(String, skillID: String)
    case emptyInlineDescription(skillID: String)
    case externalPathMissing(skillID: String)
    case skillBundleNotFound(path: String, skillID: String)
    case skillEntryPointMissing(path: String, skillID: String)
    case emptyExternalSkill(path: String, skillID: String)
    case unknownBuiltin(name: String, skillID: String)
    case unresolvedEnvironmentVariable(String, path: String, skillID: String)
    case missingRequiredSpecialization(skillID: String, role: String)

    var errorDescription: String? {
        switch self {
        case .skillNotFound(let skillID):
            return "Skill '\(skillID)' not found in catalog"
        case .unsupportedSkillType(let type, let skillID):
            return "Skill '\(skillID)' uses unsupported type '\(type)'"
        case .emptyInlineDescription(let skillID):
            return "Inline skill '\(skillID)' has an empty description"
        case .externalPathMissing(let skillID):
            return "External skill '\(skillID)' is missing a path"
        case .skillBundleNotFound(let path, let skillID):
            return "External skill '\(skillID)' bundle not found at \(path)"
        case .skillEntryPointMissing(let path, let skillID):
            return "External skill '\(skillID)' is missing SKILL.md under \(path)"
        case .emptyExternalSkill(let path, let skillID):
            return "External skill '\(skillID)' has empty SKILL.md content at \(path)"
        case .unknownBuiltin(let name, let skillID):
            return "Builtin skill '\(skillID)' references unknown builtin '\(name)'"
        case .unresolvedEnvironmentVariable(let variable, let path, let skillID):
            return "External skill '\(skillID)' path '\(path)' depends on unset environment variable '\(variable)'"
        case .missingRequiredSpecialization(let skillID, let role):
            return "Skill '\(skillID)' requires a specialization mapping for role '\(role)'"
        }
    }
}
