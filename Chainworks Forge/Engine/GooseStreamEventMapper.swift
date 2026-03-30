import Foundation

// MARK: - GooseStreamEventMapper (Proposal 005, Section 5.3)

/// Maps goosed server `MessageEvent` SSE payloads into the app's `GooseStreamEvent` enum.
///
/// The real `goosed` server emits SSE events in a different format than the bespoke
/// transport contract. Each SSE `data:` line contains a JSON object with a `type` field
/// that determines the event kind.
///
/// goosed MessageEvent types:
/// - `Message`  → agent produced a message (text, tool call, or tool response)
/// - `Finish`   → stream complete
/// - `Error`    → agent error
/// - `Ping`     → heartbeat (every 500ms)
/// - `Notification` → MCP notification
/// - `UpdateConversation` → full conversation replacement
/// - `ActiveRequests` → in-flight request IDs
///
/// This mapper is a single file — easy to update if goosed's event format changes.
enum GooseStreamEventMapper {

    // MARK: - Public API

    /// Map a raw SSE `data:` JSON string from goosed into a `GooseStreamEvent`.
    /// Returns `nil` for events that should be silently ignored (e.g., Ping).
    static func map(_ rawJSON: String) -> GooseStreamEvent? {
        guard let data = rawJSON.data(using: .utf8) else {
            return .error(message: "Invalid UTF-8 in SSE data")
        }

        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = json["type"] as? String else {
            return .error(message: "Malformed SSE JSON: \(String(rawJSON.prefix(200)))")
        }

        switch type {
        case "Message":
            return mapMessage(json)
        case "Finish":
            return mapFinish(json)
        case "Error":
            let errorMessage = json["error"] as? String ?? "Unknown error"
            return .error(message: errorMessage)
        case "Ping":
            // Silently ignored — heartbeat
            return nil
        case "Notification":
            // MCP notification — currently not actionable, ignore
            return nil
        case "UpdateConversation":
            // Full conversation replacement — not used in single-turn model, ignore
            return nil
        case "ActiveRequests":
            // In-flight request IDs — informational, ignore
            return nil
        case "ModelChange":
            // Model/mode selection event — informational, ignore
            return nil
        default:
            return .unknown(type: type, data: rawJSON)
        }
    }

    // MARK: - Private: Message Mapping

    /// Map a `Message` event. A goosed `Message` has role + content array.
    /// Content items can be Text, ToolRequest, or ToolResponse.
    private static func mapMessage(_ json: [String: Any]) -> GooseStreamEvent {
        guard let messageDict = json["message"] as? [String: Any],
              let contentArray = messageDict["content"] as? [[String: Any]] else {
            // Message without content — emit as text chunk with raw data
            return .textChunk(text: "")
        }

        // Process content items and return the most significant event.
        // In practice, goosed sends one content item per Message event.
        for content in contentArray {
            guard let contentType = content["type"] as? String else { continue }

            switch contentType.lowercased() {
            case "text":
                let text = content["text"] as? String ?? ""
                return .textChunk(text: text)

            case "toolrequest":
                let toolName = extractToolName(from: content)
                let raw = serializeToJSON(content) ?? "{}"
                return .toolCallStarted(toolName: toolName, raw: raw)

            case "toolresponse":
                let toolName = extractToolNameFromResponse(content)
                let raw = serializeToJSON(content) ?? "{}"
                return .toolCallFinished(toolName: toolName, raw: raw)

            case "thinking":
                // Thinking content — treat as text chunk for transparency
                let text = content["text"] as? String ?? ""
                if !text.isEmpty {
                    return .textChunk(text: "[thinking] \(text)")
                }
                continue

            default:
                continue
            }
        }

        // If we get here, the message had content but nothing we could map
        return .textChunk(text: "")
    }

    // MARK: - Private: Finish Mapping

    /// Map a `Finish` event into a terminal marker.
    private static func mapFinish(_ json: [String: Any]) -> GooseStreamEvent {
        let reason = json["reason"] as? String ?? "stop"

        var totalTokens: Int?
        if let tokenState = json["token_state"] as? [String: Any] {
            totalTokens = tokenState["total_tokens"] as? Int
        }

        return .finish(reason: reason, totalTokens: totalTokens, raw: serializeToJSON(json) ?? "{}")
    }

    // MARK: - Private: Tool Name Extraction

    /// Extract tool name from a ToolRequest content item.
    /// ToolRequest structure: `{ "id": "...", "tool_call": { "name": "...", "arguments": {...} } }`
    private static func extractToolName(from content: [String: Any]) -> String {
        // Try nested tool_call structure (goosed format)
        if let toolCall = content["tool_call"] as? [String: Any],
           let name = toolCall["name"] as? String {
            return name
        }
        // Try camelCase variant
        if let toolCall = content["toolCall"] as? [String: Any],
           let name = toolCall["name"] as? String {
            return name
        }
        return "unknown"
    }

    /// Extract tool name from a ToolResponse content item.
    /// The response doesn't carry the tool name directly — we use the id to correlate.
    private static func extractToolNameFromResponse(_ content: [String: Any]) -> String {
        // ToolResponse may not have the tool name. Use id as fallback.
        if let id = content["id"] as? String {
            return id
        }
        return "unknown"
    }

    // MARK: - Private: Serialization

    private static func serializeToJSON(_ dict: [String: Any]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: dict) else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }
}
