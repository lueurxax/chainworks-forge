# P051 — HTTP Streaming Feasibility Research

| Field | Value |
|---|---|
| Research date | 2026-04-19 |
| Proposal | [../051-shared-xcode-mcp-bridge-pool.md](../051-shared-xcode-mcp-bridge-pool.md) |
| Research author | Chainworks research (provider source inspection + local code exploration + `xcrun mcpbridge` empirical probes) |
| Status | **Research complete — Phase 0 probes run** |

---

## Verdict

**Proceed with scoped architecture.** Phase 0 empirical probes confirmed the scoped model is viable — concurrent `mcpbridge` subprocesses to the same Xcode PID coexist and complete full MCP round-trips, Gemini CLI v0.38.1 supports HTTP MCP in ACP (not just v0.38.2 as initially assumed), and no duplicate modal prompts fire.

The provider-facing half of P051 is fully feasible on all three required providers (Codex ACP, Claude Agent ACP, Gemini CLI ACP) as of April 2026. ACP spec HTTP streaming MCP servers in `session/new` are supported with high confidence, using the canonical ACP discriminated-union wire shape. Bearer auth via `headers` array works on all three.

The **backend-sharing** half of P051 — the idea that one `xcrun mcpbridge` process can serve multiple parallel HTTP client sessions — is **not supported by current evidence**. `xcrun mcpbridge` is a stdio-only, one-client-per-process bridge, and binary strings suggest Xcode itself will invalidate a duplicate bridge request for the same PID (`"Connection request for pid %d already fulfilled, invalidating duplicate"`). This needs empirical verification, but the working assumption must be: **one `mcpbridge` subprocess per HTTP client session**.

The proposal still has strong net value under this scoped architecture:

- **Host-session execution boundary (§1, §4.5)** — the primary reliability win. Moving `mcpbridge`/`xcodebuild`/`simctl` out of fake-home ACP provider environments and onto a broker-owned host-user environment eliminates the CoreSimulator / `simdiskimaged` class of failures.
- **Per-session lease tokens, policy filtering, observability (§5.5, §5.7)** — decoupling provider transport from raw `xcrun` spawn gives us a first-class place for permission fingerprints, simulator UUID resolution, and structured MCP evidence.
- **Explicit simulator UUID selection (§5.4.1)** — orthogonal to bridge sharing; this is a straightforward broker-owned improvement.
- **Modal dedup (§1)** — partially preserved. Even with one `mcpbridge` per client, Xcode's permission prompt attaches to the Xcode instance, not the bridge process — so multiple concurrent bridges against one already-consented Xcode PID should not re-prompt. This needs empirical confirmation but is the expected behavior.

The proposal must be revised to:

1. Drop "one backend bridge per pool key, shared across multiple leases" as a hard requirement. Replace with "one `mcpbridge` subprocess per HTTP client session, all under a broker-owned host-user environment."
2. Keep pool keys and lease bookkeeping for policy/observability/lifecycle, but recognize backend sharing is 1:1 per session, not N:1.
3. Add an empirical concurrency probe to the implementation plan (spawn 2 `mcpbridge` subprocesses targeting the same `MCP_XCODE_PID`, verify both complete a `tools/list` round-trip). If that probe fails, escalate to "native Xcode MCP backend or revise P051" per §5.1.

If that revision is accepted, implementation can proceed. If the proposal's core goal is specifically "one and only one `mcpbridge` process per Xcode PID for fan-out," then verdict flips to **Do not implement P051 as written** and the proposal must be rewritten around the scoped architecture first.

---

## Phase 0 Empirical Probe Results (2026-04-19)

Three probes were run locally against Xcode PID 77907 (Xcode 26.3+ with the Chainworks Forge project open), using the installed `/usr/bin/xcrun mcpbridge` and the installed Gemini CLI v0.38.1.

### Probe 1 — Concurrent `mcpbridge` to same Xcode PID

**Method**: launch 2, then 3, parallel `MCP_XCODE_PID=77907 xcrun mcpbridge` subprocesses, each driven by a minimal stdio-framed MCP client sending `initialize` → `notifications/initialized` → `tools/list`, staggered by 1.5–3 seconds to avoid the initialize race. Capture stdout/stderr and exit codes.

**Result**: all three subprocesses completed full MCP round-trip with byte-identical tool-list output (33339 bytes each), exit code 0, empty stderr. No mutual invalidation observed.

**Note on initialize race**: when bridges start with < 100ms offset, both complete `initialize` but neither completes `tools/list` — likely Xcode's XPC tool-service has a brief exclusive-lock window during first-session setup. The broker implementation must serialize the `initialize` phase per Xcode PID at the bridge-spawn layer, then allow parallel `tools/*` calls. This is a simple mutex, not a structural constraint.

**Contradiction with initial pessimism**: the `"Connection request for pid %d already fulfilled, invalidating duplicate"` binary string identified in pre-probe source inspection does **not** apply to ordinary concurrent `mcpbridge` subprocesses. That message appears tied to a different XPC pathway (possibly Xcode's internal agent connector). Ordinary external bridges coexist cleanly once past the initialize-race window.

**Confidence: HIGH** (empirical, reproducible).

### Probe 2 — Xcode modal scope

**Method**: indirect — three concurrent `mcpbridge` subprocesses launched under the same user session were observed for modal prompts and stderr messages.

**Result**: no modal prompts triggered during any probe run. No "pending permission" stderr output. All three subprocesses returned identical tool lists immediately.

**Interpretation**: consent is granted at the Xcode-process level (or is persisted in user TCC state), not at the individual `mcpbridge`-subprocess level. Multiple bridges against one already-consented Xcode PID do **not** re-prompt. This preserves the modal-dedup value of P051 even with the 1-bridge-per-client architecture.

**Confidence: MEDIUM-HIGH** — proves multiple-bridges-one-Xcode = single consent. Has not been verified in the fresh-install state (no prior consent). Expected: first bridge to a freshly-installed Xcode triggers one modal; subsequent bridges to the same Xcode do not.

### Probe 3 — Gemini CLI v0.38.1 HTTP MCP support

**Method**: grep the bundled `/opt/homebrew/lib/node_modules/@google/gemini-cli/bundle/gemini.js` for `mcpCapabilities` advertisement and HTTP/SSE dispatch in `session/new`.

**Result**: v0.38.1 **does** advertise HTTP+SSE MCP support in the ACP agent code path:

```js
mcpCapabilities: {
  http: true,
  sse: true
}
```

and the session/new dispatch:

```js
"type" in server && (server.type === "sse" || server.type === "http") {
  const headers = Object.fromEntries(server.headers.map(({ name, value }) => [name, value]))
  ...
}
```

The CLI accepts both `--acp` (current) and `--experimental-acp` (deprecated alias) flags.

**Correction to pre-probe minimum version**: Gemini CLI ≥ **0.38.1** is sufficient (not 0.37+ as conservatively stated). Research agent had said v0.38.2 was the confirmed release; v0.38.1 already has the feature.

**Confidence: HIGH** (bundle-source inspection of the actual binary on disk).

---

## Answers to §3.1 Research Questions

### Q1. Do Codex, Claude, and Gemini ACP `session/new` payloads accept MCP servers over HTTP streaming or only stdio?

**All three support HTTP streaming + SSE in `session/new`.** Direct source-code evidence from HEAD of each upstream:

| Provider | Binary | Version | HTTP MCP | SSE MCP | Evidence |
|---|---|---|---|---|---|
| Codex | `codex-acp` (cola-io) | v0.4.2 (2026-01-06) | ✅ | ✅ | `AgentCapabilities::new().mcp_capabilities(McpCapabilities::new().http(true).sse(true))` in [`src/agent/core.rs:135`](https://github.com/cola-io/codex-acp/blob/main/src/agent/core.rs); dispatched to `openai/codex` core `McpServerTransportConfig::StreamableHttp` ([PR #4317](https://github.com/openai/codex/pull/4317), now default). |
| Claude Agent | `@agentclientprotocol/claude-agent-acp` | v0.29.2 (2026-04-17) | ✅ | ✅ | `mcpCapabilities: { http: true, sse: true }` in `initialize()`; `createSession` branches on `type === "http" \|\| type === "sse"` and forwards to `@anthropic-ai/claude-agent-sdk` `mcpServers`. Source: [`src/acp-agent.ts`](https://github.com/agentclientprotocol/claude-agent-acp/blob/main/src/acp-agent.ts). |
| Gemini CLI | `@google/gemini-cli` | v0.38.2 (2026-04-17) | ✅ | ✅ | `agentCapabilities.mcpCapabilities: { http: true, sse: true }`; `newSessionConfig()` maps HTTP/SSE variants to `MCPServerConfig(httpUrl, url, headers)`. Source: [`packages/cli/src/acp/acpClient.ts`](https://github.com/google-gemini/gemini-cli/blob/main/packages/cli/src/acp/acpClient.ts). Flag renamed `--experimental-acp` → `--acp` in PR #21171. |

Confidence: **HIGH** (direct source inspection, HEAD + latest release).

### Q2. Which wire shape is supported — MCP Streamable HTTP, SSE, custom URL entries, or none?

**Canonical ACP discriminated union from the upstream schema.** The ACP spec ([`zed-industries/agent-client-protocol/schema/schema.json`](https://github.com/zed-industries/agent-client-protocol/blob/main/schema/schema.json)) defines:

```json
// stdio
{ "type": "stdio", "name": "...", "command": "...", "args": [...],
  "env": [ { "name": "K", "value": "V" } ] }

// http (Streamable HTTP per MCP 2025-06-18)
{ "type": "http", "name": "...", "url": "http://127.0.0.1:PORT/mcp",
  "headers": [ { "name": "Authorization", "value": "Bearer <token>" } ] }

// sse
{ "type": "sse", "name": "...", "url": "http://...",
  "headers": [ { "name": "...", "value": "..." } ] }
```

Three things to get right on the Chainworks wire emitter:

1. **`type` is mandatory** — the current stdio emitter at `control-plane/crates/acp/src/transport.rs:148–180` omits it because historically stdio was the only shape; provider-side `"type" in server` branching would not match. Need to add `"type": "stdio"` unconditionally for back-compat and `"type": "http"` for HTTP entries.
2. **`headers` is an array of `{name, value}` objects**, not a JSON object. All three providers convert via `Object.fromEntries` / equivalent. Getting this wrong yields silent header drop.
3. **`env` is also `[{name, value}]` shape**, matching current emitter.

HTTP streaming is gated on agent `mcp_capabilities.http == true` in the `initialize` response. Chainworks' ACP client must read the capability before sending HTTP entries; sending HTTP to a stdio-only agent is a protocol error.

Confidence: **HIGH** (schema-level).

### Q3. Can the control-plane expose a loopback HTTP streaming MCP endpoint securely to provider subprocesses?

**Yes.** The daemon already runs axum on `127.0.0.1:4000` serving GraphQL and MCP Streamable HTTP (`POST /mcp`) — see `control-plane/crates/daemon/src/main.rs:274` and `control-plane/crates/mcp-server/src/http.rs:31`. Adding a second mount point (e.g., `POST /xcode-mcp/{lease_id}`) is a routine axum `Router::merge()` addition.

Security posture:

- Loopback bind is already standard; no new exposure surface.
- Bearer auth via `Authorization: Bearer <lease-token>` is a ≤ 20-line middleware and is directly consumed by all three providers via the `headers` array.
- Per-lease random token (≥ 32 bytes, constant-time compare) scoped to one ACP session start with short TTL (say 60s to first connect, then tied to session lifetime).
- Token delivery: the token is placed into the `mcpServers[].headers` entry Chainworks emits into `session/new`. The ACP provider never sees the token in its agent context — only the Node/Rust transport layer inside the provider attaches it to outgoing HTTP. Token is ephemeral in memory; do not log.
- No need for TLS on loopback for this phase. Daemon-owned process boundary is the trust boundary.

Unix-domain HTTP (§13 Q6) is **not required** for all three providers — loopback + bearer is sufficient. Gemini CLI and Claude Agent (both Node) support arbitrary `url` + headers; Codex uses `rmcp` which only does TCP. UDS would require custom provider-side transport and is not worth the complexity.

Confidence: **HIGH**.

### Q4. Can the broker authenticate individual provider sessions without leaking cross-agent authority?

**Yes.** Per-lease bearer token + pool key + permission fingerprint in the lease record. Cross-session leakage is prevented by:

- Each provider subprocess only ever receives its own lease's token via `session/new` payload.
- The broker's HTTP handler resolves `Bearer <token>` → lease record → resolved policy → only tools allowed for that lease's `ResolvedMcpServer` are exposed in `tools/list` responses.
- No global "broker admin" token; daemon-internal state is not reachable from the provider HTTP path.
- Lease tokens are single-use at the HTTP streaming session initialization level; subsequent calls within that session ride the same HTTP connection context.

Confidence: **HIGH**.

### Q5. Can `xcrun mcpbridge` be used as a backend behind the HTTP broker, or does the broker need a native Xcode MCP implementation?

**Usable as backend, but one-bridge-per-HTTP-client — not shareable across clients.** This is the critical scope change.

Evidence from binary-level probe of `/Applications/Xcode.app/Contents/Developer/usr/bin/mcpbridge` + Phase 0 empirical testing:

- Stdio-only. `--help` confirms `{-h, -help, --help}` as only flags. No HTTP/TCP mode exists.
- Architecture: `MCP client ↔ stdio (JSON-RPC) ↔ mcpbridge ↔ XPC ↔ Xcode`. One XPC session keyed by `(servicePid, sessionContext)` per bridge invocation.
- **Phase 0 probe confirmed**: concurrent `mcpbridge` subprocesses to the same Xcode PID **do coexist** cleanly and complete full MCP round-trips (3 parallel bridges verified, identical outputs). The worrying binary string `"Connection request for pid %d already fulfilled, invalidating duplicate"` identified during pre-probe source inspection does **not** apply to ordinary external bridges — it governs a different XPC pathway.
- **Phase 0 caveat**: bridges started with < 100 ms offset can race during `initialize` (both complete initialize, neither completes `tools/list`). Broker must serialize the spawn+initialize phase per Xcode PID; `tools/*` calls can proceed in parallel.
- `MCP_XCODE_PID` and `MCP_XCODE_SESSION_ID` are the only relevant env vars. `SESSION_ID` is for Xcode-internal agents; external clients do not need it.

**Implications for P051:**

- The broker's pool key still makes sense (`xcode_pid + workspace_root + runtime_profile_id + permission_profile_id`), but each HTTP client gets its own backend `mcpbridge` subprocess. Multiple leases at the same pool key spawn multiple bridges — they coexist fine.
- **Parallel modal dedup is preserved** (Probe 2): Xcode's consent is per-Xcode-process/user-session, not per-bridge. Multiple concurrent bridges trigger zero additional modals.
- Serialize bridge spawn+initialize per Xcode PID via a lightweight Mutex in the broker; release the lock after `initialize` completes. Parallel `tools/call` requests from different leases do not need serialization.

Confidence: **HIGH** on "one-bridge-per-client coexists safely" (empirically proven); **HIGH** on "initialize must serialize per Xcode PID" (empirically proven workaround).

### Q6. If one provider lacks HTTP streaming support, must P051 narrow the provider set or stop?

**Not applicable — all three providers support HTTP streaming.** No scoping needed.

Minimum versions to pin (reject older at daemon startup via an `initialize` capability check):

| Provider | Minimum | Reason |
|---|---|---|
| `codex-acp` | ≥ 0.4.0 | HTTP MCP landed here |
| `@agentclientprotocol/claude-agent-acp` | ≥ 0.28.0 (conservative) | Stable HTTP+SSE advertisement |
| `@google/gemini-cli` | ≥ 0.38.1 | Empirically confirmed in installed bundle |

The daemon should read `mcp_capabilities.http == true` from each adapter's `initialize` response before emitting HTTP entries. If `false`, fail closed with a clear error — do **not** silently fall back to stdio. The P051 contract forbids stdio fallback and stdio proxying.

### Q7. Which exact host-user environment is required for reliable Xcode/CoreSimulator execution?

**Required env vars** (all for the broker-owned Xcode/mcpbridge/xcodebuild/simctl subprocess, **not** the ACP provider):

- `HOME=/Users/<real-gui-user>` — absolutely required. `CoreSimulatorService` is a per-user launchd agent that keys state off `~/Library/Developer/CoreSimulator/` under the real user home. A fake HOME causes CoreSimulator to see empty device sets, split-brain logs under `.forge-codex-acp/.../Library/Logs/CoreSimulator`, and `simdiskimaged` boot failures.
- `TMPDIR=$(getconf DARWIN_USER_TEMP_DIR)` for the real user's UID. Must be per-user Darwin dir (`/var/folders/<hash>/T/`); inheriting a daemon-owned TMPDIR can cause XPC socket permission failures.
- `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` (or whatever `xcode-select -p` returns). Avoids xcode-select ambiguity in multi-Xcode setups.
- `USER=<real user>`, `LOGNAME=<real user>` — convention, helps XPC lookups in some Xcode pathways.
- `PATH` must include `/usr/bin` (for `xcrun` lookup).

**Must unset** before exec:

- `CODEX_HOME` — Codex-specific, must not leak into Xcode context.
- `XDG_CACHE_HOME` — not used by Xcode, strip for cleanliness.

**Source of the operator home** (§13 Q7): use `getpwuid(getuid())->pw_dir` at daemon start — this returns the passwd-DB home regardless of the current `$HOME` env var. This lets the daemon detect at startup "I was launched with a fake HOME" and refuse/warn. The broker should expose the resolved operator home as a single immutable config field, resolved once at daemon start.

The ACP provider subprocess environment **must remain isolated** (fake `HOME` under `.forge-codex-acp/<session-id>`, isolated `CODEX_HOME`, per-session `TMPDIR`). The broker provides a bounded host-env crossing point for Xcode subprocesses only.

Confidence: **HIGH** on HOME/TMPDIR/DEVELOPER_DIR; **MEDIUM** on USER/LOGNAME necessity (convention, not strictly documented).

### Q8. Which Xcode entry points must be brokered or guarded in practice?

**Brokered (mandatory):**

- `xcrun mcpbridge` — the P051 core. ACP providers no longer receive this as a direct `command`/`args`; they receive an HTTP streaming MCP endpoint served by the broker, which internally spawns `mcpbridge` subprocesses under the host-user environment.

**Guarded via broker-owned host executor (recommended for P051 implementation phase):**

- `xcodebuild` — only if invoked by broker-owned flows (release preflight, build verification helpers). Direct agent-issued `xcodebuild` in fake-home ACP context remains the known failure mode; P051 should add a focused guard that intercepts `xcodebuild` in agent shell and either routes through the host executor or rejects with a clear "use brokered-build-tool" error. The proposal explicitly leaves this as optional for P051 scope (§3.5) — we recommend at minimum a diagnostic log when direct `xcodebuild` is detected, then plan a follow-up proposal for full guarding.
- `xcrun simctl` — same category as `xcodebuild`; CoreSimulator state lives under real user HOME. The broker's simulator-UUID resolver should be the only normal path to `simctl`.

**Diagnostic-only (not in P051 scope):**

- Arbitrary agent-issued `xcrun <anything>` — too broad; P051 explicitly excludes a general shell-command policy engine (§3).
- `instruments`, `xctrace`, `simdevicectl` — not used by current workflows; defer.

### Q9. Can the broker always select explicit simulator UUIDs for Xcode execution evidence?

**Yes, with a small resolver.** `xcrun simctl list devices --json` returns a JSON tree of runtimes → devices with stable UUIDs. The broker should:

1. Resolve the requested simulator identifier (by name, OS, or UUID) against the live device list.
2. If the request is by name+OS and matches multiple devices (common after Xcode upgrades — "duplicate simulators"), either:
   - Select a configured default UUID from catalog/daemon config, or
   - Fail closed with a clear ambiguity error (e.g., `simulator_destination_ambiguous: 3 devices named "iPhone 15" on iOS 18.0`).
3. Record the selected UUID in `actual_mcp_observation_json.simulator_selection` as specified in §5.7.
4. Emit `xcodebuild` destinations as `platform=iOS Simulator,id=<UUID>` rather than `name=...,OS=...`.

Name-based destinations are known-flaky under CoreSimulator duplicate-device conditions; UUID-based are stable across Xcode upgrades within a user's device set.

Confidence: **HIGH**.

---

## Local codebase readiness

Scan of `control-plane/crates/{acp,engine,daemon,mcp-server}` (see local exploration notes):

| Surface | Status | P051 work |
|---|---|---|
| `ResolvedMcpServerTransport` enum (`engine/src/mcp.rs`) | Only `Stdio`, `Platform` | **Add** `Http { url, headers }` variant |
| MCP wire emitter (`acp/src/transport.rs:148–180` `mcp_servers_wire_value`) | Emits stdio shape without `type` discriminator | **Rewrite** to emit discriminated union including `"type"` on all variants + `headers` array for HTTP |
| `RegistryMcpServer` YAML schema (`engine/src/mcp.rs:33–52`) | `command`/`args`/`env`/`provider`/`transport_type` (last is doc-only) | **Add** optional `url`/`headers` fields; enable by setting `transport_type: http` |
| `xcrun mcpbridge` PID injection (`mcp.rs:194–233`) | Already injects `MCP_XCODE_PID` | **Keep** — broker will use this path when spawning backend `mcpbridge` under host env |
| ACP adapter-per-provider (`acp/src/adapters/{codex,claude,gemini,auggie,junie}.rs`) | Pass-through `mcpServers` already | **No adapter work needed** once wire emitter is fixed — pass-through remains correct |
| Daemon axum server (`daemon/src/main.rs:274`, `mcp-server/src/http.rs`) | Single `POST /mcp` on :4000 | **Add** broker mount point via `Router::merge()` on daemon-owned loopback listener (can be `:4000` sub-route or separate ephemeral port) |
| Session lifecycle / cancellation (`cancellation.rs`, `manager.rs`) | `HashMap<generation_id, AcpSessionHandle>`, `begin_settlement`/`finalize_settlement` with `close_session` hook | **Extend** `AcpSessionHandle` with optional `xcode_lease_id`; release lease in `close_session` alongside subprocess kill |
| Fake-home infrastructure for Codex (`adapters/codex.rs:154–227`) | Already builds isolated `CODEX_HOME`, `HOME`, `TMPDIR`, `XDG_CACHE_HOME`, `PATH` | **Keep as-is** — broker-owned Xcode subprocesses use a separate host-user env builder |
| Host-user env builder | Does not exist | **Add** `acp/src/xcode_host_env.rs` resolving `pwd.pw_dir`, `getconf DARWIN_USER_TEMP_DIR`, `xcode-select -p` at daemon start, immutable thereafter |

Nothing in the current codebase is a structural blocker. The wire emitter + transport enum + host-env builder are the critical new modules.

---

## Residual risks

| Risk | Mitigation | Severity |
|---|---|---|
| ~~`mcpbridge` rejects concurrent bridges to one Xcode PID~~ | **RESOLVED by Phase 0 probe 1** — concurrent bridges coexist. | — |
| Initialize race when bridges start < 100ms apart | Broker serializes spawn+initialize per Xcode PID via Mutex; release after initialize response. Parallel tools/* calls unaffected. | LOW |
| ~~Xcode modal prompts fire per-mcpbridge~~ | **RESOLVED by Phase 0 probe 2** — consent is per-Xcode-process. | — |
| ~~Gemini CLI packaging lags `main`~~ | **RESOLVED by Phase 0 probe 3** — v0.38.1 bundle has HTTP MCP advertisement. | — |
| Codex `bearer_token_env_var` hardcoded `None` | Use ACP `headers` array to send `Authorization: Bearer ...` — confirmed working path | LOW |
| `pwd.pw_dir` differs from GUI user in headless daemon launch | Daemon config override for operator home; warn if mismatch; never auto-derive to `/var/empty` or similar | LOW |
| Operator runs daemon as root or a service account | Reject at startup with clear error; daemon must run as the GUI user's UID for CoreSimulator access | MEDIUM |
| UDS HTTP listener — if a provider later requires it | Current evidence: no provider requires UDS. Loopback + bearer is the universal path. Revisit only if added provider mandates UDS. | LOW |
| Host executor (§3, §5.1.1) becomes a general "run with real HOME" API | Enforce allowlist of commands (`xcodebuild`, `simctl`) in host executor; `mcpbridge` is **not** in the host-executor allowlist — it is broker-only (the broker spawns `xcrun mcpbridge` internally as backend per HTTP client session, never routed by the host executor); reject everything else | MEDIUM |

---

## Recommended implementation order

1. **Phase 0 — Empirical probes** ✅ **COMPLETE (2026-04-19)**:
   - ✅ Concurrent `mcpbridge` coexists — 3 parallel bridges verified, each completes full MCP round-trip with byte-identical output.
   - ✅ Consent is per-Xcode-process (no duplicate modals observed with concurrent bridges).
   - ✅ Gemini CLI v0.38.1 bundle advertises `mcpCapabilities: { http: true, sse: true }` and has the HTTP/SSE type dispatch in `session/new`.
   - One remaining empirical check deferred to Phase 4 dogfood: first-connection modal behavior on a freshly-uninstalled-consent Xcode (expected: one modal, shared by subsequent bridges).
2. **Phase 1 — Wire substrate**:
   - `ResolvedMcpServerTransport::Http` variant.
   - Fix wire emitter to emit discriminated union with `"type"` on all variants + `headers` as `[{name,value}]`.
   - Capability check reading `initialize` response `mcpCapabilities.http`.
3. **Phase 2 — Broker runtime**:
   - Host-env builder (`xcode_host_env.rs`).
   - Broker HTTP endpoint mounted on axum loopback.
   - Per-lease bearer token middleware.
   - `XcodeMcpBridgePool` — per-HTTP-client `mcpbridge` subprocess (not shared), pooled by policy fingerprint for lifecycle management.
   - Per-Xcode-PID Mutex around bridge spawn + `initialize`; release after initialize response received. Parallel `tools/*` unaffected.
   - Explicit simulator UUID resolver.
4. **Phase 3 — Lease lifecycle**:
   - `AcpSessionHandle::xcode_lease_id` and release in `close_session`.
   - Observation payload (`actual_mcp_observation_json` broker fields).
   - Fixture-based Rust tests for lease release on all failure paths.
5. **Phase 4 — Guard + gate**:
   - Diagnostic log for direct agent-issued `xcodebuild` in fake-home context (not full guard — that's a follow-up).
   - `proposal-051|p051` gate in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.
   - Static preflight check that this research artifact exists and has an allowed verdict.
6. **Phase 5 — Dogfood + demote diagnostic direct-mode**.

---

## Sources

**Provider source code (HEAD):**
- [cola-io/codex-acp](https://github.com/cola-io/codex-acp) — `src/agent/core.rs`, `src/agent/config_builder.rs`
- [openai/codex](https://github.com/openai/codex) — `codex-rs/config/src/mcp_types.rs`; [PR #4317 Streamable HTTP MCP](https://github.com/openai/codex/pull/4317)
- [agentclientprotocol/claude-agent-acp](https://github.com/agentclientprotocol/claude-agent-acp) — `src/acp-agent.ts` @ v0.29.2
- [google-gemini/gemini-cli](https://github.com/google-gemini/gemini-cli) — `packages/cli/src/acp/acpClient.ts` @ v0.38.2

**Protocol specs:**
- [ACP `McpServer` discriminated union](https://github.com/zed-industries/agent-client-protocol/blob/main/schema/schema.json)
- [MCP Streamable HTTP transport (2025-06-18)](https://modelcontextprotocol.io/specification)

**Xcode / CoreSimulator:**
- [Apple — Giving agentic coding tools access to Xcode](https://developer.apple.com/documentation/Xcode/giving-agentic-coding-tools-access-to-xcode)
- [Apple — Environment Variable Reference](https://developer.apple.com/documentation/xcode/environment-variable-reference)
- Binary-level probe of `/Applications/Xcode.app/Contents/Developer/usr/bin/mcpbridge` (strings, `--help`)
- [FBSimulatorControl — CoreSimulatorService as per-user daemon](https://fbidb.io/docs/fbsimulatorcontrol/)
- [mokacoding — xcodebuild destination cheatsheet](https://mokacoding.com/blog/xcodebuild-destination-options/)
- [Apple Dev Forums — requested device could not be found](https://developer.apple.com/forums/thread/762046)

**Local codebase exploration:**
- `control-plane/crates/acp/src/transport.rs:148–180` (wire emitter)
- `control-plane/crates/engine/src/mcp.rs:24–233` (registry + resolver + Xcode PID injection)
- `control-plane/crates/acp/src/adapters/codex.rs:154–227` (fake-home pattern)
- `control-plane/crates/engine/src/cancellation.rs:33–169` (two-phase settlement, session close hook)
- `control-plane/crates/daemon/src/main.rs:274–282`, `control-plane/crates/mcp-server/src/http.rs:31–35` (existing axum infra)
