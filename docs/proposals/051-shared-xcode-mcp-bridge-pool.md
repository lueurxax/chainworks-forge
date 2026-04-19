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

Success is framed around four properties, not bridge-count reduction (Phase 0 confirmed each lease gets its own `mcpbridge` subprocess; P051 no longer claims "one bridge for N clients").

1. **Single host-session consent boundary.** Can a parallel stage with two Xcode-capable ACP sessions complete with at most one Xcode consent modal per Xcode process, regardless of how many `mcpbridge` subprocesses the broker spawns?
2. **Transport abstraction.** Can each ACP provider consume Xcode MCP through HTTP streaming without knowing that Chainworks is spawning backend `mcpbridge` subprocesses under a host-user environment?
3. **Modal-prone initialize serialization.** Can the broker serialize the per-Xcode-PID initialize phase so concurrent lease requests never race at Xcode's XPC tool-service setup, while keeping `tools/*` calls parallel?
4. **Per-lease backend isolation.** Can one stuck, crashed, or cancelled lease release cleanly — closing only its own backend `mcpbridge` subprocess — without affecting sibling leases on the same Xcode PID?
5. **Policy and capability preservation.** Can the broker preserve per-agent read/write policy, MCP tool-allowlist filtering, and capability reporting when brokering MCP traffic?
6. **Fail-closed behavior.** Can the system fail closed before lease/port/token allocation when the provider does not advertise `mcpCapabilities.http`, and when the broker cannot start, host-env is unavailable, or Xcode PID drifts?
7. **Host-session execution boundary.** Can Xcode and CoreSimulator commands run from a stable host-user environment without giving the whole ACP agent process access to the real user `HOME`?
8. **Direct-command containment (scoped to PATH-based and catalog-declared paths).** Can the runtime prevent direct Xcode execution (`xcodebuild`, `simctl`, `mcpbridge`, via-`xcrun` variants) from isolated fake-home ACP sessions via an enforceable shim for PATH-based invocations plus catalog lint for catalog-declared absolute paths, with an opt-in host-executor route for the narrow set of commands that legitimately need it (excluding `mcpbridge`, which remains broker-only within this enforced boundary)? LLM-improvised absolute paths at prompt time remain a residual risk with post-run warnings — they are outside the enforced boundary and handled by separate mitigations (see §5.1.1 Scope of guarantee).
9. **Observable evidence.** Can operators and tests prove the above via structured runtime observations (`backend_start_disposition`, `backend_initialize_wait_ms`, `xcode_home_disposition`, `xcode_shim_rejected`/`xcode_shim_routed`, `backend_failure_class`, selected simulator UUID) rather than inferring from log patterns?

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

### 5.1.2 Provider capability preflight and capability cache

ACP HTTP MCP must not be sent to a provider that has not advertised `mcpCapabilities.http == true` in its `initialize` response. The P051 wire emitter must gate HTTP MCP on a checked capability, and that check must happen **before** engine MCP resolution returns a transport — otherwise `resolve_mcp_servers` could mint an HTTP `ResolvedMcpServerTransport` for a stdio-only provider, a lease would be reserved, and failure would surface late inside `session/new`.

**Capability owner.** `AcpRuntimeManager` owns a `ProviderCapabilityCache: HashMap<ProbeKey, AgentCapabilities>`. The probe key captures **every input that could change what the provider advertises** — not just binary identity, because the same binary can report different capabilities under different launch configurations:

```rust
struct ProbeKey {
    adapter_family: AdapterFamily,
    runtime_profile_id: RuntimeProfileId,
    binary_fingerprint: BinaryFingerprint,   // path + mtime + size
    launch_args_fingerprint: sha256,          // sorted argv the probe will use
    launch_env_fingerprint: sha256,           // sorted capability-relevant env
    adapter_settings_fingerprint: sha256,     // AcpSessionConfig.extra / mode / config_options
}
```

`launch_env_fingerprint` covers the `capability_env` subset of env vars (`PATH` excluding shim-dir prefix, `HOME`, `TMPDIR`, `DEVELOPER_DIR`, `CODEX_HOME`, `GEMINI_API_KEY` presence flag, `CLAUDE_AGENT_*` feature flags, `ACP_EXPERIMENTAL_*`) — never secret values, only redacted/boolean fingerprints. It does **not** include `credential_env` (shim token, shim socket, shim-dir `PATH` prefix), so rotating the shim token between reuses does not invalidate the capability cache. `adapter_settings_fingerprint` covers the `AcpSessionConfig` knobs that alter provider behavior (mode, config options, structured-output intent). Any change in any fingerprinted input forces a fresh probe.

```rust
impl ProviderCapabilityProbe {
    // Probe API takes a launchable spec, not a fingerprint key.
    // ProbeKey is reserved for cache identity and audit; it cannot spawn a process.
    pub async fn probe(
        launch_spec: &ProviderLaunchSpec,
    ) -> Result<AgentCapabilities, CapabilityProbeError>;
}
```

The probe launches the provider binary **from `launch_spec` using its concrete `binary_path`, `launch_args`, `launch_env`, and `adapter_settings`** — the exact shape a real `session/new` would use, minus the actual `mcpServers` list (probe sends `mcpServers: []`). It sends `initialize` with minimal client capabilities, records the returned `AgentCapabilities`, and immediately closes via `session/close` or stdin EOF.

`AcpRuntimeManager::ensure_provider_capabilities(&launch_spec)` computes `launch_spec.probe_key()` for cache lookup, and on miss calls `ProviderCapabilityProbe::probe(launch_spec)` to launch the probe. The manager then caches `AgentCapabilities` by `ProbeKey`. One probe per unique `ProbeKey` per daemon process lifetime; result cached in-memory.

No public API in this module accepts a bare `ProbeKey` to spawn a process — keys are for cache lookup and audit, launches require a full spec.

**Preflight timing.** During `ExecutionRequest` construction, the caller first assembles a `ProviderLaunchSpec` (the exact launch shape a real `session/new` would use — binary path, argv, env, adapter settings, mode, config options), then asks `AcpRuntimeManager::ensure_provider_capabilities(&launch_spec)` for capabilities. The manager computes `ProbeKey::from(&launch_spec)` internally and returns the cached or freshly probed `AgentCapabilities`. This runs **before** `engine::mcp::resolve_mcp_servers` is called.

**Capability env vs credential env.** Shim credentials (`$CHAINWORKS_XCODE_SHIM_TOKEN`, `$XCODE_SHIM_DISPATCH_SOCKET`, shim-dir `PATH` prefix) are minted **per provider session**. A capability probe runs before any provider session exists and before any lease is minted — if the probe were required to carry real shim credentials, we'd have to mint a token for a probe that might be rejected before ever reaching `session/new`, or generate inert placeholders that make the probe diverge from the real session. Both paths break either the lifecycle fail-closed rule or the byte-identical ProbeKey guarantee.

P051 splits the launch env into two disjoint subsets:

- **Capability env** (`capability_env`): env vars that could change provider-advertised capabilities — `PATH` (excluding shim-dir prefix), `HOME`, `TMPDIR`, `DEVELOPER_DIR`, `CODEX_HOME`, `ACP_EXPERIMENTAL_*`, and any other capability-gating variables declared in the adapter's allowlist. This subset is byte-identical between probe and real session, and it is the **only** env contribution to `ProbeKey`.
- **Credential env** (`credential_env`): session-scoped credentials that do not influence provider capability advertisement — `CHAINWORKS_XCODE_SHIM_TOKEN`, `XCODE_SHIM_DISPATCH_SOCKET`, and the shim-dir `PATH` prefix. Present only in the real session's env; absent from the probe's env. Not fingerprinted in `ProbeKey`.

The probe's actual subprocess env is exactly `capability_env`. The real session's subprocess env is `capability_env ⊕ credential_env` (merged with credential env taking precedence on key collision — only `PATH` would collide, where the shim-dir prefix is prepended).

```rust
pub struct ProviderLaunchSpec {
    pub adapter_family: AdapterFamily,
    pub runtime_profile_id: RuntimeProfileId,
    pub binary_path: PathBuf,
    pub launch_args: Vec<String>,
    pub capability_env: BTreeMap<String, String>,  // byte-identical across probe + session; feeds ProbeKey
    pub credential_env: BTreeMap<String, String>,  // real-session only; NOT in ProbeKey; empty for probe
    pub adapter_settings: AcpSessionConfig,        // mode, extra, config_options
}

impl ProviderLaunchSpec {
    /// ProbeKey derives fingerprints from capability_env ONLY. credential_env is
    /// intentionally excluded so shim token rotation across reuses does not
    /// invalidate the capability cache.
    pub fn probe_key(&self) -> ProbeKey { /* derive from capability_env + args + settings */ }

    /// The env used to launch the capability-probe subprocess.
    pub fn probe_env(&self) -> &BTreeMap<String, String> { &self.capability_env }

    /// The env used to launch the real session subprocess.
    pub fn session_env(&self) -> BTreeMap<String, String> {
        // Merge credential_env over capability_env; PATH is the only known colliding key.
        let mut merged = self.capability_env.clone();
        for (k, v) in &self.credential_env {
            match k.as_str() {
                "PATH" => {
                    // Prepend credential PATH (shim dir) onto capability PATH.
                    if let Some(cap_path) = merged.get(k) {
                        merged.insert(k.clone(), format!("{}:{}", v, cap_path));
                    } else {
                        merged.insert(k.clone(), v.clone());
                    }
                }
                _ => { merged.insert(k.clone(), v.clone()); }
            }
        }
        merged
    }
}
```

impl AcpRuntimeManager {
    pub async fn ensure_provider_capabilities(
        &self,
        launch_spec: &ProviderLaunchSpec,
    ) -> Result<AgentCapabilities, CapabilityProbeError>;
}

fn resolve_mcp_servers(
    request: &ResolveMcpRequest,
    provider_caps: &AgentCapabilities,
) -> Result<Vec<ResolvedMcpServer>, McpResolutionError>;
```

The `ProviderLaunchSpec` is constructed once per `ExecutionRequest` and reused by both the capability probe **and** the real `session/new` dispatch. This closes the gap where a probe's args/env could diverge from the actual session's, producing a stale capability result.

**Adapter API contract.** The current `AcpAdapter` trait owns binary/env/config construction privately inside `open_session(&ExecutionRequest)`. Without a trait-level change, implementers could add the probe while each adapter's `open_session` continues to derive a different env or config, invalidating the preflight guarantee. P051 splits the adapter contract into two distinct specs and three methods:

- **`ProviderLaunchSpec`** — everything needed to **launch the process**: binary path, argv, `capability_env` (stable), `credential_env` (session-only; empty for probe), adapter settings. The probe consumes `probe_env() = capability_env`; the real session consumes `session_env() = capability_env ⊕ credential_env`. The `ProbeKey`-contributing slice (binary, argv, `capability_env`, adapter_settings) is byte-identical between probe and session; `credential_env` is allowed to differ (empty vs populated) and is explicitly not fingerprinted. No MCP payload fields in `ProviderLaunchSpec`.
- **`SessionNewSpec`** — everything needed to **construct the `session/new` payload**: resolved `mcpServers` list (with broker HTTP URL + bearer token for Xcode entries), effective cwd, mode, model, `_meta`, config options. Never used by the probe; used only by the real session.

```rust
pub trait AcpAdapter: Send + Sync {
    /// Build the process launch spec from the execution request. All env/args/config
    /// derivation lives here — no side-channel construction inside open_session.
    /// Produces capability_env, launch_args, binary_path, adapter_settings.
    /// Does NOT mint shim credentials — credential_env is returned empty at this phase.
    /// Shim credentials are attached by runtime.attach_session_credentials only after
    /// capability preflight and MCP resolution both succeed (see two-phase flow).
    /// Does NOT depend on resolved MCP servers — those are a session/new protocol concern.
    fn prepare_launch_spec(
        &self,
        request: &ExecutionRequest,
    ) -> Result<ProviderLaunchSpec, AdapterError>;

    /// Build the session/new protocol payload from resolved MCP servers + request.
    /// This is how resolved Xcode broker HTTP entries (URL + bearer) reach the
    /// real session. The probe never calls this — capability probing sends an
    /// empty mcpServers list directly.
    fn prepare_session_new_spec(
        &self,
        request: &ExecutionRequest,
        resolved_servers: &[ResolvedMcpServer],
    ) -> Result<SessionNewSpec, AdapterError>;

    /// Launch the provider session using the exact launch spec passed to the
    /// capability probe, then send session/new using session_new_spec. No env/args
    /// mutation is allowed in this method — the process launch is fully specified
    /// by launch_spec, and the protocol payload is fully specified by session_new_spec.
    async fn open_session_with_specs(
        &self,
        launch_spec: &ProviderLaunchSpec,
        session_new_spec: &SessionNewSpec,
    ) -> Result<AcpSessionHandle, AdapterError>;
}
```

The old `open_session(&ExecutionRequest)` method is retired. Callers (the executor in `control-plane/crates/engine/src/executor.rs`) use:

```rust
let mut launch_spec = adapter.prepare_launch_spec(&request)?; // credential_env empty
let cap_slice_before = launch_spec.capability_slice();
let caps = runtime.ensure_provider_capabilities(&launch_spec).await?;
let servers = resolve_mcp_servers(&request.mcp, &caps)?;
let session_new_spec = adapter.prepare_session_new_spec(&request, &servers)?;
runtime.attach_session_credentials(&mut launch_spec, &request).await?;
debug_assert_eq!(cap_slice_before, launch_spec.capability_slice());
let handle = adapter
    .open_session_with_specs(&launch_spec, &session_new_spec)
    .await?;
```

**Capability probe vs real session divergence — what is allowed:**

| Field | Capability probe | Real session |
|---|---|---|
| `launch_spec.binary_path` | same | same |
| `launch_spec.launch_args` | same | same |
| `launch_spec.capability_env` | same (no shim credentials) | same |
| `launch_spec.credential_env` | empty `{}` | populated (`CHAINWORKS_XCODE_SHIM_TOKEN`, `XCODE_SHIM_DISPATCH_SOCKET`, shim-dir `PATH` prefix) — not fingerprinted |
| `launch_spec.adapter_settings` | same | same |
| `session_new_spec.mcpServers` | always `[]` | resolved list (may include broker HTTP entries) |
| `session_new_spec.cwd` | frozen workspace root | frozen workspace root |
| `session_new_spec.mode` | same as real | same |
| `session_new_spec.model`, `_meta`, config | same | same |

The probe's one-shot `session/new` sends `mcpServers: []` — it is **only** measuring provider capability. The real session later sends the resolved list. The launch spec's **capability slice** (binary, argv, `capability_env`, adapter_settings) is byte-identical between probe and session; `credential_env` is intentionally different (empty vs populated). The `session_new_spec` differs only in the `mcpServers` field.

**Shared config builder.** A single `launch_spec_builder` module (`control-plane/crates/acp/src/launch_spec.rs`) holds the common derivation logic shared across adapters: resolving binary path from runtime profile, composing env from the adapter-specific baseline plus shim injection plus P050 meta-root plus host isolation, deriving adapter_settings from `AcpSessionConfig`. Each adapter's `prepare_launch_spec` calls into this shared builder and adds its adapter-specific overrides (Codex fake-home, Claude mode flags, Gemini `--acp` flag) on top.

**Invariant check (scoped to capability slice, not credential_env).** The whole `ProviderLaunchSpec` cannot be byte-identical between probe and session because `credential_env` intentionally differs. The enforced invariant is that the `CapabilityLaunchSlice` — the subset `(binary_path, launch_args, capability_env, adapter_settings)` that feeds `ProbeKey` — is the same value at both call sites:

```rust
impl ProviderLaunchSpec {
    pub fn capability_slice(&self) -> CapabilityLaunchSlice<'_> {
        CapabilityLaunchSlice {
            binary_path: &self.binary_path,
            launch_args: &self.launch_args,
            capability_env: &self.capability_env,
            adapter_settings: &self.adapter_settings,
        }
    }
}

// In the executor:
let slice_before = launch_spec.capability_slice();
let caps = runtime.ensure_provider_capabilities(&launch_spec).await?;
// ... resolve_mcp_servers, prepare_session_new_spec ...
debug_assert_eq!(slice_before, launch_spec.capability_slice());
let handle = adapter.open_session_with_specs(&launch_spec, &session_new_spec).await?;
```

In release builds the invariant is enforced structurally — the same `ProviderLaunchSpec` value (moved or borrowed) passes through both call sites; adapters cannot derive a fresh spec inside `open_session_with_specs` because the method receives it as input. `credential_env` is intentionally allowed to differ between `probe_env()` (empty) and `session_env()` (populated) — tests assert the two derived envs **separately**: probe env contains no shim credentials; session env contains shim credentials.

**Adapter tests.** Each adapter (Codex, Claude, Gemini, Auggie, Junie) has a focused test that: constructs an `ExecutionRequest`, calls `prepare_launch_spec`, then launches a fixture provider and inspects the resulting subprocess's env/args (via `/proc/<pid>/environ` on Linux, `ps eww` on macOS, or a fixture that echoes its args/env and exits). The test asserts env and argv match the launch spec field-by-field. Any divergence (additional env var, reordered arg, missed shim env injection) fails the test.

**Fail-closed contract.** If the catalog or broker resolves an MCP entry that requires HTTP transport (Xcode MCP under broker mode) but `provider_caps.mcp_capabilities.http == false`, `resolve_mcp_servers` returns a blocking issue **before any per-lease resources are allocated and before `session/new` is sent**. Note the daemon's shared loopback axum listener for `/mcp`, `/graphql`, and `/xcode-mcp` is always bound by daemon start (see §7 daemon integration) — that is a daemon-lifetime invariant, not a per-request resource. What capability failure prevents is the per-lease state: no `lease_id` is minted, no per-lease bearer token is generated, no `XcodeMcpBridgePool` entry is created, no `mcpbridge` backend subprocess is spawned, no `XcodeShimDispatchLease` is recorded, and no `session/new` payload is constructed. Specifically:

- no `XcodeMcpBridgePool` lease is created,
- no broker HTTP port or token is minted,
- `AgentExecution.actual_mcp_observation_json` records `provider_http_mcp_unsupported` with the adapter family and binary fingerprint,
- the stage settles as `failed_before_output`.

**Cache invalidation.** The cache entry is keyed on the full `ProbeKey` tuple. Any of these invalidates a lookup:

- provider upgrade (binary path, mtime, or size changes),
- runtime profile change (e.g., switching from `codex-reasoning-high` to `codex-reasoning-medium` where effort-dependent capabilities might differ),
- launch args change (e.g., Gemini switching between `--acp` and `--experimental-acp`, which happen to be synonyms today but could diverge),
- launch env change (a capability-gating env var was added/removed/toggled),
- adapter-settings change (e.g., Codex `mode: full-access` vs `mode: bypassPermissions` affecting advertised tool sets).

A lookup miss triggers a fresh probe, records the fresh result, and proceeds. The cache never returns cross-profile results.

### 5.1.1 Host-user Xcode execution boundary

The broker is the only normal path for Xcode MCP access. It owns the host-session execution boundary for Xcode-bound subprocesses.

Broker-owned Xcode processes must run with:

- `HOME` set to the configured operator home, normally `/Users/user`
- `TMPDIR` set to the value returned by `getconf DARWIN_USER_TEMP_DIR` for the operator account, not the ACP fake-home temp directory
- `CODEX_HOME` unset or set only for Codex runtime subprocesses, not used as Xcode's home
- `MCP_XCODE_PID` set when launching `xcrun mcpbridge`
- explicit simulator UUIDs whenever a simulator destination is required

ACP provider subprocesses continue to run with isolated per-session home state. The broker must not "fix" Xcode by launching the entire provider with the real user home.

#### Direct Xcode command guard

The reliability goal of P051 is not achieved if the same Xcode-dependent agents still invoke `xcodebuild`, `xcrun simctl`, or `xcrun mcpbridge` directly through their shell tool from a fake-home context — the CoreSimulator / `simdiskimaged` failure mode returns. P051 therefore defines a minimum, enforceable guard rather than leaving direct commands as "diagnostic-only":

**Injection condition — evaluated pre-resolution.** The shim is injected when an agent has **any modeled Xcode-dependent capability**. Because `prepare_launch_spec` runs **before** `resolve_mcp_servers`, all three injection triggers must be decidable from pre-resolution inputs — the agent catalog entry as written, plus static catalog metadata. MCP resolution output is not consulted at this stage:

1. any **requested** MCP entry in the catalog entry with server id `xcode` or `adapter_family: xcode_*`. The compiled `ResolvedAgent` exposes the requested MCP inventory (catalog author's declared intent) independent of whether resolution later succeeds. This is a static catalog property, not a runtime one.
2. any declared `requires_xcode_host_execution: true` (catalog field, per §7 catalog chain).
3. any **modeled direct-command declaration** in the agent catalog entry that matches a known Xcode-tool lexeme (`xcodebuild`, `simctl`, `mcpbridge`, `xcrun`, or any absolute path beginning with `/Applications/Xcode.app/Contents/Developer/`). The lint scans **every** field in the catalog entry where direct shell commands can be declared: `run` blocks, `shell_allowlist` entries (current catalog shape), `allowed_commands`, `tools.shell.commands`, and any adapter-specific shell-capability field defined by the catalog schema. A bare `xcodebuild` (or equivalent) in any of these fields triggers injection. Detection is performed by the catalog lint pass, which runs **at run-start compilation**, also before `resolve_mcp_servers` — its output is a static `XcodeShimInjectionSignal` attached to the `ResolvedAgent`.

The `XcodeShimInjectionSignal` is a compile-time boolean available on `ResolvedAgent` before any resolution runs. `prepare_launch_spec` reads it from `ExecutionRequest.resolved_agent.xcode_shim_injection_signal` but **does not mint any credentials** at this phase — `credential_env` is returned empty. Credentials are attached in a second explicit phase (`attach_session_credentials`) only after capability preflight and MCP resolution both succeed.

**Two-phase credential attachment.** `prepare_launch_spec` runs before capability preflight, so it cannot mint a shim token or dispatch lease — if it did, an HTTP-incompatible provider would leak per-session state before the fail-closed path runs. The executor calls:

```rust
// Phase 1: pre-preflight. credential_env is empty.
let launch_spec = adapter.prepare_launch_spec(&request, /* shim signal only */)?;
let cap_slice_before = launch_spec.capability_slice();

// Phase 2: preflight.
let caps = runtime.ensure_provider_capabilities(&launch_spec).await?;
// If this fails (e.g., provider_http_mcp_unsupported): NO token minted, NO lease,
// launch_spec is discarded. Fail-closed path returns.

// Phase 3: MCP resolution.
let servers = resolve_mcp_servers(&request.mcp, &caps)?;
let session_new_spec = adapter.prepare_session_new_spec(&request, &servers)?;

// Phase 4: credential attachment — mints token + XcodeShimDispatchLease ONLY if
// all prior phases succeeded AND XcodeShimInjectionSignal is true.
runtime.attach_session_credentials(&mut launch_spec, &request).await?;

// Phase 5: debug-assert the capability slice is unchanged after attach.
debug_assert_eq!(cap_slice_before, launch_spec.capability_slice());

// Phase 6: launch real session.
let handle = adapter.open_session_with_specs(&launch_spec, &session_new_spec).await?;
```

`attach_session_credentials` is the sole code path that mints `XcodeShimDispatchLease` and writes to `launch_spec.credential_env`. It only runs if capability preflight succeeded, MCP resolution succeeded, and the shim signal is set. Failure before this phase → no shim state allocated, per the fail-closed contract.

The debug-assert after Phase 4 verifies that `attach_session_credentials` only mutates `credential_env`, never the capability slice — i.e., the capability-slice equality (§5.1.2 invariant) is preserved across credential attachment.

An agent that uses direct `xcodebuild` but has no Xcode MCP server still receives the shim, the token, and the socket — the third trigger fires at catalog compilation time. An agent that only requests Xcode MCP (no direct commands, no `requires_xcode_host_execution`) also receives the shim via the first trigger — the catalog *requests* Xcode MCP regardless of whether later resolution succeeds or fails. Agents with zero Xcode signals get no shim; their provider subprocess keeps its unmodified `PATH`.

If resolution **later** fails (e.g., `provider_http_mcp_unsupported`), the shim env is already in the `credential_env`, but the provider session never starts, so the shim dispatch socket is never used. This is intentional — the shim injection decision is frozen at catalog compilation, and a later resolution failure simply aborts the run with per §5.1.2 before the provider launches.

**Guard mechanism — PATH shim + absolute-path catalog lint.** For agents meeting any injection trigger, the broker prepends a daemon-managed shim directory to the provider's `PATH`:

```text
$XCODE_SHIM_DIR:$ORIGINAL_PATH
```

The shim directory contains executables named `xcodebuild`, `simctl`, `mcpbridge`, **and `xcrun`** that intercept shell invocations. Each shim is a small Rust binary that:

1. reads the invocation arguments and the current process's effective home (from `$HOME`),
2. reads a daemon-issued dispatch token from the environment (`CHAINWORKS_XCODE_SHIM_TOKEN`),
3. connects to the broker's local Unix-domain dispatch socket (`$XCODE_SHIM_DISPATCH_SOCKET`),
4. dispatches to one of three paths based on agent policy (below).

**`xcrun` shim — option-aware subcommand parsing.** `xcrun` accepts several leading options before the subcommand, e.g. `xcrun --sdk iphonesimulator simctl list` or `xcrun --toolchain swift-latest mcpbridge`. Naive `argv[1]` inspection would miss these and pass-through dangerous invocations. The shim implements a proper option parser with the known `xcrun` flag set from `xcrun --help`:

- option-with-arg flags consume the next token: `--sdk <name>`, `--toolchain <name>`, `--log <path>`, `-r <path>`, `--run <path>`.
- option-without-arg flags: `--find`, `-f`, `--help`, `-h`, `--version`, `--verbose`, `--no-cache`, `--kill-cache`, `--show-sdk-path`, `--show-sdk-version`, `--show-sdk-platform-path`, `--show-sdk-platform-version`, `--show-sdk-build-version`.
- Unknown flag: fail closed (reject with `xcrun_unknown_option`) rather than silently pass through. The flag set is updated when Apple ships new `xcrun` options; until then, unknown is treated as adversarial.

After skipping options, the first non-option token is treated as the subcommand. The intercepted set is **`xcodebuild`, `simctl`, and `mcpbridge`** — matching the host-executor allowlist plus the mcpbridge unconditional reject. For each:

- `xcodebuild` — apply reject/route/diagnostic policy identical to bare `xcodebuild`.
- `simctl` — apply reject/route/diagnostic policy identical to bare `simctl`.
- `mcpbridge` — unconditional reject (even with `requires_xcode_host_execution: true`), identical to bare `mcpbridge`.

Any other subcommand (`dtrace`, `xar`, `notarytool`, `swiftc`, `xcode-select`, `clang`, etc.) does `execve("/usr/bin/xcrun", argv, envp)` pass-through, preserving the provider's isolated environment. Pass-through is silent (no observation). Absolute path `/usr/bin/xcrun` avoids shim recursion via `PATH`.

**Absolute-path containment (catalog lint).** PATH shims cannot intercept direct absolute-path invocations like `/usr/bin/xcrun simctl list` or `/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild build`. P051 closes this with a **catalog lint step**, not a runtime interceptor, because libc `execve`-level audit requires `DYLD_INSERT_LIBRARIES` (SIP-protected for Apple-signed binaries) or kernel extensions (deprecated). The lint runs at run-start during workflow compilation.

**Lint blast radius — structured executable fields only.** The hard-fail lint scans only **structured executable fields** that the compiler knows will be passed to `execve`:

- every agent's resolved `run` block command path and argv,
- every agent's resolved `run` block env values (specifically `DEVELOPER_DIR=…` paired with Xcode tools),
- required-tool declarations, if the catalog schema models them separately.

For each structured field, absolute paths matching the patterns below **fail run-start** with `xcode_absolute_path_forbidden`:

- `/usr/bin/xcrun`, `/usr/bin/xcodebuild`,
- `/Applications/Xcode.app/Contents/Developer/`,
- `/Applications/Xcode*.app/Contents/Developer/`,
- `DEVELOPER_DIR=…xcodebuild`, `DEVELOPER_DIR=…simctl` assignments.

Catalog authors must instead use bare tool names (which the shim can intercept) or opt into `requires_xcode_host_execution: true` with host-executor routing.

**Prompt/system-instruction text — warning only.** Prompt text, system instructions, agent descriptions, and any other free-form narrative fields **do not hard-fail the lint**. P051 documentation itself, internal audit notes, and normal review prompts legitimately quote `/Applications/Xcode…`, `/usr/bin/xcrun`, and `DEVELOPER_DIR=…` as inert evidence. Scanning these would block runs whose context merely discusses the forbidden commands, not runs that execute them.

Instead, prompt-text matches are emitted as a **warning** (`xcode_absolute_path_in_prompt`) captured into the agent's `actual_xcode_runtime_observation_json` as an advisory event. Operators see this warning in reports and may choose to sanitize the prompt; it does not block the run.

**Rationale for the split.** Hard-fail on executable fields, soft-warn on narrative. Executable fields are unambiguous: if they are present, the agent *will* attempt to run that command. Narrative fields are ambiguous: quoting a path in a review prompt is benign, but an LLM later synthesizing a shell command from that text is a residual risk handled by the other mitigations below.

Diagnostic mode (`CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1`) skips the hard-fail and warning scans **only for `xcodebuild` and `simctl` paths**. Any structured field whose absolute path resolves to `mcpbridge` (bare `/Applications/Xcode.app/.../mcpbridge`, `/usr/bin/xcrun mcpbridge`, or any option-prefixed `xcrun` variant that ends in `mcpbridge`) still hard-fails run-start with `xcode_absolute_path_forbidden` in diagnostic mode. This preserves the broker-only-mcpbridge contract: PATH shims cannot intercept absolute invocations, so if the lint also stepped aside for `mcpbridge`, an agent shell could run raw stdio MCP bridge via `/usr/bin/xcrun mcpbridge` outside the broker's bearer/filter/observation path. The mcpbridge exclusion is absolute — there is no diagnostic mode override anywhere in P051.

Diagnostic mode logs a WARN at daemon start and an additional WARN each time the lint is skipped for a specific agent's structured field.

**Scope of guarantee.** The containment model is:

- **PATH-based invocations** of `xcodebuild`, `simctl`, `mcpbridge`, and `xcrun <subcommand>`: enforced by shim at runtime.
- **Absolute-path invocations**: enforced by catalog lint at run-start; not enforced at runtime.
- **Agent prompt-time improvisation** (an LLM synthesizing an absolute path mid-run): not blocked by P051. This is a residual risk. Mitigations: (a) prompt templates for Xcode-dependent agents include explicit "use bare `xcodebuild`, never absolute paths" instruction, (b) agent-shell output observation flags commands starting with `/Applications/Xcode.app/` or `DEVELOPER_DIR=` to a warning stream for operator review post-run, (c) a follow-up proposal can add libc audit or sandbox-exec profiles if the residual risk becomes a real failure mode in dogfood.

P051 does not claim the residual risk is zero. It claims the enforceable boundary covers **PATH-based commands and catalog-declared absolute paths**, which is the common case observed today.

**Scoped product guarantees.** All "always rejected" and "broker-only" statements elsewhere in P051 (product questions §2, acceptance criteria §11, resolved decisions §13) apply to this enforceable boundary only. Specifically:

- "Direct `mcpbridge` is always rejected" → means rejected **for PATH-based invocations and for catalog-declared structured fields**. An LLM that synthesizes `/usr/bin/xcrun mcpbridge` in a prompt-response shell string at runtime is **not** blocked by the PATH shim (absolute path) and was not scanned by catalog lint (prompt-time, not catalog-time). This residual is handled by mitigation (b) above (post-run warning) and scoped out of the enforced boundary.
- "Broker is the only code path that spawns `xcrun mcpbridge`" → means the only code path **owned by Chainworks**. It does not claim the OS will prevent any process from spawning `xcrun mcpbridge` — an agent with shell access always has that capability; P051's mechanism is to remove it from catalog-declared and PATH-based paths. A future proposal can add libc/sandbox-exec enforcement for true OS-level rejection.

Acceptance criteria and product questions must be read with this scope. The reliability goal (no more CoreSimulator fake-home failures) is achieved for catalog-authored and PATH-invoked commands — which is the real operational failure mode that prompted P051. LLM-improvised absolute paths mid-prompt are a separate, orthogonal concern with different mitigations.

**Agent policy outcomes.** The agent catalog gains one optional field:

```yaml
agents:
  - id: xcode_ux_reviewer
    requires_xcode_host_execution: false   # default
```

- `requires_xcode_host_execution: false` (default): shim **rejects** the invocation with structured stderr:
  ```
  xcodebuild: direct invocation blocked by Chainworks broker.
  Agent <id> must use Xcode MCP tools instead of shell commands.
  Set requires_xcode_host_execution: true in catalog to opt in.
  ```
  exit code 127. `actual_xcode_runtime_observation_json.xcode_shim_events` appends an entry with the tool, via_xcrun flag, argv, cwd, and `policy_reason`.

- `requires_xcode_host_execution: true`: shim **routes** the invocation through the broker's host executor over the Unix dispatch socket. The broker runs the actual `/Applications/Xcode.app/Contents/Developer/usr/bin/<tool>` under full host-user environment, streams stdout/stderr/exit back to the shim, appends `policy_decision: "routed"` to `xcode_shim_events`, and appends a matching entry to `actual_xcode_runtime_observation_json.xcode_host_executor_events` with argv, cwd, selected simulator UUID, host-env disposition, env allowlist applied, and exit status.

**Dispatch authority is separate from MCP lease authority.** Shim authentication cannot reuse the broker HTTP MCP bearer token, because:

1. direct-Xcode-only agents have no MCP lease, no HTTP endpoint, and no MCP bearer token — but they still need a dispatch token to reach the broker,
2. mixing the two authorities would let an MCP lease token be used to request host-executor routing, and vice versa; orthogonal authorities are safer.

P051 introduces a separate **`XcodeShimDispatchToken`** minted by the broker per **provider session** at provider-launch time, whenever the shim is injected (§5.1.1 injection condition — any of the three triggers). It is distinct from any MCP bearer token.

**Token scope: provider-session, not per-execution.** A reused ACP provider session does not relaunch the provider process and therefore cannot receive a new environment variable for a later execution. Binding the token to a single `AgentExecution` would mean: (a) later executions on the same session invoke shims with a stale token whose `agent_execution_id` no longer matches the live execution, (b) frozen workspace/policy data would outlive the execution it was meant to describe. Both are wrong. The token lifetime therefore matches the provider session, not any one execution.

**Current-execution mapping and active-prompt-window model.** The broker owns per-session state:

- `current_execution_id: Option<AgentExecutionId>` — `Some` while a prompt is actively being served, `None` between prompts.
- `current_prompt_epoch: u64` — monotonically incremented on each `session/prompt`.
- `active_prompt_started_at: Option<SystemTime>` — timestamp at prompt start; `None` between prompts.

Transitions:

1. **On `session/prompt` send**: increment `current_prompt_epoch`; set `current_execution_id = Some(new_exec)`; set `active_prompt_started_at = Some(now)`.
2. **On `session/prompt` response or cancel**: set `current_execution_id = None`; set `active_prompt_started_at = None`. The broker considers the prompt window **closed**; `current_prompt_epoch` stays at its last value (for observation).

**Active-prompt-window dispatch rule.** Because the shim is a fresh subprocess spawned by the provider on each shell invocation and reads its env at spawn (env is frozen at provider launch), the shim cannot carry its own "origin epoch" in the dispatch DTO. P051 therefore does not attempt to distinguish "which epoch the shim was spawned for" — it enforces a simpler invariant:

- **Shim dispatch is only valid during an active prompt.** If `current_execution_id.is_some()`, dispatch is attributed to that execution with `dispatch_prompt_epoch: current_prompt_epoch` recorded. This is the in-prompt case.
- **Shim dispatch during an idle window** (`current_execution_id.is_none()`, i.e., between prompts): rejected with `xcode_shim_no_active_prompt`. The rejection event lands in the lease's `originating_execution_id`'s orphan bucket. This catches delayed subprocesses from a prior prompt that fire after the prompt ends but before the next one starts.

**No cross-prompt attribution — shim-enabled executions do not reuse provider sessions.** A cross-prompt attribution gap would violate P051's per-execution observation truth. Rather than document it as an accepted limitation, P051 eliminates it structurally: **when an execution's `XcodeShimInjectionSignal` is true, the run cannot reuse an existing provider session.**

Reuse-compat check (§5.6) now has an additional rule stacked on top of the existing MCP/policy/workspace matches:

- **Shim-enabled executions force fresh provider sessions.** If either (a) the new execution has `XcodeShimInjectionSignal: true`, or (b) the candidate live session was opened with `XcodeShimInjectionSignal: true`, the P047 `SessionReuseDisposition` is forced to `FreshSessionRequired`. A new provider session is started, a new shim dispatch lease minted, and the new execution is the only one that ever runs on it.

This means shim-enabled agents never share a provider session across executions. There are no cross-prompt dispatches by construction: each provider session serves exactly one `AgentExecution`, so every shim event dispatched on that session's socket belongs to exactly one execution.

**Trade-off.** Shim-enabled agents lose session-reuse savings (no prompt-cycle reuse for them). In return, observation truth is preserved without needing an origin-epoch in the shim dispatch DTO and without any `accepted limitation` caveats. Non-shim-enabled agents (pure Xcode MCP, no direct `xcodebuild` via shell, no `requires_xcode_host_execution`) continue to enjoy full session reuse.

**Future opt-in.** A future proposal (not in P051) can add `allows_shim_dispatch_reuse: true` with a dispatch-tracking mechanism (e.g., broker requires `in_progress_shim_dispatches == 0` before accepting reuse, plus epoch tracking via shim spawn-time socket query) to restore reuse for shim-enabled agents. P051 takes the simpler route: forbid reuse for shim-enabled.

**Dispatch-window model** (still applies within a single provider session, which is now always 1:1 with execution):

- `current_execution_id: Option<AgentExecutionId>` is either `Some(the one execution)` during its prompt or `None` between prompts (only the first prompt matters since there are no later prompts on this session).
- Shim dispatch during the active prompt: attribute to `current_execution_id`.
- Shim dispatch during the idle window or after session close: reject with `xcode_shim_no_active_prompt` in the lease's `originating_execution_id` orphan bucket.

This is the narrow case that still supports observation truth: even within one execution, late-firing background dispatches after the prompt completes but before session close → orphan. That's still a clean boundary because the single execution either is the active one or not.

**Dispatch attribution summary:**

| Broker state at dispatch | Behavior |
|---|---|
| Active prompt, same session | Attribute to `current_execution_id`; record `dispatch_prompt_epoch` |
| Idle window (between prompts) | Reject with `xcode_shim_no_active_prompt`; record to `originating_execution_id` orphan bucket with `is_orphan: true` |
| Provider session closed / lease released | Reject with `xcode_shim_invalid_token` (lease no longer exists) |

Rejected dispatches still produce an observation event, but attributed to a synthetic per-session `orphan` bucket:

```json
{
  "xcode_shim_events": [
    {
      "tool": "xcodebuild",
      "policy_decision": "rejected",
      "policy_reason": "xcode_shim_stale_prompt_epoch",
      "claimed_execution_id": "<whatever current was at dispatch>",
      "originating_execution_hint": "<best-effort — last known execution on this lease>",
      "dispatch_arrived_epoch": 3,
      "session_current_epoch_at_dispatch": 5
    }
  ]
}
```

The orphan bucket lives on the provider session's **originating** execution's observation (the one that first opened the lease) with an `is_orphan: true` flag, so operators can trace back delayed events even though they are rejected. This avoids silently losing observability while keeping dispatch attribution strict for live prompts.

**Scope note.** P051 does not support agents that intentionally spawn long-running Xcode processes as background daemons outlasting their prompt — such agents should instead be restructured to keep Xcode work inline. A future proposal can add a `requires_delayed_shim_dispatch: true` opt-in with explicit late-event ownership; P051 fails closed.

The broker owns a per-session mutable pointer `current_execution_id` that is updated **before each `session/prompt` dispatch**:

```rust
struct XcodeShimDispatchLease {
    token: String,                              // 32+ random bytes, constant-time compared by broker
    provider_session_id: ProviderSessionId,     // lifetime owner
    current_execution_id: Option<AgentExecutionId>, // Some during an active prompt, None between prompts
    current_prompt_epoch: u64,                  // monotonically incremented on each session/prompt
    active_prompt_started_at: Option<SystemTime>, // Some during active prompt
    originating_execution_id: AgentExecutionId, // IMMUTABLE — first execution that opened the lease; owns orphan bucket
    workspace_root: PathBuf,                    // frozen agent worktree root; set at session/new
    requires_host_execution: bool,              // frozen at session/new
    issued_at: SystemTime,
    expires_at: SystemTime,                     // max = provider session lifetime; typically 24h
}
```

Broker-owned state: `HashMap<String /* token */, XcodeShimDispatchLease>`. Minted in memory only; not persisted. Lifetime matches the provider session — released on provider-session close alongside the MCP lease (if any). Tokens are never shared across sessions.

Shim dispatch request handling:

1. `ShimDispatchRequest` arrives at broker with `token`.
2. Broker looks up lease, validates token (constant-time) and expiry.
3. Broker reads the current `current_execution_id` — events are appended to **that** execution's `actual_xcode_runtime_observation_json`, not to the execution that originally opened the provider session.
4. Policy (`requires_host_execution`, `workspace_root` cwd boundary) is read from the frozen lease fields. This is the critical invariant: if a new execution's workspace or policy would differ, the reuse-compat check (below) forces a fresh session instead of mutating frozen fields.

**Reuse-compat extension.** §5.6's compatibility check now also covers shim dispatch authority. A reused session is compatible for a new execution only if **all** of:

- P047 `SessionReuseDisposition` OK,
- accepted MCP set at `session/new` matches the new request's resolved set (§5.6 original rule),
- `workspace_root` of the new execution equals the lease's frozen `workspace_root`,
- `requires_xcode_host_execution` of the new execution's agent equals the lease's frozen flag,
- the lease itself is not expired.

Any mismatch forces reuse-incompatible supersession: old provider session closed, old shim lease released, fresh provider session + fresh token + fresh lease minted with the new execution's frozen values.

**Token delivery and visibility (threat model).** The token reaches the shim via `$CHAINWORKS_XCODE_SHIM_TOKEN` in the provider subprocess environment (alongside `$XCODE_SHIM_DISPATCH_SOCKET`). **The token is observable to any shell command the ACP agent runs** — `env`, `printenv`, and reading the env of any subprocess all reveal it. P051 does **not** rely on env-token secrecy.

The token is an **identifier**, not a secret-from-agent. It identifies which provider session's policy/workspace/current-execution state applies to a shim request. All meaningful authorization is **server-side**:

- tool is in the allowlist (`{xcodebuild, simctl}` for route, `mcpbridge` always reject),
- `requires_host_execution` policy is the frozen lease value (the agent cannot flip it mid-session by forging a different token — a forged token fails constant-time validation),
- `cwd` must be within the lease's frozen `workspace_root`,
- the token is not expired,
- argv is well-formed and matches the tool's known subcommand shape.

A shell-capable agent that reads `$CHAINWORKS_XCODE_SHIM_TOKEN` and issues a crafted `ShimDispatchRequest` directly to the dispatch socket achieves nothing beyond what it could already do by running the shimmed command — the broker still runs the full policy pipeline. The only thing it cannot do is forge a **different** session's token (since tokens are random + constant-time compared) or mint a token for a session that has no lease.

The secrecy we **do** enforce:

- token values are redacted from Chainworks logs, tracing spans, and stored observations (replaced with a hash prefix like `shim-token-a7b2…`),
- broker-internal HashMap is not exposed to any northbound surface,
- cross-session tokens cannot be used (each session has its own token; another session's token lookup fails).

What we **do not** claim:

- that an agent cannot read its own `$CHAINWORKS_XCODE_SHIM_TOKEN`,
- that an agent cannot connect to `$XCODE_SHIM_DISPATCH_SOCKET` directly without going through the shim binary.

**Dispatch DTO.** The shim sends a structured request over the Unix socket (not just argv):

```rust
struct ShimDispatchRequest {
    token: String,              // $CHAINWORKS_XCODE_SHIM_TOKEN (XcodeShimDispatchLease.token)
    tool: ShimTool,             // Xcodebuild | Simctl | Mcpbridge | XcrunPassthrough
    argv: Vec<String>,          // as received by the shim, minus argv[0]
    cwd: PathBuf,               // shim captures getcwd() at invocation time
    provider_env_snapshot: BTreeMap<String, String>, // safe subset (below)
    claimed_provider_pid: u32,  // client-supplied; broker treats as claim only
    invocation_ts: SystemTime,
}
```

**Peer-credential derivation.** Because a shell-capable agent can connect directly to the dispatch socket with a valid token (see token threat model above), fields supplied in the DTO cannot be trusted as audit identity. The broker independently derives the peer process identity from the Unix socket:

- **macOS**: `getsockopt(fd, SOL_LOCAL, LOCAL_PEERPID)` for pid; `getpeereid(fd, &uid, &gid)` for uid/gid.
- **Linux** (daemon deployments outside macOS, if any): `getsockopt(fd, SOL_SOCKET, SO_PEERCRED)` for `struct ucred { pid, uid, gid }`.

These yield a `DerivedPeerIdentity { pid, uid, gid }` that the broker records alongside the DTO's `claimed_provider_pid`. The derived identity is authoritative for audit; the claimed pid is preserved as diagnostic evidence of the shim's own view.

**Broker authorization pipeline on receiving `ShimDispatchRequest`:**

1. accept the socket connection; derive peer identity via the platform-appropriate call above. Fail closed with `xcode_shim_peer_cred_unavailable` on platforms where derivation is unsupported.
2. look up `XcodeShimDispatchLease` by `token`; reject with `xcode_shim_invalid_token` on miss or on constant-time mismatch; reject with `xcode_shim_token_expired` on `SystemTime::now() > expires_at`.
3. verify the derived `peer_uid` matches the daemon's own uid (the lease is minted for a provider subprocess that inherits the daemon's uid — any other uid is adversarial); reject with `xcode_shim_peer_uid_mismatch`.
4. resolve `agent_execution_id` from the lease; the broker uses this to append events to the correct `actual_xcode_runtime_observation_json`.
5. apply `requires_host_execution` policy from the lease (source of truth — not re-read from the agent catalog, to prevent runtime catalog mutation bypassing the decision frozen at provider launch).
6. apply `workspace_root` cwd check: reject routed `xcodebuild`/`simctl` requests whose `cwd` is outside the lease's `workspace_root`.
7. dispatch to reject path or host-executor route; append events with both `derived_peer_pid` and `claimed_provider_pid` recorded. If they disagree, emit a `peer_pid_mismatch` warning into the observation (does not reject — a legitimate shim might fork-exec through an intermediate, so the claim may trail by one pid) but the audit record uses the derived value.

**MCP bearer token vs shim dispatch token — non-overlap:**

- An MCP lease's HTTP bearer token authorizes HTTP MCP traffic to the broker's `/xcode-mcp/<lease_id>` endpoint. It cannot be used against the shim dispatch socket.
- An `XcodeShimDispatchLease.token` authorizes shim dispatch requests on the Unix-domain socket. It cannot be used as an HTTP bearer header.
- Different tokens, different sockets, different state maps. The broker rejects cross-use by construction (different code paths, different validators).

**cwd handoff.** The shim records `getcwd()` and sends it to the broker. The host executor `chdir`s to this cwd before `execve`ing the real tool — unless the cwd is outside the agent's frozen workspace root (in which case the host executor rejects with `cwd_outside_workspace`). Without cwd propagation, `xcodebuild` run from the agent's working directory (`.chainworks/worktrees/<id>`) would execute in whatever arbitrary directory the broker happened to be running in, breaking workspace-relative invocations.

**Env propagation (allowlist, not pass-through).** The host executor **does not inherit** the provider's env. It constructs the Xcode subprocess env from scratch and selectively merges a narrow allowlist from `provider_env_snapshot`:

| Env var | Policy |
|---|---|
| `HOME` | **Overridden** to operator home by host executor. Provider's fake `$HOME` is ignored. |
| `TMPDIR` | **Overridden** to operator `DARWIN_USER_TEMP_DIR`. Provider's fake `$TMPDIR` is ignored. |
| `USER`, `LOGNAME` | **Overridden** to operator account name. |
| `DEVELOPER_DIR` | **Overridden** to `xcode-select -p` value resolved at daemon start. Provider's value is ignored unless the agent declares `xcode_developer_dir_override` (future extension). |
| `PATH` | **Overridden** to a minimal Xcode-appropriate `PATH`: `/usr/bin:/bin:/usr/sbin:/sbin:<DEVELOPER_DIR>/usr/bin`. Provider's shim-prefixed `PATH` is stripped to avoid shim recursion in the subprocess. |
| `CODEX_HOME` | **Unset**. Codex fake-home must not leak into Xcode. |
| `XDG_CACHE_HOME` | **Unset**. Not used by Xcode. |
| `CHAINWORKS_*` | **Unset**. No control-plane internal env leaks to Xcode subprocess. |
| `SCHEME`, `CONFIGURATION`, `DESTINATION` | **Propagated** from provider snapshot if present (build-input env). |
| `CODE_SIGN_STYLE`, `DEVELOPMENT_TEAM` | **Propagated** if present (signing env). |
| Other env vars | **Dropped by default.** Add to allowlist in host executor config if a legitimate build flow needs them. |

The allowlist is small by design: the host executor is the trust crossing point, so anything not explicitly on the list is untrusted. Agents that need custom env should declare it in the agent catalog `env` block — that declaration is then carried into the snapshot allowlist.

**Workspace root + simulator UUID.** The broker resolves the selected simulator UUID (from `xcrun simctl list --json` at daemon start + per-request resolution if needed) and includes it in the `xcode_host_executor_events` entry. The broker verifies the agent's frozen workspace root matches a known Xcode project/workspace location before running `xcodebuild`; mismatch → reject with `policy_reason: "workspace_mismatch"` in `xcode_shim_events`.

**Streaming contract.** Host executor streams stdout/stderr back to the shim over the same Unix socket as framed chunks (`{stream: "stdout"|"stderr", bytes: <base64>}`) so the agent sees command output interleaved as normal shell would. Exit code delivered as `{exit: <i32>}` terminator. The shim then exits with the same code.

- Diagnostic escape hatch: daemon config `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` (not per-agent — global, for local debugging of `xcodebuild`/`simctl` wrapper edge cases) makes the `xcodebuild`, `simctl`, and `xcrun` (non-mcpbridge branches) shims transparent passthroughs to the real binaries. Logged at WARN level on daemon start if set. Not valid in production deployments.
- **Diagnostic does NOT bypass the `mcpbridge` guard.** `mcpbridge`, `xcrun mcpbridge`, and option-prefixed variants remain rejected even with `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1`. The broker's MCP policy boundary — bearer tokens, tool-allowlist filtering, request-id rewriting, per-lease observations — is absolute. Engineers debugging Xcode MCP issues can run `xcrun mcpbridge` directly from their own shell outside any ACP agent; there is no scenario where an agent shell should need raw stdio MCP access. If a future operator-approved privileged diagnostic exception is ever required for mcpbridge, it must be a separate flag (not the direct-diagnostic flag), must be authenticated per-use (not a set-and-forget env var), must record an audit event per invocation, and must not be settable by provider-authored shell commands.

**Allowlist scope.** The broker host executor accepts only **`xcodebuild`** and **`simctl`** (whether invoked directly or via `xcrun <subcommand>`). Any other `xcrun`-routed tool (`dtrace`, `xcode-select`, `notarytool`, etc.) is not shimmed and continues to run under the ACP provider's isolated environment — those do not depend on per-user CoreSimulator state.

**`mcpbridge` is never routed through the host executor.** Direct `mcpbridge` or `xcrun mcpbridge` invocations from an ACP provider subprocess are **always rejected by the shim**, regardless of `requires_xcode_host_execution`. Allowing raw `mcpbridge` execution under the operator home would hand the agent a stdio MCP bridge that bypasses the broker's bearer-token lease, per-lease policy filtering, tool-allowlist enforcement, request-id rewriting, and observability. Agents that need Xcode MCP access use the brokered HTTP streaming endpoint delivered through `session/new.mcpServers[]`. The only code path that spawns `xcrun mcpbridge` is the broker itself, internally, one subprocess per lease, never exposed to agent shells.

This keeps the guard narrowly scoped: P051 is not a general "run with real HOME" API, and the host executor is not a back door around the broker's policy boundary.

**Catalog migration.** `examples/agents/agents.yaml` entries that currently include direct `xcodebuild` commands must be updated in the P051 implementation PR:

- agents that should use MCP tools instead: drop the `xcodebuild` allowance, add `requires_xcode_host_execution: false` (explicit).
- agents that genuinely need direct execution (e.g., release preflight `xcodebuild clean`): add `requires_xcode_host_execution: true`.

The canonical proof gate must include a fixture test that a fake-home agent with `requires_xcode_host_execution: false` gets an explicit shim rejection rather than a CoreSimulator failure deep in the call stack.

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

**Ownership model: provider-session-owned leases.** MCP servers are immutable in a provider session — they are established during `session/new` and cannot be replaced while the session is alive. A lease's lifetime therefore must equal the lifetime of the provider session's MCP binding, not the execution that happened to trigger its creation. An Xcode MCP lease is owned by the ACP provider session that received the `session/new` payload referencing it, and survives across multiple `session/prompt` cycles on that session.

Lease states:

- `reserved`: an HTTP streaming endpoint and lease token are created but the provider has not sent `session/new` yet.
- `active`: provider received `session/new` with the lease's HTTP MCP entry, initialized over HTTP streaming, and is attached to the broker session.
- `closing`: ACP provider session is ending (normal close / cancel / timeout / crash / session reset / reuse-incompatible supersession) and HTTP stream shutdown has started.
- `released`: HTTP client session is gone and the backend `mcpbridge` subprocess has exited.
- `orphaned`: provider process died or the daemon lost the HTTP stream before normal release.

**Release triggers.** A lease transitions from `active` to `closing` only on provider-session-scoped events, not on single-execution success:

- provider session closes normally (ACP `session/close` or stdin EOF),
- execution is cancelled (cancels the provider session, which in turn releases the lease),
- provider subprocess exits unexpectedly (crash, kill, timeout),
- operator-triggered session reset (see [session-lineage-reuse-and-operator-reset.md](../reference/session-lineage-reuse-and-operator-reset.md)),
- reuse-incompatible supersession: a new execution on the same lineage fails the reuse-compat check (§5.6); the existing provider session is superseded by a fresh session, and the old lease is released as part of that supersession.

Successful completion of an individual execution does **not** release the lease — a subsequent prompt cycle on the same provider session continues to use the same lease and the same backend `mcpbridge` subprocess. This is what makes Xcode MCP session reuse (§5.6) executable.

**Backend liveness (per lease, since each lease owns its own `mcpbridge`):**

- Each lease's backend `mcpbridge` subprocess is alive while the lease is `reserved`, `active`, or `closing`.
- When the lease transitions to `released`, its backend bridge is closed (stdin close → graceful exit, kill after timeout).
- When the last lease for a given Xcode PID releases, the broker's per-PID Mutex and any cached Xcode metadata (consent state, `tools/list` snapshot) stay in broker memory for a short grace period (default 60s) so fast retry does not pay re-initialize cost — this is memory state, not a running process.
- After the grace period elapses with no new lease, the broker drops cached state for that Xcode PID.

There is no long-lived shared backend process to keep warm across unrelated leases.

**HTTP endpoint lifecycle and token model.**

- The endpoint must bind only to loopback or a daemon-owned Unix-domain HTTP listener if supported by the provider.
- Each lease holds a random bearer token (≥ 32 bytes, constant-time compared).

Tokens are **session-lifetime, not single-use**. Three distinct TTL concerns are modeled explicitly:

| Concern | Field | Behavior |
|---|---|---|
| First-connect wait | `first_connect_deadline` | How long the broker waits for the provider to make its initial HTTP MCP connection after the `session/new` payload was delivered. Default 60 s. Exceeding this moves the lease to `orphaned` and releases it. |
| MCP session binding | `bound_mcp_session_id` (set on first `initialize` response) | After the provider's first `initialize` succeeds, the broker records the `Mcp-Session-Id` header value returned to the provider and considers the lease bound to that MCP session. Subsequent requests must present both the bearer and the same `Mcp-Session-Id`. |
| Single active stream | `active_stream_count` (integer, guarded by Mutex) | The broker allows **at most one active HTTP stream** on the lease at any time. A second concurrent HTTP connection presenting the same bearer is rejected (`xcode_mcp_concurrent_stream_rejected`) as long as `active_stream_count > 0`. On stream drop (connection close, FIN, timeout), the count is decremented and a reconnect becomes eligible. |
| Session lifetime | `expires_at` (= provider session's max lifetime, typically 24 h) | The bearer remains valid for the entire provider session, supporting reconnect on transport hiccup. |

**Why no peer-pid / peer-uid check on the HTTP bearer path.** Loopback TCP does not expose `LOCAL_PEERPID` (macOS) or `SO_PEERCRED` (Linux) — those APIs require Unix-domain sockets. All three provider adapters require HTTP streaming MCP via loopback TCP (per the feasibility research), and switching to UDS would break provider compatibility. Peer-credential derivation is therefore only available on the **Unix-domain shim dispatch socket** (§5.1.1), not on the HTTP broker path. The HTTP path relies on a different identity model:

1. **Bearer + MCP session-id binding.** Each request must present the lease's bearer in `Authorization: Bearer <token>` **and** the matching `Mcp-Session-Id` header that was issued at first `initialize`. A replayed bearer without the session-id (or with a forged/different session-id) is rejected by the broker or by MCP's own session-id validation — MCP Streamable HTTP session-ids are unique per `initialize` and the server enforces uniqueness.
2. **Single-active-stream invariant.** At most one HTTP stream holds the lease at a time. A replay attempting to open a parallel stream with the same bearer is rejected. This collapses the concurrent-replay attack surface: an attacker who somehow observed the bearer (e.g., by also observing `credential_env` as a shell-capable agent) cannot open a second stream while the provider holds its own.
3. **Lease expiry.** Bearer becomes invalid after `expires_at` or on provider-session close, whichever comes first.

**Reconnect semantics (TCP-compatible):**

- If the provider's HTTP stream drops and no other stream is active, `active_stream_count` falls to 0 and the broker accepts a reconnect that presents the same bearer **and** the same `Mcp-Session-Id`. A new stream is opened, `active_stream_count` becomes 1 again, the backend `mcpbridge` subprocess continues serving.
- A reconnect that presents the same bearer but a **different** `Mcp-Session-Id` is rejected with `xcode_mcp_session_id_mismatch`. This blocks a replay attacker who learned the bearer but not the original session-id, and it blocks a fresh client accidentally presenting a stale bearer.
- A reconnect after `expires_at` is rejected with `xcode_mcp_lease_token_expired` regardless of session-id.

**Token is not single-use.** Any §9 wording still claiming single-use tokens is an error — the correct model is "session-lifetime bearer bound to a unique MCP session-id, limited to one active stream at a time."

**Reserved lease expiration** (first-connect failure): if the provider does not send `session/new` + connect within `first_connect_deadline`, the lease moves to `orphaned` and the broker closes any already-spawned `mcpbridge` subprocess.

### 5.4 Xcode PID drift

The pool key includes the current Xcode PID. Xcode PID drift is a **shared-state failure**, not a per-lease event — the old Xcode process is gone or a new one has replaced it, so the XPC tool-service that every stale-pool bridge was bound to is no longer valid. The runtime cannot safely leave those leases running.

If `pgrep -n -x Xcode` returns a different PID from the pool's backend-PID target:

- every lease on the stale pool key receives a terminal MCP error with `backend_failure_class = "pool_pid_drift"`,
- each stale lease's `mcpbridge` subprocess is closed (stdin close → SIGTERM 3s → SIGKILL 10s),
- the stale pool key is removed from broker state along with its in-memory cache (Mutex, `tools/list` snapshot, consent-granted flag),
- new leases use a new pool key tied to the new Xcode PID and start fresh backend bridges,
- telemetry records `xcode_pid_changed` with `{old_pid, new_pid, closed_lease_count}`.

This is the same termination contract described in §8.2 for shared-state failures — the two sections now agree.

If no Xcode PID is available:

- the runtime falls back to direct fail-closed behavior for Xcode MCP resolution
- it must not silently start an untargeted bridge

### 5.4.1 Simulator destination stability

Brokered Xcode execution must prefer simulator UUIDs over `platform=iOS Simulator,name=...,OS=...` destination strings. Duplicate simulator names under the same runtime are allowed by CoreSimulator and have been observed in local diagnosis. Name-based destinations are therefore ambiguous and can turn a healthy host into a flaky run.

Recording a selected UUID in observation is not sufficient by itself — the spawned `xcodebuild` subprocess must actually receive the UUID-form destination in its argv, not the original name/OS string. The host executor therefore owns an **argv rewrite contract** for `-destination` arguments:

**Destination parser (`control-plane/crates/acp/src/xcode_host_executor.rs` or equivalent).** Before `execve`ing the real `xcodebuild`, the host executor scans `argv` for `-destination <value>` pairs and `-destination=<value>` forms. For each destination value:

1. **`id=<UUID>` — pass through.** Value already names an explicit device. No rewrite; UUID recorded in observation.
2. **`platform=...,id=<UUID>[,...]` — pass through.** Platform-prefixed UUID form. No rewrite.
3. **`platform=iOS Simulator,name=<name>,OS=<os>` or similar name/OS form — resolve.** Host executor calls `xcrun simctl list devices --json` and finds all devices matching the given `name` and `OS`:
   - exactly one match: **rewrite** argv to `platform=iOS Simulator,id=<UUID>` (drop `name` and `OS`, add `id`). Record the resolved UUID plus the original name/OS in observation.
   - two or more matches (ambiguous): **reject** with `policy_reason: "simulator_destination_ambiguous"` and record `{name, os, candidate_uuids: [...]}` in the rejection observation. Does not spawn `xcodebuild`.
   - zero matches: **reject** with `policy_reason: "simulator_destination_not_found"`.
4. **Bare `platform=macOS`, `platform=OS X`, `generic/platform=iOS Simulator`** — pass through (no simulator selection needed or possible).
5. **Unparseable destination string** — reject with `policy_reason: "simulator_destination_unparseable"`.

Pass-through paths (1, 2, 4) record the destination verbatim in the `xcode_host_executor_events[]` entry. Rewrite path (3) records both original and rewritten form so operators can see what changed.

**Destination-list caching.** Host executor caches `xcrun simctl list devices --json` output at daemon start and refreshes the cache when a rewrite resolution misses (to catch newly-added simulators). The cache is not persisted.

**Observation fields.** `xcode_host_executor_events[].simulator_selection`:

```json
{
  "mode": "explicit_uuid|rewritten_from_name_os|no_simulator|rejected",
  "simulator_id": "1BFCE41D-127E-495F-807D-55B9083A7AF1",
  "original_argv_destination": "platform=iOS Simulator,name=iPhone 15,OS=17.4",
  "rewritten_argv_destination": "platform=iOS Simulator,id=1BFCE41D-127E-495F-807D-55B9083A7AF1"
}
```

### 5.5 Permission and policy boundaries

The broker must not become a policy bypass.

Policy rules:

- MCP resolution still owns which agents may request `xcode`.
- The pool key includes permission profile or an equivalent permission fingerprint.
- An HTTP lease may only call tools exposed to the originating `ResolvedMcpServer`.
- The broker must not let a lower-permission agent reuse a higher-permission backend capability set.
- Backend `tools/list` is filtered per lease before returning to the provider when policies differ.

### 5.6 Session reuse interaction

P051 composes with ACP session reuse (P047) but introduces a **reusable-session compatibility check** for MCP/broker-lease identity. MCP server configuration is established in `session/new` only; a reused session receives `session/prompt` and never receives a new MCP payload. Without the check, an execution requesting Xcode MCP could reuse a provider session that was opened without a broker lease (no `xcode` MCP entry in its `session/new`), and the per-request predicted/resolved MCP truth would be recorded as if the provider accepted it — but the live provider session has no way to call brokered Xcode tools.

**Reuse compatibility rule.** An existing provider session is eligible for reuse for a new execution request only if **all** of:

1. the P047 `SessionReuseDisposition` policy returns `Reused` or `ReusedAfterResume` as before, **and**
2. the **MCP server set accepted at the live session's `session/new`** equals the MCP server set that the new execution's `resolve_mcp_servers` output requires, **and**
3. **neither the new execution nor the live session has `XcodeShimInjectionSignal: true`** (shim-enabled executions never reuse — see §5.3 "No cross-prompt attribution" rule and preserve per-execution observation truth by construction).

Equality is evaluated on the MCP server inventory that was delivered to `session/new` (server name, transport type, endpoint identity for HTTP, command/args/env for stdio). Because Xcode MCP in broker mode resolves to an HTTP endpoint with a **per-lease bearer token and lease-bound URL**, two different Xcode-MCP requests never compare equal — even for the same agent — once the prior lease has been released. The reused-session MCP set equality therefore means:

- same set of MCP server names (requested set matches),
- for each name, same transport variant (stdio vs http/sse),
- for HTTP entries, same **requested broker contract** — the stable inputs that would resolve to a lease: broker-mode boolean, workspace root, `requires_xcode_host_execution` flag, resolved tool allowlist hash, permission profile id, Xcode PID at resolution time. The **random lease token and lease-id are not part of this comparison** — those are runtime state minted after the reuse decision, not inputs to it.

This last condition is the Xcode-specific piece: a live provider session that holds an `active` broker lease for Xcode MCP is reuse-compatible. Because leases are provider-session-owned (§5.3), an alive provider session always has its lease alive — a "released lease while provider alive" only happens on reuse-incompatible supersession (where the lease is released *because* the session is being superseded) or on explicit operator reset. If the provider session itself has closed, there is nothing to reuse and the question is moot — P047 treats that as a fresh start anyway.

**Outcomes.**

- **Reuse-compatible**: the existing provider session is reused; `session/prompt` is sent; the existing lease count continues; no new `mcpbridge` subprocess is spawned for this request.
- **Reuse-incompatible** (requested Xcode MCP set differs from the live session's accepted set, or the prior lease is no longer `active`): P047 disposition is forced to `FreshSessionRequired`. A fresh provider session is started, a fresh broker lease is acquired, and a fresh `mcpbridge` subprocess is spawned.

**Fingerprint layer vs live-session layer.** P047 binding fingerprints are built at prompt construction, write-once, and must only depend on stable requested-contract inputs — hashing a random per-session lease id is not executable because the lease does not yet exist when the fingerprint is computed. P051 therefore splits compatibility into two layers:

1. **Binding fingerprint layer (persisted, P047 write-once)** — hashes the *requested broker contract* for HTTP MCP entries: `{ broker_mode: bool, workspace_root, requires_xcode_host_execution, resolved_tool_allowlist_hash, permission_profile_id, xcode_pid_at_resolution }`. Any change in these forces `FreshSessionRequired` at the fingerprint-comparison layer.
2. **Live-session compatibility check (runtime, new for P051)** — run *after* the fingerprint check passes, verifies that the live provider session still holds an `active` broker lease with matching frozen fields. If the fingerprint matches but the lease is gone/expired/invalidated (e.g., mid-run broker degradation previously released it), the runtime check demotes the P047 disposition to `FreshSessionRequired` and a fresh session + fresh lease + fresh token are minted.

This gives the reuse decision two independent gates: persisted-contract hash (fast, pre-lease) and runtime liveness (post-fingerprint, no hashing of volatile ids). Neither gate hashes the lease token.

**Cancellation and cleanup under reuse.** When an ACP provider session is closed (normal close, cancel, crash, timeout, operator reset) its associated broker lease is released — regardless of whether the session was originally fresh or reused, regardless of how many successful executions ran on it. A reused provider session inherits lease ownership from whichever execution first opened the session; subsequent executions borrow the lease without changing its ownership, and none of them release it individually.

**Timeline examples.**

- First prompt in two parallel ACP sessions: each gets its own HTTP streaming lease and its own backend `mcpbridge` subprocess. The broker-held Mutex serializes the brief initialize window per Xcode PID; after that both bridges serve `tools/*` concurrently. Both leases share the same broker-owned host-user Xcode environment and the same already-consented Xcode process (so no duplicate modals).
- Successful first execution followed by a reused prompt on the same provider session with same MCP set: reuse-compatible. The provider session stays alive, its lease stays `active`, the `mcpbridge` subprocess continues running, and `session/prompt` is sent. No new bridge spawn.
- Retry/resume after the provider session closed (lease released with the session): not reuse — this is a fresh start. New provider session + new lease + new bridge.
- New execution on the same lineage that now requires Xcode MCP when the live session was opened **without** Xcode MCP: reuse-incompatible. The live session is superseded with a fresh session; the old lease (if any) is released as part of supersession.
- New execution on the same lineage whose requested MCP set differs from the live session's accepted set (e.g., Xcode MCP now with different permission fingerprint): reuse-incompatible. Live session superseded, old lease released, fresh session + fresh lease.
- Retry/resume after ACP session died but broker grace period is still active: provider starts a new ACP session and a new lease; the bridge subprocess for the dead lease is already closed. Grace period exists to suppress Xcode spin-up cost via cached metadata, not to share backend state across leases.
- Retry/resume after grace expiry: new backend bridge start is allowed and recorded.

### 5.7 Observability

**Envelope field.** Each `AgentExecution` gets a new sibling field alongside `actual_mcp_observation_json` named **`actual_xcode_runtime_observation_json`**. This is a dedicated envelope for everything Xcode-runtime-related so broker MCP truth and direct-command events never overwrite each other, and direct-Xcode-only agents (no MCP server request at all) still have a durable home for their shim evidence.

The envelope shape is append-only-per-execution:

```json
{
  "mcp_broker_observations": [          // one entry per brokered Xcode MCP lease
    {
      "source": "xcode_mcp_broker",
      "backend_start_disposition": "spawned|restarted_after_pid_change|reused_existing_provider_session_lease",
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
      "http_session_startup_latency_ms": 42,
      "backend_failure_class": null,     // null | "per_lease_backend" | "pool_pid_drift" | "broker_infrastructure" | "host_env_unavailable"
      // Fields populated only when backend_start_disposition == "reused_existing_provider_session_lease":
      "originating_execution_id": null,  // AgentExecutionId that first opened this lease (when reused)
      "prompt_cycle_index": 0            // 0 for the originating execution, 1+ for reused-in prompt cycles
    }
  ],
  "xcode_shim_events": [                // one entry per shim invocation (reject or pass-through-logged)
    {
      "ts": "2026-04-19T12:34:56.789Z",
      "tool": "xcodebuild|simctl|mcpbridge",
      "via_xcrun": false,
      "argv": ["-project", "Foo.xcodeproj", "build"],
      "cwd": "/path/to/agent/worktree",
      "policy_decision": "rejected|routed",
      "policy_reason": "requires_xcode_host_execution_false|mcpbridge_broker_only|xcrun_unknown_option|xcode_shim_invalid_token|xcode_shim_token_expired|xcode_shim_peer_uid_mismatch|...",
      "derived_peer_pid": 48217,        // from Unix socket peer creds, authoritative
      "derived_peer_uid": 501,
      "claimed_provider_pid": 48217,    // client-supplied; diagnostic only
      "peer_pid_mismatch": false,        // true if derived != claimed
      "exit_status": 127                // exit code delivered to the agent shell
    }
  ],
  "xcode_host_executor_events": [       // one entry per routed host-executor run
    {
      "ts": "2026-04-19T12:34:56.789Z",
      "tool": "xcodebuild|simctl",
      "argv": ["build", "-scheme", "Foo"],
      "cwd": "/path/to/agent/worktree",
      "host_env_disposition": "host_user_home",
      "env_allowlist_applied": ["HOME", "TMPDIR", "DEVELOPER_DIR", "PATH", "USER", "LOGNAME", "SCHEME", "CONFIGURATION"],
      "env_dropped_from_provider": ["CODEX_HOME", "XDG_CACHE_HOME", "CHAINWORKS_XCODE_SHIM_TOKEN"],
      "selected_simulator_id": "1BFCE41D-127E-495F-807D-55B9083A7AF1",
      "exit_status": 0,
      "duration_ms": 18234
    }
  ]
}
```

**Array semantics.**

- Each array is **append-only within one execution**. Multiple leases → multiple entries in `mcp_broker_observations`. Multiple direct-command invocations → multiple entries in `xcode_shim_events`. Never truncated, never overwritten.
- An execution with no Xcode MCP request but with direct `xcodebuild` invocation: `mcp_broker_observations: []`, `xcode_shim_events: [...]`.
- An execution with Xcode MCP and no direct commands: `mcp_broker_observations: [...]`, `xcode_shim_events: []`.
- A mixed execution records both.
- Each array entry is immutable once written; updates to lease state (e.g., `backend_failure_class` after a later crash) append a new entry with the same `lease_id` and a `status_update` field rather than mutating the original.

**Reuse observation entries.** When a later `AgentExecution` on the same lineage reuses a provider session and its existing Xcode broker lease, the executor appends an entry to the later execution's `mcp_broker_observations[]` with:

- `backend_start_disposition: "reused_existing_provider_session_lease"`,
- `lease_id`: same as the originating execution's entry,
- `backend_process_id`: same `mcpbridge` pid (not restarted),
- `originating_execution_id`: the `AgentExecutionId` that first opened the lease,
- `prompt_cycle_index`: incremented from 0 (originating execution records 0; first reuse records 1),
- `backend_initialize_wait_ms: 0` (no initialize serialization — no new `session/new`),
- `backend_startup_latency_ms: 0` (no new bridge spawn),
- `backend_failure_class: null` unless the reuse itself failed.

Every execution therefore has an independent observation entry, so reports, comparison, and readback surfaces can attribute Xcode MCP use to each execution without relying on lineage joins. The acceptance criterion "runtime observations distinguish backend start from backend reuse" is executable on every reused execution, not only the first.

**Northbound projection.** GraphQL `AgentExecution` type exposes `actualXcodeRuntimeObservation` as a structured typed field (arrays of typed records, not opaque JSON strings). MCP `reports.get` returns the same envelope at `execution.xcode_runtime_observation`. Reports and comparison readers must handle the three arrays as independent evidence streams.

**Why a new field rather than extending `actual_mcp_observation_json`.** The existing field is execution-level MCP observation (per `session/new` outcome) — it is not append-only and not scoped to Xcode. Direct-Xcode-only agents have no MCP observation at all, so there is no existing slot for shim events. Rather than overload the MCP field with multiplexed shape, P051 introduces a dedicated Xcode-runtime envelope.

**Structured event log.** In addition to the durable envelope above, the broker emits structured tracing events for live observability:

- `xcode_mcp_lease_acquired`
- `xcode_mcp_bridge_spawned`
- `xcode_mcp_initialize_wait_ms`
- `xcode_mcp_lease_released`
- `xcode_mcp_bridge_closed`
- `xcode_mcp_pool_invalidated` (on Xcode PID drift or broker infrastructure failure)
- `xcode_shim_rejected`, `xcode_shim_routed` (for each shim invocation)
- `xcode_host_executor_command` (for each routed host-executor run)

These events are transient (stderr/tracing subscriber) and do not replace the durable envelope.

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
  - Own `ProviderCapabilityCache: HashMap<ProbeKey, AgentCapabilities>` (see §5.1.2 for full `ProbeKey` shape).
  - Expose `ensure_provider_capabilities(&ProviderLaunchSpec) -> AgentCapabilities` as preflight called before `engine::mcp::resolve_mcp_servers`. The `ProviderLaunchSpec` is the single source of launch truth; implementation must assert the **capability slice** (binary, argv, capability_env, adapter_settings) is unchanged between the probe call and the later `open_session_with_specs` call. `credential_env` is intentionally empty at preflight and populated only after by `attach_session_credentials`.
  - Expose `attach_session_credentials(&mut ProviderLaunchSpec, &ExecutionRequest)` as the **sole** code path that mints shim tokens, creates `XcodeShimDispatchLease`, and writes to `credential_env`. Runs only after capability preflight and MCP resolution both succeed. Never called when `provider_http_mcp_unsupported` or any prior fail-closed path fires — guarantees §8.1 "no per-lease state allocated" contract.
  - Acquire leases before `adapter.open_session`.
  - Release leases on normal close, provider error, timeout, cancellation, and drop paths.
  - Refuse broker mode unless the P051 HTTP streaming feasibility research verdict allows the current provider set.

- `control-plane/crates/acp/src/provider_probe.rs` (new)
  - `ProviderCapabilityProbe::probe(&ProviderLaunchSpec) -> AgentCapabilities` — one-shot `initialize` subprocess launched from the spec's concrete `binary_path`/`launch_args`/`launch_env`/`adapter_settings` (minus `mcpServers`), records returned capabilities, closes immediately. `ProbeKey` is computed via `launch_spec.probe_key()` only for cache lookup and audit.
  - `ProbeKey` composed of `(adapter_family, runtime_profile_id, binary_fingerprint, launch_args_fingerprint, capability_env_fingerprint, adapter_settings_fingerprint)`. `credential_env` is intentionally excluded so rotating session credentials does not invalidate the probe cache.
  - Binary fingerprinting: path + mtime + size. No signature verification in P051.
  - Env fingerprint allowlist restricted to capability-gating env vars (never raw secret values; boolean/redacted fingerprint only).

- `control-plane/crates/acp/src/lib.rs`
  - Add request/result fields only if required for pool observations.

- `control-plane/crates/acp/src/transport.rs`
  - Ensure session close and force-kill paths release leases.
  - Add tests for lease release on subprocess startup error and close timeout.
  - Serialize HTTP streaming MCP server entries into ACP `session/new` only for providers proven compatible by the research gate.

- `control-plane/crates/acp/src/xcode_mcp_broker.rs` (new)
  - Broker pool, backend `mcpbridge` owner, lease lifecycle, request routing, id correlation, telemetry, per-PID initialize Mutex.
  - Exposes an `axum::Router` factory `broker_router(state: Arc<XcodeMcpBrokerState>) -> Router` that the daemon mounts. The broker does not bind its own listener — it rides the daemon's loopback listener.

- `control-plane/crates/acp/src/xcode_mcp_http.rs` (new) or equivalent module
  - Provider-facing MCP HTTP streaming transport shape, bearer token middleware, request/response framing, and per-lease connection lifecycle.

- `control-plane/crates/daemon/src/main.rs` (existing, extended)
  - **Route mount.** After the existing `/mcp` and `/graphql` routes are composed, the daemon merges the broker router at `/xcode-mcp`:
    ```rust
    let app = Router::new()
        .merge(graphql_routes(gql_state))
        .merge(mcp_server::http::routes(mcp_state))
        .merge(xcode_mcp_broker::broker_router(broker_state));
    ```
    Mount path for provider-facing streams: `POST /xcode-mcp/{lease_id}` (MCP Streamable HTTP over the per-lease path). Authorization: `Authorization: Bearer <lease_token>` header, validated by middleware before any MCP frame is parsed.
  - **Shared state.** `XcodeMcpBrokerState` is an `Arc` shared between (a) the router (reads lease state, dispatches MCP frames to backend `mcpbridge` via in-process channels), (b) `AcpRuntimeManager` (writes lease state, spawns backends). The shared state lives in the daemon process for its entire lifetime; dropped on SIGTERM.
  - **Bind order.** The daemon binds the axum listener first via `packaging::bind_with_fallback()`, then starts `AcpRuntimeManager` — but `AcpRuntimeManager` is **not allowed** to mint a lease URL or token until the daemon's listener is accepting connections. A readiness gate (`broker_state.set_router_ready()` called once the daemon's `axum::serve` future is spawned) blocks `ensure_provider_capabilities` and `resolve_mcp_servers` from producing HTTP MCP entries until the route is live.
  - **Broker health states.** The broker distinguishes two mid-run unhealthy states with different recovery contracts:
    - **Degraded** (`broker_state.health() == Degraded`): a non-infrastructure subsystem is unhealthy but the HTTP listener and Unix socket are still live — e.g., capability cache is temporarily unavailable, catalog lint subsystem is stalled, or a transient non-fatal broker internal error. `resolve_mcp_servers` returns `McpResolutionError::BrokerDegraded`; no new lease is minted. **Existing streams continue serving** — bearer validation, `Mcp-Session-Id` binding, and per-lease `mcpbridge` subprocesses are unaffected. Auto-recovers to `Healthy` when the degraded condition clears. This is the "degraded no-new-lease" mode.
    - **Failed infrastructure** (`broker_state.health() == Failed`): the HTTP listener died, the Unix dispatch socket died, or the broker internal token/lease store is corrupted beyond repair — i.e., existing streams cannot be served correctly. Treated as the `backend_failure_class = "broker_infrastructure"` path from §8.2: all active leases terminally close, `mcpbridge` subprocesses are reaped, `resolve_mcp_servers` returns `McpResolutionError::BrokerFailed` and the daemon flags itself unhealthy. Does not auto-recover mid-run — operator restart is required.
  - **Startup-time broker failure.** If the broker router fails to mount at daemon start (state construction error, port bind already taken), the daemon fails to start — there is no "start-degraded" mode.

- `control-plane/crates/acp/src/xcode_host_dispatch.rs` (new)
  - Unix-domain socket listener for `ShimDispatchRequest`. Owns the `XcodeShimDispatchLease` state map. Exposes a similar readiness gate — shim dispatch tokens are not minted until the socket is listening.

- `control-plane/crates/acp/src/xcode_host_env.rs` (new) or equivalent module
  - Resolve the configured operator home.
  - Resolve host temp/cache paths for the operator account.
  - Build the environment used by broker-owned Xcode subprocesses.
  - Redact host paths and tokens from logs where appropriate.

- `control-plane/crates/acp/src/xcode_host_executor.rs` (new)
  - Own `XcodeShimDispatchLease` state map (`HashMap<token, lease>`), mint tokens at provider-launch time for any shim-injected execution (whether or not the execution has an Xcode MCP lease), release on provider-session close.
  - Execute approved Xcode-bound commands (`xcodebuild`, `simctl`) under the host-user environment contract.
  - Allowlist-bound: rejects any binary not in `{xcodebuild, simctl}`. `mcpbridge` is explicitly **not** in the host-executor allowlist — see §5.1.1 direct-command guard rationale.
  - Authorize `ShimDispatchRequest` via constant-time token comparison; enforce `workspace_root` cwd check; apply frozen `requires_host_execution` flag from the lease (not re-read from catalog).
  - Prefer simulator UUIDs and reject ambiguous simulator destinations.
  - Append observation events to the correct `AgentExecution.actual_xcode_runtime_observation_json` via repository `append_xcode_runtime_observation`.
  - Expose a Unix-domain dispatch socket consumed by the shim binaries (below).

- `control-plane/crates/acp/src/xcode_shim/` (new crate or module + thin binaries)
  - Four shim executables under a daemon-managed `$XCODE_SHIM_DIR`:
    - `xcodebuild`, `simctl` — apply reject/route policy based on agent's `requires_xcode_host_execution` flag.
    - `mcpbridge` — **always rejects** direct invocation, regardless of `requires_xcode_host_execution`.
    - `xcrun` — **option-aware argv parser** covering the known `xcrun` flag set (with-arg: `--sdk`, `--toolchain`, `--log`, `-r`, `--run`; without-arg: `-f`, `--find`, `--help`, `-h`, `--version`, `--verbose`, `--no-cache`, `--kill-cache`, `--show-sdk-*`). Skips options to find first non-option subcommand; intercepts `xcodebuild`/`simctl`/`mcpbridge` and applies the corresponding policy; otherwise `execve("/usr/bin/xcrun", argv, envp)` as pass-through. Unknown option fails closed with `xcrun_unknown_option`.
  - Each shim: read argv + `$HOME` + `$CHAINWORKS_XCODE_SHIM_TOKEN` + `getcwd()` + capability-relevant env subset, connect to `$XCODE_SHIM_DISPATCH_SOCKET`, dispatch via `ShimDispatchRequest` DTO.
  - Reject path: exit 127 with structured stderr; emit `xcode_shim_rejected` observation with `{tool, via_xcrun: bool, argv, cwd, policy_reason}`.
  - Route path: stream stdout/stderr/exit from broker's host executor via framed chunks over the same Unix socket; emit `xcode_shim_routed` observation with argv, cwd, selected simulator UUID, exit status.
  - Pass-through path (`xcrun` only, non-Xcode subcommands): silent `execve`, no observation.
  - Diagnostic bypass: `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` makes `xcodebuild`, `simctl`, and `xcrun` (non-mcpbridge branches) transparent passthroughs, with WARN log at daemon start and per-invocation WARN log per shim. The `mcpbridge` shim (and `xcrun mcpbridge` variants) remains **rejected in diagnostic mode**. Policy boundary is absolute.

- `control-plane/crates/workflow/src/catalog_lint.rs` (new)
  - Run at run-start during workflow compilation.
  - **Hard-fail scope (run-blocking)**: scan only structured executable fields — every agent's resolved `run` block command path and argv, env values, and required-tool declarations — for Xcode-tool absolute paths (`/usr/bin/xcrun`, `/usr/bin/xcodebuild`, `/Applications/Xcode*.app/Contents/Developer/`) and `DEVELOPER_DIR=...xcodebuild`/`simctl` env assignments. Fail run-start with `xcode_absolute_path_forbidden` unless `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1`.
  - **`mcpbridge` hard-fail is absolute**: structured fields whose path or `xcrun` subcommand resolves to `mcpbridge` fail run-start regardless of `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC`. Diagnostic mode's bypass covers `xcodebuild`/`simctl` only; `mcpbridge` has no diagnostic exception anywhere in P051.
  - **Soft-warn scope (advisory, not blocking)**: scan prompt/system-instruction and description free-form text for the same patterns. Emit `xcode_absolute_path_in_prompt` advisory events appended to `actual_xcode_runtime_observation_json` as warnings; does **not** block run-start. This exists so P051 documentation, review prompts, and evidence quotes are not false-positives.
  - Produce the agent-level `XcodeShimInjectionSignal: bool` at compilation time, attached to `ResolvedAgent` and available pre-resolution. Triggered by: (1) requested Xcode MCP server in catalog entry, (2) `requires_xcode_host_execution: true`, (3) any bare Xcode-tool lexeme in **any** direct-command declaration field — `run` blocks, `shell_allowlist`, `allowed_commands`, `tools.shell.commands`, and any adapter-specific shell-capability field defined by the catalog schema. Existing catalog shape places Xcode tools in `shell_allowlist` rather than `run` blocks, so the lint must cover both. `prepare_launch_spec` reads the signal and populates `credential_env` before `resolve_mcp_servers` runs.

- `control-plane/crates/acp/src/launch_spec.rs` (new)
  - Shared `ProviderLaunchSpec` builder used by every adapter's `prepare_launch_spec`.
  - Resolves binary path, composes base env from runtime profile, applies P050 meta-root, applies shim env injection when triggers fire, applies host isolation (fake HOME for Codex fake-home pattern), and captures adapter settings from `AcpSessionConfig`.
  - Each adapter calls into this then overlays adapter-specific overrides.

- `control-plane/crates/acp/src/adapters/mod.rs`
  - Define `AcpAdapter` trait with three methods:
    - `prepare_launch_spec(&ExecutionRequest) -> ProviderLaunchSpec` (process-launch only, no MCP payload, `credential_env` empty).
    - `prepare_session_new_spec(&ExecutionRequest, &[ResolvedMcpServer]) -> SessionNewSpec` (protocol-payload only, carries resolved MCP entries including broker HTTP URL + bearer).
    - `open_session_with_specs(&ProviderLaunchSpec, &SessionNewSpec) -> AcpSessionHandle`.
  - Retire old `open_session(&ExecutionRequest)` signature.

- `control-plane/crates/acp/src/adapters/{codex,claude,gemini,auggie,junie}.rs`
  - Implement `prepare_launch_spec`, `prepare_session_new_spec`, and `open_session_with_specs` per adapter. `prepare_launch_spec` is the only place env/args/config are derived; `prepare_session_new_spec` is the only place mcpServers/cwd/mode/model/_meta are assembled for the protocol payload.
  - Shim env injection: for agents that meet **any** of the three injection triggers (Xcode MCP entry, `requires_xcode_host_execution: true`, or any Xcode-tool lexeme detected by catalog lint), the shared builder prepends `$XCODE_SHIM_DIR` to `PATH` and injects `CHAINWORKS_XCODE_SHIM_TOKEN` and `XCODE_SHIM_DISPATCH_SOCKET`. Adapters do not add shim env independently.
  - Agents with zero Xcode signals keep their normal `PATH`.
  - Each adapter gets two focused tests: (1) a fixture provider asserts env/argv match the launch spec field-by-field via `/proc/<pid>/environ` or an argv-echo fixture; (2) the probe's `session/new` payload has `mcpServers: []` while the real session's `session/new` payload has the resolved HTTP MCP entries — inspected via an ACP wire-capture fixture.

### Engine crate

- `control-plane/crates/engine/src/mcp.rs`
  - Change `resolve_mcp_servers` signature to accept `&AgentCapabilities` as input.
  - Resolve `xcode` MCP entries to broker HTTP streaming transport when broker mode is enabled **and** `provider_caps.mcp_capabilities.http == true`.
  - When broker mode is requested but capabilities say HTTP is unsupported, return `McpResolutionError::ProviderHttpMcpUnsupported { adapter_family, binary_fingerprint }` before lease reservation.
  - Preserve `MCP_XCODE_PID` targeting.
  - Preserve isolated ACP provider `HOME`/`CODEX_HOME`; do not make the whole provider host-home-backed only because Xcode MCP is requested.
  - Direct `xcrun mcpbridge` transport is **not** retained behind a diagnostic flag or runtime config. The broker is the only code path that spawns `xcrun mcpbridge`; engineers debugging Xcode MCP run the tool from their own shell outside any ACP agent.
  - Do not implement a provider-facing stdio proxy fallback in P051.

- `control-plane/crates/engine/src/executor.rs` (or equivalent ExecutionRequest builder)
  - Build the `ProviderLaunchSpec` from the resolved runtime profile, binding metadata, and adapter config **once** per request.
  - Call `AcpRuntimeManager::ensure_provider_capabilities(&launch_spec)` before `resolve_mcp_servers`.
  - Thread resulting `AgentCapabilities` into MCP resolution.
  - Pass the **same** `launch_spec` into the adapter's `open_session_with_specs`. Debug-assert **capability-slice** equality between probe call time and session call time — `credential_env` is allowed to differ (empty at probe, populated after `attach_session_credentials`).
  - Surface `ProviderHttpMcpUnsupported` as a blocking execution error with structured observation.

- `control-plane/crates/engine/src/executor.rs`
  - Persist broker observation fields into `actual_xcode_runtime_observation_json.mcp_broker_observations[]` (append-only); persist provider capability failures that predate lease creation into `actual_mcp_observation_json` as before.
  - Persist host-env and simulator-selection observations for brokered Xcode executions.
  - Keep current Gemini + Xcode session reuse fallback.

- `control-plane/crates/engine/src/recovery.rs`
  - Ensure startup repair invalidates DB session generations whose live broker leases are gone.

### Domain / DB

P051 adds exactly one durable column — the runtime-observation envelope defined in §5.7. Runtime state (broker pool, leases, bridges, Mutexes, caches, dispatch tokens) remains in-memory and is not persisted.

**Migration** (next sequential migration number in `control-plane/crates/db/migrations/`):

```sql
ALTER TABLE agent_executions
  ADD COLUMN actual_xcode_runtime_observation_json TEXT;
-- nullable, no default; legacy rows read back as NULL.
```

**Domain model** (`control-plane/crates/domain/src/agent.rs`):

```rust
pub struct AgentExecution {
    // ...existing fields...
    pub actual_xcode_runtime_observation_json: Option<String>,
}

// Typed envelope for serialize/deserialize:
pub struct XcodeRuntimeObservation {
    pub mcp_broker_observations: Vec<McpBrokerObservation>,
    pub xcode_shim_events: Vec<XcodeShimEvent>,
    pub xcode_host_executor_events: Vec<XcodeHostExecutorEvent>,
}
```

**Repository append semantics** (`control-plane/crates/db/src/repos/agent_executions.rs`):

Implement `append_xcode_runtime_observation(execution_id, update: XcodeRuntimeObservationUpdate)` where:

- the update is one of: push `McpBrokerObservation`, push `XcodeShimEvent`, push `XcodeHostExecutorEvent`, or patch an existing `mcp_broker_observations[]` entry identified by `lease_id` (for post-hoc `backend_failure_class` updates after a later crash). The patch path writes a new entry with a `status_update` discriminator rather than mutating the existing one — see §5.7 immutability rule.
- the write path is read-modify-write within a transaction to preserve atomicity across concurrent appends (fan-out leases, simultaneous shim invocations from parallel tool calls). SQLite WAL handles the concurrent-reader side.
- serialization failure (e.g., corrupt prior JSON) is logged at ERROR and the update is dropped — the execution does not fail because of observation-write failure, to match the precedent set by `actual_mcp_observation_json`.

**Legacy / null semantics:**

- Rows created before the migration: `actual_xcode_runtime_observation_json = NULL`. GraphQL exposes this as `null` (not as an empty envelope) so clients can distinguish "not instrumented" from "instrumented but no events".
- Rows created after the migration on executions that never touch Xcode MCP or shim: `actual_xcode_runtime_observation_json = NULL`. Same GraphQL behavior.
- Rows with any event: `actual_xcode_runtime_observation_json` contains the envelope with the relevant array populated and the other two as `[]`.

**Rationale for in-memory runtime state (unchanged from prior revision):**

- The broker itself is a runtime resource, not run truth.
- Active leases, bridge subprocesses, Mutexes, `tools/list` caches, and dispatch tokens do not survive daemon restart. On restart, session generations without live handles are already invalidated by existing P047 recovery and new leases are acquired.

If implementation discovers that operator debugging needs persisted bridge history beyond agent-execution observations, add a follow-up proposal for durable runtime telemetry rows instead of expanding P051.

### MCP server / GraphQL

**GraphQL schema addition** (`control-plane/crates/graphql-server/src/types/agent_execution.rs`):

```graphql
type AgentExecution {
  # ...existing fields...
  actualXcodeRuntimeObservation: XcodeRuntimeObservation
}

type XcodeRuntimeObservation {
  mcpBrokerObservations: [McpBrokerObservation!]!
  xcodeShimEvents: [XcodeShimEvent!]!
  xcodeHostExecutorEvents: [XcodeHostExecutorEvent!]!
}

type McpBrokerObservation { ... }   # typed fields from §5.7
type XcodeShimEvent { ... }
type XcodeHostExecutorEvent { ... }
```

Resolver reads `actual_xcode_runtime_observation_json`, deserializes, and exposes typed arrays. `null` column yields GraphQL `null` for the top-level field (not empty arrays) so downstream readers can distinguish "not instrumented".

**MCP `reports.get`** projects the same envelope at `execution.xcode_runtime_observation` with identical shape.

No new **mutating** MCP or GraphQL tools are introduced. Optional debug readback may be added behind existing debug surfaces only if the repo already has an internal runtime-status path. It must be read-only and must not expose a way to force-close shared bridges in P051.

### Catalog / compiler / engine chain

`requires_xcode_host_execution` is a new agent-catalog field that must be carried end-to-end so the broker's shim lease reads the correct frozen policy and the binding fingerprint invalidates reuse on change. Every layer in the catalog → compiler → runtime pipeline must know about it:

- **Catalog YAML schema** (`control-plane/crates/workflow/src/catalog.rs` or equivalent): add optional `requires_xcode_host_execution: bool` on the agent-entry struct; default `false`. Parser rejects any other value type.
- **Compiled `ResolvedAgent`** (`control-plane/crates/workflow/src/plan.rs`): carry `requires_xcode_host_execution: bool` onto the compiled agent struct the compiler produces.
- **`ExecutionRequest`** (`control-plane/crates/engine/src/executor.rs` or equivalent): thread the flag into the request the ACP runtime receives at lease minting time. `prepare_launch_spec` consumes it for shim-injection env decisions; `XcodeShimDispatchLease` freezes it at lease creation.
- **Binding fingerprint input** (P047 `BindingFingerprint`, see [session-lineage-reuse-and-operator-reset.md](../reference/session-lineage-reuse-and-operator-reset.md)): the fingerprint hashes the frozen `requires_xcode_host_execution` flag along with other binding components. A change to only this field between executions on the same lineage produces a different fingerprint, which forces `SessionReuseDisposition::FreshSessionRequired` in the P047 policy layer.
- **Reuse-compat check (§5.6)**: as already specified, the compat check requires the new execution's `requires_xcode_host_execution` to equal the lease's frozen flag. Because this is now part of the binding fingerprint, the compat check is enforced at the fingerprint layer as well — it is defence-in-depth, not redundant coverage.

### Catalog / profiles

- `examples/agents/agents.yaml`
- `examples/agents/agents_mcp_profiles_v2.yaml`

Keep explicit `session_reuse_scope: same_agent_family_within_run` for Gemini Xcode reviewers. Broker sharing is a backend optimization; catalog reuse remains useful for retries. During migration, annotate each agent that currently invokes `xcodebuild` directly with an explicit `requires_xcode_host_execution` value — `true` where host execution is genuinely needed (release preflight), `false` for agents that should migrate to Xcode MCP tools.

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
- `actual_xcode_runtime_observation_json.mcp_broker_observations[]` appends an entry with `backend_start_failed` and `backend_failure_class` set.
- No direct unbrokered fallback and no stdio proxy fallback is attempted. Xcode MCP access is either brokered (per §5.1) or the run fails; there is no diagnostic mode that routes MCP traffic outside the broker. `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` affects only the direct-command shim for `xcodebuild`/`simctl`/`xcrun`-non-`mcpbridge`; it does not touch MCP resolution or broker mode.

### 8.1.1 Host-user environment cannot be resolved

If the broker cannot resolve a valid operator home or host temp directory:

- Xcode MCP resolution fails closed.
- The agent is not retried with fake-home direct Xcode execution.
- `actual_xcode_runtime_observation_json.mcp_broker_observations[]` appends an entry with `backend_failure_class: "host_env_unavailable"`.
- The error message must identify the missing host-env prerequisite without leaking unrelated user-home contents.

### 8.2 HTTP client connects but backend fails mid-call

Because each lease owns its own backend `mcpbridge` subprocess, a single backend failure is **per-lease by default**:

- the failing backend's HTTP stream returns a terminal MCP error to **its own lease only**,
- that lease moves to `closing` then `released`,
- the failed `mcpbridge` subprocess is reaped,
- **sibling leases on the same Xcode PID remain active** and unaffected,
- the affected agent execution fails through ACP transport handling.

Pool-wide invalidation — terminal closure of all existing leases on the affected pool key — is reserved for **shared-state failure signals where existing bridges cannot continue correctly**:

- **Xcode PID drift** (`pgrep -n -x Xcode` returns a different PID than the pool key): all leases on the stale pool key receive terminal errors; their `mcpbridge` subprocesses are SIGTERM/SIGKILL'd; the pool is invalidated; new leases use a new pool key. `backend_failure_class = "pool_pid_drift"`.
- **Broker HTTP infrastructure failure** (loopback listener dies, token store corrupted): all active leases receive terminal errors because their provider HTTP streams are no longer reachable; broker marks itself unhealthy. `backend_failure_class = "broker_infrastructure"`.

**Host-env resolution failure is narrower: it is per-lease at the shim-route boundary, not pool-wide.** Already-spawned `mcpbridge` subprocesses were launched under a valid host-env snapshot and their XPC connections to Xcode remain intact; closing them would be gratuitously destructive. Instead:

- new lease acquisitions fail closed with `backend_failure_class = "host_env_unavailable"` because the broker cannot spawn a fresh bridge under an invalid host-env,
- existing MCP streams continue serving `tools/*` calls as long as the already-spawned `mcpbridge` subprocess is healthy,
- any shim-route request (routed `xcodebuild`/`simctl`) fails with `xcode_host_executor_events[]` entry recording `exit_status: host_env_unavailable` because the host executor cannot resolve host-env to launch the routed tool,
- if the operator home recovers, subsequent new leases succeed; existing leases were never disturbed.

`actual_xcode_runtime_observation_json.mcp_broker_observations[].backend_failure_class` distinguishes these: `per_lease_backend` | `pool_pid_drift` | `broker_infrastructure` | `host_env_unavailable` (last one is per-lease-at-acquire, not pool-wide).

### 8.3 One client cancels

If one ACP session is cancelled:

- its lease moves to `closing` then `released`,
- **its own `mcpbridge` subprocess is closed** (stdin close → graceful exit → SIGTERM after 3s → SIGKILL after 10s),
- sibling leases on the same Xcode PID are untouched — their backend bridges remain running, their HTTP streams remain connected.

The per-Xcode-PID Mutex and in-memory cache (e.g., cached `tools/list` snapshot, consent-granted flag) remain in broker state while at least one lease exists; those are broker memory constructs, not running processes.

### 8.4 Provider never connects to HTTP broker

Reserved leases have a short connect timeout.

On timeout:

- lease becomes `orphaned`
- the orphaned lease's own backend `mcpbridge` subprocess (if already spawned during the reservation race) is closed; sibling lease bridges are untouched
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
- Lease tokens are random (≥ 32 bytes, constant-time compared), **session-lifetime** (not single-use), bound to a unique `Mcp-Session-Id` at first `initialize`, and limited to a single active HTTP stream at a time. Reconnects must present both the bearer and the matching session-id. Peer-credential derivation is not available on the loopback TCP path; cross-session replay is blocked by the `Mcp-Session-Id` binding and the single-active-stream invariant rather than by peer pid/uid checks. The Unix-domain shim dispatch socket (a separate authority, see §5.1.1) does use peer credentials.
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
- **provider capability preflight**: when `ProviderCapabilityCache` records `mcpCapabilities.http = false` for a runtime profile, `resolve_mcp_servers` returns a blocking issue before any lease is reserved; no HTTP endpoint is bound, no token is minted, no `session/new` is sent; `actual_mcp_observation_json` records `provider_http_mcp_unsupported`
- capability probe caches `AgentCapabilities` per full `ProbeKey` (adapter family, runtime profile id, binary fingerprint, launch args fingerprint, capability-relevant launch env fingerprint, adapter settings fingerprint) and invalidates on any component change — not only binary upgrade
- two concurrent Xcode lease requests serialize through the per-PID initialize Mutex and both complete a `tools/list` round-trip without interference
- parallel `tools/call` requests on sibling leases do not serialize (lock is released after initialize)
- each lease spawns its own `mcpbridge` subprocess; no backend sharing across leases
- **backend crash isolation**: fatal MCP error from one lease's backend closes only that lease's subprocess; sibling leases on the same Xcode PID remain active and continue serving `tools/*` calls
- **pool-wide terminal closure triggers only on shared-state failure**: Xcode PID drift and broker HTTP infrastructure failure — verified by separate fixture tests. Host-env loss is per-lease at acquire/shim-route boundary and does not disturb running MCP streams.
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

**Simulator argv rewrite (P2 new)**

- pass-through: `xcodebuild -destination 'platform=iOS Simulator,id=1BFCE41D-...'` is spawned with unchanged argv; `simulator_selection.mode = "explicit_uuid"`
- rewrite on unique match: `xcodebuild -destination 'platform=iOS Simulator,name=iPhone 15,OS=17.4'` with one matching device is spawned with argv rewritten to `id=<UUID>` form; fixture inspects spawned subprocess argv via `/proc/<pid>/cmdline` or an argv-echo fixture; `simulator_selection.mode = "rewritten_from_name_os"`, both original and rewritten destination strings recorded
- reject on ambiguity: same name/OS request with two matching devices fails with `simulator_destination_ambiguous`; `xcodebuild` is not spawned; observation records the candidate UUIDs
- reject on no-match: name/OS not in device list fails with `simulator_destination_not_found`
- reject on unparseable: malformed `-destination` value fails with `simulator_destination_unparseable`
- non-simulator passes through: `xcodebuild -destination 'platform=macOS'` argv is unchanged; `simulator_selection.mode = "no_simulator"`
- **direct Xcode command guard**: fake-home agent with `requires_xcode_host_execution: false` invoking `xcodebuild` or `xcrun simctl` via shell receives an explicit shim rejection (exit 127 with structured stderr) rather than a CoreSimulator failure deep in the call stack; `actual_xcode_runtime_observation_json.xcode_shim_events[]` appends the rejection
- agent with `requires_xcode_host_execution: true` has its direct Xcode command routed through the broker host executor under host-user environment; both `xcode_shim_events[]` (policy_decision: routed) and `xcode_host_executor_events[]` (argv, cwd, simulator UUID, env allowlist, exit) are appended
- `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` makes the `xcodebuild`, `simctl`, and `xcrun`-non-`mcpbridge` shims transparent passthroughs; `mcpbridge` and `xcrun mcpbridge` remain rejected (see "mcpbridge hard-fail is absolute"); startup WARN logged

### Integration tests

Add ACP fixture tests for:

- two parallel `ExecutionRequest`s with Xcode MCP each get their own fixture backend; their initialize phases serialize through the broker's per-PID Mutex but complete successfully, and parallel `tools/call` is not serialized
- provider-side `mcpServers` contains an HTTP streaming Xcode MCP endpoint, not direct `xcrun mcpbridge`, when broker mode is enabled
- `actual_xcode_runtime_observation_json.mcp_broker_observations[]` records `backend_start_disposition = "spawned"` for each lease and `backend_initialize_wait_ms` for the serialization latency on late-starters
- reuse observation: a second `AgentExecution` that reuses an existing provider session's Xcode lease appends an `mcp_broker_observations[]` entry with `backend_start_disposition = "reused_existing_provider_session_lease"`, identical `lease_id` and `backend_process_id` to the originating execution, `originating_execution_id` pointing back, and `prompt_cycle_index >= 1`
- brokered Xcode execution records `xcode_home_disposition = "host_user_home"` and does not expose the real home to the provider environment
- explicit simulator UUID selection is recorded in broker observation data
- cancellation of one execution releases only its lease
- ACP startup failure releases reserved leases

**Catalog field propagation (P1 new)**

- YAML `requires_xcode_host_execution: true` parsed into `AgentCatalogEntry.requires_xcode_host_execution == true`; round-trip fixture
- compiled `ResolvedAgent.requires_xcode_host_execution` equals the YAML value; fixture asserts no lossy compilation
- `ExecutionRequest.requires_xcode_host_execution` equals the compiled value; fixture asserts no rewrite
- `XcodeShimDispatchLease.requires_host_execution` frozen at lease-mint time equals the `ExecutionRequest` value
- binding fingerprint changes when and only when `requires_xcode_host_execution` changes (with all other inputs held constant); fixture computes fingerprint for `false` and `true` and asserts they differ
- changing only `requires_xcode_host_execution` between two executions on the same lineage forces `SessionReuseDisposition::FreshSessionRequired`; fixture asserts the second execution gets a fresh provider session, fresh broker lease, and fresh shim token

**Session reuse compatibility**

- reuse-compatible fixture: successful execution on a session with Xcode MCP; a second execution on same lineage with same MCP set reuses the session, its lease, and its `mcpbridge` subprocess. `session/prompt` is sent. Asserts: bridge subprocess PID equals the first execution's, HTTP endpoint/token unchanged, `mcp_broker_observations[]` does not append a new spawn entry
- lease-lifetime fixture: successful execution does **not** release the lease. After execution returns success, assert the lease is still `active`, bridge subprocess is alive, HTTP endpoint still responds to a probe
- reuse-incompatible fixture (MCP set differs): live session opened without Xcode MCP, new request adds Xcode MCP → supersession forced; old lease (if any) released, fresh provider session + fresh lease + fresh bridge
- reuse-incompatible fixture (permission fingerprint differs): same MCP server name but different resolved tool allowlist hash / permission profile id → fingerprint differs → supersession forced at the fingerprint layer, before any lease is consulted
- supersession cleanup fixture: when supersession releases the old lease, the old `mcpbridge` subprocess is closed (SIGTERM 3s → SIGKILL) before the new lease acquires its bridge
- provider-session-close fixture: normal `session/close` or stdin EOF from provider releases the lease and closes the bridge
- cancellation fixture: run cancel releases the lease and closes the bridge regardless of execution success status
- operator-reset fixture: operator-triggered session reset releases the lease and closes the bridge
- binding fingerprint hashes the **requested broker contract** (broker_mode, workspace_root, requires_xcode_host_execution, resolved_tool_allowlist_hash, permission_profile_id, xcode_pid_at_resolution) — fixture asserts fingerprint differs when any of these differ and matches when all are equal; random lease tokens/ids are **not** in the input set
- live-session liveness check (new): fingerprint matches but the prior lease has been released/expired → runtime demotes disposition to `FreshSessionRequired`; fixture forces a mid-run degradation that releases the lease while provider session stays alive, then triggers a new execution on the lineage and asserts fresh lease is minted

**Observation envelope parity**

- direct-Xcode-only execution (no MCP server request at all) receives a valid `XcodeShimDispatchToken`, persists `actual_xcode_runtime_observation_json.xcode_shim_events[]` with rejection evidence; `mcp_broker_observations` is empty `[]`; execution's durable row has non-null envelope
- mixed execution (brokered Xcode MCP + direct `xcodebuild` with `requires_xcode_host_execution: true`) preserves both `mcp_broker_observations[]` and `xcode_host_executor_events[]` — neither array overwrites the other; MCP bearer token and shim dispatch token are distinct
- GraphQL `AgentExecution.actualXcodeRuntimeObservation` exposes the three arrays as typed fields; MCP `reports.get` returns the same envelope at `execution.xcode_runtime_observation`
- repeated shim invocations (three direct `xcodebuild` calls in one execution) produce three entries in `xcode_shim_events`, not one merged entry
- legacy row (pre-migration): `actualXcodeRuntimeObservation` resolves to GraphQL `null`, not an empty envelope
- post-migration execution that never touches Xcode: column remains `NULL`, GraphQL resolves to `null`

**Shim dispatch token authority**

- direct-Xcode-only execution: `XcodeShimDispatchToken` minted at provider-launch time; broker authorizes shim dispatch by token→lease lookup and appends events to the correct `AgentExecution`
- **reused session event ownership**: a single provider session runs two successful executions (E1 then E2) that both invoke `xcodebuild` via shell. Fixture asserts the broker updated `current_execution_id` and incremented `current_prompt_epoch` before E2's `session/prompt`, so E1's shim event lands in E1's `actual_xcode_runtime_observation_json.xcode_shim_events` and E2's lands in E2's — **no cross-execution contamination**. Same token value is reused across both; only the broker's internal pointers move.
- **idle-window dispatch rejection**: E1 spawns a background `xcodebuild &`; E1 prompt completes (broker sets `current_execution_id = None`); **before E2 starts**, the backgrounded `xcodebuild` reaches the shim socket. Fixture asserts the late dispatch is rejected with `xcode_shim_no_active_prompt`, the rejection lands in the lease's `originating_execution_id` orphan bucket with `is_orphan: true`, `dispatch_arrived_epoch`, `session_current_epoch_at_dispatch`, and no attribution to any live execution.
- **shim-enabled reuse is forbidden**: agent with `XcodeShimInjectionSignal: true` runs E1 → provider session 1 closed → E2 on same lineage → forced fresh provider session 2 with fresh shim lease. Fixture asserts P047 disposition is forced to `FreshSessionRequired` for shim-enabled agents even when MCP/policy/workspace inputs match. A second fixture covers the inverse: agent **without** shim signal (pure Xcode MCP, no direct xcodebuild) reuses normally per §5.6 rule 1-2. By construction, cross-prompt dispatch gap cannot occur because no shim-enabled session ever serves more than one execution.
- **reuse-incompatible on shim authority**: second execution on same lineage has a different `workspace_root` → supersession forced. Old session + old token retired; fresh session + fresh token minted with E2's workspace_root. Fixture asserts E1's shim lease is `released` before E2's is minted.
- **reuse-incompatible on policy change**: second execution has `requires_xcode_host_execution: true` while the live session was opened with `false` → supersession forced
- forged token: shim dispatch with a token not in broker's state map is rejected with `xcode_shim_invalid_token`; no event is appended to any execution
- expired token: shim dispatch with a token past `expires_at` is rejected with `xcode_shim_token_expired`
- cross-authority attempt: using an MCP bearer token as `$CHAINWORKS_XCODE_SHIM_TOKEN` (or vice versa) fails — different sockets, different validators, never cross-accepted
- **peer-credential audit**: direct socket connection with a valid token but a forged `claimed_provider_pid` in the DTO is accepted (the token is sufficient for policy), but the recorded event's `derived_peer_pid` reflects the real connecting process's pid (from `LOCAL_PEERPID`/`SO_PEERCRED`), and `peer_pid_mismatch: true` is set in the observation. A fixture runs a test process with pid X that sends a request claiming pid Y; the observation stores X and flags the mismatch.
- **peer-uid mismatch rejection**: fixture simulates a connection from a different uid (fork + setuid) and asserts the broker rejects with `xcode_shim_peer_uid_mismatch` before token validation even runs.
- token redaction: fixture asserts token values are redacted in Chainworks logs, tracing spans, and stored observations (replaced with a hash prefix). **The proposal does not claim tokens are unobservable to shell-capable agents** — `env`/`printenv` trivially expose them; P051's threat model treats the token as an identifier, not a secret from the agent
- server-side policy enforcement: even with a known token value, an agent cannot (a) escalate `requires_host_execution: false` → `true` (lease flag is frozen, forged different-token attempts fail constant-time validation), (b) route `mcpbridge` (not in host-executor allowlist), (c) change `workspace_root` (frozen at `session/new`), (d) ride another session's token (cross-session lookup fails), (e) extend expiry (broker-owned clock)
- cwd boundary: routed `xcodebuild` with `cwd` outside the lease's `workspace_root` is rejected

### Gate

`./scripts/test-gate.sh proposal-051` should run from the repository root (`scripts/test-gate.sh` is at repo root, not under `control-plane/`). The gate internally `cd`s into `control-plane/` for `cargo` invocations:

```bash
cargo test -p engine p051_http_streaming_research_gate -- --nocapture
cargo test -p acp xcode_mcp_broker -- --nocapture
cargo test -p acp http_streaming_mcp -- --nocapture
cargo test -p engine xcode_mcp_broker -- --nocapture
cargo test -p engine broker_observation -- --nocapture
```

The exact test names may differ, but the gate must prove:

**Core broker behavior**

- two parallel Xcode ACP sessions each spawn their own `mcpbridge` subprocess, both under host-user environment, both complete full MCP round-trip
- initialize phases serialize per Xcode PID (late-starter's `backend_initialize_wait_ms` > 0)
- HTTP streaming endpoint shape is used for brokered Xcode MCP
- policy-separated leases get independent broker state (no cross-lease policy leakage)

**Lease token lifecycle (P2 new)**

- first-connect deadline fixture: provider never connects within `first_connect_deadline` → lease moves to `orphaned`, any already-spawned `mcpbridge` is closed, observation records the expiry
- MCP session-id binding: provider connects, completes `initialize`, receives an `Mcp-Session-Id`; broker records `bound_mcp_session_id`. A second request with the same bearer but missing or different session-id is rejected with `xcode_mcp_session_id_mismatch`
- single-active-stream invariant: while a stream holds the lease (`active_stream_count > 0`), a second concurrent HTTP connection with the same bearer is rejected with `xcode_mcp_concurrent_stream_rejected`
- reconnect-after-hiccup fixture: drop the active HTTP stream; wait for `active_stream_count` to reach 0; reconnect with the same bearer **and** the same `Mcp-Session-Id`; accepted; backend `mcpbridge` subprocess continues serving without restart
- reconnect-with-wrong-session-id: reconnect presents valid bearer but a fresh/forged `Mcp-Session-Id` → rejected
- reconnect-after-expiry fixture: force `expires_at` to pass; reconnect is rejected with `xcode_mcp_lease_token_expired` regardless of session-id
- token is **not** single-use: fixture sends multiple MCP requests on the same stream with the same bearer; all accepted; token is not invalidated by the first use
- loopback TCP cannot assert peer-pid: fixture does **not** attempt peer-credential derivation on the HTTP bearer path; peer-cred fixtures exist only for the Unix-domain shim dispatch socket (§5.1.1)

**Daemon integration (P1 new)**

- broker router mounts at `POST /xcode-mcp/{lease_id}` on the daemon's loopback listener; fixture sends an MCP frame to the path with a valid `Authorization: Bearer <lease_token>` header and asserts the frame reaches the backend
- bind-order readiness: fixture asserts `ensure_provider_capabilities` / `resolve_mcp_servers` do not emit an HTTP MCP entry before `broker_state.set_router_ready()` is called (router ready-gate blocks lease minting)
- broker degraded (fixture sets `broker_state.health() = Degraded`): new `resolve_mcp_servers` calls return `McpResolutionError::BrokerDegraded`; no new lease/HTTP entry minted; existing streams continue serving successfully
- broker failed infrastructure (fixture sets `health() = Failed`, e.g., forces HTTP listener drop): all active leases receive terminal errors with `backend_failure_class = "broker_infrastructure"`; `mcpbridge` subprocesses are reaped; new `resolve_mcp_servers` returns `McpResolutionError::BrokerFailed`; daemon health flipped to unhealthy
- auto-recovery: Degraded → Healthy is automatic when the degraded condition clears; Failed does not auto-recover (fixture asserts restart required)
- daemon start failure when broker router cannot mount (fixture forces state construction error): daemon exits non-zero; no partial startup
- shim Unix socket readiness: `XcodeShimDispatchLease` tokens are not minted until the dispatch socket is listening

**Capability preflight**

- HTTP-incompatible provider (fixture with `mcpCapabilities.http = false`) fails closed with `provider_http_mcp_unsupported` **before** any per-lease state is allocated — no `lease_id` minted, no bearer token generated, no `XcodeMcpBridgePool` entry created, no `mcpbridge` backend spawned, no `XcodeShimDispatchLease` recorded, no `session/new` payload constructed, `credential_env` is still empty on the discarded `launch_spec`. Fixture asserts `attach_session_credentials` was never called on this path. The daemon's shared loopback listener remains bound (it's a daemon-lifetime resource) — the test asserts no per-lease resources, not that the listener comes down.
- capability-slice invariance across credential attach: fixture captures `cap_slice_before = launch_spec.capability_slice()`, runs the happy path through `attach_session_credentials`, asserts `cap_slice_before == launch_spec.capability_slice()` afterwards. Only `credential_env` changed.
- capability probe cache hit for a previously-seen full `ProbeKey` (same adapter/profile/binary/args/env/settings) skips the probe subprocess
- binary fingerprint change (path, mtime, or size) invalidates the cached entry and triggers a fresh probe
- runtime profile change forces a fresh probe even when the binary is unchanged — fixture: two profiles on the same Codex binary with different `mode` or `config_options` produce independent cache entries
- launch-env fingerprint change forces a fresh probe — fixture: toggling an `ACP_EXPERIMENTAL_*` env var between probes yields two distinct cache entries
- launch-args fingerprint change forces a fresh probe — fixture: Gemini switching between `--acp` and `--experimental-acp` produces two distinct cache entries
- **capability-slice identity**: fixture asserts `launch_spec.capability_slice()` (binary, argv, `capability_env`, `adapter_settings`) is byte-identical at `ensure_provider_capabilities` call time and at `open_session_with_specs` call time. `credential_env` is **not** part of this equality — it is intentionally empty for the probe and populated for the real session. Two additional assertions cover the env split: (a) probe subprocess's actual env does not contain `CHAINWORKS_XCODE_SHIM_TOKEN` or `XCODE_SHIM_DISPATCH_SOCKET` and its `PATH` has no shim-dir prefix; (b) real session subprocess's env does contain these credentials. Debug-assert in the executor panics at test time if the capability slice diverges.

**Per-lease vs pool-wide failure isolation (P1 new)**

- one backend `mcpbridge` crash fails only its lease; a sibling lease on the same Xcode PID continues serving `tools/*` calls and completes successfully
- Xcode PID drift (simulated by changing `pgrep` output) invalidates the pool, all stale-PID leases receive terminal errors, and new leases use a new pool key; `backend_failure_class = "pool_pid_drift"` recorded
- broker HTTP infrastructure failure (fixture listener killed) terminally closes all active leases with `backend_failure_class = "broker_infrastructure"`; broker marks itself unhealthy
- host-env unavailable (fixture with unreadable operator home): **new** lease acquisition fails closed with `backend_failure_class = "host_env_unavailable"`; an **already-running** sibling lease's MCP stream continues serving `tools/*` calls; a routed `xcodebuild` request on that existing lease fails with `exit_status: host_env_unavailable` while the MCP stream remains alive. Fixture explicitly asserts: (1) existing `mcpbridge` subprocess is still running after host-env loss, (2) new lease acquisition returns the error, (3) existing lease's `tools/list` call completes successfully, (4) routed shim request fails per-lease.

**Direct Xcode command guard**

Shim injection:

- agent with only Xcode MCP entries (no direct `xcodebuild` in any command-declaration field) gets the shim injected
- agent with only a direct `xcodebuild` entry in **`run` block** (no Xcode MCP, no `requires_xcode_host_execution`) gets the shim injected via the catalog-lint injection trigger
- agent with only `shell_allowlist: [xcodebuild, …]` (current catalog shape — no `run` entry, no MCP, no `requires_xcode_host_execution`) gets the shim injected via the expanded lint scope. Fixture uses a real catalog entry shape from `examples/agents/agents.yaml`.
- agent with `allowed_commands: [xcodebuild, …]` or `tools.shell.commands: [xcodebuild, …]` similarly triggers injection
- agent with `requires_xcode_host_execution: true` but no other Xcode signal gets the shim injected
- agent with zero Xcode signals gets no shim; its `PATH` remains unchanged

Shim enforcement (PATH-based invocations):

- fake-home agent with `requires_xcode_host_execution: false` invoking `xcodebuild -project …` via shell receives exit 127; `xcode_shim_rejected` observation with `{tool: "xcodebuild", via_xcrun: false}`
- fake-home agent invoking `xcrun xcodebuild build -scheme Foo` receives exit 127 via the xcrun shim's xcodebuild interception; `xcode_shim_rejected` with `{tool: "xcodebuild", via_xcrun: true}`
- fake-home agent invoking `xcrun --sdk macosx xcodebuild build -scheme Foo` (option-prefixed xcodebuild) is correctly parsed and receives exit 127; `xcode_shim_rejected` with `{tool: "xcodebuild", via_xcrun: true}`
- fake-home agent invoking `xcrun simctl list` receives exit 127; `xcode_shim_rejected` with `{tool: "simctl", via_xcrun: true}`
- fake-home agent invoking `xcrun --sdk iphonesimulator simctl list` (option-prefixed form) is correctly parsed by the option-aware `xcrun` shim and receives exit 127; `xcode_shim_rejected` with `{tool: "simctl", via_xcrun: true}`
- fake-home agent invoking `xcrun mcpbridge` receives exit 127 regardless of `requires_xcode_host_execution` value; `xcode_shim_rejected` with `{tool: "mcpbridge", via_xcrun: true, policy_reason: "mcpbridge_broker_only"}`
- fake-home agent invoking `xcrun dtrace` (non-Xcode subcommand, no options) passes through transparently; no observation emitted
- fake-home agent invoking `xcrun --toolchain swift-latest swiftc …` (option-prefixed non-Xcode) passes through transparently after option parse
- fake-home agent invoking `xcrun --bogus-unknown-flag simctl list` receives exit 127 (unknown flag fail-closed); `xcode_shim_rejected` with `policy_reason: "xcrun_unknown_option"`

Absolute-path catalog lint — structured fields (hard-fail):

- catalog with a `run` block command starting with `/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild` fails run-start with `xcode_absolute_path_forbidden`; run is not created
- catalog with an env assignment `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` alongside a direct `xcodebuild` invocation fails the lint
- catalog with bare `xcodebuild` (no absolute path) passes the lint and proceeds to shim-based enforcement

Absolute-path catalog lint — prompt text (soft-warn):

- catalog with a prompt that quotes `/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild` as inert documentation (e.g., a review agent's instructions describing what is blocked) **passes** run-start; fixture asserts run is created and an `xcode_absolute_path_in_prompt` warning is appended to the agent's `actual_xcode_runtime_observation_json`
- fixture using this P051 proposal text itself as the prompt body compiles cleanly — no false-positive on documentation that quotes the forbidden paths
- a prompt that contains **both** a quoted path AND a structured `run` block command with the same absolute path: run-start fails on the structured field; the prompt warning is still recorded

Absolute-path catalog lint — diagnostic mode scope:

- diagnostic mode + absolute `/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild` in structured `run` block: lint is bypassed with WARN; the shim is also transparent-passthrough (does not route); run proceeds and direct `xcodebuild` executes under the ACP provider's **current environment** (fake-home state — not rehosted), which is the documented trade-off of diagnostic mode. The fixture asserts `$HOME` observed by the `xcodebuild` subprocess matches the provider's fake home, not the operator home. This explicitly accepts the CoreSimulator failure risk for local debugging; diagnostic mode is not valid in production.
- diagnostic mode + absolute `/usr/bin/xcrun mcpbridge` in structured `run` block: **still hard-fails** with `xcode_absolute_path_forbidden`; fixture asserts run is not created even with `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1`
- diagnostic mode + absolute `/Applications/Xcode.app/Contents/Developer/usr/bin/mcpbridge` in structured `run` block: still hard-fails
- diagnostic mode + `xcrun --sdk iphonesimulator mcpbridge` in structured `run` block: still hard-fails after option-aware argv parse

Host executor routing:

- agent with `requires_xcode_host_execution: true` invoking `xcodebuild build -scheme Foo` is routed; `xcode_shim_routed` observation records argv, selected simulator UUID, exit status, host-env disposition, **and `cwd`**
- routed `xcodebuild` executes with `chdir` to the agent's cwd; fixture confirms build artifact landing in the correct workspace-relative path
- routed command with cwd outside frozen workspace root is rejected with `cwd_outside_workspace`
- routed command's subprocess env contains host-user `HOME`/`TMPDIR`/`DEVELOPER_DIR` (via fixture inspection of `/usr/bin/env` output), **not** the provider's fake-home values
- provider env like `CODEX_HOME`, `XDG_CACHE_HOME`, `CHAINWORKS_*` is absent from routed subprocess env
- build-input env (`SCHEME`, `CONFIGURATION`, `DESTINATION`) present in provider snapshot is propagated to routed subprocess
- `mcpbridge` is not routable via the host executor even with `requires_xcode_host_execution: true` — fixture confirms rejection is unconditional
- `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` makes `xcodebuild`/`simctl`/`xcrun`-non-mcpbridge transparent (catalog lint skipped) with WARN log at daemon start and per invocation; **fixture explicitly asserts that `mcpbridge` and `xcrun mcpbridge` remain rejected in diagnostic mode**

**Catalog migration**

- agent catalog entries with direct `xcodebuild` commands declare explicit `requires_xcode_host_execution` value (either `true` or `false`); missing declaration blocks the lint step

No Xcode UI automation is required for this gate. Use fixture backend processes for deterministic proof.

---

## 11. Acceptance Criteria

Implementation is complete when:

- The HTTP streaming feasibility research artifact exists, has an allowed verdict, and the implementation scope matches that verdict.
- `ProviderCapabilityCache` is populated by a one-shot `initialize` probe keyed on the full `ProbeKey` derived from a `ProviderLaunchSpec`. The executor builds a single `ProviderLaunchSpec` per request and passes it to `ensure_provider_capabilities`, then to `attach_session_credentials` (post-preflight), then to the adapter's `open_session_with_specs`. Debug-assert enforces **capability-slice** equality across these call sites; `credential_env` is empty at probe and populated only after `attach_session_credentials`. `resolve_mcp_servers` fails closed with `provider_http_mcp_unsupported` before lease/port/token allocation when HTTP MCP is unsupported.
- Parallel ACP sessions that request Xcode MCP each get their own lease and backend `mcpbridge` subprocess; their initialize phases serialize per Xcode PID; their `tools/*` calls run in parallel.
- ACP providers receive an HTTP streaming MCP endpoint for brokered Xcode access.
- Single backend crash fails only its lease; sibling leases continue. Xcode PID drift and broker HTTP infrastructure failure terminally close all affected leases with their respective `backend_failure_class`. Host-env loss fails **new lease acquisitions** and **shim-route requests** but does not disturb already-running MCP streams.
- PATH shim (`xcodebuild`, `simctl`, `mcpbridge`, option-aware `xcrun`) is injected into an ACP provider subprocess when the compile-time `XcodeShimInjectionSignal` is set, triggered by any of: (1) catalog requests Xcode MCP, (2) agent declares `requires_xcode_host_execution: true`, (3) catalog lint detects any bare Xcode-tool lexeme in any direct-command declaration field (`run` blocks, `shell_allowlist`, `allowed_commands`, `tools.shell.commands`, adapter-specific shell-capability fields). Signal is decidable before MCP resolution runs. Agents with zero Xcode signals keep their unmodified `PATH`.
- Absolute-path catalog lint hard-fails run-start with `xcode_absolute_path_forbidden` **only** when a structured executable field (`run` block command path/argv, env assignments of `DEVELOPER_DIR`) contains Xcode-tool absolute paths. Prompt/system-instruction/description text gets a soft `xcode_absolute_path_in_prompt` warning recorded in the runtime observation envelope but does not block run-start. `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` skips the lint **only for `xcodebuild`/`simctl` paths** (structured and prompt). Any structured field whose absolute path resolves to `mcpbridge` (bare, via `/usr/bin/xcrun mcpbridge`, or any option-prefixed `xcrun` variant ending in `mcpbridge`) still hard-fails even in diagnostic mode — the broker-only-mcpbridge boundary has no diagnostic override anywhere in P051.
- Default shim policy rejects direct `xcodebuild`/`simctl` invocations with exit 127 and `xcode_shim_rejected` observation; `requires_xcode_host_execution: true` opt-in routes through the broker host executor with full `ShimDispatchRequest` DTO (cwd, env allowlist, provider snapshot).
- Direct `mcpbridge` (bare or via `xcrun mcpbridge`, option-prefixed or not) is **always rejected within the enforced boundary** — PATH-based invocations via the shim, catalog-declared structured-field paths via the lint — regardless of `requires_xcode_host_execution` **and regardless of `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC`**. `mcpbridge` is not routable via the host executor. LLM-improvised absolute `/usr/bin/xcrun mcpbridge` at prompt time is outside the enforced boundary (documented residual, §5.1.1).
- **Shim dispatch token authority is separate from MCP lease authority and scoped to the provider session, not a single execution.** `XcodeShimDispatchToken` is minted per provider session at provider-launch time for any shim-injected execution (including direct-Xcode-only agents with no MCP lease). The broker maintains a mutable `current_execution_id` pointer updated before each `session/prompt` so shim events always append to the currently-active execution. Reuse-compat check forces supersession if `workspace_root` or `requires_xcode_host_execution` would differ between executions. Tokens are constant-time validated, have explicit expiry, enforce the frozen `workspace_root` cwd boundary, and cannot be cross-used with MCP HTTP bearer tokens. Tokens are **not** assumed secret from shell-capable agents (env delivery is readable via `env`/`printenv`); all authorization is server-side.
- **Durable schema.** `agent_executions.actual_xcode_runtime_observation_json` column exists (migration added), legacy rows read back as GraphQL `null`, and the repository supports append-only semantics across the three arrays. GraphQL and MCP expose typed envelope.
- Broker-owned Xcode subprocesses run with host-user `HOME`/`TMPDIR`; ACP providers continue to run with isolated fake-home state.
- The implementation does not grant the entire ACP provider process the real user home as the normal Xcode fix.
- Xcode destination handling prefers explicit simulator UUIDs and fails clearly on ambiguous name/OS requests.
- Direct multi-client sharing of raw `xcrun mcpbridge` stdio is not used.
- Provider-facing stdio proxying is not implemented as P051's architecture.
- Broker leases are **provider-session-owned**. A lease is released on provider-session close, cancellation, timeout, provider process death, operator session reset, or reuse-incompatible supersession — **not** on individual execution success. A reused provider session retains its lease and backend `mcpbridge` subprocess across prompt cycles.
- Runtime observations distinguish backend start from backend reuse.
- Permission-separated agents get distinct broker state keyed by permission fingerprint; no policy leakage across leases.
- Existing session reuse behavior still works.
- `MCP_XCODE_PID` targeting remains active.
- Xcode host-env, simulator-selection, and shim/host-executor evidence is present in `actual_xcode_runtime_observation_json` as three append-only arrays (`mcp_broker_observations`, `xcode_shim_events`, `xcode_host_executor_events`); direct-Xcode-only executions with no MCP server request still populate the envelope.
- `proposal-051|p051` is registered in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.
- The proposal-specific gate passes.

---

## 12. Rollout Plan

1. ✅ Produce `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md` — complete 2026-04-19.
2. ✅ Include host-env feasibility in the same research artifact — covered.
3. ✅ Current verdict is `Proceed with scoped architecture`; P051 has been revised accordingly (per-lease backend, initialize Mutex, shim + catalog lint, `ProbeKey` cache, host-executor DTO). No further proposal revision is gated on research.
4. Implement broker behind default-on runtime config for Xcode only.
5. Implement host-user Xcode environment handling inside the broker or host executor before dogfooding Xcode-dependent agents.
6. `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` bypasses only the `xcodebuild`/`simctl`/`xcrun`-non-`mcpbridge` shims. Direct `mcpbridge` and `xcrun mcpbridge` remain rejected — no diagnostic escape hatch for raw `mcpbridge` exists. Engineers debugging Xcode MCP run `xcrun mcpbridge` from their own shell outside any ACP agent; never exposed to provider subprocesses. Do not add a stdio proxy fallback.
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

## 13. Resolved Decisions

These were originally open questions; all were resolved by the research gate or by this revision. Listed here as decision record for implementation.

| Decision | Resolution | Source |
|---|---|---|
| ACP providers supporting HTTP MCP in `session/new` | Codex-acp ≥ 0.4.0, Claude Agent ACP ≥ 0.28, Gemini CLI ≥ 0.38.1 — all three confirmed | Research artifact + Phase 0 probe 3 |
| `session/new.mcpServers[]` wire shape | ACP spec discriminated union: `{"type":"http","name","url","headers":[{"name","value"}]}` | ACP schema |
| Loopback HTTP + bearer vs Unix-domain | Loopback HTTP with bearer via `headers` array is sufficient for all three providers; UDS not required | Research artifact |
| Operator home source | `getpwuid(getuid()).pw_dir` at daemon start, with daemon-config override and startup warning if the daemon UID does not match the active GUI user | Research artifact Q7 |
| Direct Xcode commands in fake-home context | PATH shim for `xcodebuild`/`simctl`/`mcpbridge`/`xcrun` + catalog lint for absolute paths; `requires_xcode_host_execution` opt-in routes `xcodebuild`/`simctl` only; `mcpbridge` always rejected (diagnostic bypass applies to `xcodebuild`/`simctl`/`xcrun`-non-mcpbridge only) | §5.1.1 (this proposal) |
| Backend-sharing model | Per-lease `mcpbridge` subprocess; serialize spawn+initialize per Xcode PID via Mutex; parallel `tools/*` unserialized | Phase 0 probe 1 |
| Modal scope | Per-Xcode-process consent; no duplicate modals for sibling bridges under the same Xcode PID | Phase 0 probe 2 |

## 14. Open Questions (non-blocking)

Implementation can proceed with the conservative defaults below; these are tuning knobs for later revisions, not gates.

1. Should broker idle grace be fixed at 60 seconds or configurable per runtime profile?
2. Should broker debug state be exposed through MCP `runtime.status` later, or is `actual_mcp_observation_json` enough for P051?
3. Does Xcode mcpbridge expose any server-side session state that makes tool-list caching unsafe after file/project changes? (Conservative default: invalidate cache on any Xcode workspace switch detected via `NSWorkspace` or file-mtime probing.)
4. Should policy-separated leases be keyed by permission profile name, resolved tool allowlist hash, or both?

**Conservative defaults for implementation:**

- 60-second grace for broker in-memory state (cache, Mutex-owning pool entry) after last lease releases.
- No new northbound debug MCP tool in P051.
- Cache only `tools/list` per Xcode PID, invalidate on Xcode PID drift or workspace switch.
- Lease keyed by resolved tool allowlist hash + permission profile id.
