import Foundation
import Testing
@testable import Chainworks_Forge

@Suite("Runtime Timeline subscription recovery", .tags(.fast))
@MainActor
struct RuntimeTimelineSubscriptionRecoveryTests {
    private enum ProbeError: Error {
        case disconnected
    }

    @Test("Timeline reconnects after the live stream disconnects")
    func timelineReconnectsAfterDisconnect() async {
        var attempts = 0
        var shouldContinue = true
        var receivedEventIDs: [String] = []
        let event = P031RuntimeTimelineEventPresentation(
            id: "timeline-event-1",
            runID: "run-1",
            stageID: "review",
            agentID: "prepush_code_reviewer",
            provider: "codex_acp",
            eventKind: "meaningful_progress",
            title: "Agent thought",
            detail: "Reviewing changes",
            surfaceLabel: "agent_thought_chunk",
            sessionGenerationID: "session-generation-1",
            timestamp: Date(timeIntervalSince1970: 1_786_179_600),
            rawDetail: "Reviewing changes",
            rawDetailBytes: 17,
            rawDetailTruncated: false,
            rawDetailHandle: nil,
            rawDetailDigest: nil,
            fullRawAvailable: true,
            detailDigest: nil,
            detailCharCount: 17,
            chunkCount: 1,
            isStreaming: false,
            isTerminal: false,
            stateLabel: "review"
        )

        await P031RuntimeTimelineSubscriptionLoop.run(
            runID: "run-1",
            reconnectDelayNanoseconds: 0,
            shouldContinue: { shouldContinue },
            subscribe: { _ in
                attempts += 1
                if attempts == 1 {
                    return AsyncThrowingStream { continuation in
                        continuation.finish(throwing: ProbeError.disconnected)
                    }
                }
                return AsyncThrowingStream { continuation in
                    continuation.yield(event)
                    continuation.finish()
                }
            },
            onEvent: { received in
                receivedEventIDs.append(received.id)
                shouldContinue = false
            }
        )

        #expect(attempts == 2)
        #expect(receivedEventIDs == [event.id])
    }
}
