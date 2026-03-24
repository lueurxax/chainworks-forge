# Proposal 005: Goose Server Transport Adapter — Migrate from Bespoke REST to goose-server API Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/005-goose-server-transport-adapter.md` |
| Repository Root | `.` |
| Git SHA | `1b829e6` |
| Working Tree | `dirty` |
| Audited At | `2026-03-24T07:41:52+0200` |
| Proposal State | `Active (Draft, not superseded)` |
| Overall Status | `Partial` |

## Verdict

Proposal 005 is no longer blocked on missing transport implementation. The protocol extraction, `GooseServerTransport`, SSE mapper, runtime transport selection, `ChatRequest` encoding unit coverage, mock-HTTP round-trip coverage, fixture preservation, and live clean-room Goose proof all passed in this audit. The overall result is still `Partial`, not `Implemented`, because the proposal’s last explicit evidence gate remains open: this audit did not find or produce a fresh app-launched end-to-end Chainworks run against a real Goose server with real artifacts.

## Proposal Contract

### Scope
- Replace the bespoke live transport contract with a real `goosed agent` transport adapter.
- Extract a transport protocol so live execution can switch between bespoke, fixture, and real Goose server transports.
- Preserve fixture-backed execution behavior.
- Make Live mode work against a real locally running Goose server and real LLM provider.

### Locked Decisions
- `LOCKED-001`: transport protocol extraction before adding the new adapter.
- `LOCKED-002`: single-turn execution per session.
- `LOCKED-003`: system prompt is embedded in the user message.
- `LOCKED-004`: fixture mode is not touched.

### Acceptance Criteria
- `GooseTransportProtocol` exists and `GooseTransport`, `GooseServerTransport`, and `FixtureGooseTransport` all conform.
- `GooseSessionBridge`, `GooseAgentExecutor`, and `ExecutionService` depend on the protocol, not concrete `GooseTransport`.
- `GooseServerTransport.createSession()` uses `/agent/start` + `/agent/update_provider`.
- `GooseServerTransport.submitPrompt()` posts a valid `ChatRequest` with `metadata.userVisible` and `metadata.agentVisible` and returns a streamed `AsyncThrowingStream`.
- `GooseStreamEventMapper` maps all documented `MessageEvent` types.
- `ExecutionService` selects transport from `LiveRuntimeConfiguration.transportAPI`.
- Fixture mode still passes.
- The clean-room local Goose flow produces `Message` + `Finish`.
- One manual app live run completes end-to-end against a real Goose server with real artifacts.

### Test / Evidence Requirements
- Unit coverage for `GooseStreamEventMapper`.
- Unit coverage for `GooseServerTransport` `ChatRequest` encoding.
- Integration coverage for `GooseServerTransport` against a mock HTTP server.
- Integration coverage for unchanged fixture mode.
- Manual live workflow proof in the app.

### Explicit Exclusions
- No ACP adapter.
- No server lifecycle management.
- No provider/model selection UI.
- No multi-turn conversation.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Transport protocol exists and all three transports conform
- Proposal Source: `## 5.1 Component overview`, `## 5.2 Transport protocol extraction`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:187-214`, `docs/proposals/005-goose-server-transport-adapter.md:488`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseTransport.swift:11`
  - `Chainworks Forge/Engine/GooseTransport.swift:38`
  - `Chainworks Forge/Engine/FixtureGooseTransport.swift:6`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:22`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:151`
  - `xcodebuild ... -derivedDataPath /tmp/proposal005-audit.UQyxed/tests test ...` passed
- Gap / Note: `FixtureGooseTransport` now conforms directly instead of subclassing `GooseTransport`, but that still satisfies the acceptance contract.

### REQ-002 Session bridge, executor, and execution service depend on the protocol
- Proposal Source: `## 5.1 Component overview`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:196-200`, `docs/proposals/005-goose-server-transport-adapter.md:489`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:18-22`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:31`
  - `Chainworks Forge/Engine/ExecutionService.swift:287-310`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:241`
  - `xcodebuild ... /tmp/proposal005-audit.UQyxed/tests ...` passed with `ResumeManagerTests.testExecutionServiceUsesLiveExecutorForLiveWorkflow()`
- Gap / Note: None.

### REQ-003 `GooseServerTransport.createSession()` uses `/agent/start` + `/agent/update_provider` and returns a server-assigned session ID
- Proposal Source: `## 4.2 Session bootstrap`, `## 5.3 GooseServerTransport`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:79-107`, `docs/proposals/005-goose-server-transport-adapter.md:216-257`, `docs/proposals/005-goose-server-transport-adapter.md:490`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift:89-150`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:348`
  - `Chainworks ForgeTests/GooseServerLiveIntegrationTests.swift:80-106`
  - `/tmp/proposal005-liveprobe.deF9JY/start.json`
- Gap / Note: None.

### REQ-004 `submitPrompt()` builds a valid `ChatRequest` and returns a streamed async event sequence
- Proposal Source: `## 4.3 Message submission`, `## 5.3 GooseServerTransport`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:109-149`, `docs/proposals/005-goose-server-transport-adapter.md:224-257`, `docs/proposals/005-goose-server-transport-adapter.md:491`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift:153-315`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:222-280`
  - `Chainworks ForgeTests/GooseServerLiveIntegrationTests.swift:108-145`
  - `/tmp/proposal005-liveprobe.deF9JY/reply.sse`
- Gap / Note: The request body and streaming behavior are both covered now with direct test and runtime proof.

### REQ-005 `GooseStreamEventMapper` covers all documented `MessageEvent` variants
- Proposal Source: `## 4.4 SSE event types`, `## 8. Testing strategy`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:151-161`, `docs/proposals/005-goose-server-transport-adapter.md:303-311`, `docs/proposals/005-goose-server-transport-adapter.md:492`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseStreamEventMapper.swift:21-54`
  - `Chainworks ForgeTests/GooseStreamEventMapperTests.swift:17-188`
  - `xcodebuild ... /tmp/proposal005-audit.UQyxed/tests ...` passed with `GooseStreamEventMapperTests`
- Gap / Note: None.

### REQ-006 Runtime wiring selects transport via `LiveRuntimeConfiguration.transportAPI`
- Proposal Source: `## 5.1 Component overview`, `## 5.5 Configuration`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:198-200`, `docs/proposals/005-goose-server-transport-adapter.md:267-276`, `docs/proposals/005-goose-server-transport-adapter.md:493`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:8`
  - `Chainworks Forge/Engine/ExecutionService.swift:287-310`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:183-189`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:175-202`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:241`
- Gap / Note: None.

### REQ-007 Fixture mode continues to work unchanged behind the protocol
- Proposal Source: `## 5.1 Component overview`, `## 8. Testing strategy`, `## 10. Locked decisions`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:194-195`, `docs/proposals/005-goose-server-transport-adapter.md:303-311`, `docs/proposals/005-goose-server-transport-adapter.md:475-482`, `docs/proposals/005-goose-server-transport-adapter.md:494`)
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/FixtureGooseTransport.swift:1-55`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:480-581`
  - `Chainworks ForgeTests/EndToEndTests.swift:300`
  - `xcodebuild ... /tmp/proposal005-audit.UQyxed/tests ...` passed with `EndToEndTests.testLiveProposalLoopFixtureReachesApprovalAndCompletes()`
- Gap / Note: None.

### REQ-008 The clean-room local Goose flow produces `Message` + `Finish` with the documented timeout tolerance
- Proposal Source: `## 9.8 Verified clean-room operator flow`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:395-471`, `docs/proposals/005-goose-server-transport-adapter.md:495`)
- Status: `Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift:42`
  - `Chainworks ForgeTests/GooseServerLiveIntegrationTests.swift:80-170`
  - `xcodebuild ... -derivedDataPath /tmp/proposal005-audit.UQyxed/live test -only-testing:'Chainworks ForgeTests/GooseServerLiveIntegrationTests'` passed
  - `/tmp/proposal005-audit.UQyxed/live/Logs/Test/Test-Chainworks Forge-2026.03.24_07-38-45-+0200.xcresult`
  - `/tmp/proposal005-liveprobe.deF9JY/summary.txt` showed `ping=32`, `message=1`, `finish=1`, `error=0`
- Gap / Note: Clean-room Goose proof is now reproducible in this audit environment.

### REQ-009 One manual app live run completes end-to-end against a real Goose server with real artifacts
- Proposal Source: `## 3. Product question this proposal must answer`, `## 8. Testing strategy`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:50-59`, `docs/proposals/005-goose-server-transport-adapter.md:303-311`, `docs/proposals/005-goose-server-transport-adapter.md:496`)
- Status: `Not Verifiable`
- Evidence Type: `code`, `tests-found`, `inference`
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift:157-194`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:17-29`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:149-174`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:270-313`
- Gap / Note: The app shell and transport wiring exist, but current UI automation is still fixture-backed only (`CHAINWORKS_GOOSE_FIXTURE_MODE=proposal_loop_success`). This audit produced no fresh app-launched real-Goose run, no real-Goose UI screenshot pack, and no artifact bundle proving end-to-end completion through the app itself.

### REQ-010 A dedicated unit test proves `GooseServerTransport` encodes `ChatRequest` correctly
- Proposal Source: `## 8. Testing strategy` (`docs/proposals/005-goose-server-transport-adapter.md:307-308`)
- Status: `Implemented`
- Evidence Type: `tests-run`
- Evidence:
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:222-280`
  - `xcodebuild ... /tmp/proposal005-audit.UQyxed/tests ...` passed with `GooseServerTransportTests.testChatRequestEncodingContainsRequiredMetadata()`
- Gap / Note: The missing coverage from R1 is now present.

### REQ-011 An integration test exercises `GooseServerTransport` against a mock HTTP server
- Proposal Source: `## 8. Testing strategy` (`docs/proposals/005-goose-server-transport-adapter.md:309`)
- Status: `Implemented`
- Evidence Type: `tests-run`
- Evidence:
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:348-478`
  - `xcodebuild ... /tmp/proposal005-audit.UQyxed/tests ...` passed with `GooseServerTransportTests.testFullRoundTripCreateSubmitClose()`
- Gap / Note: The missing mock round-trip coverage from R1 is now present.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/005-goose-server-transport-adapter.md`
- `git rev-parse --show-toplevel`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete" 'docs/proposals/005-goose-server-transport-adapter.md' 'docs/proposals' 'docs/reviews'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/proposal005-audit.UQyxed/build build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/proposal005-audit.UQyxed/tests test -only-testing:'Chainworks ForgeTests/GooseServerTransportTests' -only-testing:'Chainworks ForgeTests/GooseStreamEventMapperTests' -only-testing:'Chainworks ForgeTests/GooseAgentExecutorTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests/testExecutionServiceUsesLiveExecutorForLiveWorkflow' -only-testing:'Chainworks ForgeTests/EndToEndTests/testLiveProposalLoopFixtureReachesApprovalAndCompletes'`
- Result bundle: `/tmp/proposal005-audit.UQyxed/tests/Logs/Test/Test-Chainworks Forge-2026.03.24_07-38-07-+0200.xcresult`
- `PATH="$HOME/.local/bin:$PATH" CHAINWORKS_LIVE_INTEGRATION_TEST=1 xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/proposal005-audit.UQyxed/live test -only-testing:'Chainworks ForgeTests/GooseServerLiveIntegrationTests'`
- Result bundle: `/tmp/proposal005-audit.UQyxed/live/Logs/Test/Test-Chainworks Forge-2026.03.24_07-38-45-+0200.xcresult`
- Clean-room live probe:
  - `curl -sk https://127.0.0.1:51200/status`
  - `curl -sk -X POST https://127.0.0.1:51200/agent/start ...`
  - `curl -sk -X POST https://127.0.0.1:51200/agent/update_provider ...`
  - `curl -sk -N --max-time 300 -X POST https://127.0.0.1:51200/reply ...`
  - `curl -sk -X DELETE https://127.0.0.1:51200/sessions/<id> ...`
  - Probe artifacts saved under `/tmp/proposal005-liveprobe.deF9JY`

## Recommended Next Actions

- Add one dedicated app-launched real-Goose proof path: launch the app with `CHAINWORKS_GOOSE_BASE_URL`, complete a real Live run, and preserve screenshots plus the produced artifact set.
- Keep the new clean-room live probe and `GooseServerLiveIntegrationTests` in the release checklist so future Goose server regressions are caught before transport changes ship.
