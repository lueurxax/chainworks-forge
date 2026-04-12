import Testing
import Foundation
@testable import Chainworks_Forge

@Suite("RunTimelineInspectorView", .tags(.fast))
struct RunTimelineInspectorViewTests {
    @Test("Focused timeline spine merges live and persisted entries into one sorted stream")
    func focusedTimelineSpineMergesLiveAndPersistedEntries() {
        let liveEntry = LiveExecutionTimelineEntry(
            agentID: "code_writer",
            agentTitle: "Code Writer",
            stageID: "state_8_implementation_continued",
            event: ExecutionEvent(
                type: .toolCallFinished,
                timestamp: Date(timeIntervalSince1970: 200),
                detail: "Tool completed: edit"
            )
        )
        let persistedEntry = WorkflowMapPersistedTimelineEntry(
            id: "persisted-1",
            title: "Implementation continued",
            detail: "Persisted automatic watchdog retry exhausted",
            timestamp: Date(timeIntervalSince1970: 100),
            sessionID: "persisted-session-1"
        )

        let spine = buildFocusedTimelineSpineEntries(
            liveTimeline: [liveEntry],
            persistedTimeline: [persistedEntry]
        )

        #expect(spine.count == 2)
        #expect(spine.first?.id == liveEntry.id.uuidString)
        #expect(spine.first?.surfaceLabel == "tool_call_finished")
        #expect(spine.last?.id == persistedEntry.id)
        #expect(spine.last?.surfaceLabel == "persisted")
        #expect(spine.last?.sessionID == "persisted-session-1")
    }
}
