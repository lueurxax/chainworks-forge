import Foundation

// MARK: - ExecutionReceiptBuilder (ARCH-032: transcripts are first-class outputs)

/// Produces structured receipt/transcript artifacts from a live execution.
/// Receipts capture the full execution timeline, tool calls, and outputs
/// for debugging and provenance inspection.
///
/// Proposal 004 locked decision: transcript/receipt artifacts are first-class
/// outputs of live execution. Debuggability is part of the product, not an afterthought.
struct ExecutionReceiptBuilder: Sendable {

    // MARK: - Build Receipt

    /// Build a complete execution receipt from the event bridge state.
    /// Returns a dictionary of receipt artifact names to their Data content.
    static func buildReceipt(
        agentID: String,
        sessionID: String,
        stageID: String,
        iteration: Int,
        attemptNumber: Int,
        startedAt: Date,
        completedAt: Date,
        events: [ExecutionEvent],
        toolCalls: [ToolCallRecord],
        finalContent: String?,
        succeeded: Bool,
        errorMessage: String?,
        provider: String,
        model: String,
        effort: String,
        sessionReuseDisposition: String? = nil,
        sessionReuseScope: String? = nil,
        sessionFamilyID: String? = nil
    ) -> [String: Data] {
        var artifacts: [String: Data] = [:]

        // 1. Structured receipt (JSON) — Proposal 018: includes session provenance
        let receipt = ExecutionReceipt(
            receiptVersion: "1.1",
            agentID: agentID,
            sessionID: sessionID,
            stageID: stageID,
            iteration: iteration,
            attemptNumber: attemptNumber,
            startedAt: startedAt,
            completedAt: completedAt,
            durationSeconds: completedAt.timeIntervalSince(startedAt),
            succeeded: succeeded,
            errorMessage: errorMessage,
            provider: provider,
            model: model,
            effort: effort,
            toolCallCount: toolCalls.count,
            toolCalls: toolCalls.map { tc in
                ReceiptToolCall(
                    toolName: tc.toolName,
                    startedAt: tc.startedAt,
                    completedAt: tc.completedAt,
                    succeeded: tc.succeeded ?? false
                )
            },
            eventCount: events.count,
            sessionReuseDisposition: sessionReuseDisposition,
            sessionReuseScope: sessionReuseScope,
            sessionFamilyID: sessionFamilyID
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        if let receiptData = try? encoder.encode(receipt) {
            artifacts["\(agentID)_receipt.json"] = receiptData
        }

        // 2. Full transcript (Markdown)
        let transcript = buildTranscript(
            agentID: agentID,
            sessionID: sessionID,
            stageID: stageID,
            startedAt: startedAt,
            completedAt: completedAt,
            events: events,
            finalContent: finalContent,
            succeeded: succeeded,
            errorMessage: errorMessage,
            provider: provider,
            model: model
        )
        if let transcriptData = transcript.data(using: .utf8) {
            artifacts["\(agentID)_transcript.md"] = transcriptData
        }

        return artifacts
    }

    // MARK: - Build Transcript (Markdown)

    private static func buildTranscript(
        agentID: String,
        sessionID: String,
        stageID: String,
        startedAt: Date,
        completedAt: Date,
        events: [ExecutionEvent],
        finalContent: String?,
        succeeded: Bool,
        errorMessage: String?,
        provider: String,
        model: String
    ) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        var lines: [String] = []

        lines.append("# Execution Transcript")
        lines.append("")
        lines.append("| Field | Value |")
        lines.append("|---|---|")
        lines.append("| Agent | \(agentID) |")
        lines.append("| Session | \(sessionID) |")
        lines.append("| Stage | \(stageID) |")
        lines.append("| Provider | \(provider) |")
        lines.append("| Model | \(model) |")
        lines.append("| Started | \(formatter.string(from: startedAt)) |")
        lines.append("| Completed | \(formatter.string(from: completedAt)) |")
        lines.append("| Duration | \(String(format: "%.1f", completedAt.timeIntervalSince(startedAt)))s |")
        lines.append("| Result | \(succeeded ? "✅ Success" : "❌ Failed") |")
        if let error = errorMessage {
            lines.append("| Error | \(error) |")
        }
        lines.append("")

        // Event timeline
        lines.append("## Event Timeline")
        lines.append("")
        for event in events {
            let ts = formatter.string(from: event.timestamp)
            lines.append("- `\(ts)` **\(event.type.rawValue)**: \(event.detail)")
        }
        lines.append("")

        // Final output
        if let content = finalContent {
            lines.append("## Final Output")
            lines.append("")
            lines.append("```")
            lines.append(content)
            lines.append("```")
            lines.append("")
        }

        return lines.joined(separator: "\n")
    }
}

// MARK: - ExecutionReceipt (structured JSON artifact)

struct ExecutionReceipt: Codable, Sendable {
    let receiptVersion: String
    let agentID: String
    let sessionID: String
    let stageID: String
    let iteration: Int
    let attemptNumber: Int
    let startedAt: Date
    let completedAt: Date
    let durationSeconds: TimeInterval
    let succeeded: Bool
    let errorMessage: String?
    let provider: String
    let model: String
    let effort: String
    let toolCallCount: Int
    let toolCalls: [ReceiptToolCall]
    let eventCount: Int
    // Proposal 018: Session provenance fields for receipt consumers (REQ-006, PROD-001)
    let sessionReuseDisposition: String?
    let sessionReuseScope: String?
    let sessionFamilyID: String?

    enum CodingKeys: String, CodingKey {
        case receiptVersion = "receipt_version"
        case agentID = "agent_id"
        case sessionID = "session_id"
        case stageID = "stage_id"
        case iteration
        case attemptNumber = "attempt_number"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case durationSeconds = "duration_seconds"
        case succeeded
        case errorMessage = "error_message"
        case provider, model, effort
        case toolCallCount = "tool_call_count"
        case toolCalls = "tool_calls"
        case eventCount = "event_count"
        case sessionReuseDisposition = "session_reuse_disposition"
        case sessionReuseScope = "session_reuse_scope"
        case sessionFamilyID = "session_family_id"
    }
}

struct ReceiptToolCall: Codable, Sendable {
    let toolName: String
    let startedAt: Date
    let completedAt: Date?
    let succeeded: Bool

    enum CodingKeys: String, CodingKey {
        case toolName = "tool_name"
        case startedAt = "started_at"
        case completedAt = "completed_at"
        case succeeded
    }
}
