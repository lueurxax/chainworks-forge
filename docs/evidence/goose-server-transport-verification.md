# Goose Server Transport -- Verification Record

Status: **Verified** (2026-03-24)

## Summary

Three integration tests were executed against a real running Goose.app
instance, validating the full `GooseServerTransport` pipeline from Swift app
to goosed backend. All tests passed.

## Test Results

| Test | Result | Duration | Notes |
|------|--------|----------|-------|
| `testAppLaunchedRealGooseConnection` | PASS | 81.2s | Full SSE round-trip with evidence JSON |
| `testTransportAuthHeaderIsSecretKey` | PASS | 4.1s | X-Secret-Key auth validated |
| `testFullAgentExecutorPipelineWithRealGoose` | PASS | 291.5s | Full GooseAgentExecutor pipeline |

**Environment:** macOS 26.3.1 (Build 25D771280a), Xcode, Goose.app

---

## Real Goose Connection Details

From `live_goose_connection_proof.json`:

- **Session ID:** `20260324_28`
- **Goose URL:** `https://127.0.0.1:51200` (discovered via `/private/tmp/chainworks_goose_discovery.json`)
- **Session creation:** 2.73s (includes `/agent/start` + `/agent/update_provider`)
- **Prompt round-trip:** 75.2s (includes cold-start + claude-code execution)
- **SSE Events Received:** 5
  1. `session_started` -- synthetic, from GooseServerTransport
  2. `prompt_submitted` -- synthetic, from GooseServerTransport
  3. `text_chunk` -- **"CHAINWORKS_PROOF_OK"** (real response from claude-code)
  4. `final_output` -- "Finish: stop"
  5. `session_closed` -- clean shutdown
- **Session closed cleanly:** true
- **Verdict:** PASS -- real Goose session created, prompt submitted, SSE response received

---

## What Was Verified

1. **GooseServerTransport** correctly speaks the goosed API:
   - `POST /agent/start` creates a session
   - `POST /agent/update_provider` configures claude-code provider
   - `POST /reply` submits a prompt and streams SSE
   - `DELETE /sessions/{id}` closes the session

2. **X-Secret-Key authentication** works (not Bearer token)

3. **GooseStreamEventMapper** correctly parses real goosed SSE events:
   - `Message` events with `Text` content --> `.textChunk`
   - `Finish` events --> `.finalOutput`
   - `Ping` events silently filtered

4. **LocalhostTrustDelegate** correctly handles self-signed TLS certificates

5. **GooseAgentExecutor** full pipeline:
   - Workspace validation (ARCH-025)
   - Session creation via GooseSessionBridge (ARCH-027)
   - Execution packet construction (system prompt + task directive + context)
   - SSE event processing via ExecutionEventBridge
   - Execution receipt generation (ARCH-032)

6. **Discovery mechanism** works for Xcode test runner:
   - `/private/tmp/chainworks_goose_discovery.json` bridges Goose env vars to test sandbox

## Architecture Decisions Validated

- **LOCKED-001:** Transport protocol extraction -- `GooseServerTransport` conforms to `GooseTransportProtocol`
- **LOCKED-002:** Single-turn execution per session
- **LOCKED-003:** System prompt embedded in user message
- **LOCKED-004:** Fixture transport unaffected -- runs independently behind the protocol

---

## Files

- **Test:** `Chainworks ForgeTests/LiveGooseConnectionProofTests.swift`
- **Raw evidence:** [`docs/evidence/live_goose_connection_proof.json`](live_goose_connection_proof.json)
- **Reference doc:** [`docs/reference/goose-server-transport.md`](../reference/goose-server-transport.md)
- **Discovery file:** `/private/tmp/chainworks_goose_discovery.json` (runtime only)

## How to Reproduce

```bash
# 1. Ensure Goose.app is running

# 2. Write discovery file (from a Goose session terminal)
echo '{"port":"'$GOOSE_PORT'","secret_key":"'$GOOSE_SERVER__SECRET_KEY'"}' \
  > /private/tmp/chainworks_goose_discovery.json

# 3. Run proof tests
xcodebuild test \
  -project "Chainworks Forge.xcodeproj" \
  -scheme "Chainworks Forge" \
  -destination "platform=macOS" \
  -only-testing:"Chainworks ForgeTests/LiveGooseConnectionProofTests"
```
