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

    @Test("Xcode runtime observations decode into structured inspector rows")
    func xcodeRuntimeObservationsDecodeIntoStructuredRows() throws {
        let agentExecutionID = try #require(UUID(uuidString: "11111111-1111-1111-1111-111111111111"))
        let observationJSON = """
        {
          "version": 1,
          "mcp_broker_observations": [{
            "source": "xcode_mcp_broker",
            "backend_start_disposition": "started",
            "pool_id": "pool-1",
            "lease_id": "lease-1",
            "xcode_pid": "4242",
            "backend_process_id": 5252,
            "xcode_home_disposition": "host_home",
            "xcode_tmpdir_disposition": "host_tmpdir",
            "simulator_selection": {"mode": "explicit", "simulator_id": "SIM-123"},
            "backend_initialize_wait_ms": 41,
            "backend_startup_latency_ms": 84,
            "backend_failure_class": null,
            "status_update": "active"
          }],
          "xcode_shim_events": [{
            "kind": "warning",
            "ts": "2026-04-21T12:00:00Z",
            "policy_reason": "residual_absolute_path",
            "source_field": "session_update",
            "matched_substring": "/usr/bin/xcodebuild",
            "excerpt": "ran /usr/bin/xcodebuild directly"
          }, {
            "kind": "warning",
            "ts": "2026-04-21T12:00:01Z",
            "policy_reason": "residual_absolute_path",
            "source_field": "session_update",
            "matched_substring": "/usr/bin/xcodebuild",
            "excerpt": "ran /usr/bin/xcodebuild directly"
          }],
          "xcode_host_executor_events": [{
            "ts": "2026-04-21T12:00:01Z",
            "tool": "xcodebuild",
            "argv": ["xcodebuild", "build"],
            "cwd": "/workspace",
            "host_env_disposition": "allowlist_applied",
            "env_allowlist_applied": ["SCHEME"],
            "env_dropped_from_provider": ["TOKEN"],
            "selected_simulator_id": "SIM-123",
            "exit_status": 0,
            "duration_ms": 120
          }],
          "storage": {
            "max_events": 1000,
            "max_bytes": 1048576,
            "truncated": false,
            "total_events_dropped": 0,
            "mcp_broker_observations_dropped": 0,
            "xcode_shim_events_dropped": 0,
            "xcode_host_executor_events_dropped": 0,
            "corrupt_json_recovery_count": 0,
            "corrupt_json_quarantined_bytes": 0
          }
        }
        """

        let stage = RunStageSnapshot(
            id: try #require(UUID(uuidString: "22222222-2222-2222-2222-222222222222")),
            stageID: "state_8_implementation_continued",
            label: "Implementation continued",
            startedAt: Date(timeIntervalSince1970: 100),
            completedAt: nil,
            status: .running,
            iteration: 1,
            attemptNumber: 1,
            recoverySnapshotJSON: nil,
            agentExecutions: [
                RunStageAgentSnapshot(
                    id: agentExecutionID,
                    agentID: "code_writer",
                    agentTitle: "Code Writer",
                    taskName: "continue_implementation",
                    agentAttemptNumber: 1,
                    supersedesAgentExecutionID: nil,
                    startedAt: Date(timeIntervalSince1970: 101),
                    completedAt: nil,
                    status: .running,
                    provider: "codex",
                    effort: "high",
                    runtimeSessionID: nil,
                    costCents: nil,
                    logSnippet: nil,
                    resolvedModel: nil,
                    providerReceiptPresent: false,
                    sessionLineageID: nil,
                    retryReason: nil,
                    canonicalOutcome: nil,
                    supervisionClassification: nil,
                    transportErrorKind: nil,
                    outputPresence: nil,
                    providerStopReason: nil,
                    actualXcodeRuntimeObservationJSON: Data(observationJSON.utf8)
                )
            ]
        )

        let observations = buildXcodeRuntimeObservations(from: [stage])
        let observation = try #require(observations.first)

        #expect(observations.count == 1)
        #expect(observation.latestBrokerObservation?.leaseID == "lease-1")
        #expect(observation.latestBrokerObservation?.backendProcessID == 5252)
        #expect(observation.selectedSimulatorID == "SIM-123")
        #expect(observation.shimWarnings.count == 2)
        #expect(observation.coalescedShimWarnings.count == 1)
        #expect(observation.shimWarnings.first?.matchedSubstring == "/usr/bin/xcodebuild")
        #expect(observation.hostExecutorEvents.first?.hostEnvDisposition == "allowlist_applied")
    }

    @Test("Xcode bridge progress status maps broker runtime states")
    func xcodeBridgeProgressStatusMapsBrokerRuntimeStates() throws {
        let waiting = xcodeObservation(
            broker: xcodeBrokerObservation(
                disposition: "queue_waiting",
                statusUpdate: "Waiting for Xcode MCP bridge lease capacity"
            )
        )
        let starting = xcodeObservation(
            broker: xcodeBrokerObservation(
                disposition: "lease_reserved",
                statusUpdate: "Reserved brokered Xcode MCP lease 'lease-1'"
            )
        )
        let actionRequired = xcodeObservation(
            broker: xcodeBrokerObservation(
                disposition: "initialize_blocked",
                failureClass: "xcode_mcp_action_required",
                statusUpdate: "Action Required: Check Xcode"
            )
        )

        #expect(waiting.bridgeProgressStatus?.label == "Waiting for Xcode Bridge lock")
        #expect(starting.bridgeProgressStatus?.label == "Starting Xcode Bridge")
        #expect(actionRequired.bridgeProgressStatus?.label == "Action Required: Check Xcode")
        #expect(
            latestXcodeBridgeProgressStatus(in: [waiting, actionRequired])?.kind == .actionRequired
        )
    }

    @Test("Xcode failure classes use friendly recovery text")
    func xcodeFailureClassesUseFriendlyRecoveryText() throws {
        let presentation = TimelineErrorPresentation(
            rawDetail: "xcode_mcp_initialize_timeout: timed out waiting for initialize"
        )

        #expect(presentation.summary == "Xcode bridge initialization timed out")
        #expect(presentation.highlights.first?.contains("Check for an Xcode consent modal") == true)
        #expect(XcodeRuntimeFriendlyFailure.first(in: "host_env_unavailable")?.title == "Host Xcode environment unavailable")
    }

    @Test("Agent catalog decodes Xcode infrastructure flags")
    func agentCatalogDecodesXcodeInfrastructureFlags() throws {
        let json = """
        {
          "id": "ui_reviewer",
          "title": "UI Reviewer",
          "mode": "review",
          "backend_profile": "gemini_review_pro",
          "permission_profile": "RO_REVIEW",
          "skill_ref": "ui_review",
          "inputs": [],
          "outputs": [],
          "requires_human_approval": false,
          "xcode_broker_required": true,
          "xcode_shim_injection_signal": true,
          "requires_xcode_host_execution": true,
          "prompt": "Review the UI."
        }
        """

        let agent = try JSONDecoder().decode(AgentDefinition.self, from: Data(json.utf8))

        #expect(agent.xcodeBrokerRequired == true)
        #expect(agent.xcodeShimInjectionSignal == true)
        #expect(agent.requiresXcodeHostExecution == true)
    }

    private func xcodeObservation(
        broker: WorkflowMapXcodeBrokerObservation
    ) -> WorkflowMapXcodeRuntimeObservation {
        WorkflowMapXcodeRuntimeObservation(
            id: UUID().uuidString,
            stageID: "implementation",
            stageLabel: "Implementation",
            agentExecutionID: UUID(),
            agentTitle: "Code Writer",
            brokerObservations: [broker],
            shimInvocations: [],
            shimWarnings: [],
            hostExecutorEvents: [],
            storage: WorkflowMapXcodeRuntimeStorageStatus(
                truncated: false,
                totalEventsDropped: 0,
                corruptJSONRecoveryCount: 0
            )
        )
    }

    private func xcodeBrokerObservation(
        disposition: String,
        failureClass: String? = nil,
        statusUpdate: String
    ) -> WorkflowMapXcodeBrokerObservation {
        WorkflowMapXcodeBrokerObservation(
            source: "xcode_mcp_broker",
            backendStartDisposition: disposition,
            poolID: "pool-1",
            leaseID: "lease-1",
            xcodePID: "4242",
            backendProcessID: nil,
            xcodeHomeDisposition: nil,
            xcodeTmpdirDisposition: nil,
            siblingLeasesAtSpawn: nil,
            backendInitializeWaitMilliseconds: nil,
            backendStartupLatencyMilliseconds: nil,
            backendFailureClass: failureClass,
            statusUpdate: statusUpdate,
            simulatorSelectionMode: nil,
            simulatorID: nil
        )
    }
}
