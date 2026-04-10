import Foundation

// MARK: - ExecutionEventBridge (Provider events -> App events)

/// Converts raw runtime SSE stream events into app-friendly execution events
/// for UI display, logging, and receipt building.
///
/// This bridge provides a stable, provider-agnostic event interface.
/// The UI reads `ExecutionEvent`, never raw `RuntimeStreamEvent`.
final class ExecutionEventBridge: @unchecked Sendable {

    // MARK: - State

    private let _lock = NSLock()

    /// Accumulated text output from the execution.
    private var _accumulatedText: String = ""

    /// Tool calls observed during execution.
    private var _toolCalls: [ToolCallRecord] = []

    /// Best-known tool names keyed by provider tool-call id.
    private var _toolNamesByCallID: [String: String] = [:]

    /// All events in order (for receipt building).
    private var _eventLog: [ExecutionEvent] = []

    /// Whether the execution completed with final output.
    private var _hasFinalOutput: Bool = false
    private var _finishReason: String?
    private var _finishRaw: String?

    // MARK: - Public Accessors

    var accumulatedText: String {
        withLock { _accumulatedText }
    }

    var toolCalls: [ToolCallRecord] {
        withLock { _toolCalls }
    }

    var eventLog: [ExecutionEvent] {
        withLock { _eventLog }
    }

    var hasFinalOutput: Bool {
        withLock { _hasFinalOutput }
    }

    var finishReason: String? {
        withLock { _finishReason }
    }

    var finishRaw: String? {
        withLock { _finishRaw }
    }

    // MARK: - Event Processing

    /// Process a stream of RuntimeStreamEvents and yield app-friendly ExecutionEvents.
    func processStream(
        _ stream: AsyncThrowingStream<RuntimeStreamEvent, Error>,
        onEvent: @Sendable @escaping (ExecutionEvent) -> Void
    ) async throws -> ExecutionStreamResult {
        var finalContent: String?

        for try await event in stream {
            let appEvent = mapToAppEvent(event)
            record(appEvent)
            onEvent(appEvent)

            switch event {
            case .finalOutput(let content):
                finalContent = content
                withLock {
                    _hasFinalOutput = true
                }
            case .finish(let reason, _, let raw):
                withLock {
                    _finishReason = reason
                    _finishRaw = raw
                }
            case .textChunk(let text):
                withLock {
                    _accumulatedText += text
                }
            case .toolCallStarted(let toolName, let raw):
                let resolvedToolName = appEvent.toolName ?? toolName
                withLock {
                    _toolCalls.append(ToolCallRecord(
                        toolName: resolvedToolName,
                        rawPayload: raw,
                        startedAt: Date(),
                        completedAt: nil,
                        succeeded: nil,
                        responseRawPayload: nil
                    ))
                }
            case .toolCallFinished(let toolName, let raw):
                let resolvedToolName = appEvent.toolName ?? toolName
                withLock {
                    let idx =
                        _toolCalls.lastIndex(where: { $0.toolName == resolvedToolName && $0.completedAt == nil })
                        ?? _toolCalls.lastIndex(where: { $0.completedAt == nil })
                    if let idx {
                        _toolCalls[idx].completedAt = Date()
                        _toolCalls[idx].succeeded = true
                        _toolCalls[idx].responseRawPayload = raw
                    }
                }
            case .error(let message):
                throw ExecutionEventBridgeError.streamFailed(message: message)
            default:
                break
            }
        }

        return ExecutionStreamResult(
            finalContent: finalContent,
            accumulatedText: accumulatedText,
            toolCalls: toolCalls,
            succeeded: hasFinalOutput,
            finishReason: finishReason,
            finishRaw: finishRaw
        )
    }

    // MARK: - Private: Event Mapping

    private func mapToAppEvent(_ event: RuntimeStreamEvent) -> ExecutionEvent {
        let timestamp = Date()

        switch event {
        case .sessionStarted(let raw):
            let sessionID = parseMetadataValue(from: raw, keys: ["session_id", "sessionId", "id"])
            let detail = sessionID.map { "Session started: \($0)" } ?? "Session started"
            return ExecutionEvent(
                type: .sessionStarted,
                timestamp: timestamp,
                detail: detail,
                sessionID: sessionID
            )
        case .promptSubmitted(let raw):
            let requestID = parseMetadataValue(from: raw, keys: ["request_id", "requestId", "id"])
            let detail = requestID.map { "Prompt submitted: \($0)" } ?? "Prompt submitted"
            return ExecutionEvent(
                type: .promptSubmitted,
                timestamp: timestamp,
                detail: detail,
                requestID: requestID
            )
        case .toolCallStarted(let toolName, _):
            let resolvedToolName = resolveToolCallStartedName(toolName: toolName, raw: event.rawPayload)
            return ExecutionEvent(
                type: .toolCallStarted,
                timestamp: timestamp,
                detail: "Tool: \(resolvedToolName)",
                toolName: resolvedToolName
            )
        case .toolCallFinished(let toolName, _):
            let resolvedToolName = resolveToolCallFinishedName(toolName: toolName, raw: event.rawPayload)
            return ExecutionEvent(
                type: .toolCallFinished,
                timestamp: timestamp,
                detail: "Tool completed: \(resolvedToolName)",
                toolName: resolvedToolName
            )
        case .textChunk(let text):
            return ExecutionEvent(type: .textChunk, timestamp: timestamp, detail: text)
        case .finalOutput:
            return ExecutionEvent(type: .finalOutput, timestamp: timestamp, detail: "Final output received")
        case .finish(let reason, let totalTokens, _):
            let suffix = totalTokens.map { " (\($0) tokens)" } ?? ""
            return ExecutionEvent(type: .finish, timestamp: timestamp, detail: "Finish: \(reason)\(suffix)")
        case .error(let message):
            return ExecutionEvent(type: .error, timestamp: timestamp, detail: message)
        case .sessionClosed:
            return ExecutionEvent(type: .sessionClosed, timestamp: timestamp, detail: "Session closed")
        case .unknown(let type, _):
            return ExecutionEvent(type: .unknown, timestamp: timestamp, detail: "Unknown event: \(type)")
        }
    }

    private func parseMetadataValue(from raw: String, keys: [String]) -> String? {
        guard let data = raw.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }

        for key in keys {
            if let value = json[key] as? String, !value.isEmpty {
                return value
            }
        }

        return nil
    }

    private func record(_ event: ExecutionEvent) {
        withLock {
            if event.type == .textChunk,
               let lastEvent = _eventLog.last,
               lastEvent.type == .textChunk {
                _eventLog[_eventLog.count - 1] = ExecutionEvent(
                    type: .textChunk,
                    timestamp: lastEvent.timestamp,
                    detail: lastEvent.detail + event.detail
                )
                return
            }
            _eventLog.append(event)
        }
    }

    /// Reset state for a new execution.
    func reset() {
        withLock {
            _accumulatedText = ""
            _toolCalls = []
            _toolNamesByCallID = [:]
            _eventLog = []
            _hasFinalOutput = false
            _finishReason = nil
            _finishRaw = nil
        }
    }

    private func withLock<T>(_ body: () -> T) -> T {
        _lock.lock()
        defer { _lock.unlock() }
        return body()
    }

    private func resolveToolCallStartedName(toolName: String, raw: String) -> String {
        let metadata = parseToolPayloadMetadata(from: raw)
        let fallbackFromCallID = metadata.callID.flatMap { callID in
            withLock { _toolNamesByCallID[callID] }
        }
        let resolved = preferredToolName(
            primary: toolName,
            fallback: metadata.toolName ?? fallbackFromCallID
        )

        if let callID = metadata.callID, isDisplayableToolName(resolved) {
            withLock {
                _toolNamesByCallID[callID] = resolved
            }
        }

        return resolved
    }

    private func resolveToolCallFinishedName(toolName: String, raw: String) -> String {
        let metadata = parseToolPayloadMetadata(from: raw)
        let fallbackFromCallID = metadata.callID.flatMap { callID in
            withLock { _toolNamesByCallID[callID] }
        }
        return preferredToolName(primary: toolName, fallback: metadata.toolName ?? fallbackFromCallID)
    }

    private func preferredToolName(primary: String, fallback: String?) -> String {
        if isDisplayableToolName(primary) {
            return primary
        }
        if let fallback, isDisplayableToolName(fallback) {
            return fallback
        }
        return "unknown"
    }

    private func isDisplayableToolName(_ candidate: String?) -> Bool {
        guard let candidate else { return false }
        return !candidate.isEmpty && candidate != "unknown" && !candidate.hasPrefix("call_")
    }

    private func parseToolPayloadMetadata(from raw: String) -> ToolPayloadMetadata {
        guard let data = raw.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return ToolPayloadMetadata(callID: nil, toolName: nil)
        }

        let callID = firstString(
            in: json,
            paths: [
                ["id"],
                ["tool_call_id"],
                ["toolCallId"],
                ["call_id"],
                ["callId"]
            ]
        )

        let toolName = firstString(
            in: json,
            paths: [
                ["tool_name"],
                ["toolName"],
                ["tool_call", "name"],
                ["toolCall", "name"],
                ["tool", "name"],
                ["function", "name"],
                ["name"]
            ]
        )

        return ToolPayloadMetadata(callID: callID, toolName: toolName)
    }

    private func firstString(in json: [String: Any], paths: [[String]]) -> String? {
        for path in paths {
            if let value = stringValue(in: json, path: path), !value.isEmpty {
                return value
            }
        }
        return nil
    }

    private func stringValue(in json: [String: Any], path: [String]) -> String? {
        var current: Any = json

        for key in path {
            guard let dictionary = current as? [String: Any],
                  let next = dictionary[key] else {
                return nil
            }
            current = next
        }

        return current as? String
    }
}

private struct ToolPayloadMetadata {
    let callID: String?
    let toolName: String?
}

private extension RuntimeStreamEvent {
    var rawPayload: String {
        switch self {
        case .sessionStarted(let raw),
                .promptSubmitted(let raw),
                .toolCallStarted(_, let raw),
                .toolCallFinished(_, let raw),
                .finish(_, _, let raw),
                .sessionClosed(let raw):
            return raw
        case .textChunk(let text):
            return text
        case .finalOutput(let content):
            return content
        case .error(let message):
            return message
        case .unknown(_, let data):
            return data
        }
    }
}

enum ExecutionEventBridgeError: LocalizedError {
    case streamFailed(message: String)

    var errorDescription: String? {
        switch self {
        case .streamFailed(let message):
            return message
        }
    }
}

// MARK: - ExecutionEvent (app-friendly event)

/// App-friendly execution event. The UI and receipt builder consume this.
struct ExecutionEvent: Sendable, Identifiable {
    let id: UUID = UUID()
    let type: EventType
    let timestamp: Date
    let detail: String
    let sessionID: String?
    let requestID: String?
    let toolName: String?

    init(
        type: EventType,
        timestamp: Date,
        detail: String,
        sessionID: String? = nil,
        requestID: String? = nil,
        toolName: String? = nil
    ) {
        self.type = type
        self.timestamp = timestamp
        self.detail = detail
        self.sessionID = sessionID
        self.requestID = requestID
        self.toolName = toolName
    }

    enum EventType: String, Sendable {
        case sessionStarted = "session_started"
        case promptSubmitted = "prompt_submitted"
        case toolCallStarted = "tool_call_started"
        case toolCallFinished = "tool_call_finished"
        case textChunk = "text_chunk"
        case finalOutput = "final_output"
        case finish = "finish"
        case error = "error"
        case sessionClosed = "session_closed"
        case unknown = "unknown"
    }
}

// MARK: - ToolCallRecord

/// Record of a tool call during execution.
struct ToolCallRecord: Sendable {
    let toolName: String
    let rawPayload: String
    let startedAt: Date
    var completedAt: Date?
    var succeeded: Bool?
    var responseRawPayload: String?
}

// MARK: - ExecutionStreamResult

/// Final result after processing the entire event stream.
struct ExecutionStreamResult: Sendable {
    let finalContent: String?
    let accumulatedText: String
    let toolCalls: [ToolCallRecord]
    let succeeded: Bool
    let finishReason: String?
    let finishRaw: String?
}
