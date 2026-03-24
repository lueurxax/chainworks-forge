# Proposal 005 Clean-Room Probe R2 — SUCCESS

**Date:** 2026-03-24T05:20-05:21 UTC
**goosed version:** v1.28 (/Applications/Goose.app/Contents/Resources/bin/goosed)
**Provider:** claude-code (claude CLI v2.1.81)
**Port:** 51200
**Secret:** chainworks-dev-secret

## Root Cause of R1 Ping-Only Issue

goosed was started WITHOUT `~/.local/bin` in PATH.
The `claude-code` provider could not find the `claude` CLI binary, so it silently hung.

**Fix:** Start goosed with `PATH="$HOME/.local/bin:$PATH"`

## Probe Flow

1. `POST /agent/start` → session `20260324_1` ✅
2. `POST /agent/update_provider` (claude-code, default) → 200 ✅
3. `POST /reply` ("Reply with exactly: hello world today") → SSE stream ✅
   - 141 Ping events (~70s cold-start)
   - 1 ModelChange event (new, not in proposal)
   - 1 Message event: `"hello world today"`
   - 1 Finish event: `reason: "stop"`, `totalTokens: 9`

## Evidence

- `reply-full-events.txt` — complete SSE event log
- goosed logs confirmed: `trace_output: "hello world today"`, `duration_ms: 70132`, `total_tokens: 9`

## Learnings

- Cold-start ~70s on warm machine (provider spawns claude CLI subprocess)
- New `ModelChange` event type discovered — added to `GooseStreamEventMapper`
- 300s timeout in GooseServerTransport is necessary and sufficient
