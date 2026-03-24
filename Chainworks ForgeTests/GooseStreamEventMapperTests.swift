import XCTest
import Foundation
@testable import Chainworks_Forge

// MARK: - GooseStreamEventMapperTests (Proposal 005, Section 8)

/// Unit tests for GooseStreamEventMapper.
/// Validates that all goosed MessageEvent types are correctly mapped to GooseStreamEvent.
final class GooseStreamEventMapperTests: XCTestCase {

    // MARK: - Message Events

    func testMapTextMessage() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"text","text":"Hello world"}]},"token_state":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .textChunk(let text) = event {
            XCTAssertEqual(text, "Hello world")
        } else {
            XCTFail("Expected .textChunk, got \\(String(describing: event))")
        }
    }

    func testMapToolRequestMessage() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"toolRequest","id":"call_1","tool_call":{"name":"read_file","arguments":{"path":"/tmp/test.txt"}}}]},"token_state":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .toolCallStarted(let toolName, _) = event {
            XCTAssertEqual(toolName, "read_file")
        } else {
            XCTFail("Expected .toolCallStarted, got \\(String(describing: event))")
        }
    }

    func testMapToolResponseMessage() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"toolResponse","id":"call_1","tool_result":"file contents here"}]},"token_state":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .toolCallFinished(let toolName, _) = event {
            XCTAssertEqual(toolName, "call_1")
        } else {
            XCTFail("Expected .toolCallFinished, got \\(String(describing: event))")
        }
    }

    // MARK: - Finish Events

    func testMapFinishEvent() {
        let json = """
        {"type":"Finish","reason":"stop","token_state":{"total_tokens":42}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .finalOutput(let content) = event {
            XCTAssertTrue(content.contains("Finish"))
            XCTAssertTrue(content.contains("stop"))
            XCTAssertTrue(content.contains("42"))
        } else {
            XCTFail("Expected .finalOutput, got \\(String(describing: event))")
        }
    }

    func testMapFinishEventWithoutTokens() {
        let json = """
        {"type":"Finish","reason":"stop"}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .finalOutput(let content) = event {
            XCTAssertTrue(content.contains("stop"))
        } else {
            XCTFail("Expected .finalOutput, got \\(String(describing: event))")
        }
    }

    // MARK: - Error Events

    func testMapErrorEvent() {
        let json = """
        {"type":"Error","error":"Provider not set"}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .error(let message) = event {
            XCTAssertEqual(message, "Provider not set")
        } else {
            XCTFail("Expected .error, got \\(String(describing: event))")
        }
    }

    // MARK: - Ignored Events

    func testMapPingReturnsNil() {
        let json = """
        {"type":"Ping"}
        """
        let event = GooseStreamEventMapper.map(json)
        XCTAssertNil(event, "Ping events should be silently ignored")
    }

    func testMapNotificationReturnsNil() {
        let json = """
        {"type":"Notification","request_id":"req_1","message":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        XCTAssertNil(event, "Notification events should be silently ignored")
    }

    func testMapUpdateConversationReturnsNil() {
        let json = """
        {"type":"UpdateConversation","conversation":{"messages":[]}}
        """
        let event = GooseStreamEventMapper.map(json)
        XCTAssertNil(event, "UpdateConversation events should be silently ignored")
    }

    func testMapActiveRequestsReturnsNil() {
        let json = """
        {"type":"ActiveRequests","request_ids":["req_1","req_2"]}
        """
        let event = GooseStreamEventMapper.map(json)
        XCTAssertNil(event, "ActiveRequests events should be silently ignored")
    }

    func testMapModelChangeReturnsNil() {
        let json = """
        {"type":"ModelChange","model":"default","mode":"lead"}
        """
        let event = GooseStreamEventMapper.map(json)
        XCTAssertNil(event, "ModelChange events should be silently ignored")
    }

    // MARK: - Edge Cases

    func testMapUnknownType() {
        let json = """
        {"type":"FutureEventType","data":"something"}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .unknown(let type, _) = event {
            XCTAssertEqual(type, "FutureEventType")
        } else {
            XCTFail("Expected .unknown, got \\(String(describing: event))")
        }
    }

    func testMapMalformedJSON() {
        let event = GooseStreamEventMapper.map("not json at all")
        if case .error(let message) = event {
            XCTAssertTrue(message.contains("Malformed"))
        } else {
            XCTFail("Expected .error, got \\(String(describing: event))")
        }
    }

    func testMapEmptyMessageContent() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[]}}
        """
        let event = GooseStreamEventMapper.map(json)
        // Empty content should produce a textChunk with empty text
        if case .textChunk(let text) = event {
            XCTAssertEqual(text, "")
        } else {
            XCTFail("Expected .textChunk with empty text, got \\(String(describing: event))")
        }
    }

    func testMapMessageWithThinkingContent() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"thinking","text":"Let me think about this..."}]}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .textChunk(let text) = event {
            XCTAssertTrue(text.contains("thinking"))
            XCTAssertTrue(text.contains("Let me think about this..."))
        } else {
            XCTFail("Expected .textChunk with thinking content, got \\(String(describing: event))")
        }
    }

    // MARK: - camelCase Tool Call Structure

    func testMapToolRequestWithCamelCaseToolCall() {
        let json = """
        {"type":"Message","message":{"role":"assistant","content":[{"type":"toolRequest","id":"call_2","toolCall":{"name":"write_file","arguments":{"path":"/tmp/out.txt","content":"hello"}}}]},"token_state":{}}
        """
        let event = GooseStreamEventMapper.map(json)
        if case .toolCallStarted(let toolName, _) = event {
            XCTAssertEqual(toolName, "write_file")
        } else {
            XCTFail("Expected .toolCallStarted, got \\(String(describing: event))")
        }
    }
}
