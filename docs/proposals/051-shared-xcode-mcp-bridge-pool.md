# Proposal 051: Shared Xcode MCP Bridge Pool for ACP Sessions

| Field | Value |
|---|---|
| Date | 2026-04-17 (revised 2026-04-19 after Phase 0 probes) |
| Status | Research-gated, scope revised |
| Author | Andrey Khasanov |
| Depends on | [025-per-agent-mcp-policy-and-runtime-validation.md](025-per-agent-mcp-policy-and-runtime-validation.md), [026-acp-first-runtime-transport-and-goose-decoupling.md](026-acp-first-runtime-transport-and-goose-decoupling.md), [029-mcp-northbound-control-plane-server.md](029-mcp-northbound-control-plane-server.md), [037-acp-execution-supervision-and-idle-watchdog.md](037-acp-execution-supervision-and-idle-watchdog.md), [049-context-strategy-management-mcp-tools.md](049-context-strategy-management-mcp-tools.md) |
| Priority | P1 / High |
| Scope | Implement a Chainworks-owned HTTP streaming Xcode MCP broker that (a) serves provider-facing Xcode MCP via loopback HTTP with per-session bearer tokens, (b) spawns one `xcrun mcpbridge` backend subprocess per HTTP client session under a broker-owned **host-user environment**, (c) serializes bridge spawn + `initialize` per Xcode PID while allowing parallel `tools/*` calls, (d) centralizes simulator UUID selection and policy filtering. The broker is the Xcode host-session execution boundary so isolated ACP agents do not run Xcode tools directly from fake per-run `HOME`/`TMPDIR` environments. |
| Goal | Eliminate CoreSimulator/`simdiskimaged` failures caused by isolated agent home directories (primary reliability goal), preserve one-modal-per-Xcode-process consent behavior across parallel reviewers, reduce MCP startup latency through pooled lifecycle, and preserve per-agent session isolation, permission policy, MCP capability reporting, and recovery semantics. |
| Research artifact | [051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md](051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md) — verdict **Proceed with scoped architecture**, Phase 0 probes complete. |

**Gate naming note:** this proposal owns the new canonical gate alias `proposal-051|p051`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md`; it must not reuse the existing P044, P045, P049, or P050 gates.

---

## 1. Context and Motivation

The control-plane can already keep ACP sessions alive and reuse them for later prompts in the same run. That reduces repeat starts on retry/resume. It does not solve the first-start problem for a parallel stage where two or more agents require Xcode MCP at the same time.

Current behavior for a proposal-review stage with Gemini UX and Gemini UI reviewers:

1. The orchestrator enqueues both reviewers in the same phase.
2. Each ACP provider receives an `mcpServers` entry for `xcode`.
3. Each provider starts its own `xcrun mcpbridge`.
4. Xcode may show a modal or permission prompt for each bridge/client connection.
5. Gemini waits until the operator responds to each modal, so parallel startup becomes manually serialized.

Recent runtime fixes reduce the damage:

- `MCP_XCODE_PID` is injected so bridge startup targets the already-open Xcode process.
- Gemini + Xcode agents default to `same_agent_family_within_run` session reuse when no explicit reuse scope is present.
- Retry now rejects active latest stage attempts, preventing duplicate retries while an agent is still running.

Those fixes still leave two unavoidable first starts for two parallel ACP sessions. This proposal addresses that remaining source of friction.

A second, higher-priority reliability issue is now in scope. ACP agents intentionally run with isolated per-session state, including a fake `HOME` such as `.forge-codex-acp/<session-id>`. That is useful for Codex/plugin/cache isolation, but Xcode is not a normal per-run CLI dependency. `xcodebuild`, CoreSimulator, `simdiskimaged`, Xcode MCP bridge prompts, DerivedData, simulator devices, and CloudKit/iCloud state are tied to the macOS GUI user's host session and `~/Library` tree. Running Xcode from a fake home produces a split-brain environment: some Xcode paths resolve under `.forge-codex-acp/.../Library`, while the underlying services still belong to `/Users/user`. Recent failures showed this as CoreSimulator runtime discovery errors, `simdiskimaged` startup failures, and missing CoreSimulator log directories under the fake home.

P051 therefore must not be treated as only a modal-deduplication optimization. It is also the correct boundary for host-session-bound Xcode execution. The target design keeps agent runtime state isolated, but moves Xcode/CoreSimulator access behind a Chainworks-owned bridge that runs with the real macOS user context required by Xcode.

---

## 2. Product Questions This Proposal Must Answer

1. Can a parallel stage with two Xcode-capable ACP sessions start with only one real `xcrun mcpbridge` connection to Xcode?
2. Can each ACP provider consume Xcode MCP through HTTP streaming without knowing that Chainworks is brokering the backend bridge?
3. Can the broker preserve per-agent read/write policy and MCP capability reporting?
4. Can one stuck or cancelled ACP session release its lease without killing sibling sessions that still use the shared bridge?
5. Can the system fail closed when the broker cannot start or when Xcode PID changes?
6. Can operators and tests prove the number of real Xcode bridge starts and modal-prone handshakes is reduced?
7. Can Xcode and CoreSimulator commands run from a stable host-user environment without giving the whole ACP agent access to the real user `HOME`?
8. Can the runtime prevent direct Xcode execution from isolated fake-home ACP sessions except through an explicit diagnostic escape hatch?

---

## 3. Scope

This proposal includes:

- A Rust Xcode MCP broker owned by the control-plane process.
- A mandatory pre-implementation research gate proving whether Codex, Claude, and Gemini ACP providers can consume MCP servers through HTTP streaming in `session/new`.
- An HTTP streaming MCP endpoint handed to ACP providers in place of direct `xcrun mcpbridge`.
- A lease model keyed by Xcode PID, workspace root, MCP server id, runtime profile, and permission class.
- Broker lifecycle management inside `acp::AcpRuntimeManager` or a closely owned ACP runtime service.
- MCP request routing and response correlation so each HTTP client session drives its own backend bridge subprocess safely, with serialized bridge-spawn + initialize per Xcode PID.
- Read-only observability for active bridge pools, leases, startup latency, backend PID, and HTTP client session counts.
- A host-session execution contract for Xcode-bound tools: broker-owned Xcode processes use the real macOS user `HOME`, normal macOS temp/cache directories, explicit simulator identifiers, and `MCP_XCODE_PID`; generic ACP agent processes keep isolated `CODEX_HOME` and fake home state.
- Focused Rust tests proving parallel Xcode ACP sessions each get an isolated backend bridge while Xcode consent modals remain one-per-Xcode-process and initialize phases serialize cleanly.
- Canonical proof gate `proposal-051|p051`.

This proposal does **not** include:

- Sharing one ACP language-model session between different reviewers. UX and UI remain separate ACP sessions because they have different roles, prompts, and output contracts.
- Changing XcodeBuildMCP itself.
- Removing `MCP_XCODE_PID` injection. The broker still uses it to target the intended Xcode instance.
- Implementing a stdio proxy compatibility layer as the P051 architecture. If HTTP streaming is not feasible, P051 must return to proposal revision instead of silently falling back to stdio proxying.
- UI changes in the Swift app.
- Cross-daemon bridge sharing. The pool is per control-plane daemon process.
- General pooling for every MCP server. P051 is Xcode-only; other MCP backends can be considered later after this broker proves stable.
- Broadly disabling per-agent home isolation. P051 must not solve Xcode by launching the entire ACP provider with `/Users/user` as `HOME`.
- A general-purpose shell command policy engine for every direct `xcodebuild` invocation. P051 may add focused Xcode guards/wrappers only where needed to prevent the broker contract from being bypassed.

### 3.1 Pre-Implementation Research Gate — **COMPLETE (2026-04-19)**

The research artifact is complete and committed:

```text
docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md
```

**Verdict: Proceed with scoped architecture** (one `mcpbridge` subprocess per HTTP client session, serialized initialize per Xcode PID). Phase 0 empirical probes confirm concurrent bridge coexistence, per-Xcode-process consent scope, and Gemini CLI v0.38.1 HTTP MCP support.

The artifact answered, with current primary-source or direct local evidence:

1. Do Codex, Claude, and Gemini ACP `session/new` payloads accept MCP servers over HTTP streaming or only stdio?
2. Which wire shape is supported by each provider: MCP Streamable HTTP, SSE, custom URL entries, or none?
3. Can the control-plane expose a loopback HTTP streaming MCP endpoint securely to provider subprocesses?
4. Can the broker authenticate individual provider sessions without leaking cross-agent authority?
5. Can `xcrun mcpbridge` be used as a backend behind the HTTP broker, or does the broker need a native Xcode MCP implementation?
6. If one provider lacks HTTP streaming support, must P051 narrow the provider set or stop?
7. Which exact host-user environment is required for reliable Xcode/CoreSimulator execution on the target macOS/Xcode versions?
8. Which Xcode entry points must be brokered or guarded in practice: Xcode MCP bridge only, `xcodebuild`, `xcrun simctl`, or all Xcode developer tool invocations?
9. Can the broker always select explicit simulator UUIDs for Xcode execution evidence, avoiding ambiguous `name + OS` destinations when duplicate devices exist?

The research verdict is one of:

- `Proceed`: all required providers support an HTTP streaming shape that preserves policy and observability.
- `Proceed with scoped architecture`: backend-sharing model must be revised (per-lease backend rather than per-pool-key shared backend) but HTTP streaming and host-env goals remain viable. **← current verdict**.
- `Proceed with scoped provider set`: only a named subset supports it; the proposal must be revised to limit scope before implementation.
- `Do not implement P051 as written`: HTTP streaming is not currently viable; the proposal must be revised or replaced before any implementation starts.

The P051 proof gate includes a static check that this research artifact exists and contains a non-placeholder verdict before implementation PR sign-off.

---

## 4. Problem Statement

### 4.1 ACP session reuse does not reduce first-start parallel bridge count

Session reuse works only after a provider session exists. In a fan-out phase, UX and UI reviewers both need their first session. If each first session spawns a direct `xcrun mcpbridge`, Xcode can present two modal interactions before either agent can make progress.

### 4.2 Direct stdio MCP is not an acceptable P051 architecture

The obvious implementation - pass the same `xcrun mcpbridge` process to multiple ACP providers - is unsafe. MCP over stdio assumes one JSON-RPC client owns stdin/stdout. Two independent providers cannot concurrently write to the same stdio stream without corrupting request/response ordering and lifecycle state.

A second option, wrapping provider-facing stdio with a Chainworks proxy process, is also out of scope for P051. It preserves current provider compatibility, but it keeps the architecture anchored to stdio and leaves several unresolved risks:

- every provider would still believe it owns a local process lifecycle, while Chainworks would hide a shared backend behind it
- process supervision would now span provider subprocesses, proxy subprocesses, broker state, and backend bridge state
- permission isolation would rely on proxy correctness rather than a first-class server-side session/auth model
- future replacement with HTTP streaming would require another transport migration

P051 therefore requires HTTP streaming feasibility research before implementation. If that research does not support HTTP streaming for the required providers, implementation must stop and the proposal must be revised.

### 4.3 Current runtime cannot report bridge sharing

`AgentExecution` records requested/predicted/actual MCP extensions and startup latency, but it does not distinguish:

- direct backend bridge startup
- HTTP client session startup
- backend bridge reuse
- backend bridge restart after Xcode PID drift

Without this evidence, operators cannot prove whether a retry avoided new Xcode bridge starts.

### 4.4 Failure semantics are unclear

If one ACP session is cancelled, times out, or exits after `session/close`, the direct process model kills that session's MCP bridge. With a brokered model (per-lease backend), the runtime must explicitly define when a lease is released and when its backend bridge subprocess is closed without affecting sibling leases that share the same Xcode PID.

### 4.5 Xcode is host-session-bound, not safely fake-home-bound

The existing ACP isolation model gives each provider process its own home-like root. That remains correct for model/client/plugin state. It is not a reliable execution model for Xcode.

Xcode tools depend on host-user state and services:

- CoreSimulator device sets and runtime services under the real user's `~/Library/Developer/CoreSimulator`
- Xcode DerivedData and test result bundles under `~/Library/Developer/Xcode`
- launchd/XPC services such as CoreSimulatorService and `simdiskimaged`
- Xcode MCP bridge authorization and GUI modal prompts tied to the active user session
- CloudKit/iCloud account availability for tests that initialize CloudKit-backed stores

When an isolated ACP agent runs `xcodebuild` or `xcrun mcpbridge` directly, Xcode observes a fake `HOME`/`TMPDIR` while its services still belong to the real GUI user. That can produce false "no runtimes" and simulator startup failures even though the same host shell can list simulators successfully.

P051 must therefore define Xcode as a host-session-bound capability:

- the ACP provider keeps fake `HOME` and isolated `CODEX_HOME`
- Xcode/CoreSimulator work is delegated to the broker or a broker-owned host executor
- broker-owned Xcode subprocesses run with `HOME=/Users/user` or the daemon's configured operator home, `TMPDIR` from `getconf DARWIN_USER_TEMP_DIR`, and normal macOS cache semantics
- lease tokens and permission filters remain per agent execution
- the whole ACP provider must not be granted the real home only because it requested Xcode

This makes P051 a reliability and isolation boundary, not only a bridge pooling optimization.

---

## 5. Core Behavior

### 5.1 Xcode MCP HTTP streaming broker

The control-plane owns a singleton `XcodeMcpBridgePool` inside the ACP runtime layer.

Pool key:

```text
xcode_pid + workspace_root + mcp_server_id + runtime_profile_id + permission_profile_id
```

The broker exposes one HTTP streaming MCP endpoint per ACP session (per lease). ACP providers receive an HTTP streaming MCP server entry in `session/new`; they do not receive a direct `xcrun mcpbridge` command and they do not receive a Chainworks stdio proxy command.

**Backend model (post-research):** `xcrun mcpbridge` is stdio-only and single-client-per-process by design, but Phase 0 probes confirmed that multiple `mcpbridge` subprocesses targeting the same Xcode PID coexist cleanly. The broker therefore spawns **one backend `mcpbridge` subprocess per HTTP client session (per lease)**, not one shared backend across leases:

```bash
HOME=<host-user-home> TMPDIR=<host-user-temp> MCP_XCODE_PID=<pid> xcrun mcpbridge
```

**Initialize-phase serialization:** bridges started within ~100 ms of each other race at the Xcode XPC tool-service setup and neither completes `tools/list`. The broker serializes bridge spawn + `initialize` per Xcode PID using a Mutex; the lock is released the moment `initialize` responds. Parallel `tools/call` and `tools/list` requests from sibling leases proceed concurrently.

The pool key still groups leases by policy fingerprint for permission filtering, lifecycle accounting, and observability — but it does not imply a shared backend process across leases. The provider-facing contract for P051 is HTTP streaming only.

### 5.1.1 Host-user Xcode execution boundary

The broker is the only normal path for Xcode MCP access. It owns the host-session execution boundary for Xcode-bound subprocesses.

Broker-owned Xcode processes must run with:

- `HOME` set to the configured operator home, normally `/Users/user`
- `TMPDIR` set to the value returned by `getconf DARWIN_USER_TEMP_DIR` for the operator account, not the ACP fake-home temp directory
- `CODEX_HOME` unset or set only for Codex runtime subprocesses, not used as Xcode's home
- `MCP_XCODE_PID` set when launching `xcrun mcpbridge`
- explicit simulator UUIDs whenever a simulator destination is required

ACP provider subprocesses continue to run with isolated per-session home state. The broker must not "fix" Xcode by launching the entire provider with the real user home.

If implementation needs direct host execution of `xcodebuild` or `xcrun simctl` for verification helpers, those commands must go through a broker-owned host executor or a narrowly scoped wrapper that applies the same host-user environment and records observation data. Direct agent-issued absolute-path `xcodebuild` invocations are diagnostic-only until the proposal defines and gates a guard that routes or rejects them.

### 5.2 Broker as HTTP streaming MCP server facade

For each provider client, the broker exposes a virtual MCP server session over HTTP streaming.

Required minimum MCP methods:

- `initialize`
- `tools/list`
- `tools/call`
- `resources/list` when the backend exposes resources
- `resources/read` when the backend exposes resources
- cancellation/close handling if supported by the provider transport

The broker must:

- keep provider-side JSON-RPC ids isolated per HTTP client session
- rewrite backend request ids to a broker-owned sequence when a backend bridge is used
- map backend responses back to the originating HTTP client session
- serialize backend calls unless backend capabilities prove concurrent calls are safe
- cache `tools/list` after backend initialize, with invalidation on backend restart
- preserve backend errors without converting them to success responses
- reject unauthenticated HTTP clients and clients with expired lease tokens

### 5.3 Lease lifecycle

An ACP execution that requests Xcode MCP obtains a lease before `session/new`.

Lease states:

- `reserved`: an HTTP streaming endpoint and lease token are created but the provider has not connected yet.
- `active`: provider has initialized over HTTP streaming and is attached to the broker session.
- `closing`: ACP execution is ending and HTTP stream shutdown has started.
- `released`: HTTP client session is gone and the lease no longer counts toward backend liveness.
- `orphaned`: provider process died or the daemon lost the HTTP stream before normal release.

Backend liveness (per lease, since each lease owns its own `mcpbridge`):

- Each lease's backend `mcpbridge` subprocess is alive while the lease is `reserved`, `active`, or `closing`.
- When the lease transitions to `released`, its backend bridge is closed (stdin close → graceful exit, kill after timeout).
- When the last lease for a given Xcode PID releases, the broker's per-PID Mutex and any cached Xcode metadata (consent state, `tools/list` snapshot) stay in broker memory for a short grace period (default 60s) so fast retry does not pay re-initialize cost — this is memory state, not a running process.
- After the grace period elapses with no new lease, the broker drops cached state for that Xcode PID.

There is no long-lived shared backend process to keep warm across unrelated leases.

HTTP endpoint lifecycle:

- The endpoint must bind only to loopback or a daemon-owned Unix-domain HTTP listener if supported by the provider.
- Each lease uses a random bearer token or equivalent one-time auth secret.
- Tokens are scoped to one agent execution and one pool key.
- Reserved leases expire if the provider does not connect within the startup timeout.

### 5.4 Xcode PID drift

The pool key includes the current Xcode PID.

If `pgrep -n -x Xcode` returns a different PID from the backend's PID target:

- new leases use a new pool key and start a new backend bridge
- existing leases continue until their ACP sessions end
- the old backend is closed when its leases drain
- telemetry records `xcode_pid_changed`

If no Xcode PID is available:

- the runtime falls back to direct fail-closed behavior for Xcode MCP resolution
- it must not silently start an untargeted bridge

### 5.4.1 Simulator destination stability

Brokered Xcode execution must prefer simulator UUIDs over `platform=iOS Simulator,name=...,OS=...` destination strings. Duplicate simulator names under the same runtime are allowed by CoreSimulator and have been observed in local diagnosis. Name-based destinations are therefore ambiguous and can turn a healthy host into a flaky run.

The broker or host executor must record the selected simulator id in observation data when it participates in Xcode execution. If a requested simulator name maps to multiple devices, the broker must either select a configured default UUID or fail with a clear ambiguity error.

### 5.5 Permission and policy boundaries

The broker must not become a policy bypass.

Policy rules:

- MCP resolution still owns which agents may request `xcode`.
- The pool key includes permission profile or an equivalent permission fingerprint.
- An HTTP lease may only call tools exposed to the originating `ResolvedMcpServer`.
- The broker must not let a lower-permission agent reuse a higher-permission backend capability set.
- Backend `tools/list` is filtered per lease before returning to the provider when policies differ.

### 5.6 Session reuse interaction

P051 complements session reuse; it does not replace it.

- First prompt in two parallel ACP sessions: each gets its own HTTP streaming lease and its own backend `mcpbridge` subprocess. The broker-held Mutex serializes the brief initialize window per Xcode PID; after that both bridges serve `tools/*` concurrently. Both leases share the same broker-owned host-user Xcode environment and the same already-consented Xcode process (so no duplicate modals).
- Retry/resume of the same agent while its ACP session is live: session reuse avoids both `session/new` and a new HTTP MCP client session — the existing lease and bridge subprocess continue.
- Retry/resume after ACP session died but broker grace period is still active: provider starts a new ACP session and a new lease; the bridge subprocess for the dead lease is closed during grace-period sweep. Grace period exists to suppress Xcode spin-up cost, not to share backend state across leases.
- Retry/resume after grace expiry: new backend bridge start is allowed and recorded.

### 5.7 Observability

Each `AgentExecution.actual_mcp_observation_json` should include Xcode broker fields when applicable:

```json
{
  "source": "xcode_mcp_broker",
  "backend_start_disposition": "spawned|restarted_after_pid_change",
  "pool_id": "...",
  "lease_id": "...",
  "xcode_pid": "77907",
  "backend_process_id": 24837,
  "http_endpoint": "127.0.0.1:<redacted>",
  "xcode_home_disposition": "host_user_home",
  "xcode_tmpdir_disposition": "host_user_temp",
  "simulator_selection": {
    "mode": "explicit_uuid",
    "simulator_id": "1BFCE41D-127E-495F-807D-55B9083A7AF1"
  },
  "sibling_leases_at_spawn": 1,
  "backend_initialize_wait_ms": 420,
  "backend_startup_latency_ms": 23031,
  "http_session_startup_latency_ms": 42
}
```

`sibling_leases_at_spawn` records how many other live leases existed on the same Xcode PID at bridge spawn (useful for correlating fan-out topology). `backend_initialize_wait_ms` is time spent waiting on the per-PID Mutex before calling `session/new` → `initialize`; for the first lease this is near zero.

The runtime should also log structured events:

- `xcode_mcp_lease_acquired`
- `xcode_mcp_bridge_spawned`
- `xcode_mcp_initialize_wait_ms`
- `xcode_mcp_lease_released`
- `xcode_mcp_bridge_closed`
- `xcode_mcp_pool_invalidated` (on Xcode PID drift)

---

## 6. User-Facing Behavior

For an operator running a stage with parallel UX/UI Gemini reviewers:

1. The first Xcode MCP-dependent reviewer obtains a lease, the broker spawns its dedicated `mcpbridge` subprocess under host-user environment, and Xcode's one-time consent modal may appear (if not already granted).
2. The second reviewer obtains its own lease; the broker briefly serializes against the per-Xcode-PID Mutex while the first bridge finishes `initialize`, then spawns a second `mcpbridge` subprocess. Xcode does **not** re-prompt — consent is already granted for this Xcode process.
3. If Xcode requires first-time operator confirmation, the operator handles exactly one modal for this Xcode process; siblings reuse that consent.
4. Both reviewers proceed with separate ACP sessions, separate HTTP streaming leases, separate `mcpbridge` subprocesses, and separate output files.
5. Runtime evidence shows two provider sessions, two brokered bridges under host-user environment, and one already-consented Xcode PID.

The system does not claim "one bridge serves multiple clients." It claims "one Xcode consent covers multiple bridges, and all bridges run under the correct host environment." That is the observed and implementable behavior.

For an operator running Xcode-dependent verification from an isolated ACP agent:

1. The agent remains isolated and does not receive the real user home as its process `HOME`.
2. Xcode/CoreSimulator access is delegated to the broker or a broker-owned host executor.
3. Xcode sees the normal macOS user home/temp/cache environment it requires.
4. Runtime evidence records whether Xcode ran through the host-user boundary and which simulator UUID was selected.
5. Failures caused by missing Xcode host state are reported as broker/host-executor failures, not misclassified as agent implementation failures.

---

## 7. Implementation Inventory

### ACP crate

- `control-plane/crates/acp/src/manager.rs`
  - Own `XcodeMcpBridgePool`.
  - Acquire leases before `adapter.open_session`.
  - Release leases on normal close, provider error, timeout, cancellation, and drop paths.
  - Refuse broker mode unless the P051 HTTP streaming feasibility research verdict allows the current provider set.

- `control-plane/crates/acp/src/lib.rs`
  - Add request/result fields only if required for pool observations.

- `control-plane/crates/acp/src/transport.rs`
  - Ensure session close and force-kill paths release leases.
  - Add tests for lease release on subprocess startup error and close timeout.
  - Serialize HTTP streaming MCP server entries into ACP `session/new` only for providers proven compatible by the research gate.

- `control-plane/crates/acp/src/xcode_mcp_broker.rs` (new)
  - Broker pool, HTTP streaming server, backend owner when needed, lease lifecycle, request routing, id correlation, and telemetry.

- `control-plane/crates/acp/src/xcode_mcp_http.rs` (new) or equivalent module
  - Provider-facing MCP HTTP streaming transport shape, authentication tokens, request/response framing, and connection lifecycle.

- `control-plane/crates/acp/src/xcode_host_env.rs` (new) or equivalent module
  - Resolve the configured operator home.
  - Resolve host temp/cache paths for the operator account.
  - Build the environment used by broker-owned Xcode subprocesses.
  - Redact host paths and tokens from logs where appropriate.

- `control-plane/crates/acp/src/xcode_host_executor.rs` (new, only if direct Xcode command execution is needed)
  - Execute approved Xcode-bound commands through the same host-user environment contract.
  - Prefer simulator UUIDs and reject ambiguous simulator destinations.
  - Record observation data for command, selected simulator id, host-env disposition, and exit status.
  - Remain diagnostic or broker-owned; do not expose arbitrary host shell execution to ACP agents.

### Engine crate

- `control-plane/crates/engine/src/mcp.rs`
  - Resolve `xcode` MCP entries to broker HTTP streaming transport when broker mode is enabled and research has proven provider support.
  - Preserve `MCP_XCODE_PID` targeting.
  - Preserve isolated ACP provider `HOME`/`CODEX_HOME`; do not make the whole provider host-home-backed only because Xcode MCP is requested.
  - Keep direct `xcrun mcpbridge` transport available only behind a diagnostic feature flag or explicit runtime config.
  - Do not implement a provider-facing stdio proxy fallback in P051.

- `control-plane/crates/engine/src/executor.rs`
  - Persist broker observation fields into `actual_mcp_observation_json`.
  - Persist host-env and simulator-selection observations for brokered Xcode executions.
  - Keep current Gemini + Xcode session reuse fallback.

- `control-plane/crates/engine/src/recovery.rs`
  - Ensure startup repair invalidates DB session generations whose live broker leases are gone.

### Domain / DB

No durable schema is required for P051.

Rationale:

- The broker is a runtime resource, not run truth.
- `AgentExecution.actual_mcp_observation_json` already carries runtime MCP evidence.
- Active leases do not survive daemon restart. On restart, session generations without live handles are already invalidated and new leases are acquired.

If implementation discovers that operator debugging needs persisted bridge history beyond agent execution observations, add a follow-up proposal for durable runtime telemetry rows instead of expanding P051.

### MCP server / GraphQL

No new mutating MCP or GraphQL tools are required.

Optional debug readback may be added behind existing debug surfaces only if the repo already has an internal runtime-status path. It must be read-only and must not expose a way to force-close shared bridges in P051.

### Catalog / profiles

- `examples/agents/agents.yaml`
- `examples/agents/agents_mcp_profiles_v2.yaml`

Keep explicit `session_reuse_scope: same_agent_family_within_run` for Gemini Xcode reviewers. Broker sharing is a backend optimization; catalog reuse remains useful for retries.

### Gate ownership

- `scripts/test-gate.sh`
  - Add `proposal-051|p051`.
  - Add a static preflight that fails if `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md` is missing or lacks an allowed verdict.

- `docs/reference/test-gates.md`
  - Add `proposal-051|p051` with Rust-only proof inventory.

---

## 8. Failure Semantics

### 8.1 Broker cannot start

If the HTTP streaming broker or backend Xcode bridge cannot start:

- MCP resolution or ACP startup fails closed.
- Agent execution is marked failed with `session_failed`.
- `actual_mcp_observation_json` records `backend_start_failed`.
- No direct unbrokered fallback and no stdio proxy fallback is attempted unless an explicit diagnostic config disables broker mode before run start.

### 8.1.1 Host-user environment cannot be resolved

If the broker cannot resolve a valid operator home or host temp directory:

- Xcode MCP resolution fails closed.
- The agent is not retried with fake-home direct Xcode execution.
- `actual_mcp_observation_json` records `xcode_host_env_unavailable`.
- The error message must identify the missing host-env prerequisite without leaking unrelated user-home contents.

### 8.2 HTTP client connects but backend fails mid-call

If the backend bridge or native Xcode MCP backend returns a fatal error:

- all active leases on that pool receive terminal MCP errors
- the pool is marked invalidated
- new leases start a fresh backend only if Xcode PID is still valid
- affected agent executions fail normally through ACP transport handling

### 8.3 One client cancels

If one ACP session is cancelled:

- its lease moves to `closing` then `released`
- sibling leases remain active
- backend remains alive while sibling lease count is nonzero

### 8.4 Provider never connects to HTTP broker

Reserved leases have a short connect timeout.

On timeout:

- lease becomes `orphaned`
- backend is not closed if other leases are active
- session startup fails with a clear `xcode_mcp_http_connect_timeout` reason

### 8.5 Broker process crashes

The broker is in-process with the control-plane daemon. If the daemon crashes:

- live ACP sessions are lost
- startup recovery invalidates missing live sessions
- later retries acquire fresh broker leases

No attempt is made to reconnect to orphaned provider HTTP streams after daemon restart.

---

## 9. Security and Isolation

The broker must not widen access.

Required safeguards:

- Pool keys include permission profile or a policy fingerprint.
- Tool list responses are filtered per lease, not globally trusted from the backend.
- HTTP clients cannot request arbitrary pool ids without a broker-issued lease token.
- Lease tokens are random, single-use, and scoped to one ACP session startup.
- Broker HTTP listener binds to loopback or a daemon-owned local transport proven compatible by research.
- Lease tokens are delivered only to the intended provider process and are redacted from logs.
- Logs must include ids and counts, not raw MCP request payloads by default.
- The real operator home is visible only to broker-owned Xcode subprocesses, not to arbitrary ACP provider shell commands.
- The host executor, if added, must allow only explicitly modeled Xcode-bound commands. It must not become a general "run with real HOME" API.
- The fake ACP `CODEX_HOME` remains the storage root for model/client/plugin state; Xcode host-home use must not cause Codex sessions, memories, plugin caches, or shell snapshots to write into `/Users/user`.

---

## 10. Test Plan

### Unit tests

Add focused tests for:

- research-gate preflight rejects missing or placeholder HTTP streaming feasibility artifact
- MCP resolver refuses broker mode for providers not proven compatible by the research verdict
- two concurrent Xcode lease requests serialize through the per-PID initialize Mutex and both complete a `tools/list` round-trip without interference
- parallel `tools/call` requests on sibling leases do not serialize (lock is released after initialize)
- each lease spawns its own `mcpbridge` subprocess; no backend sharing across leases
- different Xcode PIDs create independent broker state (different pool keys, independent Mutexes)
- different permission fingerprints create different pools (policy is not shared across leases)
- lease release closes its own backend bridge; sibling leases unaffected
- broker cached state (consent awareness, tools/list snapshot per PID) is dropped after grace period
- backend restart after Xcode PID change invalidates cached tool list
- request routing returns responses to the correct HTTP client session
- explicit direct-mode diagnostic config bypasses broker only when configured
- host-env builder uses operator home and host temp paths for Xcode subprocesses
- ACP provider environment remains fake-home-backed while broker-owned Xcode environment is host-home-backed
- ambiguous simulator name/OS requests are rejected or resolved to a configured UUID with recorded evidence

### Integration tests

Add ACP fixture tests for:

- two parallel `ExecutionRequest`s with Xcode MCP each get their own fixture backend; their initialize phases serialize through the broker's per-PID Mutex but complete successfully, and parallel `tools/call` is not serialized
- provider-side `mcpServers` contains an HTTP streaming Xcode MCP endpoint, not direct `xcrun mcpbridge`, when broker mode is enabled
- `actual_mcp_observation_json` records `backend_start_disposition = "spawned"` for each lease and `backend_initialize_wait_ms` for the serialization latency on late-starters
- brokered Xcode execution records `xcode_home_disposition = "host_user_home"` and does not expose the real home to the provider environment
- explicit simulator UUID selection is recorded in broker observation data
- cancellation of one execution releases only its lease
- ACP startup failure releases reserved leases

### Gate

`./scripts/test-gate.sh proposal-051` should run from `control-plane/`:

```bash
cargo test -p engine p051_http_streaming_research_gate -- --nocapture
cargo test -p acp xcode_mcp_broker -- --nocapture
cargo test -p acp http_streaming_mcp -- --nocapture
cargo test -p engine xcode_mcp_broker -- --nocapture
cargo test -p engine broker_observation -- --nocapture
```

The exact test names may differ, but the gate must prove:

- two parallel Xcode ACP sessions each spawn their own `mcpbridge` subprocess, both under host-user environment, both complete full MCP round-trip
- initialize phases serialize per Xcode PID (late-starter's `backend_initialize_wait_ms` > 0)
- HTTP streaming endpoint shape is used for brokered Xcode MCP
- policy-separated leases get independent broker state (no cross-lease policy leakage)
- lease cleanup on failure/cancellation closes only its own bridge subprocess
- runtime observation includes `backend_start_disposition`, `backend_initialize_wait_ms`, and host-user Xcode environment disposition
- runtime observation includes selected simulator UUID when applicable
- fake-home provider isolation remains active for Xcode-capable ACP sessions

No Xcode UI automation is required for this gate. Use fixture backend processes for deterministic proof.

---

## 11. Acceptance Criteria

Implementation is complete when:

- The HTTP streaming feasibility research artifact exists, has an allowed verdict, and the implementation scope matches that verdict.
- Parallel ACP sessions that request Xcode MCP each get their own lease and backend `mcpbridge` subprocess; their initialize phases serialize per Xcode PID; their `tools/*` calls run in parallel.
- ACP providers receive an HTTP streaming MCP endpoint for brokered Xcode access.
- Broker-owned Xcode subprocesses run with host-user `HOME`/`TMPDIR`; ACP providers continue to run with isolated fake-home state.
- The implementation does not grant the entire ACP provider process the real user home as the normal Xcode fix.
- Xcode destination handling prefers explicit simulator UUIDs and fails clearly on ambiguous name/OS requests.
- Direct multi-client sharing of raw `xcrun mcpbridge` stdio is not used.
- Provider-facing stdio proxying is not implemented as P051's architecture.
- Broker leases are released on success, failure, cancellation, timeout, and provider process death.
- Runtime observations distinguish backend start from backend reuse.
- Permission-separated agents get distinct broker state keyed by permission fingerprint; no policy leakage across leases.
- Existing session reuse behavior still works.
- `MCP_XCODE_PID` targeting remains active.
- Xcode host-env and simulator-selection evidence is present in `actual_mcp_observation_json` or the equivalent runtime observation surface.
- `proposal-051|p051` is registered in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.
- The proposal-specific gate passes.

---

## 12. Rollout Plan

1. Produce `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`.
2. Include host-env feasibility in the same research artifact: real operator home, host temp resolution, Xcode/CoreSimulator command shape, and simulator UUID strategy.
3. Revise P051 if the research verdict is anything other than `Proceed`.
4. Implement broker behind default-on runtime config for Xcode only.
5. Implement host-user Xcode environment handling inside the broker or host executor before dogfooding Xcode-dependent agents.
6. Keep a diagnostic escape hatch to force direct `xcrun mcpbridge` for local debugging, but do not add a stdio proxy fallback.
7. Run fixture-based gate.
8. Dogfood on a proposal-review stage with parallel Gemini UX/UI.
9. Confirm logs show:
   - two ACP sessions
   - two HTTP streaming leases
   - two backend `mcpbridge` subprocesses spawned, both under host-user environment
   - second lease's initialize waits briefly on per-PID Mutex (measured in `backend_initialize_wait_ms`)
   - one Xcode PID, one consent modal total (or zero if already granted)
   - host-user Xcode env disposition recorded in observation data
   - explicit simulator UUID when a simulator is selected
10. Remove or demote the diagnostic direct-mode path only after several successful dogfood runs.

---

## 13. Open Questions

1. Should broker idle grace be fixed at 60 seconds or configurable per runtime profile?
2. Should broker debug state be exposed through MCP `runtime.status` later, or is `actual_mcp_observation_json` enough for P051?
3. Does Xcode mcpbridge expose any server-side session state that makes tool-list caching unsafe after file/project changes?
4. Should policy-separated pools be based on permission profile name, resolved tool allowlist hash, or both?
5. Which ACP providers support HTTP streaming MCP server entries today, and what exact `session/new` wire shape do they require?
6. Is loopback HTTP with bearer tokens sufficient for local provider subprocesses, or is a Unix-domain HTTP listener required?
7. Should the configured operator home be derived from the daemon launch user, a daemon config field, or the active GUI user?
8. Which direct Xcode commands, if any, must be routed through a host executor in P051 instead of leaving direct shell invocations diagnostic-only?

Questions 1-4 do not block implementation if the conservative defaults below are used. Questions 5-8 are part of the mandatory research gate and block implementation until answered.

- 60-second grace.
- no new northbound debug tool.
- cache only `tools/list`, invalidate on backend restart.
- pool by resolved tool allowlist hash plus permission profile id.
