import Testing
import Foundation
@testable import Chainworks_Forge

// MARK: - GooseStreamEventMapperTests (Proposal 005, Section 8)

/// Unit tests for GooseStreamEventMapper.
/// Validates that all goosed MessageEvent types are correctly mapped to GooseStreamEvent.
@Suite("GooseStreamEventMapper")
struct GooseStreamEventMapperTests {

    // MARK: - Message Events

    @Test("map text message extracts text content")
    func mapTextMessage() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"text","text":"Hello world"}]},"token_state":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .textChunk(let text) = event {
            #expect(text == "Hello world")
        } else {
            Issue.record("Expected .textChunk, got \(String(describing: event))")
        }
    }

    @Test("map tool request message extracts tool name")
    func mapToolRequestMessage() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"toolRequest","id":"call_1","tool_call":{"name":"read_file","arguments":{"path":"/tmp/test.txt"}}}]},"token_state":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .toolCallStarted(let toolName, _) = event {
            #expect(toolName == "read_file")
        } else {
            Issue.record("Expected .toolCallStarted, got \(String(describing: event))")
        }
    }

    @Test("map tool response message extracts tool id")
    func mapToolResponseMessage() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"toolResponse","id":"call_1","tool_result":"file contents here"}]},"token_state":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .toolCallFinished(let toolName, _) = event {
            #expect(toolName == "call_1")
        } else {
            Issue.record("Expected .toolCallFinished, got \(String(describing: event))")
        }
    }

    @Test("map tool request with camelCase toolCall key")
    func mapToolRequestWithCamelCaseToolCall() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"toolRequest","id":"call_2","toolCall":{"name":"write_file","arguments":{"path":"/tmp/out.txt","content":"hello"}}}]},"token_state":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .toolCallStarted(let toolName, _) = event {
            #expect(toolName == "write_file")
        } else {
            Issue.record("Expected .toolCallStarted, got \(String(describing: event))")
        }
    }

    // MARK: - Finish Events

    @Test("map finish event with tokens")
    func mapFinishEvent() {
        let json = """
        {"type":"Finish","reason":"stop","token_state":{"total_tokens":42}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .finish(let reason, let totalTokens, let raw) = event {
            #expect(reason == "stop")
            #expect(totalTokens == 42)
            #expect(raw.contains("\"type\":\"Finish\""))
        } else {
            Issue.record("Expected .finish, got \(String(describing: event))")
        }
    }

    @Test("map finish event without tokens")
    func mapFinishEventWithoutTokens() {
        let json = """
        {"type":"Finish","reason":"stop"}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .finish(let reason, let totalTokens, _) = event {
            #expect(reason == "stop")
            #expect(totalTokens == nil)
        } else {
            Issue.record("Expected .finish, got \(String(describing: event))")
        }
    }

    // MARK: - Error Events

    @Test("map error event extracts error message")
    func mapErrorEvent() {
        let json = """
        {"type":"Error","error":"Provider not set"}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .error(let message) = event {
            #expect(message == "Provider not set")
        } else {
            Issue.record("Expected .error, got \(String(describing: event))")
        }
    }

    // MARK: - Ignored Events (parameterized)

    @Test("ignored events return nil", arguments: [
        #"{"type":"Ping"}"#,
        #"{"type":"Notification","request_id":"req_1","message":{}}"#,
        #"{"type":"UpdateConversation","conversation":{"messages":[]}}"#,
        #"{"type":"ActiveRequests","request_ids":["req_1","req_2"]}"#,
        #"{"type":"ModelChange","model":"default","mode":"lead"}"#
    ])
    func ignoredEventsReturnNil(json: String) {
        let event = GooseStreamEventMapper.map(json)
        #expect(event == nil)
    }

    // MARK: - Edge Cases

    @Test("map unknown type extracts type name")
    func mapUnknownType() {
        let json = """
        {"type":"FutureEventType","data":"something"}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .unknown(let type, _) = event {
            #expect(type == "FutureEventType")
        } else {
            Issue.record("Expected .unknown, got \(String(describing: event))")
        }
    }

    @Test("map malformed JSON returns error")
    func mapMalformedJSON() {
        let event = GooseStreamEventMapper.map("not json at all")
        if case .error(let message) = event {
            #expect(message.contains("Malformed"))
        } else {
            Issue.record("Expected .error, got \(String(describing: event))")
        }
    }

    @Test("map empty message content returns empty text chunk")
    func mapEmptyMessageContent() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[]}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .textChunk(let text) = event {
            #expect(text == "")
        } else {
            Issue.record("Expected .textChunk with empty text, got \(String(describing: event))")
        }
    }

    @Test("map message with thinking content")
    func mapMessageWithThinkingContent() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"thinking","text":"Let me think about this..."}]}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .textChunk(let text) = event {
            #expect(text.contains("thinking"))
            #expect(text.contains("Let me think about this..."))
        } else {
            Issue.record("Expected .textChunk with thinking content, got \(String(describing: event))")
        }
    }
}
