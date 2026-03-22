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

    // MARK: - Public Accessors

    var accumulatedText: String {
        _lock.lock()
        defer { _lock.unlock() }
        return _accumulatedText
    }

    var toolCalls: [ToolCallRecord] {
        _lock.lock()
        defer { _lock.unlock() }
        return _toolCalls
    }

    var eventLog: [ExecutionEvent] {
        _lock.lock()
        defer { _lock.unlock() }
        return _eventLog
    }

    var hasFinalOutput: Bool {
        _lock.lock()
        defer { _lock.unlock() }
        return _hasFinalOutput
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
                _lock.lock()
                _hasFinalOutput = true
                _lock.unlock()
            case .textChunk(let text):
                _lock.lock()
                _accumulatedText += text
                _lock.unlock()
            case .toolCallStarted(let toolName, _):
                _lock.lock()
                _toolCalls.append(ToolCallRecord(
                    toolName: toolName,
                    startedAt: Date(),
                    completedAt: nil,
                    succeeded: nil
                ))
                _lock.unlock()
            case .toolCallFinished(let toolName, _):
                _lock.lock()
                if let idx = _toolCalls.lastIndex(where: { $0.toolName == toolName && $0.completedAt == nil }) {
                    _toolCalls[idx].completedAt = Date()
                    _toolCalls[idx].succeeded = true
                }
                _lock.unlock()
            case .error:
                break
            default:
                break
            }
        }

        return ExecutionStreamResult(
            finalContent: finalContent,
            accumulatedText: accumulatedText,
            toolCalls: toolCalls,
            succeeded: hasFinalOutput
        )
    }

    // MARK: - Private: Event Mapping

    private func mapToAppEvent(_ event: GooseStreamEvent) -> ExecutionEvent {
        let timestamp = Date()

        switch event {
        case .sessionStarted:
            return ExecutionEvent(type: .sessionStarted, timestamp: timestamp, detail: "Session started")
        case .promptSubmitted:
            return ExecutionEvent(type: .promptSubmitted, timestamp: timestamp, detail: "Prompt submitted")
        case .toolCallStarted(let toolName, _):
            return ExecutionEvent(type: .toolCallStarted, timestamp: timestamp, detail: "Tool: \(toolName)")
        case .toolCallFinished(let toolName, _):
            return ExecutionEvent(type: .toolCallFinished, timestamp: timestamp, detail: "Tool completed: \(toolName)")
        case .textChunk(let text):
            return ExecutionEvent(type: .textChunk, timestamp: timestamp, detail: String(text.prefix(200)))
        case .finalOutput:
            return ExecutionEvent(type: .finalOutput, timestamp: timestamp, detail: "Final output received")
        case .error(let message):
            return ExecutionEvent(type: .error, timestamp: timestamp, detail: message)
        case .sessionClosed:
            return ExecutionEvent(type: .sessionClosed, timestamp: timestamp, detail: "Session closed")
        case .unknown(let type, _):
            return ExecutionEvent(type: .unknown, timestamp: timestamp, detail: "Unknown event: \(type)")
        }
    }

    private func record(_ event: ExecutionEvent) {
        _lock.lock()
        _eventLog.append(event)
        _lock.unlock()
    }

    /// Reset state for a new execution.
    func reset() {
        _lock.lock()
        _accumulatedText = ""
        _toolCalls = []
        _eventLog = []
        _hasFinalOutput = false
        _lock.unlock()
    }
}

// MARK: - ExecutionEvent (app-friendly event)

/// App-friendly execution event. The UI and receipt builder consume this.
struct ExecutionEvent: Sendable, Identifiable {
    let id: UUID = UUID()
    let type: EventType
    let timestamp: Date
    let detail: String

    enum EventType: String, Sendable {
        case sessionStarted = "session_started"
        case promptSubmitted = "prompt_submitted"
        case toolCallStarted = "tool_call_started"
        case toolCallFinished = "tool_call_finished"
        case textChunk = "text_chunk"
        case finalOutput = "final_output"
        case error = "error"
        case sessionClosed = "session_closed"
        case unknown = "unknown"
    }
}

// MARK: - ToolCallRecord

/// Record of a tool call during execution.
struct ToolCallRecord: Sendable {
    let toolName: String
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
}
