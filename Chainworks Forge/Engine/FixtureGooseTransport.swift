import Foundation

/// Deterministic Goose transport used for Proposal 004 integration/UI proof without a real backend.
final class FixtureGooseTransport: GooseTransport, @unchecked Sendable {
    enum Scenario {
        case proposalLoopSuccess
    }

    private let scenario: Scenario
    private let stateQueue = DispatchQueue(label: "FixtureGooseTransport.state")
    private var sessionRequests: [String: GooseSessionRequest] = [:]

    nonisolated override init(
        baseURL: URL,
        apiKey: String? = nil,
        requestTimeout: TimeInterval = 300
    ) {
        self.scenario = .proposalLoopSuccess
        super.init(baseURL: baseURL, apiKey: apiKey, requestTimeout: requestTimeout)
    }

    nonisolated init(scenario: Scenario, baseURL: URL) {
        self.scenario = scenario
        super.init(baseURL: baseURL)
    }

    override func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse {
        let sessionID = "fixture-\(UUID().uuidString.prefix(8))"
        stateQueue.sync {
            sessionRequests[sessionID] = request
        }
        return GooseSessionResponse(sessionId: sessionID, status: "active")
    }

    override func submitPrompt(
        sessionID: String,
        prompt: GoosePromptRequest
    ) -> AsyncThrowingStream<GooseStreamEvent, Error> {
        let request = stateQueue.sync { sessionRequests[sessionID] }
        let events = buildEvents(sessionID: sessionID, prompt: prompt, request: request)

        return AsyncThrowingStream { continuation in
            Task {
                for event in events {
                    continuation.yield(event)
                }
                continuation.finish()
            }
        }
    }

    override func closeSession(sessionID: String) async throws {
        _ = stateQueue.sync {
            sessionRequests.removeValue(forKey: sessionID)
        }
    }

    private func buildEvents(
        sessionID: String,
        prompt: GoosePromptRequest,
        request: GooseSessionRequest?
    ) -> [GooseStreamEvent] {
        switch scenario {
        case .proposalLoopSuccess:
            return proposalLoopSuccessEvents(sessionID: sessionID, prompt: prompt, request: request)
        }
    }

    private func proposalLoopSuccessEvents(
        sessionID: String,
        prompt: GoosePromptRequest,
        request: GooseSessionRequest?
    ) -> [GooseStreamEvent] {
        let taskName = parseTaskName(from: prompt.content)
        let agentID = request?.metadata?["agent_id"] ?? "unknown_agent"
        let outputDirectory = parseOutputDirectory(from: prompt.content)
        let outputNames = parseOutputNames(from: prompt.content)

        if let outputDirectory {
            writeFixtureOutputs(
                for: taskName,
                agentID: agentID,
                outputNames: outputNames,
                outputDirectory: outputDirectory
            )
        }

        let finalOutput = makeFinalOutput(taskName: taskName, agentID: agentID)

        return [
            .sessionStarted(raw: #"{"session_id":"\#(sessionID)"}"#),
            .promptSubmitted(raw: #"{"task":"\#(taskName)"}"#),
            .toolCallStarted(toolName: "read_workspace", raw: "{}"),
            .toolCallFinished(toolName: "read_workspace", raw: "{}"),
            .textChunk(text: "Fixture backend executing \(taskName)..."),
            .finalOutput(content: finalOutput),
            .sessionClosed(raw: #"{"session_id":"\#(sessionID)"}"#)
        ]
    }

    private func parseTaskName(from prompt: String) -> String {
        guard let line = prompt
            .components(separatedBy: .newlines)
            .first(where: { $0.hasPrefix("## Task:") }) else {
            return "unknown_task"
        }
        return line.replacingOccurrences(of: "## Task:", with: "").trimmingCharacters(in: .whitespaces)
    }

    private func parseOutputNames(from prompt: String) -> [String] {
        guard let sectionStart = prompt.range(of: "### Expected Outputs") else { return [] }
        let tail = prompt[sectionStart.upperBound...]
        var names: [String] = []
        for rawLine in tail.components(separatedBy: .newlines) {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            if line.hasPrefix("Output directory:") || line.hasPrefix("### Stop Condition") {
                break
            }
            if line.hasPrefix("- ") {
                names.append(String(line.dropFirst(2)))
            }
        }
        return names
    }

    private func parseOutputDirectory(from prompt: String) -> URL? {
        guard let line = prompt
            .components(separatedBy: .newlines)
            .first(where: { $0.hasPrefix("Output directory:") }) else {
            return nil
        }
        let path = line.replacingOccurrences(of: "Output directory:", with: "").trimmingCharacters(in: .whitespaces)
        guard !path.isEmpty else { return nil }
        return URL(fileURLWithPath: path, isDirectory: true)
    }

    private func writeFixtureOutputs(
        for taskName: String,
        agentID: String,
        outputNames: [String],
        outputDirectory: URL
    ) {
        try? FileManager.default.createDirectory(at: outputDirectory, withIntermediateDirectories: true)

        for outputName in outputNames {
            let content = makeOutputContent(
                taskName: taskName,
                agentID: agentID,
                outputName: outputName
            )
            let url = outputDirectory.appendingPathComponent(outputName)
            try? content.data(using: .utf8)?.write(to: url)
        }
    }

    private func makeFinalOutput(taskName: String, agentID: String) -> String {
        makeOutputContent(taskName: taskName, agentID: agentID, outputName: "final_output")
    }

    private func makeOutputContent(taskName: String, agentID: String, outputName: String) -> String {
        switch taskName {
        case "normalize_idea_and_prepare_proposal_brief":
            return """
            # Idea Brief

            Agent: \(agentID)
            Output: \(outputName)
            The idea is normalized and ready for proposal drafting.
            """
        case "draft_initial_proposal", "refine_proposal_based_on_review":
            if outputName == "proposal_revision_summary" {
                return """
                # Proposal Revision Summary

                Updated from reviewer feedback.
                """
            }
            return """
            # Proposal Draft

            This proposal is ready for review and approval.
            """
        case "review_proposal_from_product_perspective":
            return reviewerJSON(role: "product_owner", score: 9.4)
        case "review_proposal_from_ux_perspective":
            return reviewerJSON(role: "ux", score: 9.2)
        case "review_proposal_from_ui_perspective":
            return reviewerJSON(role: "ui", score: 9.1)
        case "review_proposal_from_architecture_perspective":
            return reviewerJSON(role: "architect", score: 9.3)
        case "aggregate_proposal_reviews":
            return """
            {
              "aggregate_score": 9.25,
              "min_individual_score": 9.1,
              "blocker_count": 0,
              "passes": true,
              "summary": "Proposal passes the review target."
            }
            """
        default:
            return """
            # Fixture Output

            Task: \(taskName)
            Agent: \(agentID)
            Output: \(outputName)
            """
        }
    }

    private func reviewerJSON(role: String, score: Double) -> String {
        """
        {
          "role": "\(role)",
          "score": \(score),
          "blocker_count": 0,
          "summary": "Looks good for the live proposal loop."
        }
        """
    }
}
