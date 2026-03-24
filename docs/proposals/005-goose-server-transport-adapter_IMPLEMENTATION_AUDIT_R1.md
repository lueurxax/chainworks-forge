# Proposal 005: Goose Server Transport Adapter — Migrate from Bespoke REST to goose-server API Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/005-goose-server-transport-adapter.md` |
| Repository Root | `.` |
| Git SHA | `59b28ea` |
| Working Tree | dirty |
| Audited At | `2026-03-23T23:53:26+02:00` |
| Proposal State | Active (Draft) |
| Overall Status | Not Implemented |

## Verdict

Proposal 005 is partly landed in current code: the protocol extraction, `GooseServerTransport`, `transportAPI` routing, fixture preservation, and event-mapper coverage are all present and targeted tests passed in this audit. The proposal still does not clear its own full contract because two explicit test/evidence requirements are missing (`ChatRequest` encoding unit coverage and mock-server round-trip coverage), the clean-room Goose flow did not produce `Message` + `Finish` in this audit even with a 300-second timeout, and there is no fresh app-launched real-Goose end-to-end proof with real artifacts.

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
- `GooseTransportProtocol` exists and `GooseTransport`, `GooseServerTransport`, and `FixtureGooseTransport` conform.
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
| Implemented | 7 |
| Partially Implemented | 1 |
| Missing | 2 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Transport protocol exists and all three transports conform
- Proposal Source: `## 5.1 Component overview`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:191-199`, `docs/proposals/005-goose-server-transport-adapter.md:488`)
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseTransport.swift:3-20`
  - `Chainworks Forge/Engine/GooseTransport.swift:38-38`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:21-21`
  - `Chainworks Forge/Engine/FixtureGooseTransport.swift:4-6`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:52-74`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-audit005-tests test ...` passed
- Gap / Note: `FixtureGooseTransport` now conforms directly instead of subclassing `GooseTransport`, but it still satisfies the proposal’s explicit acceptance contract.

### REQ-002 Session bridge, executor, and execution service depend on the protocol
- Proposal Source: `## 5.1 Component overview`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:196-200`, `docs/proposals/005-goose-server-transport-adapter.md:489`)
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:17-24`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:29-35`
  - `Chainworks Forge/Engine/ExecutionService.swift:282-313`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:241-296`
  - `xcodebuild ... test` passed with `ResumeManagerTests.testExecutionServiceUsesLiveExecutorForLiveWorkflow()`
- Gap / Note: None.

### REQ-003 `GooseServerTransport.createSession()` uses `/agent/start` + `/agent/update_provider` and returns a server-assigned session ID
- Proposal Source: `## 4.2 Session bootstrap`, `## 5.3 GooseServerTransport`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:79-107`, `docs/proposals/005-goose-server-transport-adapter.md:218-223`, `docs/proposals/005-goose-server-transport-adapter.md:490`)
- Status: Implemented
- Evidence Type: `code`, `runtime`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift:71-133`
  - Fresh live probe on local `goosed agent` 1.28.0:
    - `POST /agent/start` returned session `20260323_2`
    - `POST /agent/update_provider` returned HTTP 200
- Gap / Note: None.

### REQ-004 `submitPrompt()` builds a valid `ChatRequest` and returns a streamed async event sequence
- Proposal Source: `## 4.3 Message submission`, `## 5.3 GooseServerTransport`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:109-149`, `docs/proposals/005-goose-server-transport-adapter.md:224-257`, `docs/proposals/005-goose-server-transport-adapter.md:491`)
- Status: Implemented
- Evidence Type: `code`, `runtime`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift:135-217`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:254-297`
  - Fresh live probe accepted the posted `/reply` request shape and streamed SSE heartbeat frames instead of rejecting with `422` or schema errors.
- Gap / Note: The behavior contract is implemented, but Proposal 005’s separate dedicated unit-test requirement for request encoding is still unfulfilled; that gap is tracked in `REQ-010`.

### REQ-005 `GooseStreamEventMapper` covers all documented `MessageEvent` variants
- Proposal Source: `## 4.4 SSE event types`, `## 8. Testing strategy`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:151-162`, `docs/proposals/005-goose-server-transport-adapter.md:307`, `docs/proposals/005-goose-server-transport-adapter.md:492`)
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseStreamEventMapper.swift:5-59`
  - `Chainworks Forge/Engine/GooseStreamEventMapper.swift:64-167`
  - `Chainworks ForgeTests/GooseStreamEventMapperTests.swift:13-186`
  - `xcodebuild ... test` passed with `GooseStreamEventMapperTests`
- Gap / Note: None.

### REQ-006 Runtime wiring selects transport via `LiveRuntimeConfiguration.transportAPI`
- Proposal Source: `## 5.1 Component overview`, `## 5.5 Configuration`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:198-200`, `docs/proposals/005-goose-server-transport-adapter.md:267-276`, `docs/proposals/005-goose-server-transport-adapter.md:493`)
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:5-27`
  - `Chainworks Forge/Engine/ExecutionService.swift:282-313`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:157-198`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:76-119`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:271-296`
- Gap / Note: None.

### REQ-007 Fixture mode continues to work unchanged behind the protocol
- Proposal Source: `## 5.1 Component overview`, `## 8. Testing strategy`, `## 10. Locked decisions`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:195`, `docs/proposals/005-goose-server-transport-adapter.md:310`, `docs/proposals/005-goose-server-transport-adapter.md:482`, `docs/proposals/005-goose-server-transport-adapter.md:494`)
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/FixtureGooseTransport.swift:3-58`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:121-185`
  - `Chainworks ForgeTests/EndToEndTests.swift:300-399`
  - `xcodebuild ... test` passed with `EndToEndTests.testLiveProposalLoopFixtureReachesApprovalAndCompletes()`
- Gap / Note: None.

### REQ-008 The clean-room local Goose flow produces `Message` + `Finish` with the documented timeout tolerance
- Proposal Source: `## 9.8 Verified clean-room operator flow`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:395-471`, `docs/proposals/005-goose-server-transport-adapter.md:495`)
- Status: Partially Implemented
- Evidence Type: `code`, `runtime`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift:42-43`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:60-66`
  - Fresh clean-room probe on a separate local `goosed agent` instance:
    - `/status` returned `ok`
    - `/agent/start` succeeded
    - `/agent/update_provider` succeeded
    - first `/reply` timed out after 300s with `600` `Ping` events and `0` `Message` / `Finish`
    - warm-session retry timed out after 120s with `240` `Ping` events and `0` `Message` / `Finish`
- Gap / Note: The transport timeout is implemented, but the proposal’s success criterion for the clean-room flow did not reproduce in this audit environment.

### REQ-009 One manual app live run completes end-to-end against a real Goose server with real artifacts
- Proposal Source: `## 3. Product question this proposal must answer`, `## 8. Testing strategy`, `## 11. Acceptance criteria` (`docs/proposals/005-goose-server-transport-adapter.md:52-59`, `docs/proposals/005-goose-server-transport-adapter.md:311`, `docs/proposals/005-goose-server-transport-adapter.md:496`)
- Status: Not Verifiable
- Evidence Type: `code`, `inference`
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift:157-198`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:13-30`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:149-187`
- Gap / Note: Current UI proof covers fixture-backed live execution only. This audit found no fresh app-launched real-Goose run, no real-Goose UI artifact pack, and no persisted artifact bundle proving end-to-end completion against a live server.

### REQ-010 A dedicated unit test proves `GooseServerTransport` encodes `ChatRequest` correctly
- Proposal Source: `## 8. Testing strategy` (`docs/proposals/005-goose-server-transport-adapter.md:307-309`)
- Status: Missing
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:7-185`
  - `xcodebuild ... test` passed with `GooseServerTransportTests`
- Gap / Note: The current test file covers raw values, initialization, protocol conformance, runtime config, and fixture behavior only. It does not assert the `/reply` request body or verify `metadata.userVisible` / `metadata.agentVisible`.

### REQ-011 An integration test exercises `GooseServerTransport` against a mock HTTP server
- Proposal Source: `## 8. Testing strategy` (`docs/proposals/005-goose-server-transport-adapter.md:309`)
- Status: Missing
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `rg -n "URLProtocol|mock server|Mock.*Server|HTTPServer|LocalServer|/agent/start|/reply|update_provider|X-Secret-Key|metadata.userVisible|agentVisible" 'Chainworks ForgeTests'` returned no matching mock-server integration test coverage.
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:7-185`
- Gap / Note: There is no repository test that proves create → update_provider → reply → close against a controllable HTTP test server.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/005-goose-server-transport-adapter.md`
- `rg -n -i 'superseded|deprecated|replaced by|obsolete' 'docs/proposals/005-goose-server-transport-adapter.md' docs/proposals docs/reviews`
- `rg -n "gooseServer|transportAPI|live.*run|GooseServer|CHAINWORKS_GOOSE_BASE_URL|Start Run|Live" 'Chainworks ForgeUITests' 'Chainworks ForgeTests'`
- `rg -n "URLProtocol|mock server|Mock.*Server|HTTPServer|LocalServer|/agent/start|/reply|update_provider|X-Secret-Key|metadata.userVisible|agentVisible" 'Chainworks ForgeTests'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-audit005-build build` — passed
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-audit005-tests test -only-testing:'Chainworks ForgeTests/GooseServerTransportTests' -only-testing:'Chainworks ForgeTests/GooseStreamEventMapperTests' -only-testing:'Chainworks ForgeTests/GooseAgentExecutorTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests/testExecutionServiceUsesLiveExecutorForLiveWorkflow' -only-testing:'Chainworks ForgeTests/EndToEndTests/testLiveProposalLoopFixtureReachesApprovalAndCompletes'` — passed
- Test result bundle: `/tmp/codex-dd-audit005-tests/Logs/Test/Test-Chainworks Forge-2026.03.23_23-44-01-+0200.xcresult`
- Clean-room Goose runtime checks:
  - `PATH="$HOME/.local/bin:$PATH" GOOSE_SERVER__SECRET_KEY=chainworks-dev-secret GOOSE_PORT=51210 /Applications/Goose.app/Contents/Resources/bin/goosed agent`
  - `curl -sk https://127.0.0.1:51210/status`
  - `curl -sk -X POST https://127.0.0.1:51210/agent/start ...`
  - `curl -sk -X POST https://127.0.0.1:51210/agent/update_provider ...`
  - `curl -skN --max-time 300 -X POST https://127.0.0.1:51210/reply ...`
  - `curl -skN --max-time 120 -X POST https://127.0.0.1:51210/reply ...`
  - Probe artifacts saved under `/tmp/proposal005-live-probe/`

## Recommended Next Actions

- Add a dedicated `GooseServerTransport` unit test that inspects the encoded `/reply` payload and asserts `metadata.userVisible` / `metadata.agentVisible`.
- Add a mock-HTTP integration test that drives `createSession()` and `submitPrompt()` end-to-end without depending on a live Goose installation.
- Capture a fresh app-launched real-Goose run with persisted artifacts, or add a dedicated UI/manual harness that proves Proposal 005’s app-level acceptance path.
- Investigate why the current local `goosed` clean-room probe remained Ping-only for both the 300-second cold attempt and the 120-second warm retry.
