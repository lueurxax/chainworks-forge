import Foundation
@testable import Chainworks_Forge

/// A mock AgentExecutor for testing purposes.
final class SimulatedAgentExecutor: AgentExecutor, @unchecked Sendable {
    struct ExecutionRecord: Sendable {
        let task: String
        let agentID: String
        let stageID: String
    }

    private var _executedTasks: [ExecutionRecord] = []
    private var _failingAgentIDs: Set<String> = []
    private var _customOutputs: [String: [String: Data]] = [:] // agentID -> [outputName -> Data]
    let simulatedDelay: Double

    private let lock = NSLock()

    init(simulatedDelay: Double = 0, catalog: AgentCatalog? = nil) {
        self.simulatedDelay = simulatedDelay
    }

    var executedTasks: [ExecutionRecord] {
        lock.lock(); defer { lock.unlock() }
        return _executedTasks
    }

    var failingAgentIDs: Set<String> {
        get { lock.lock(); defer { lock.unlock() }; return _failingAgentIDs }
        set { lock.lock(); defer { lock.unlock() }; _failingAgentIDs = newValue }
    }

    var customOutputs: [String: [String: Data]] {
        get { lock.lock(); defer { lock.unlock() }; return _customOutputs }
        set { lock.lock(); defer { lock.unlock() }; _customOutputs = newValue }
    }

    func reset() {
        lock.lock(); defer { lock.unlock() }
        _executedTasks = []
        _failingAgentIDs = []
        _customOutputs = [:]
    }

    func execute(
        task: AgentTask,
        agent: ResolvedAgent,
        context: ExecutionContext
    ) async throws -> AgentResult {
        lock.lock()
        let record = ExecutionRecord(task: task.task, agentID: agent.id, stageID: context.stageID)
        _executedTasks.append(record)
        let isFailing = _failingAgentIDs.contains(agent.id)
        let agentOutputs = _customOutputs[agent.id] ?? [:]
        lock.unlock()

        if isFailing {
            return AgentResult(
                outputs: [:],
                logSnippet: "Simulated failure for \(agent.id)",
                costCents: 100,
                succeeded: false,
                errorMessage: "Simulated failure",
                sessionID: "sim-session-\(UUID().uuidString.prefix(4))",
                durationSeconds: 0.1,
                providerReceipt: nil,
                resolvedModel: agent.model,
                configuredProviderID: nil,
                adapterVersion: "sim-1.0",
                outputPresence: .none
            )
        }

        // Produce expected outputs from the task/agent definition if not in customOutputs
        var finalOutputs = agentOutputs
        for outputName in agent.outputs {
            if finalOutputs[outputName] == nil {
                let (data, _) = OutputContractTemplates.generate(
                    contractID: agent.outputContract ?? "default",
                    agentID: agent.id,
                    stageID: context.stageID
                )
                finalOutputs[outputName] = data
            }
        }

        return AgentResult(
            outputs: finalOutputs,
            logSnippet: "Simulated success for \(agent.id)",
            costCents: 100,
            succeeded: true,
            errorMessage: nil,
            sessionID: "sim-session-\(UUID().uuidString.prefix(4))",
            durationSeconds: 0.1,
            providerReceipt: nil,
            resolvedModel: agent.model,
            configuredProviderID: nil,
            adapterVersion: "sim-1.0",
            outputPresence: .durableOutput
        )
    }
}

/// Helpers for generating simulated artifact content matching canonical schemas.
struct OutputContractTemplates {
    static func generate(
        contractID: String,
        agentID: String,
        stageID: String
    ) -> (data: Data, format: ArtifactFormat) {
        switch contractID {
        case "proposal_review_v1", "prepush_review_v1":
            let json = ["status": "accepted", "findings": []] as [String : Any]
            return (try! JSONSerialization.data(withJSONObject: json), .json)
        case "proposal_review_summary_v2", "implementation_review_summary_v1":
            let json = ["summary": "Everything looks good", "status": "approved"] as [String : Any]
            return (try! JSONSerialization.data(withJSONObject: json), .json)
        case "implementation_self_assessment_v1":
            let json = [
                "implementation_complete": true,
                "verification_green": true,
                "remaining_code_tasks": [],
                "handoff_tasks": [],
                "known_risks": [],
                "tests_run": [],
                "docs_impacted": []
            ] as [String : Any]
            return (try! JSONSerialization.data(withJSONObject: json), .json)
        case "audit_report_v1":
            let json = ["verdict": "Ready", "requirements": []] as [String : Any]
            return (try! JSONSerialization.data(withJSONObject: json), .json)
        case "security_report_v1":
            let json = ["severity": "low", "issues": []] as [String : Any]
            return (try! JSONSerialization.data(withJSONObject: json), .json)
        case "docs_report_v1":
            let json = ["status": "complete", "coverage": 1.0] as [String : Any]
            return (try! JSONSerialization.data(withJSONObject: json), .json)
        case "git_push_receipt_v1":
            let json = ["commit": "abc", "branch": "main"] as [String : Any]
            return (try! JSONSerialization.data(withJSONObject: json), .json)
        case "connect_upload_receipt_v1":
            let json = ["bundle_id": "com.test", "version": "1.0"] as [String : Any]
            return (try! JSONSerialization.data(withJSONObject: json), .json)
        default:
            return ("Simulated Output for \(contractID)".data(using: .utf8)!, .markdown)
        }
    }
}
