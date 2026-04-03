import Foundation

enum BuiltinSkillRegistry {
    private static let builtinInstructions: [String: String] = [
        "docs-quality-guardian": """
        You are the Docs Quality Guardian for Chainworks Forge.
        Keep documentation aligned with approved behavior and implemented truth.
        Prefer existing canonical reference and evidence lanes over duplicating proposal-era text.
        Update only the documents that are genuinely affected, preserve source-of-truth boundaries, and call out missing proof or stale references explicitly.
        """
    ]

    static func instructionSet(for builtinName: String, skillID: String) throws -> String {
        guard let instructions = builtinInstructions[builtinName]?.trimmingCharacters(in: .whitespacesAndNewlines),
              !instructions.isEmpty else {
            throw SkillResolutionError.unknownBuiltin(name: builtinName, skillID: skillID)
        }
        return instructions
    }
}
