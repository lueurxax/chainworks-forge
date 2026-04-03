import Foundation

enum SkillInjector {
    static func injectedContent(
        skillID: String,
        type: SkillType,
        content: String
    ) -> String {
        """
        ## Skill: \(skillID)
        Type: \(type.rawValue)

        \(content)
        """
    }
}
