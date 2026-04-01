import Foundation

struct InvocationOwnerKeyBuilder {
    static func build(
        runID: UUID,
        agentID: String,
        stageLineageID: String,
        taskName: String,
        ownerExecutionLineageID: UUID // From AgentExecution.id or similar?
    ) -> String {
        let components = [
            runID.uuidString,
            agentID,
            stageLineageID,
            taskName,
            ownerExecutionLineageID.uuidString
        ]
        return components.joined(separator: ":")
    }
}
