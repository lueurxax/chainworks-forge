import Foundation

// MARK: - ACPStreamEventMapper (Proposal 026, Phase 3 — Step 3.5)

/// Maps ACP `session/update` JSON-RPC notification payloads to canonical `RuntimeStreamEvent` values.
///
/// Handles both Claude Agent ACP and Gemini CLI ACP event taxonomies, which share
/// the same ACP notification vocabulary (`session/update`) with type-discriminated payloads.
///
/// ACP notification format (ndjson over stdio):
/// ```json
/// {"jsonrpc": "2.0", "method": "session/update", "params": {"type": "agent_message_chunk", "content": "..."}}
/// ```
///
/// Event taxonomy from live ACP research probes:
/// - `agent_message_chunk`      -> `.textChunk(text:)`
/// - `agent_thought_chunk`      -> `.textChunk(text: "[thinking] " + content)`
/// - `tool_call` (pending)      -> `.toolCallStarted(toolName:, raw:)`
/// - `tool_call_update` (completed) -> `.toolCallFinished(toolName:, raw:)`
/// - `usage_update`             -> `.unknown(type: "usage_update", data:)`
/// - `available_commands_update` -> nil (ignored)
/// - `current_mode_update`      -> nil (ignored)
/// - `config_option_update`     -> nil (ignored)
/// - `session_info_update`      -> nil (ignored)
/// - `user_message_chunk`       -> nil (ignored — Forge owns prompt truth)
/// - Stream end / final result  -> `.finish(reason:, totalTokens:, raw:)`
/// - Error                      -> `.error(message:)`
enum ACPStreamEventMapper {

    // MARK: - Public API

    /// Map an ACP `session/update` notification payload to a `RuntimeStreamEvent`.
    /// Returns `nil` for events that should be silently ignored.
    ///
    /// - Parameter json: The `params` dictionary from the JSON-RPC notification.
    static func mapSessionUpdate(_ json: [String: Any]) -> RuntimeStreamEvent? {
        // ACP session/update structure: params.update.sessionUpdate = "event_type"
        // Content fields are siblings of sessionUpdate inside the update dict.
        let update = json["update"] as? [String: Any] ?? json
        guard let type = update["sessionUpdate"] as? String ?? update["type"] as? String else {
            // Log first unrecognized event to diagnose structure
            let keys = json.keys.sorted().joined(separator: ",")
            let updateKeys = (json["update"] as? [String: Any])?.keys.sorted().joined(separator: ",") ?? "nil"
            ForgeLogger.bridge.info("Unrecognized session/update: topKeys=[\(keys)] updateKeys=[\(updateKeys)] raw=\(serializeToJSON(json)?.prefix(300) ?? "nil")")
            return .unknown(type: "unknown_session_update", data: serializeToJSON(json) ?? "{}")
        }
        // Merge update dict as the working payload for content extraction
        let payload = update

        switch type {
        case "agent_message_chunk":
            let content = extractChunkText(from: payload["content"], logPrefix: "agent_message_chunk")
            guard !content.isEmpty else { return nil }
            return .textChunk(text: content)

        case "agent_thought_chunk":
            let content = extractChunkText(from: payload["content"], logPrefix: "agent_thought_chunk")
            guard !content.isEmpty else { return nil }
            return .textChunk(text: "[thinking] \(content)")

        case "tool_call":
            return mapToolCall(payload)

        case "tool_call_update":
            return mapToolCallUpdate(payload)

        case "usage_update":
            let raw = serializeToJSON(payload) ?? "{}"
            return .unknown(type: "usage_update", data: raw)

        case "plan":
            let raw = serializeToJSON(payload) ?? "{}"
            return .unknown(type: "plan", data: raw)

        case "available_commands_update",
             "current_mode_update",
             "config_option_update",
             "session_info_update",
             "user_message_chunk":
            return nil

        default:
            let raw = serializeToJSON(payload) ?? "{}"
            return .unknown(type: type, data: raw)
        }
    }

    /// Map a full JSON-RPC notification (method + params) to a `RuntimeStreamEvent`.
    /// Dispatches to `mapSessionUpdate` for `session/update` notifications,
    /// and handles `session/request_permission` and error notifications.
    ///
    /// - Parameters:
    ///   - method: The JSON-RPC method string.
    ///   - params: The JSON-RPC params dictionary (may be nil).
    static func mapNotificationEvents(method: String, params: [String: Any]?) -> [RuntimeStreamEvent] {
        switch method {
        case "session/update":
            guard let params, let event = mapSessionUpdate(params) else { return [] }
            return [event]

        case "session/request_permission":
            let toolName = extractPermissionToolName(from: params)
            let raw = serializeToJSON(params ?? [:]) ?? "{}"
            // Permission requests are only started here. Completion must be inferred from
            // subsequent provider progress, otherwise the UI falsely claims permission
            // succeeded even when the provider rejected or failed to deserialize the response.
            return [.toolCallStarted(toolName: "permission:\(toolName)", raw: raw)]

        case "session/error":
            let message: String
            if let params,
               let nestedError = params["error"] as? [String: Any] {
                message = ACPProtocolSupport.formatJSONRPCError(
                    nestedError,
                    fallback: "Unknown ACP session error"
                )
            } else if let params {
                message = ACPProtocolSupport.formatJSONRPCError(
                    params,
                    fallback: "Unknown ACP session error"
                )
            } else {
                message = "Unknown ACP session error"
            }
            return [.error(message: message)]

        default:
            return []
        }
    }

    static func mapNotification(method: String, params: [String: Any]?) -> RuntimeStreamEvent? {
        mapNotificationEvents(method: method, params: params).first
    }

    static func extractPermissionToolNameForDiagnostics(from params: [String: Any]?) -> String {
        extractPermissionToolName(from: params)
    }

    // MARK: - Result Mapping

    /// Map a JSON-RPC result (response to `session/prompt`) to a terminal `RuntimeStreamEvent`.
    /// Called when the prompt request completes with a final result payload.
    static func mapPromptResult(_ result: [String: Any]) -> RuntimeStreamEvent {
        let stopReason = result["stopReason"] as? String ?? "end_turn"

        var totalTokens: Int?
        if let usage = result["usage"] as? [String: Any] {
            totalTokens = usage["totalTokens"] as? Int
        }
        // Gemini surfaces usage under _meta.quota
        if totalTokens == nil, let meta = result["_meta"] as? [String: Any],
           let quota = meta["quota"] as? [String: Any],
           let tokenCount = quota["token_count"] as? Int {
            totalTokens = tokenCount
        }

        let raw = serializeToJSON(result) ?? "{}"
        return .finish(reason: stopReason, totalTokens: totalTokens, raw: raw)
    }

    // MARK: - Private: Tool Call Mapping

    /// Map a `tool_call` event. Observed statuses: `pending`, `in_progress`.
    private static func mapToolCall(_ json: [String: Any]) -> RuntimeStreamEvent {
        let toolName = extractToolName(from: json)
        let raw = serializeToJSON(json) ?? "{}"

        let status = json["status"] as? String ?? "pending"
        switch status {
        case "completed":
            return .toolCallFinished(toolName: toolName, raw: raw)
        default:
            // pending, in_progress, or any other status -> started
            return .toolCallStarted(toolName: toolName, raw: raw)
        }
    }

    /// Map a `tool_call_update` event. Observed statuses: `completed`, `failed`, `in_progress`.
    private static func mapToolCallUpdate(_ json: [String: Any]) -> RuntimeStreamEvent {
        let toolName = extractToolName(from: json)
        let raw = serializeToJSON(json) ?? "{}"

        let status = json["status"] as? String ?? "in_progress"
        switch status {
        case "completed", "failed":
            return .toolCallFinished(toolName: toolName, raw: raw)
        default:
            // in_progress updates — treat as refinements (started)
            return .toolCallStarted(toolName: toolName, raw: raw)
        }
    }

    // MARK: - Private: Tool Name Extraction

    /// Extract tool name from a tool_call or tool_call_update payload.
    /// ACP tool events may carry the name in different locations depending on the adapter.
    private static func extractToolName(from json: [String: Any]) -> String {
        // Direct name field
        if let name = json["name"] as? String, !name.isEmpty {
            return name
        }
        // Nested toolCall structure (Claude Agent ACP)
        if let toolCall = json["toolCall"] as? [String: Any],
           let name = toolCall["name"] as? String, !name.isEmpty {
            return name
        }
        // Nested tool_call structure (snake_case variant)
        if let toolCall = json["tool_call"] as? [String: Any],
           let name = toolCall["name"] as? String, !name.isEmpty {
            return name
        }
        // Kind field (Claude Agent ACP edit flows)
        if let kind = json["kind"] as? String, !kind.isEmpty {
            return kind
        }
        // Tool name from _meta.claudeCode (Claude Agent ACP shell tools)
        if let meta = json["_meta"] as? [String: Any],
           let claudeCode = meta["claudeCode"] as? [String: Any],
           let name = claudeCode["tool"] as? String, !name.isEmpty {
            return name
        }
        return "unknown"
    }

    private static func extractChunkText(from rawContent: Any?, logPrefix: String) -> String {
        guard let rawContent else { return "" }

        if let str = rawContent as? String {
            return str
        }

        if let dict = rawContent as? [String: Any] {
            if let text = dict["text"] as? String {
                return text
            }

            if let content = dict["content"] {
                let nested = extractChunkText(from: content, logPrefix: logPrefix)
                if !nested.isEmpty { return nested }
            }

            if let parts = dict["parts"] as? [Any] {
                let joined = parts
                    .map { extractChunkText(from: $0, logPrefix: logPrefix) }
                    .joined()
                if !joined.isEmpty { return joined }
            }

            if let items = dict["items"] as? [Any] {
                let joined = items
                    .map { extractChunkText(from: $0, logPrefix: logPrefix) }
                    .joined()
                if !joined.isEmpty { return joined }
            }
        }

        if let array = rawContent as? [Any] {
            return array
                .map { extractChunkText(from: $0, logPrefix: logPrefix) }
                .joined()
        }

        ForgeLogger.bridge.info("\(logPrefix) content is NOT String, actual: \(Swift.type(of: rawContent)), value: \(String(describing: rawContent).prefix(200))")
        return ""
    }

    /// Extract tool name from a `session/request_permission` payload.
    private static func extractPermissionToolName(from params: [String: Any]?) -> String {
        guard let params else { return "unknown" }
        if let toolCall = params["toolCall"] as? [String: Any] {
            if let kind = toolCall["kind"] as? String, !kind.isEmpty {
                return kind
            }
            if let name = toolCall["name"] as? String, !name.isEmpty {
                return name
            }
        }
        if let tool = params["tool"] as? String, !tool.isEmpty {
            return tool
        }
        return "unknown"
    }

    // MARK: - Private: Serialization

    private static func serializeToJSON(_ dict: [String: Any]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys]) else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }
}
