import Foundation

// MARK: - ExecutionEventBridge (Provider events -> App events)

/// Converts raw Goose SSE stream events into app-friendly execution events
/// for UI display, logging, and receipt building.
///
/// This bridge provides a stable, provider-agnostic event interface.
/// The UI reads `ExecutionEvent`, never raw `GooseStreamEvent`.
final class ExecutionEventBridge: @unchecked Sendable {

    // MARK: - State

    private let _lock = NSLock()

    /// Accumulated text output from the execution.
    private var _accumulatedText: String = ""

    /// Tool calls observed during execution.
    private var _toolCalls: [ToolCallRecord] = []

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

    /// Process a stream of GooseStreamEvents and yield app-friendly ExecutionEvents.
    func processStream(
        _ stream: AsyncThrowingStream<GooseStreamEvent, Error>,
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
                withLock {
                    _toolCalls.append(ToolCallRecord(
                        toolName: toolName,
                        rawPayload: raw,
                        startedAt: Date(),
                        completedAt: nil,
                        succeeded: nil
                    ))
                }
            case .toolCallFinished(let toolName, _):
                withLock {
                    if let idx = _toolCalls.lastIndex(where: { $0.toolName == toolName && $0.completedAt == nil }) {
                        _toolCalls[idx].completedAt = Date()
                        _toolCalls[idx].succeeded = true
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

    private func mapToAppEvent(_ event: GooseStreamEvent) -> ExecutionEvent {
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
            return ExecutionEvent(
                type: .toolCallStarted,
                timestamp: timestamp,
                detail: "Tool: \(toolName)",
                toolName: toolName
            )
        case .toolCallFinished(let toolName, _):
            return ExecutionEvent(
                type: .toolCallFinished,
                timestamp: timestamp,
                detail: "Tool completed: \(toolName)",
                toolName: toolName
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
            _eventLog.append(event)
        }
    }

    /// Reset state for a new execution.
    func reset() {
        withLock {
            _accumulatedText = ""
            _toolCalls = []
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
