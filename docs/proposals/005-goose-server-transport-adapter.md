# Proposal 005: Goose Server Transport Adapter — Migrate from Bespoke REST to goose-server API

| Field | Value |
|---|---|
| Date | 2026-03-23 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 004 (Live Provider Execution Slice — GooseTransport, GooseSessionBridge, GooseAgentExecutor) |
| Goal | Connect the app to a **real running `goosed agent`** so Live mode works against an actual LLM provider |

---

## 1. Context

Proposal 004 established the live execution stack: `GooseTransport` → `GooseSessionBridge` → `GooseAgentExecutor`. That stack defines a clean HTTP contract:

- `POST /api/sessions` — create session
- `POST /api/sessions/{id}/messages` — submit prompt, stream SSE
- `DELETE /api/sessions/{id}` — close session

The problem: **no server implements this contract.** The app designed a bespoke API that never existed outside `FixtureGooseTransport`.

Meanwhile, the real Goose project (`block/goose` v1.28) ships `goosed`, a production HTTP server (binary at `/Applications/Goose.app/Contents/Resources/bin/goosed`) with a different but functionally equivalent API:

- `POST /agent/start` — create a new session with a working directory
- `POST /agent/update_provider` — set provider/model on a session (**required** before first prompt)
- `POST /reply` — submit a chat message, stream SSE
- `GET /sessions` — list sessions
- `DELETE /sessions/{id}` — delete session

The server uses `goosed agent` to start, runs on a configurable port, and streams `MessageEvent` objects over SSE — not the raw `event:` / `data:` format the app currently parses.

This proposal bridges the gap: adapt `GooseTransport` to speak the real `goosed` API so the app can run live workflows against an actual LLM provider.

---

## 2. Why this proposal now

Fixture mode proves the engine works. But:

- The `goosed` binary is **the only production-ready HTTP interface** to Goose's agent runtime.
- The new `goose serve` / ACP protocol is **not yet stable** and is still being developed.
- Waiting for ACP stabilization means waiting indefinitely — `goosed agent` **works today**.
- The Goose desktop app already uses `goosed agent` internally — the API surface is battle-tested.

The fastest path from fixture to real execution is: adapt the transport to speak the API that actually exists.

---

## 3. Product question this proposal must answer

The proposal succeeds if:

1. The engineer starts `goosed agent` locally (Section 9.8).
2. The engineer sets `CHAINWORKS_GOOSE_BASE_URL=https://127.0.0.1:51200` and `CHAINWORKS_GOOSE_API_KEY` in the Xcode scheme.
3. The engineer launches the app, creates an idea, starts a Live run.
4. The run executes against a **real LLM** through the local Goose server.
5. Real artifacts are produced, approval gates work, the run completes.
6. Fixture mode continues to work unchanged for testing without a server.

---

## 4. What we know about goosed's API

Source: `crates/goose-server/src/routes/` in `block/goose` (inspected at v1.28).

### 4.1 Session management

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/sessions` | List all sessions |
| `GET` | `/sessions/{id}` | Get session with conversation history |
| `DELETE` | `/sessions/{id}` | Delete session and cancel in-flight requests |
| `GET` | `/sessions/{id}/export` | Export session as JSON |
| `POST` | `/sessions/import` | Import session from JSON |
| `PUT` | `/sessions/{id}/name` | Rename session |
| `POST` | `/sessions/{id}/fork` | Fork/copy session |

### 4.2 Session bootstrap (critical — sessions are NOT implicit)

**Live probes proved that `/reply` with an unknown `session_id` returns `Failed to read session: Session not found`.** Sessions must be explicitly created before the first prompt.

**Step 1 — Create session:** `POST /agent/start`

```json
{
  "working_dir": "/path/to/workspace"
}
```

Response: full `Session` object with `id`, `working_dir`, `name`, `extension_data`, etc.

**Step 2 — Set provider (required):** `POST /agent/update_provider`

```json
{
  "session_id": "<id from step 1>",
  "provider": "claude-code",
  "model": "default"
}
```

Without this step, `/reply` returns `{"type":"Error","error":"Provider not set"}` even with a valid session.

The provider value must match an entry in `~/.config/goose/config.yaml` (e.g. `GOOSE_PROVIDER: claude-code`).

**Step 3 — Submit prompt:** `POST /reply`

### 4.3 Message submission (the core endpoint)

**Endpoint:** `POST /reply`

**Request body (`ChatRequest`):**
```json
{
  "session_id": "<id from /agent/start>",
  "user_message": {
    "role": "user",
    "created": 1711234567,
    "content": [
      { "type": "text", "text": "..." }
    ],
    "metadata": {
      "userVisible": true,
      "agentVisible": true
    }
  },
  "override_conversation": null,
  "recipe_name": null,
  "recipe_version": null
}
```

**Required fields in `user_message`:**
- `role` — must be `"user"`
- `created` — Unix timestamp (integer)
- `content` — array of `MessageContent` objects
- `metadata.userVisible` — boolean (**required; omitting causes HTTP 422**)
- `metadata.agentVisible` — boolean (**required; omitting causes HTTP 422**)

**Response:** SSE stream (`Content-Type: text/event-stream`)

Each event is a single `data:` line containing JSON:
```
data: {"type":"Message","message":{...},"token_state":{...}}
data: {"type":"Ping"}
data: {"type":"Finish","reason":"stop","token_state":{...}}
data: {"type":"Error","error":"..."}
```

### 4.4 SSE event types (`MessageEvent`)

| Type | Payload | Meaning |
|---|---|---|
| `Message` | `{ message: Message, token_state: TokenState }` | Agent produced a message (text, tool call, or tool response) |
| `Error` | `{ error: String }` | Agent error |
| `Finish` | `{ reason: String, token_state: TokenState }` | Stream complete |
| `Notification` | `{ request_id, message: ServerNotification }` | MCP notification |
| `UpdateConversation` | `{ conversation: Conversation }` | Full conversation replacement |
| `Ping` | `{}` | Heartbeat (every 500ms) |
| `ActiveRequests` | `{ request_ids: [String] }` | In-flight request IDs |

### 4.5 Message content model

A `Message` has role (`user` / `assistant`) and `content: [MessageContent]`.

`MessageContent` variants:
- `Text { text }` — plain text
- `ToolRequest { id, tool_call: { name, arguments } }` — agent wants to call a tool
- `ToolResponse { id, tool_result }` — tool result
- `Image`, `Thinking` — other content types

### 4.6 Authentication

- Header: `x-secret-key: {secret}` (not `Authorization: Bearer`)
- The secret is configured at server startup

### 4.7 Key encoding difference

- **goosed uses camelCase** (`sessionId`, `userMessage`, `tokenState`)
- **Our current transport uses snake_case** (`session_id`, `system_prompt`)

---

## 5. What we build

### 5.1 Component overview

| Component | Change | Responsibility |
|---|---|---|
| `GooseTransportProtocol` | **New protocol** | Common interface: `createSession`, `submitPrompt`, `closeSession` |
| `GooseServerTransport` | **New class** | Speaks goosed's `/agent/start` + `/agent/update_provider` + `/reply` + `/sessions/{id}` API |
| `GooseStreamEventMapper` | **New** | Maps `MessageEvent` SSE → `GooseStreamEvent` enum |
| `GooseTransport` | **Refactor** | Conform to `GooseTransportProtocol`; keep bespoke `/api/sessions` contract |
| `FixtureGooseTransport` | **Refactor** | Conform to `GooseTransportProtocol` (currently subclasses `GooseTransport`) |
| `GooseSessionBridge` | **Refactor** | Change `transport: GooseTransport` → `transport: any GooseTransportProtocol`; currently tightly coupled |
| `GooseAgentExecutor` | **Refactor** | Change `sessionBridge.transport` dependency chain; currently reaches into concrete `GooseTransport` for `closeSession` |
| `ExecutionService` | **Refactor** | Select and construct the correct transport based on `LiveRuntimeConfiguration.transportAPI`; currently hardcodes `GooseTransport()` |
| `LiveRuntimeConfiguration` | **Extend** | Add `transportAPI: .bespoke \| .gooseServer`; currently only models `transportMode: .network \| .fixture` |
| `Chainworks_ForgeApp` | **Refactor** | Wire `transportAPI` from environment into `LiveRuntimeConfiguration`; currently only checks `CHAINWORKS_GOOSE_BASE_URL` |

**Migration scope note:** The proposal previously described `GooseSessionBridge`, `GooseAgentExecutor`, and `ExecutionService` changes as "minor". Live code inspection shows they are **concrete class dependencies**, not protocol-based, so the refactor is broader than originally stated. All five call sites above must stop depending on the concrete `GooseTransport` type.

### 5.2 Transport protocol extraction

```swift
protocol GooseTransportProtocol: Sendable {
    func createSession(request: GooseSessionRequest) async throws -> GooseSessionResponse
    func submitPrompt(sessionID: String, prompt: GoosePromptRequest) -> AsyncThrowingStream<GooseStreamEvent, Error>
    func closeSession(sessionID: String) async throws
}
```

Both `GooseTransport` (bespoke) and `GooseServerTransport` (real) conform to this protocol. `FixtureGooseTransport` continues to subclass `GooseTransport`.

### 5.3 GooseServerTransport — the new adapter

**Session creation (`createSession`):** Three HTTP calls:

1. `POST /agent/start` with `{ "working_dir": workspace.root }` → returns `Session` with `id`
2. `POST /agent/update_provider` with `{ "session_id": id, "provider": override.provider, "model": override.model }` → configures the LLM provider
3. Return `GooseSessionResponse` with the server-assigned `session_id`

**Prompt submission (`submitPrompt`):** Map our `GoosePromptRequest` to a `ChatRequest`:

```swift
let chatRequest = GooseServerChatRequest(
    sessionID: sessionID,
    userMessage: GooseServerMessage(
        role: "user",
        created: Int(Date().timeIntervalSince1970),
        content: [.text(prompt.content + contextBlock)],
        metadata: GooseMessageMetadata(userVisible: true, agentVisible: true)
    ),
    overrideConversation: nil,
    recipeName: nil,
    recipeVersion: nil
)
```

Context attachments are serialized into the message text (same approach as the system prompt — Goose's agent reads them from the prompt).

**Note on `metadata`:** The `userVisible` and `agentVisible` fields are **required** by the server. Omitting them causes HTTP 422. Both should be `true` for normal agent execution.

**SSE parsing:** Parse `data: {json}\n\n` lines, deserialize `MessageEvent`, and map to `GooseStreamEvent`:

| goosed `MessageEvent` | → | `GooseStreamEvent` |
|---|---|---|
| First `Message` | → | `.sessionStarted` + `.promptSubmitted` |
| `Message` with `ToolRequest` | → | `.toolCallStarted(toolName:)` |
| `Message` with `ToolResponse` | → | `.toolCallFinished(toolName:)` |
| `Message` with `Text` | → | `.textChunk(text:)` |
| `Finish` | → | `.finalOutput` + `.sessionClosed` |
| `Error` | → | `.error(message:)` |
| `Ping` | → | (ignored) |

**Session closure:** `DELETE /sessions/{id}` — same semantics as our current contract.

### 5.4 System prompt injection

goosed's agent already has a system prompt from its config. We need to **prepend** our agent-specific instructions. Strategy:

1. First `/reply` for a session includes the full execution packet (system prompt + task directive + context) as the user message.
2. The server's agent processes it as a regular task.
3. This is how the fixture transport already works — the system prompt is embedded in the task content.

### 5.5 Configuration

```swift
enum GooseTransportAPI: String, Codable, Sendable {
    case bespoke       // Original /api/sessions contract (never implemented server-side)
    case gooseServer   // Real goosed /agent/start + /reply contract
}
```

Environment variable: `CHAINWORKS_GOOSE_TRANSPORT_API=goose_server` (default when `CHAINWORKS_GOOSE_BASE_URL` is set).

---

## 6. What we do NOT build

- **ACP adapter.** The ACP protocol is still in flux. When it stabilizes, it becomes Proposal 006.
- **Server lifecycle management.** The engineer starts `goosed agent` manually. The app does not spawn or manage the server process.
- **Provider/model selection UI in the app.** The app passes `CHAINWORKS_LIVE_PROVIDER` / `CHAINWORKS_LIVE_MODEL` to `/agent/update_provider` when bootstrapping a session. The server resolves the actual provider from its `~/.config/goose/config.yaml`. If the provider name doesn't match a configured provider, the server returns an error.
- **Multi-turn conversation.** Each agent execution is a single-turn: one prompt, one streamed response. No conversation continuation.

---

## 7. Risks and mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| goosed API changes without notice | High | Pin to known-working goose version; transport adapter is isolated behind protocol |
| SSE format differences between goosed versions | Medium | `GooseStreamEventMapper` is a single file; easy to update |
| goosed doesn't write output files to our artifact directory | High | The system prompt explicitly tells the agent where to write; if it doesn't comply, output validation catches it |
| camelCase/snake_case encoding mismatch | Low | `GooseServerTransport` uses its own `JSONEncoder`/`JSONDecoder` with `.convertFromSnakeCase` disabled |
| goosed requires explicit provider init per session | Medium | `createSession()` always calls `/agent/update_provider` after `/agent/start`; fails loudly if provider not configured |
| Cold-start latency on first turn (30–120s+) | High | Use 300s transport timeout; treat Ping-only streams as "provider starting", not errors; consider optional warm-up prompt |
| Self-signed TLS certificate | Medium | `URLSession` must trust localhost self-signed certs; documented in Section 9.6 |

---

## 8. Testing strategy

| Layer | Test | What it proves |
|---|---|---|
| Unit | `GooseStreamEventMapper` maps all `MessageEvent` variants correctly | SSE parsing is reliable |
| Unit | `GooseServerTransport` encodes `ChatRequest` in camelCase | Encoding matches server expectations |
| Integration | `GooseServerTransport` against a mock HTTP server | Full round-trip: create → prompt → stream → close |
| Integration | End-to-end with `FixtureGooseTransport` (unchanged) | Fixture mode still works |
| Manual | Start `goosed agent`, run live workflow in app | Real LLM execution proves the adapter works |

---

## 9. How to run goosed locally

### 9.1 Server binary

The HTTP server binary is **`goosed`** — it ships inside `Goose.app`:

```
/Applications/Goose.app/Contents/Resources/bin/goosed
```

It is **not** the same as the `goose` CLI (`/opt/homebrew/bin/goose`). The CLI provides `goose session`, `goose run`, `goose acp`; the server binary provides `goosed agent` — the HTTP+SSE server.

### 9.2 Starting a standalone instance

```bash
GOOSE_SERVER__SECRET_KEY=chainworks-dev-secret \
GOOSE_PORT=51200 \
/Applications/Goose.app/Contents/Resources/bin/goosed agent
```

The server starts on `https://127.0.0.1:51200` with a **self-signed TLS certificate**.

Verify:
```bash
curl -sk -H "X-Secret-Key: chainworks-dev-secret" https://127.0.0.1:51200/sessions
```

### 9.3 Configuration reference

Configuration uses environment variables with the prefix `GOOSE_` and separator `__`:

| Environment Variable | Default | Purpose |
|---|---|---|
| `GOOSE_PORT` | `3000` | HTTP listen port |
| `GOOSE_HOST` | `127.0.0.1` | Bind address |
| `GOOSE_TLS` | `true` | Enable self-signed TLS (HTTPS) |
| `GOOSE_SERVER__SECRET_KEY` | random 32-byte hex | Auth token for `X-Secret-Key` header |

Provider and model configuration comes from `~/.config/goose/config.yaml` (set via `goose configure`).

### 9.4 Authentication

All endpoints except `/status` and `/features` require the header:

```
X-Secret-Key: {secret}
```

This is **not** `Authorization: Bearer` — it's a custom header. The secret is either set via `GOOSE_SERVER__SECRET_KEY` or generated randomly at each startup.

### 9.5 Goose.app already runs goosed

The Goose desktop app at `/Applications/Goose.app` spawns `goosed agent` internally on port **51115**. Its secret key is generated randomly and passed to the Electron renderer process — it is **not accessible** from outside the app.

For Chainworks Forge, start a **separate instance** on a different port with a known secret (Section 9.2).

### 9.6 TLS handling

`goosed` uses a self-signed certificate by default. The app's `URLSession` must either:
- Disable certificate validation for localhost (development only), or
- Set `GOOSE_TLS=false` to use plain HTTP (if supported by the goosed version).

The certificate fingerprint is printed to stderr at startup:
```
GOOSED_CERT_FINGERPRINT=07:B3:AC:34:...
```

### 9.7 Xcode scheme environment variables for real server mode

| Name | Value |
|---|---|
| `CHAINWORKS_GOOSE_BASE_URL` | `https://127.0.0.1:51200` |
| `CHAINWORKS_GOOSE_API_KEY` | `chainworks-dev-secret` |
| `CHAINWORKS_GOOSE_TRANSPORT_API` | `goose_server` |
| `CHAINWORKS_LIVE_PROVIDER` | *(advisory, server uses its own config)* |
| `CHAINWORKS_LIVE_MODEL` | *(advisory, server uses its own config)* |
| `CHAINWORKS_LIVE_EFFORT` | `high` |

Note: `CHAINWORKS_GOOSE_API_KEY` maps to the `X-Secret-Key` header in `GooseServerTransport`, not to `Authorization: Bearer`.

### 9.8 Verified clean-room operator flow

**Prerequisites:**
1. `goose configure` has been run at least once and `~/.config/goose/config.yaml` contains a valid `GOOSE_PROVIDER` (e.g. `claude-code`).
2. The provider's CLI must be in PATH. For `claude-code`, the `claude` binary must be reachable (typically at `~/.local/bin/claude`).
3. The working directory passed to `/agent/start` **must exist** on disk. The server logs a warning and extensions fail to load if it doesn't.

```bash
# 1. Start goosed (separate terminal) — ensure claude is in PATH
PATH="$HOME/.local/bin:$PATH" \
GOOSE_SERVER__SECRET_KEY=chainworks-dev-secret \
GOOSE_PORT=51200 \
/Applications/Goose.app/Contents/Resources/bin/goosed agent

# 2. Verify server is up
curl -sk https://127.0.0.1:51200/status
# Expected: "ok"

# 3. Create working directory and a new session
mkdir -p /tmp/chainworks-test
curl -sk -X POST https://127.0.0.1:51200/agent/start \
  -H "X-Secret-Key: chainworks-dev-secret" \
  -H "Content-Type: application/json" \
  -d '{"working_dir": "/tmp/chainworks-test"}'
# Expected: JSON with "id": "20260323_N"

# 4. Wait for extension loading (background async task)
sleep 3

# 5. Set provider on the session (REQUIRED — without this, /reply returns "Provider not set")
curl -sk -X POST https://127.0.0.1:51200/agent/update_provider \
  -H "X-Secret-Key: chainworks-dev-secret" \
  -H "Content-Type: application/json" \
  -d '{"session_id": "<ID_FROM_STEP_3>", "provider": "claude-code", "model": "default"}'

# 6. Wait for provider initialization
sleep 2

# 7. Send a prompt and stream the response
# IMPORTANT: First turn can take 30-120s+ due to provider cold-start (claude-code spawns CLI subprocess).
# Use 300s timeout. Ping heartbeats every 500ms indicate the server is alive and waiting for provider.
curl -sk -N --max-time 300 -X POST https://127.0.0.1:51200/reply \
  -H "X-Secret-Key: chainworks-dev-secret" \
  -H "Content-Type: application/json" \
  -d '{
    "session_id": "<ID_FROM_STEP_3>",
    "user_message": {
      "role": "user",
      "created": 1711234567,
      "content": [{"type": "text", "text": "Reply with exactly: hello world today"}],
      "metadata": {"userVisible": true, "agentVisible": true}
    }
  }'
# Expected: SSE stream with Ping events (heartbeat), then Message events, then Finish
# Note: Ping events will appear for 10-30s while the provider processes the first turn

# 8. Clean up
curl -sk -X DELETE https://127.0.0.1:51200/sessions/<ID_FROM_STEP_3> \
  -H "X-Secret-Key: chainworks-dev-secret"
```

**Evidence status: Partially verified.** Steps 1–6 are reproducible and stable. Step 7 (`/reply`) has **variable first-turn latency** that depends on provider cold-start time:

- **Observed success (2026-03-23):** On a warm session (provider already initialized from a prior timed-out attempt), `/reply` returned `Message` + `Finish` in ~22 seconds. Server logs confirmed: `trace_output="hello world today"`, `Session completed ... total_tokens=9 message_count=3`.
- **Observed failure to complete (2026-03-23):** On a fully cold `goosed agent` instance, `/reply` streamed only `Ping` heartbeats for 90+ seconds without reaching `Message` or `Finish`. The `claude-code` provider was initializing Claude CLI subprocess during this time.

**This means the `--max-time 90` in step 7 may be insufficient on cold start.** The app's `GooseServerTransport` must handle this gracefully — either with a longer timeout (300s) or by treating Ping-only streams as "provider still starting" rather than failures.

**Critical learnings from live probes:**
- Step 4 (wait) is necessary because `/agent/start` spawns async extension loading in the background.
- Step 5 is mandatory — omitting it causes `{"type":"Error","error":"Provider not set"}`.
- **Cold-start latency is real and provider-dependent.** The `claude-code` provider spawns a Claude CLI subprocess, which can take 30–120+ seconds on first invocation. Subsequent turns on the same session are faster (~10–20s).
- The app's `GooseServerTransport` must use a **300-second timeout** for prompt submission to account for cold-start, not 90 seconds.
- `PATH` must include `~/.local/bin` (or wherever `claude` is installed) when starting `goosed` — without it, the `claude-code` provider silently fails.
- The working directory must exist before `/agent/start` — otherwise extensions fail to load and the session may be unusable.

**Open question for implementation:** Should `GooseServerTransport` send a no-op "warm-up" prompt after `createSession()` to absorb the cold-start latency before the real agent task begins? This would make first-turn behavior predictable at the cost of one extra LLM call.

---

## 10. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| LOCKED-001 | Transport protocol extraction is mandatory before adding the new adapter | Keeps both transports interchangeable without `if/else` branching |
| LOCKED-002 | Single-turn execution per session | Matches the engine's one-agent-one-task model; no conversation state leaks between agents |
| LOCKED-003 | System prompt is embedded in the user message, not set via a separate API | goosed has no separate system prompt endpoint per session; this is how its desktop client works too |
| LOCKED-004 | Fixture mode is not touched | Fixture transport continues to work unchanged behind the protocol |

---

## 11. Acceptance criteria

1. `GooseTransportProtocol` exists and `GooseTransport`, `GooseServerTransport`, and `FixtureGooseTransport` all conform to it.
2. `GooseSessionBridge`, `GooseAgentExecutor`, and `ExecutionService` depend on `GooseTransportProtocol`, not on concrete `GooseTransport`.
3. `GooseServerTransport.createSession()` calls `POST /agent/start` + `POST /agent/update_provider` and returns a server-assigned session ID.
4. `GooseServerTransport.submitPrompt()` posts a valid `ChatRequest` (including `metadata.userVisible` and `metadata.agentVisible`) to `POST /reply` and returns a streamed `AsyncThrowingStream<GooseStreamEvent, Error>`.
5. `GooseStreamEventMapper` correctly maps all `MessageEvent` types (`Message`, `Finish`, `Error`, `Ping`, `Notification`, `UpdateConversation`, `ActiveRequests`) to `GooseStreamEvent`.
6. `ExecutionService` selects the correct transport based on `LiveRuntimeConfiguration.transportAPI`.
7. Fixture mode continues to pass all existing tests unchanged.
8. The clean-room flow in Section 9.8 produces a successful SSE stream (with `Message` and `Finish` events) on a local `goosed agent` instance. Cold-start latency up to 300 seconds is expected and handled by the transport timeout.
9. One manual live run completes end-to-end against a real Goose server with real artifacts.
