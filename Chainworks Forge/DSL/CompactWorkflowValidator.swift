import Foundation

nonisolated struct CompactWorkflowValidator: Sendable {

    nonisolated static func validate(_ compact: CompactWorkflowDefinition) -> [ValidationIssue] {
        var issues: [ValidationIssue] = []
        let stages = compact.workflow.stages
        let stageIDs = stages.map(\.id)
        let stageIDSet = Set(stageIDs)

        // Unique stage IDs
        let duplicates = Dictionary(grouping: stageIDs, by: { $0 }).filter { $0.value.count > 1 }
        for dup in duplicates {
            issues.append(ValidationIssue(severity: .error, message: "Duplicate stage ID '\(dup.key)'", location: "stages"))
        }

        // Needs reference existing stages
        for stage in stages {
            for need in stage.needs ?? [] {
                if !stageIDSet.contains(need) {
                    issues.append(ValidationIssue(severity: .error, message: "Stage '\(stage.id)' needs '\(need)' which doesn't exist", location: "stages.\(stage.id).needs"))
                }
            }
        }

        // Fanout has non-empty agents
        for stage in stages where stage.type == "fanout" {
            if stage.agents == nil || stage.agents!.isEmpty {
                issues.append(ValidationIssue(severity: .error, message: "Fanout stage '\(stage.id)' has no agents", location: "stages.\(stage.id).agents"))
            }
        }

        // Approval stages have approval: required
        for stage in stages where stage.type == "approval" {
            if stage.approval != "required" {
                issues.append(ValidationIssue(severity: .warning, message: "Approval stage '\(stage.id)' does not have approval: required", location: "stages.\(stage.id).approval"))
            }
        }

        // Entry point exists
        let hasEntryPoint = stages.contains(where: { $0.needs == nil || $0.needs!.isEmpty })
        if !hasEntryPoint {
            issues.append(ValidationIssue(severity: .error, message: "No entry point: all stages have needs", location: "stages"))
        }

        // Cycle detection
        issues += detectCycles(stages: stages)

        return issues
    }

    private nonisolated static func detectCycles(stages: [CompactStage]) -> [ValidationIssue] {
        var visited = Set<String>()
        var inStack = Set<String>()
        let stageMap = Dictionary(stages.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        var issues: [ValidationIssue] = []

        func dfs(_ id: String) {
            guard !visited.contains(id) else { return }
            if inStack.contains(id) {
                issues.append(ValidationIssue(severity: .error, message: "Circular needs chain detected involving '\(id)'", location: "stages.\(id)"))
                return
            }
            inStack.insert(id)
            if let stage = stageMap[id] {
                for need in stage.needs ?? [] {
                    dfs(need)
                }
            }
            inStack.remove(id)
            visited.insert(id)
        }

        for stage in stages {
            dfs(stage.id)
        }
        return issues
    }
}
