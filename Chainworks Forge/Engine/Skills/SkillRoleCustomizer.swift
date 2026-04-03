import Foundation

enum SkillRoleCustomizer {
    static func specialization(
        skillID: String,
        role: String?,
        baseContent: String,
        bundleRoot: URL?
    ) throws -> (content: String, summary: String?) {
        guard let role, !role.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return (baseContent, nil)
        }

        if skillID == "proposal_review_triad" {
            guard let mapped = triadModeMap[role] else {
                throw SkillResolutionError.missingRequiredSpecialization(skillID: skillID, role: role)
            }
            let specialized = """
            \(baseContent)

            ## Active Role: \(role)

            Mode: \(mapped.mode)

            \(mapped.instructions)
            """
            return (specialized, "mode \(mapped.mode)")
        }

        if let bundleRoot {
            let roleFile = bundleRoot.appendingPathComponent("roles/\(role).md", isDirectory: false)
            if FileManager.default.fileExists(atPath: roleFile.path),
               let roleContent = try? String(contentsOf: roleFile, encoding: .utf8)
                .trimmingCharacters(in: .whitespacesAndNewlines),
               !roleContent.isEmpty {
                let specialized = """
                \(baseContent)

                ## Active Role: \(role)

                \(roleContent)
                """
                return (specialized, "bundle role file \(role).md")
            }
        }

        let generic = """
        \(baseContent)

        ## Active Role: \(role)

        You are operating in the "\(role)" role for this skill.
        Apply all skill instructions through the lens of this role.
        """
        return (generic, "generic role block")
    }

    private static let triadModeMap: [String: (mode: String, instructions: String)] = [
        "product_owner": (
            "product-only",
            "As the product owner lens, focus on business value, user problem clarity, scope discipline, acceptance criteria, rollout risk, metrics, and dependency realism."
        ),
        "ux_designer": (
            "ux-only",
            "As the UX lens, focus on user journeys, task flow, friction, clarity, accessibility, empty states, and recovery behavior."
        ),
        "ui_designer": (
            "ui-only",
            "As the UI lens, focus on visual hierarchy, component consistency, state treatment, readability, affordance clarity, and interaction polish."
        ),
        "architect": (
            "architecture-only",
            "As the architecture lens, focus on system boundaries, ownership, persistence truth, invariants, failure handling, and implementation feasibility."
        )
    ]
}
