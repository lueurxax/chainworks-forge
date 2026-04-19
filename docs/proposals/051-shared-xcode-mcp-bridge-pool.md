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
8. **Direct-command containment.** Can the runtime prevent direct Xcode execution (`xcodebuild`, `simctl`, `mcpbridge`, via-`xcrun` variants) from isolated fake-home ACP sessions via an enforceable shim, with an opt-in host-executor route for the narrow set of commands that legitimately need it (excluding `mcpbridge`, which is broker-only), and an explicit diagnostic escape hatch?
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

`launch_env_fingerprint` covers an explicit allowlist of capability-relevant env vars (e.g., `CODEX_HOME`, `GEMINI_API_KEY` presence flag, `CLAUDE_AGENT_*` feature flags, `ACP_EXPERIMENTAL_*`) — never values for secrets, only a redacted/boolean fingerprint. `adapter_settings_fingerprint` covers the `AcpSessionConfig` knobs that alter provider behavior (mode, config options, structured-output intent). Any change in any of these forces a fresh probe.

```text
ProviderCapabilityProbe::probe(ProbeKey) -> AgentCapabilities
```

The probe launches the provider binary **with the exact args and env that a real session/new would use** (minus the actual `mcpServers` list — probe sends an empty `mcpServers: []`), sends `initialize` with minimal client capabilities, records the returned `AgentCapabilities`, and immediately closes via `session/close` or stdin EOF. One probe per unique `ProbeKey` per daemon process lifetime; result cached in-memory.

**Preflight timing.** `AcpRuntimeManager::ensure_provider_capabilities(runtime_profile_id)` runs as a preflight step during `ExecutionRequest` construction, **before** `engine::mcp::resolve_mcp_servers` is called. The resolved capabilities are threaded into MCP resolution as a function argument:

```rust
fn resolve_mcp_servers(
    request: &ResolveMcpRequest,
    provider_caps: &AgentCapabilities,
) -> Result<Vec<ResolvedMcpServer>, McpResolutionError>
```

**Fail-closed contract.** If the catalog or broker resolves an MCP entry that requires HTTP transport (Xcode MCP under broker mode) but `provider_caps.mcp_capabilities.http == false`, `resolve_mcp_servers` returns a blocking issue **before** any lease is reserved, before the HTTP listener is bound, and before `session/new` is sent. Specifically:

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

**Injection condition.** The shim is injected when an agent has **any modeled Xcode-dependent capability**. Three triggers, any of which activates injection:

1. any resolved MCP entry with `adapter_family: xcode_*` or server id `xcode`,
2. any declared `requires_xcode_host_execution: true`,
3. any entry in the agent's allowed `run` commands that matches a known Xcode-tool lexeme (`xcodebuild`, `simctl`, `mcpbridge`, `xcrun`, or any absolute path beginning with `/Applications/Xcode.app/Contents/Developer/`). Detection is performed by a catalog lint pass at run-start.

An agent that uses direct `xcodebuild` but has no Xcode MCP server still receives the shim, the token, and the socket. Agents with zero Xcode signals get no shim — the provider subprocess keeps its normal `PATH` unchanged.

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

After skipping options, the first non-option token is treated as the subcommand. If that token is `simctl` or `mcpbridge`, apply the reject/route/diagnostic policy. Otherwise `execve("/usr/bin/xcrun", argv, envp)` passes through, preserving the provider's isolated environment. Pass-through is silent (no observation). Absolute path `/usr/bin/xcrun` avoids shim recursion via `PATH`.

**Absolute-path containment (catalog lint).** PATH shims cannot intercept direct absolute-path invocations like `/usr/bin/xcrun simctl list` or `/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild build`. P051 closes this with a **catalog lint step**, not a runtime interceptor, because libc `execve`-level audit requires `DYLD_INSERT_LIBRARIES` (SIP-protected for Apple-signed binaries) or kernel extensions (deprecated). The lint runs at run-start during workflow compilation:

- scan every agent's resolved `run` block (commands, args, env values) plus prompt/system-instruction text for absolute paths matching:
  - `/usr/bin/xcrun`, `/usr/bin/xcodebuild`
  - `/Applications/Xcode.app/Contents/Developer/`
  - `/Applications/Xcode*.app/Contents/Developer/`
  - any `DEVELOPER_DIR=…xcodebuild`, `DEVELOPER_DIR=…simctl` assignments
- **fail run-start** with `xcode_absolute_path_forbidden` for matches, requiring the catalog author to use bare tool names (which the shim can intercept) or explicit `requires_xcode_host_execution: true` with opt-in routing.
- Diagnostic mode (`CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1`) skips the lint with a WARN log.

**Scope of guarantee.** The containment model is:

- **PATH-based invocations** of `xcodebuild`, `simctl`, `mcpbridge`, and `xcrun <subcommand>`: enforced by shim at runtime.
- **Absolute-path invocations**: enforced by catalog lint at run-start; not enforced at runtime.
- **Agent prompt-time improvisation** (an LLM synthesizing an absolute path mid-run): not blocked by P051. This is a residual risk. Mitigations: (a) prompt templates for Xcode-dependent agents include explicit "use bare `xcodebuild`, never absolute paths" instruction, (b) agent-shell output observation flags commands starting with `/Applications/Xcode.app/` or `DEVELOPER_DIR=` to a warning stream for operator review post-run, (c) a follow-up proposal can add libc audit or sandbox-exec profiles if the residual risk becomes a real failure mode in dogfood.

P051 does not claim the residual risk is zero. It claims the enforceable boundary covers PATH-based commands and catalog-declared absolute paths, which is the common case observed today.

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
  exit code 127. `actual_mcp_observation_json` records `xcode_shim_rejected` with the command and argv.

- `requires_xcode_host_execution: true`: shim **routes** the invocation through the broker's host executor over the Unix dispatch socket. The broker runs the actual `/Applications/Xcode.app/Contents/Developer/usr/bin/<tool>` under full host-user environment, streams stdout/stderr/exit back to the shim, and records `xcode_shim_routed` with the selected simulator UUID when applicable.

**Dispatch DTO.** The shim sends a structured request over the Unix socket (not just argv):

```rust
struct ShimDispatchRequest {
    token: String,              // CHAINWORKS_XCODE_SHIM_TOKEN (matches broker lease)
    tool: ShimTool,             // Xcodebuild | Simctl | Mcpbridge | XcrunPassthrough
    argv: Vec<String>,          // as received by the shim, minus argv[0]
    cwd: PathBuf,               // shim captures getcwd() at invocation time
    provider_env_snapshot: BTreeMap<String, String>, // safe subset (below)
    provider_pid: u32,          // for audit correlation
    invocation_ts: SystemTime,
}
```

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

**Workspace root + simulator UUID.** The broker resolves the selected simulator UUID (from `xcrun simctl list --json` at daemon start + per-request resolution if needed) and includes it in `xcode_shim_routed` observation. The broker verifies the agent's frozen workspace root matches a known Xcode project/workspace location before running `xcodebuild`; mismatch → reject with `workspace_mismatch`.

**Streaming contract.** Host executor streams stdout/stderr back to the shim over the same Unix socket as framed chunks (`{stream: "stdout"|"stderr", bytes: <base64>}`) so the agent sees command output interleaved as normal shell would. Exit code delivered as `{exit: <i32>}` terminator. The shim then exits with the same code.

- Diagnostic escape hatch: daemon config `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` (not per-agent — global, for local debugging) makes all shims transparent passthroughs to the real binaries. Logged at WARN level on daemon start if set. Not valid in production deployments.

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
  - Own `ProviderCapabilityCache: HashMap<(AdapterFamily, BinaryFingerprint), AgentCapabilities>`.
  - Expose `ensure_provider_capabilities(runtime_profile_id) -> AgentCapabilities` as preflight called before `engine::mcp::resolve_mcp_servers`.
  - Acquire leases before `adapter.open_session`.
  - Release leases on normal close, provider error, timeout, cancellation, and drop paths.
  - Refuse broker mode unless the P051 HTTP streaming feasibility research verdict allows the current provider set.

- `control-plane/crates/acp/src/provider_probe.rs` (new)
  - `ProviderCapabilityProbe::probe(ProbeKey) -> AgentCapabilities` — one-shot `initialize` subprocess with the **exact args + env + adapter settings** that a real session would use (minus `mcpServers`), records returned capabilities, closes immediately.
  - `ProbeKey` composed of `(adapter_family, runtime_profile_id, binary_fingerprint, launch_args_fingerprint, launch_env_fingerprint, adapter_settings_fingerprint)`.
  - Binary fingerprinting: path + mtime + size. No signature verification in P051.
  - Env fingerprint allowlist restricted to capability-gating env vars (never raw secret values; boolean/redacted fingerprint only).

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

- `control-plane/crates/acp/src/xcode_host_executor.rs` (new)
  - Execute approved Xcode-bound commands (`xcodebuild`, `simctl`) under the host-user environment contract.
  - Allowlist-bound: rejects any binary not in `{xcodebuild, simctl}`. `mcpbridge` is explicitly **not** in the host-executor allowlist — see §5.1.1 direct-command guard rationale.
  - Prefer simulator UUIDs and reject ambiguous simulator destinations.
  - Record observation data for command, selected simulator id, host-env disposition, exit status.
  - Expose a Unix-domain dispatch socket consumed by the shim binaries (below).

- `control-plane/crates/acp/src/xcode_shim/` (new crate or module + thin binaries)
  - Four shim executables under a daemon-managed `$XCODE_SHIM_DIR`:
    - `xcodebuild`, `simctl` — apply reject/route policy based on agent's `requires_xcode_host_execution` flag.
    - `mcpbridge` — **always rejects** direct invocation, regardless of `requires_xcode_host_execution`.
    - `xcrun` — **option-aware argv parser** covering the known `xcrun` flag set (with-arg: `--sdk`, `--toolchain`, `--log`, `-r`, `--run`; without-arg: `-f`, `--find`, `--help`, `-h`, `--version`, `--verbose`, `--no-cache`, `--kill-cache`, `--show-sdk-*`). Skips options to find first non-option subcommand; if `simctl`/`mcpbridge`, applies corresponding policy; otherwise `execve("/usr/bin/xcrun", argv, envp)` as pass-through. Unknown option fails closed with `xcrun_unknown_option`.
  - Each shim: read argv + `$HOME` + `$CHAINWORKS_XCODE_SHIM_TOKEN` + `getcwd()` + capability-relevant env subset, connect to `$XCODE_SHIM_DISPATCH_SOCKET`, dispatch via `ShimDispatchRequest` DTO.
  - Reject path: exit 127 with structured stderr; emit `xcode_shim_rejected` observation with `{tool, via_xcrun: bool, argv, cwd, policy_reason}`.
  - Route path: stream stdout/stderr/exit from broker's host executor via framed chunks over the same Unix socket; emit `xcode_shim_routed` observation with argv, cwd, selected simulator UUID, exit status.
  - Pass-through path (`xcrun` only, non-Xcode subcommands): silent `execve`, no observation.
  - Diagnostic bypass: `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` makes the shim a transparent passthrough — including `mcpbridge` — with WARN log at daemon start and per-invocation WARN log per shim.

- `control-plane/crates/workflow/src/catalog_lint.rs` (new)
  - Run at run-start during workflow compilation.
  - Scan every agent's resolved `run` block (commands, args, env values) plus prompt/system-instruction text for Xcode-tool absolute paths (`/usr/bin/xcrun`, `/usr/bin/xcodebuild`, `/Applications/Xcode*.app/Contents/Developer/`) and `DEVELOPER_DIR=...` assignments paired with Xcode tools.
  - Fail run-start with `xcode_absolute_path_forbidden` on match unless `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1`.
  - Produce the agent-level shim-injection signal based on: (1) Xcode MCP server present, (2) `requires_xcode_host_execution: true`, (3) any bare Xcode-tool lexeme in `run` block.

- `control-plane/crates/acp/src/adapters/{codex,claude,gemini,auggie,junie}.rs`
  - For agents that meet **any** of the three injection triggers (Xcode MCP entry, `requires_xcode_host_execution: true`, or any Xcode-tool lexeme detected by catalog lint): prepend `$XCODE_SHIM_DIR` to the subprocess `PATH`; inject `CHAINWORKS_XCODE_SHIM_TOKEN` and `XCODE_SHIM_DISPATCH_SOCKET`.
  - Agents with zero Xcode signals keep their normal `PATH` (no shim overhead).

### Engine crate

- `control-plane/crates/engine/src/mcp.rs`
  - Change `resolve_mcp_servers` signature to accept `&AgentCapabilities` as input.
  - Resolve `xcode` MCP entries to broker HTTP streaming transport when broker mode is enabled **and** `provider_caps.mcp_capabilities.http == true`.
  - When broker mode is requested but capabilities say HTTP is unsupported, return `McpResolutionError::ProviderHttpMcpUnsupported { adapter_family, binary_fingerprint }` before lease reservation.
  - Preserve `MCP_XCODE_PID` targeting.
  - Preserve isolated ACP provider `HOME`/`CODEX_HOME`; do not make the whole provider host-home-backed only because Xcode MCP is requested.
  - Keep direct `xcrun mcpbridge` transport available only behind a diagnostic feature flag or explicit runtime config.
  - Do not implement a provider-facing stdio proxy fallback in P051.

- `control-plane/crates/engine/src/executor.rs` (or equivalent ExecutionRequest builder)
  - Call `AcpRuntimeManager::ensure_provider_capabilities(runtime_profile_id)` before `resolve_mcp_servers`.
  - Thread resulting `AgentCapabilities` into MCP resolution.
  - Surface `ProviderHttpMcpUnsupported` as a blocking execution error with structured observation.

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

Because each lease owns its own backend `mcpbridge` subprocess, a single backend failure is **per-lease by default**:

- the failing backend's HTTP stream returns a terminal MCP error to **its own lease only**,
- that lease moves to `closing` then `released`,
- the failed `mcpbridge` subprocess is reaped,
- **sibling leases on the same Xcode PID remain active** and unaffected,
- the affected agent execution fails through ACP transport handling.

Pool-wide invalidation is reserved for **shared-state failure signals** that cannot be attributed to a single lease:

- Xcode PID drift (`pgrep -n -x Xcode` returns a different PID than the pool key). All leases on the stale pool key receive terminal errors; the pool is invalidated; new leases use a new pool key for the new Xcode PID.
- Host-env resolution failure (e.g., operator home has become unreadable, `xcode-select -p` returns an invalid path). New lease acquisitions fail closed; existing leases continue until their own bridges settle.
- Broker HTTP infrastructure failure (loopback listener dies, token store corrupted). All active leases receive terminal errors; broker marks itself unhealthy.

`actual_mcp_observation_json` distinguishes these via `backend_failure_class`: `per_lease_backend` | `pool_pid_drift` | `host_env_unavailable` | `broker_infrastructure`.

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
- **provider capability preflight**: when `ProviderCapabilityCache` records `mcpCapabilities.http = false` for a runtime profile, `resolve_mcp_servers` returns a blocking issue before any lease is reserved; no HTTP endpoint is bound, no token is minted, no `session/new` is sent; `actual_mcp_observation_json` records `provider_http_mcp_unsupported`
- capability probe caches `AgentCapabilities` per (adapter_family, binary_fingerprint) and invalidates on binary upgrade
- two concurrent Xcode lease requests serialize through the per-PID initialize Mutex and both complete a `tools/list` round-trip without interference
- parallel `tools/call` requests on sibling leases do not serialize (lock is released after initialize)
- each lease spawns its own `mcpbridge` subprocess; no backend sharing across leases
- **backend crash isolation**: fatal MCP error from one lease's backend closes only that lease's subprocess; sibling leases on the same Xcode PID remain active and continue serving `tools/*` calls
- **pool-wide invalidation triggers only on shared-state failure**: Xcode PID drift, host-env unavailable, broker HTTP infrastructure failure — verified by separate fixture tests
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
- **direct Xcode command guard**: fake-home agent with `requires_xcode_host_execution: false` invoking `xcodebuild` or `xcrun simctl` via shell receives an explicit shim rejection (exit 127 with structured stderr) rather than a CoreSimulator failure deep in the call stack; `actual_mcp_observation_json` records `xcode_shim_rejected`
- agent with `requires_xcode_host_execution: true` has its direct Xcode command routed through the broker host executor under host-user environment; `xcode_shim_routed` recorded with argv and selected simulator UUID
- `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` disables the shim (diagnostic mode); startup warning logged

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

**Core broker behavior**

- two parallel Xcode ACP sessions each spawn their own `mcpbridge` subprocess, both under host-user environment, both complete full MCP round-trip
- initialize phases serialize per Xcode PID (late-starter's `backend_initialize_wait_ms` > 0)
- HTTP streaming endpoint shape is used for brokered Xcode MCP
- policy-separated leases get independent broker state (no cross-lease policy leakage)

**Capability preflight**

- HTTP-incompatible provider (fixture with `mcpCapabilities.http = false`) fails closed with `provider_http_mcp_unsupported` **before** any lease is reserved, before the HTTP listener is bound, and before `session/new` is sent
- capability probe cache hit for a previously-seen full `ProbeKey` (same adapter/profile/binary/args/env/settings) skips the probe subprocess
- binary fingerprint change (path, mtime, or size) invalidates the cached entry and triggers a fresh probe
- runtime profile change forces a fresh probe even when the binary is unchanged — fixture: two profiles on the same Codex binary with different `mode` or `config_options` produce independent cache entries
- launch-env fingerprint change forces a fresh probe — fixture: toggling an `ACP_EXPERIMENTAL_*` env var between probes yields two distinct cache entries
- launch-args fingerprint change forces a fresh probe — fixture: Gemini switching between `--acp` and `--experimental-acp` produces two distinct cache entries

**Per-lease vs pool-wide failure isolation (P1 new)**

- one backend `mcpbridge` crash fails only its lease; a sibling lease on the same Xcode PID continues serving `tools/*` calls and completes successfully
- Xcode PID drift (simulated by changing `pgrep` output) invalidates the pool, all stale-PID leases receive terminal errors, and new leases use a new pool key; `backend_failure_class = "pool_pid_drift"` recorded
- host-env unavailable (fixture with unreadable operator home) fails closed with `backend_failure_class = "host_env_unavailable"`
- broker HTTP infrastructure failure (fixture listener killed) marks broker unhealthy with `backend_failure_class = "broker_infrastructure"`

**Direct Xcode command guard**

Shim injection:

- agent with only Xcode MCP entries (no direct `xcodebuild` in `run` block) gets the shim injected
- agent with only direct `xcodebuild` commands (no Xcode MCP entry) gets the shim injected via the catalog-lint injection trigger
- agent with `requires_xcode_host_execution: true` but no other Xcode signal gets the shim injected
- agent with zero Xcode signals gets no shim; its `PATH` remains unchanged

Shim enforcement (PATH-based invocations):

- fake-home agent with `requires_xcode_host_execution: false` invoking `xcodebuild -project …` via shell receives exit 127; `xcode_shim_rejected` observation with `{tool: "xcodebuild", via_xcrun: false}`
- fake-home agent invoking `xcrun simctl list` receives exit 127; `xcode_shim_rejected` with `{tool: "simctl", via_xcrun: true}`
- fake-home agent invoking `xcrun --sdk iphonesimulator simctl list` (option-prefixed form) is correctly parsed by the option-aware `xcrun` shim and receives exit 127; `xcode_shim_rejected` with `{tool: "simctl", via_xcrun: true}`
- fake-home agent invoking `xcrun mcpbridge` receives exit 127 regardless of `requires_xcode_host_execution` value; `xcode_shim_rejected` with `{tool: "mcpbridge", via_xcrun: true, policy_reason: "mcpbridge_broker_only"}`
- fake-home agent invoking `xcrun dtrace` (non-Xcode subcommand, no options) passes through transparently; no observation emitted
- fake-home agent invoking `xcrun --toolchain swift-latest swiftc …` (option-prefixed non-Xcode) passes through transparently after option parse
- fake-home agent invoking `xcrun --bogus-unknown-flag simctl list` receives exit 127 (unknown flag fail-closed); `xcode_shim_rejected` with `policy_reason: "xcrun_unknown_option"`

Absolute-path catalog lint:

- catalog with a `run` block command starting with `/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild` fails run-start with `xcode_absolute_path_forbidden`; run is not created
- catalog with an env assignment `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer` alongside a direct `xcodebuild` invocation fails the lint
- catalog with bare `xcodebuild` (no absolute path) passes the lint and proceeds to shim-based enforcement

Host executor routing:

- agent with `requires_xcode_host_execution: true` invoking `xcodebuild build -scheme Foo` is routed; `xcode_shim_routed` observation records argv, selected simulator UUID, exit status, host-env disposition, **and `cwd`**
- routed `xcodebuild` executes with `chdir` to the agent's cwd; fixture confirms build artifact landing in the correct workspace-relative path
- routed command with cwd outside frozen workspace root is rejected with `cwd_outside_workspace`
- routed command's subprocess env contains host-user `HOME`/`TMPDIR`/`DEVELOPER_DIR` (via fixture inspection of `/usr/bin/env` output), **not** the provider's fake-home values
- provider env like `CODEX_HOME`, `XDG_CACHE_HOME`, `CHAINWORKS_*` is absent from routed subprocess env
- build-input env (`SCHEME`, `CONFIGURATION`, `DESTINATION`) present in provider snapshot is propagated to routed subprocess
- `mcpbridge` is not routable via the host executor even with `requires_xcode_host_execution: true` — fixture confirms rejection is unconditional
- `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` makes all shims transparent (including `mcpbridge` and catalog lint skipped) with WARN log at daemon start and per invocation

**Catalog migration**

- agent catalog entries with direct `xcodebuild` commands declare explicit `requires_xcode_host_execution` value (either `true` or `false`); missing declaration blocks the lint step

No Xcode UI automation is required for this gate. Use fixture backend processes for deterministic proof.

---

## 11. Acceptance Criteria

Implementation is complete when:

- The HTTP streaming feasibility research artifact exists, has an allowed verdict, and the implementation scope matches that verdict.
- `ProviderCapabilityCache` is populated by a one-shot `initialize` probe per (adapter_family, binary_fingerprint); `resolve_mcp_servers` fails closed with `provider_http_mcp_unsupported` before lease/port/token allocation when HTTP MCP is unsupported.
- Parallel ACP sessions that request Xcode MCP each get their own lease and backend `mcpbridge` subprocess; their initialize phases serialize per Xcode PID; their `tools/*` calls run in parallel.
- ACP providers receive an HTTP streaming MCP endpoint for brokered Xcode access.
- Single backend crash fails only its lease; sibling leases continue; pool-wide invalidation occurs only on Xcode PID drift, host-env loss, or broker infrastructure failure.
- PATH shim for `xcodebuild`/`simctl`/`mcpbridge` is injected into ACP provider subprocess for agents with Xcode MCP; default policy rejects direct invocations; `requires_xcode_host_execution: true` opt-in routes through the broker host executor.
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

## 13. Resolved Decisions

These were originally open questions; all were resolved by the research gate or by this revision. Listed here as decision record for implementation.

| Decision | Resolution | Source |
|---|---|---|
| ACP providers supporting HTTP MCP in `session/new` | Codex-acp ≥ 0.4.0, Claude Agent ACP ≥ 0.28, Gemini CLI ≥ 0.38.1 — all three confirmed | Research artifact + Phase 0 probe 3 |
| `session/new.mcpServers[]` wire shape | ACP spec discriminated union: `{"type":"http","name","url","headers":[{"name","value"}]}` | ACP schema |
| Loopback HTTP + bearer vs Unix-domain | Loopback HTTP with bearer via `headers` array is sufficient for all three providers; UDS not required | Research artifact |
| Operator home source | `getpwuid(getuid()).pw_dir` at daemon start, with daemon-config override and startup warning if the daemon UID does not match the active GUI user | Research artifact Q7 |
| Direct Xcode commands in fake-home context | Minimum guard: PATH shim for `xcodebuild`/`simctl`/`mcpbridge` with `requires_xcode_host_execution` opt-in; global `CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC` diagnostic bypass | §5.1.1 (this proposal) |
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
