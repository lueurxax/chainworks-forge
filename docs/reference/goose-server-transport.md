# Goose Server Transport

Status: **Implemented** (proven against real Goose.app -- 2026-03-24)

## Purpose

The Goose server transport adapter connects Chainworks Forge to a locally
running `goosed agent` instance, enabling live LLM execution through the
real goosed API. Fixture-backed execution remains unchanged for testing.

This document is the canonical reference for the transport protocol
abstraction, the `GooseServerTransport` adapter, the SSE event mapper, the
goosed API contract, configuration, and operational setup.

---

## Table of Contents

1. [Transport Protocol](#1-transport-protocol)
2. [goosed API Contract](#2-goosed-api-contract)
3. [GooseServerTransport Implementation](#3-gooseservertransport-implementation)
4. [Configuration](#4-configuration)
5. [Running goosed Locally](#5-running-goosed-locally)
6. [Operational Notes](#6-operational-notes)
7. [Architecture Decisions](#7-architecture-decisions)
8. [Exclusions](#8-exclusions)
9. [Source File Index](#9-source-file-index)
10. [Verification Standard](#10-verification-standard)

## Related Docs

- [live-provider-execution-slice.md](live-provider-execution-slice.md) -- fixture-backed transport baseline
- [runtime-contract.md](runtime-contract.md)
- [architecture-decisions.md](architecture-decisions.md)
- [workspace-isolation-risk.md](workspace-isolation-risk.md)

---

## 1. Transport Protocol

All transport implementations conform to a single protocol:

```swift
protocol GooseTransportProtocol: Sendable {
    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse
    func submitPrompt(sessionID: String, prompt: GoosePromptRequest) -> AsyncThrowingStream<GooseStreamEvent, Error>
    func closeSession(sessionID: String) async throws
}
```

Three conforming implementations exist:

| Transport | API Contract | Auth | Use Case |
|---|---|---|---|
| `GooseServerTransport` | `/agent/start` + `/reply` (real goosed) | `X-Secret-Key` | Live LLM execution |
| `FixtureGooseTransport` | In-memory | None | Deterministic testing |
| `GooseTransport` | `/api/sessions` (bespoke) | `Authorization: Bearer` | Legacy, not server-implemented |

All consumers (`GooseSessionBridge`, `GooseAgentExecutor`, `ExecutionService`)
depend on the protocol, not concrete types. No `if/else` branching on transport
type exists in the engine.

---

## 2. goosed API Contract

Source: `crates/goose-server/src/routes/` in `block/goose` v1.28.

### 2.1 Session bootstrap

Sessions must be explicitly created before the first prompt. Two sequential
calls are required:

**Step 1 -- Create session:** `POST /agent/start`

```json
{ "working_dir": "/path/to/workspace" }
```

Returns a `Session` object with an `id` field. The working directory must
exist on disk; otherwise extensions fail to load and the session may be
unusable.

**Step 2 -- Set provider (required):** `POST /agent/update_provider`

```json
{
  "session_id": "<id from step 1>",
  "provider": "claude-code",
  "model": "default"
}
```

Without this step, `/reply` returns `{"type":"Error","error":"Provider not set"}`.

The provider value must match an entry in `~/.config/goose/config.yaml`.

### 2.2 Prompt submission

**Endpoint:** `POST /reply`

```json
{
  "session_id": "<id>",
  "user_message": {
    "role": "user",
    "created": 1711234567,
    "content": [{ "type": "text", "text": "..." }],
    "metadata": { "userVisible": true, "agentVisible": true }
  }
}
```

Both `metadata.userVisible` and `metadata.agentVisible` are required.
Omitting them causes HTTP 422.

The system prompt is embedded in the user message content. goosed has no
separate system prompt endpoint per session. This matches how the Goose
desktop app works.

**Response:** SSE stream (`Content-Type: text/event-stream`). Each event is a
`data:` line containing JSON.

### 2.3 SSE event types

| Type | Meaning | App Mapping |
|---|---|---|
| `Message` (Text content) | Agent produced text | `.textChunk` |
| `Message` (ToolRequest) | Agent wants to call a tool | `.toolCallStarted` |
| `Message` (ToolResponse) | Tool returned a result | `.toolCallFinished` |
| `Message` (Thinking) | Internal reasoning | `.textChunk` with `[thinking]` prefix |
| `Finish` | Stream complete | `.finalOutput` + `.sessionClosed` |
| `Error` | Agent error | `.error` |
| `Ping` | Heartbeat (every 500ms) | Silently ignored |
| `ModelChange` | Model switch notification | Silently ignored |
| `Notification` | MCP notification | Silently ignored |
| `UpdateConversation` | Full conversation replacement | Silently ignored |
| `ActiveRequests` | In-flight request IDs | Silently ignored |

`GooseStreamEventMapper` handles the JSON-to-enum mapping.
`GooseServerTransport` additionally emits synthetic `.sessionStarted` and
`.promptSubmitted` events at stream start, and `.sessionClosed` after `Finish`
or stream end.

### 2.4 Session closure

`DELETE /sessions/{id}` deletes the session and cancels in-flight requests.

### 2.5 Authentication

All endpoints except `/status` and `/features` require the header:

```
X-Secret-Key: {secret}
```

This is a custom header, not `Authorization: Bearer`.

### 2.6 TLS

`goosed` uses a self-signed certificate by default. `GooseServerTransport`
includes a `LocalhostTrustDelegate` that trusts localhost self-signed TLS
certificates in development.

---

## 3. GooseServerTransport Implementation

### 3.1 Session creation

`createSession()` performs two sequential HTTP calls:

1. `POST /agent/start` with the workspace root directory
2. `POST /agent/update_provider` with the configured provider and model

Returns a `GooseSessionResponse` with the server-assigned session ID.

### 3.2 Prompt submission

`submitPrompt()` constructs a `ChatRequest` with the full execution packet
(system prompt + task directive + context) as the user message, then streams
SSE events via byte-by-byte line parsing. Each `data:` line is passed to
`GooseStreamEventMapper.map()`.

### 3.3 Timeouts

The transport uses a 300-second request timeout and 600-second resource
timeout. This accommodates cold-start latency when the `claude-code` provider
spawns its CLI subprocess (observed: 30--120+ seconds on first invocation,
10--20 seconds on subsequent turns).

### 3.4 Error salvage

If the SSE stream fails but output files exist on disk, `GooseAgentExecutor`
salvages them and generates an execution receipt for audit evidence. This
handles the case where goosed agents write files via developer tools before
the stream completes.

---

## 4. Configuration

### 4.1 Transport API enum

```swift
enum GooseTransportAPI: String, Codable, Sendable {
    case bespoke
    case gooseServer = "goose_server"
}
```

### 4.2 Runtime configuration

```swift
struct LiveRuntimeConfiguration: Sendable {
    let baseURL: URL
    let apiKey: String?
    let override: LiveExecutionOverride?
    let transportMode: LiveTransportMode    // .network | .fixtureProposalLoopSuccess
    let transportAPI: GooseTransportAPI     // .bespoke | .gooseServer
}
```

### 4.3 Transport selection

`ExecutionService.executorForRun()` selects the transport:

- `transportMode == .fixtureProposalLoopSuccess` --> `FixtureGooseTransport`
- `transportMode == .network`, `transportAPI == .gooseServer` --> `GooseServerTransport`
- `transportMode == .network`, `transportAPI == .bespoke` --> `GooseTransport`

### 4.4 Environment variables

| Variable | Purpose | Default |
|---|---|---|
| `CHAINWORKS_GOOSE_FIXTURE_MODE` | `proposal_loop_success` to enable fixture backend | (disabled) |
| `CHAINWORKS_GOOSE_BASE_URL` | Goose server URL (e.g. `https://127.0.0.1:51200`) | (required for network mode) |
| `CHAINWORKS_GOOSE_API_KEY` | Maps to `X-Secret-Key` header | (optional) |
| `CHAINWORKS_GOOSE_TRANSPORT_API` | `bespoke` or `goose_server` | `goose_server` when base URL is set |
| `CHAINWORKS_LIVE_PROVIDER` | Override provider | (from agent definition) |
| `CHAINWORKS_LIVE_MODEL` | Override model | (from agent definition) |
| `CHAINWORKS_LIVE_EFFORT` | Override effort | (from agent definition) |

When `CHAINWORKS_GOOSE_FIXTURE_MODE=proposal_loop_success` is set, the app
uses `FixtureGooseTransport` and ignores the base URL entirely.

### 4.5 Xcode scheme for real server mode

| Name | Value |
|---|---|
| `CHAINWORKS_GOOSE_BASE_URL` | `https://127.0.0.1:51200` |
| `CHAINWORKS_GOOSE_API_KEY` | `chainworks-dev-secret` |
| `CHAINWORKS_GOOSE_TRANSPORT_API` | `goose_server` |

---

## 5. Running goosed Locally

### 5.1 Server binary

The HTTP server binary is `goosed` inside `Goose.app`:

```
/Applications/Goose.app/Contents/Resources/bin/goosed
```

This is not the `goose` CLI at `/opt/homebrew/bin/goose`.

### 5.2 Starting a standalone instance

```bash
PATH="$HOME/.local/bin:$PATH" \
GOOSE_SERVER__SECRET_KEY=chainworks-dev-secret \
GOOSE_PORT=51200 \
/Applications/Goose.app/Contents/Resources/bin/goosed agent
```

The server starts on `https://127.0.0.1:51200` with a self-signed TLS
certificate.

`PATH` must include `~/.local/bin` (or wherever the `claude` binary is
installed). Without it, the `claude-code` provider silently hangs on first
invocation.

### 5.3 Server configuration

| Environment Variable | Default | Purpose |
|---|---|---|
| `GOOSE_PORT` | `3000` | HTTP listen port |
| `GOOSE_HOST` | `127.0.0.1` | Bind address |
| `GOOSE_TLS` | `true` | Enable self-signed TLS |
| `GOOSE_SERVER__SECRET_KEY` | random 32-byte hex | Auth token for `X-Secret-Key` |

Provider and model configuration: `~/.config/goose/config.yaml` (set via
`goose configure`).

### 5.4 Verification

```bash
# Check server is up
curl -sk https://127.0.0.1:51200/status
# Expected: "ok"

# Check auth works
curl -sk -H "X-Secret-Key: chainworks-dev-secret" https://127.0.0.1:51200/sessions
# Expected: []
```

### 5.5 End-to-end probe

```bash
# Create session
curl -sk -X POST https://127.0.0.1:51200/agent/start \
  -H "X-Secret-Key: chainworks-dev-secret" \
  -H "Content-Type: application/json" \
  -d '{"working_dir": "/tmp/chainworks-test"}'
# Returns: {"id": "20260324_N", ...}

# Set provider (required)
curl -sk -X POST https://127.0.0.1:51200/agent/update_provider \
  -H "X-Secret-Key: chainworks-dev-secret" \
  -H "Content-Type: application/json" \
  -d '{"session_id": "<ID>", "provider": "claude-code", "model": "default"}'

# Submit prompt (300s timeout for cold-start)
curl -sk -N --max-time 300 -X POST https://127.0.0.1:51200/reply \
  -H "X-Secret-Key: chainworks-dev-secret" \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "<ID>",
    "user_message": {
      "role": "user",
      "created": 1711234567,
      "content": [{"type": "text", "text": "Reply with exactly: hello world today"}],
      "metadata": {"userVisible": true, "agentVisible": true}
    }
  }'
# Expected: Ping heartbeats, then Message event, then Finish event

# Clean up
curl -sk -X DELETE https://127.0.0.1:51200/sessions/<ID> \
  -H "X-Secret-Key: chainworks-dev-secret"
```

### 5.6 Goose.app port conflict

Goose.app spawns its own `goosed agent` on port 51115 with a randomly
generated secret. For Chainworks Forge, start a separate instance on a
different port (Section 5.2).

---

## 6. Operational Notes

### 6.1 Cold-start latency

The `claude-code` provider spawns a Claude CLI subprocess on first
invocation. Observed cold-start times: ~70 seconds on a warm machine,
30--120+ seconds range depending on machine state. Subsequent turns on the
same session: 10--20 seconds. `Ping` heartbeats every 500ms indicate the
server is alive during cold-start.

### 6.2 Discovery mechanism for tests

Live integration tests use `/private/tmp/chainworks_goose_discovery.json`
to bridge Goose environment variables into the Xcode test sandbox:

```json
{ "port": "51200", "secret_key": "chainworks-dev-secret" }
```

---

## 7. Architecture Decisions

| ID | Decision | Rationale |
|---|---|---|
| LOCKED-001 | Transport protocol extraction is mandatory | Both transports are interchangeable without `if/else` branching |
| LOCKED-002 | Single-turn execution per session | Matches the engine's one-agent-one-task model; no conversation state leaks between agents |
| LOCKED-003 | System prompt is embedded in the user message | goosed has no separate system prompt endpoint; this is how its desktop client works |
| LOCKED-004 | Fixture mode is not touched | Fixture transport continues to work unchanged behind the protocol |

---

## 8. Exclusions

| Exclusion | Owner |
|---|---|
| ACP adapter | Future work (when ACP stabilizes) |
| Server lifecycle management | Operator (manual start) |
| Provider/model selection UI | Provider settings proposal |
| Multi-turn conversation | Not planned |

---

## 9. Source File Index

**Implementation:**

| File | Role |
|---|---|
| `Chainworks Forge/Engine/GooseTransport.swift` | Protocol definition + bespoke transport |
| `Chainworks Forge/Engine/GooseServerTransport.swift` | Real goosed adapter |
| `Chainworks Forge/Engine/GooseStreamEventMapper.swift` | SSE event mapping |
| `Chainworks Forge/Engine/FixtureGooseTransport.swift` | Fixture backend |
| `Chainworks Forge/Engine/GooseSessionBridge.swift` | Session lifecycle |
| `Chainworks Forge/Engine/GooseAgentExecutor.swift` | Execution orchestration |
| `Chainworks Forge/Engine/ExecutionEventBridge.swift` | Event processing |
| `Chainworks Forge/Engine/ExecutionService.swift` | Transport selection and runtime config |
| `Chainworks Forge/Chainworks_ForgeApp.swift` | Environment variable loading |

**Tests:**

| File | Role |
|---|---|
| `Chainworks ForgeTests/GooseServerTransportTests.swift` | Unit + mock HTTP round-trip |
| `Chainworks ForgeTests/GooseStreamEventMapperTests.swift` | Event mapper coverage |
| `Chainworks ForgeTests/GooseServerLiveIntegrationTests.swift` | Live integration |
| `Chainworks ForgeTests/LiveGooseConnectionProofTests.swift` | Real Goose connection proof |
| `Chainworks ForgeTests/GooseAgentExecutorTests.swift` | Executor tests |
| `Chainworks ForgeTests/GooseSessionBridgeTests.swift` | Session bridge tests |

---

## 10. Verification Standard

The proof standard for this transport layer:

- `GooseStreamEventMapper` maps all documented `MessageEvent` variants (unit tests)
- `GooseServerTransport` encodes `ChatRequest` with required metadata fields (unit test)
- Full round-trip create/submit/close against a mock HTTP server (integration test)
- Fixture mode passes all existing tests unchanged
- Clean-room local `goosed` flow produces `Message` + `Finish` events
- App-launched real Goose connection proof: session creation, prompt submission, SSE response, clean session closure (evidence: [`docs/evidence/goose-server-transport-verification.md`](../evidence/goose-server-transport-verification.md), raw data: [`docs/evidence/live_goose_connection_proof.json`](../evidence/live_goose_connection_proof.json))
