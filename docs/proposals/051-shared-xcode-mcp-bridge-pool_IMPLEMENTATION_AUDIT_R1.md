# Proposal 051: Shared Xcode MCP Bridge Pool for ACP Sessions Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | docs/proposals/051-shared-xcode-mcp-bridge-pool.md |
| Repository Root | . |
| Git SHA | e5e6122e3b44bd76e3a86f8b9a4c6102812455fa |
| Working Tree | Dirty: many modified, deleted, and untracked files were present before this audit; this report audits the on-disk tree as found. |
| Audited At | 2026-04-19T09:58:15+03:00 |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

P051 is not implemented in the current tree. The research artifact exists and the old direct `MCP_XCODE_PID` injection remains, but the broker, HTTP MCP transport, capability preflight, launch-spec adapter boundary, host-user Xcode environment, shims, host executor, Xcode runtime observation envelope, GraphQL/MCP readback, catalog migration, and `proposal-051|p051` gate are absent.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Core broker and durable observation surfaces are missing. | High |
| Architecture | Weak | Current ACP adapter and MCP resolver seams still expose only stdio/platform paths. | High |
| Product | At Risk | Primary operator job, reliable Xcode access from isolated ACP sessions, is not achievable. | High |
| UI | Acceptable | P051 explicitly excludes Swift UI changes. | High |
| UX | At Risk | Operators still cannot see P051's promised broker/shim/runtime evidence. | High |
| Readiness | Not Ready | The canonical P051 gate is not registered and exits as unknown. | High |

## Proposal Contract

### Scope

- Implement a Rust Xcode MCP broker owned by the control-plane process.
- Serve provider-facing Xcode MCP over loopback HTTP with per-session bearer tokens.
- Spawn one `xcrun mcpbridge` backend subprocess per HTTP client session under host-user Xcode environment.
- Serialize backend bridge spawn plus `initialize` per Xcode PID while allowing parallel `tools/*`.
- Centralize simulator UUID selection, policy filtering, observability, and lifecycle.
- Guard direct Xcode shell commands from fake-home ACP provider sessions.

### Locked Decisions

- HTTP streaming is the P051 architecture; no provider-facing stdio proxy fallback.
- Backend model is one `mcpbridge` subprocess per HTTP client lease, not one shared stdio backend.
- Broker-owned Xcode subprocesses use operator `HOME` and Darwin user temp paths; ACP providers keep isolated fake home / `CODEX_HOME`.
- Provider capability preflight must happen before MCP resolution can mint HTTP transport, lease URLs, ports, or tokens.
- Shim dispatch token authority is separate from MCP HTTP bearer authority.
- Direct `mcpbridge` and `xcrun mcpbridge` are always rejected from ACP provider shells.
- `actual_xcode_runtime_observation_json` is the durable Xcode-runtime evidence envelope.

### Primary User Flows

1. A parallel proposal-review stage starts two Xcode-capable ACP sessions; each receives brokered HTTP Xcode MCP, uses its own backend bridge, and shares one Xcode consent boundary.
2. An isolated fake-home ACP agent attempts direct `xcodebuild`/`simctl`; the shim rejects by default or routes through a host executor when explicitly opted in.
3. A provider without HTTP MCP capability fails closed before lease, token, listener, or `session/new` work.
4. An operator inspects GraphQL/MCP reports and sees broker, shim, host-env, failure-class, and simulator-UUID evidence.
5. A reused ACP provider session remains compatible only when broker lease, MCP set, workspace, and host-execution policy still match.

### UI Commitments

P051 explicitly excludes Swift app UI changes.

### UX Commitments

- Operators should handle at most one Xcode consent modal per Xcode process during parallel Xcode-capable sessions.
- Failures should be classified as broker/host-executor failures instead of misleading agent implementation failures.
- Operators should be able to prove behavior from structured runtime observations rather than log-pattern inference.

### Acceptance Criteria

P051's acceptance criteria include provider capability probing, HTTP MCP endpoint delivery, per-lease bridge lifecycle, initialize serialization, direct command shim/host-executor behavior, provider-session-owned lease reuse, typed durable observation envelope, explicit simulator UUIDs, no stdio proxy fallback, preserved `MCP_XCODE_PID`, and `proposal-051|p051` gate registration.

### Test / Evidence Requirements

- Focused Rust unit/integration tests for broker behavior, capability preflight, lease failure isolation, daemon route readiness, direct command guard, catalog field propagation, observation parity, shim token authority, and `proposal-051|p051`.
- No live Xcode UI automation is required for the canonical gate; fixture backends are acceptable.

### Explicit Exclusions

- No Swift UI changes.
- No shared ACP language-model session across reviewers.
- No changes to XcodeBuildMCP itself.
- No cross-daemon bridge sharing.
- No generic MCP pooling beyond Xcode.
- No broad fake-home removal for ACP providers.
- No provider-facing stdio proxy architecture.

## Proposal Fidelity / Divergence

### Matches

- The research artifact exists at `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md` and states `Proceed with scoped architecture`.
- Current MCP resolver still injects `MCP_XCODE_PID` for direct `xcrun mcpbridge` stdio entries.
- No P051 stdio proxy implementation was found.
- Current Codex adapter still uses fake-home isolation for ACP provider state.

### Divergences

- `ResolvedMcpServerTransport` has only `Stdio` and `Platform`; there is no HTTP MCP variant.
- ACP `session/new.mcpServers` serialization still emits stdio entries without the ACP `"type"` discriminator and has no HTTP header array support.
- `AcpAdapter` still exposes `open_session(&ExecutionRequest)`; there is no `ProviderLaunchSpec`, `ProbeKey`, `prepare_launch_spec`, or `open_session_with_launch_spec`.
- `AcpRuntimeManager` owns only adapter and live-session maps; no broker, provider capability cache, lease state, or shim dispatch state exists.
- Daemon routes mount existing `/mcp`; no `/xcode-mcp/{lease_id}` broker route was found.
- `AgentExecution`, DB repository, GraphQL, and MCP report surfaces do not include `actual_xcode_runtime_observation_json`.
- `scripts/test-gate.sh proposal-051` is unknown.

### Ambiguities / Evidence Gaps

- The current working tree is heavily dirty and contains unrelated modified/untracked/deleted files. The audit did not attempt to attribute changes to specific authors.
- Runtime Xcode behavior was not validated because the required implementation surfaces were absent by code and gate evidence.
- Full regression was not run because the result is non-green; full regression is only required before reporting `Implemented`, `Ready`, or `Ready with Risks`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 3 |
| Partially Implemented | 1 |
| Missing | 20 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Research artifact exists with allowed verdict

- Proposal Source: §3.1 lines 87-116.
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md:12`
  - `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md:14`
- Gap / Note: Artifact exists and states `Proceed with scoped architecture`.

### REQ-002 Register canonical gate alias `proposal-051|p051`

- Proposal Source: header note lines 14; §7 Gate ownership lines 1035-1042; §10 Gate lines 1227-1265.
- Status: Missing
- Evidence Type: tests-run, code
- Evidence:
  - `./scripts/test-gate.sh proposal-051` exited with `error: Unknown gate: proposal-051`.
  - `./scripts/test-gate.sh list | rg -n "proposal-051|p051|051"` returned no P051 gate.
  - `docs/reference/test-gates.md` only references P051 as a dependency under P058, not as a registered gate.
- Gap / Note: The canonical proof gate is absent.

### REQ-003 Add HTTP streaming MCP transport and ACP wire shape

- Proposal Source: §5.1 lines 192-202; §5.2 lines 573-595; §7 Engine lines 918-925.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/lib.rs:113-126` defines only `Stdio` and `Platform` transport variants.
  - `control-plane/crates/acp/src/transport.rs:148-180` serializes only stdio and rejects platform.
  - `control-plane/crates/engine/src/mcp.rs:124-151` resolves only command-backed stdio or provider platform transports.
- Gap / Note: No HTTP variant, URL, bearer header array, or ACP discriminated union support exists.

### REQ-004 Probe provider HTTP MCP capability before MCP resolution

- Proposal Source: §5.1.2 lines 204-325; §7 ACP crate lines 825-837; §7 Engine lines 927-932.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/manager.rs:17-21` has only `adapters` and `live_sessions`.
  - Targeted search found no `ProviderCapabilityCache`, `ProviderCapabilityProbe`, `ProviderLaunchSpec`, `ProbeKey`, or `provider_http_mcp_unsupported`.
  - `control-plane/crates/engine/src/mcp.rs:61-65` accepts requested extensions, backend profile, and provider only; no capabilities input.
- Gap / Note: Resolution can still produce direct stdio payloads without any provider HTTP capability preflight.

### REQ-005 Refactor adapter API around reusable `ProviderLaunchSpec`

- Proposal Source: §5.1.2 lines 239-308; §7 ACP adapter inventory lines 902-914.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/adapters/mod.rs:15-29` still defines `open_session(&ExecutionRequest)` and `execute(req)`.
  - No `launch_spec.rs` file is present under `control-plane/crates/acp/src`.
  - `control-plane/crates/acp/src/adapters/codex.rs` still derives runtime home, env, command, config, and session startup inside `open_session`.
- Gap / Note: The proposal's byte-identical probe/session launch invariant is not structurally enforceable.

### REQ-006 Own an `XcodeMcpBridgePool` inside ACP runtime

- Proposal Source: §5.1 lines 182-202; §7 ACP crate lines 825-831.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/manager.rs:17-21` contains no broker or pool field.
  - Targeted search found no `XcodeMcpBridgePool`, `XcodeMcpBroker`, `xcode_mcp_broker.rs`, or lease state types.
- Gap / Note: The current runtime manager only stores live ACP sessions.

### REQ-007 Mount provider-facing `/xcode-mcp/{lease_id}` broker route on daemon loopback

- Proposal Source: §7 Daemon lines 854-866; §5.3 lines 629-634.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:256-264` constructs existing MCP routes and logs `/mcp`.
  - Targeted search found no `/xcode-mcp`, `broker_router`, `set_router_ready`, or broker route readiness gate.
- Gap / Note: There is no route owner, mount path, shared broker state, or readiness gate implementation.

### REQ-008 Spawn one host-env `xcrun mcpbridge` backend per lease and serialize initialize

- Proposal Source: §5.1 lines 194-202; §5.3 lines 620-627; §10 lines 1241-1246.
- Status: Missing
- Evidence Type: code
- Evidence:
  - Targeted search found no P051 backend owner module or per-PID initialize mutex.
  - `control-plane/crates/engine/src/mcp.rs:124-135` still resolves configured commands as direct stdio payloads to providers.
- Gap / Note: There is no broker-owned backend subprocess lifecycle.

### REQ-009 Preserve provider fake home while adding host-user Xcode env builder

- Proposal Source: §5.1.1 lines 327-339; §7 lines 870-875; §9 lines 1135-1137.
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/adapters/codex.rs` still prepares isolated runtime home and provider env inside `open_session`.
  - Targeted search found no `xcode_host_env.rs`, operator-home resolver, Darwin temp resolver, or host Xcode env builder.
- Gap / Note: Existing fake-home provider isolation is present, but the P051 host-session crossing point is missing.

### REQ-010 Direct Xcode command shim and option-aware `xcrun` parser

- Proposal Source: §5.1.1 lines 341-411; §7 lines 885-894; §10 lines 1273-1280.
- Status: Missing
- Evidence Type: code
- Evidence:
  - Targeted search found no `xcode_shim`, shim binaries, `CHAINWORKS_XCODE_SHIM_TOKEN`, `XCODE_SHIM_DISPATCH_SOCKET`, or `xcrun_unknown_option`.
- Gap / Note: Direct fake-home `xcodebuild`, `simctl`, and `mcpbridge` invocations are not intercepted.

### REQ-011 Catalog lint and `requires_xcode_host_execution` propagation

- Proposal Source: §5.1.1 lines 380-419; §7 lines 896-900; §10 lines 1182-1189.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/workflow/src/catalog.rs:102-122` has no `requires_xcode_host_execution` field.
  - `control-plane/crates/workflow/src/plan.rs:55-80` has no resolved field.
  - `control-plane/crates/engine/src/session/fingerprint.rs:23-76` has no fingerprint input for the field.
  - `examples/agents/agents.yaml:803-804` and `examples/agents/agents.yaml:1185-1186` still contain direct `xcodebuild` entries.
- Gap / Note: Neither hard-fail lint nor soft prompt warning is implemented, and the catalog migration is not done.

### REQ-012 Separate shim dispatch token authority with peer-credential audit

- Proposal Source: §5.1.1 lines 431-531; §10 lines 1212-1225.
- Status: Missing
- Evidence Type: code
- Evidence:
  - Targeted search found no `XcodeShimDispatchToken`, `XcodeShimDispatchLease`, `ShimDispatchRequest`, `LOCAL_PEERPID`, `SO_PEERCRED`, or `peer_pid_mismatch`.
- Gap / Note: No dispatch socket or authority separation exists.

### REQ-013 Host executor for approved `xcodebuild`/`simctl`, with `mcpbridge` always rejected

- Proposal Source: §5.1.1 lines 421-429 and 533-564; §7 lines 876-883.
- Status: Missing
- Evidence Type: code
- Evidence:
  - Targeted search found no `xcode_host_executor.rs`, host-executor allowlist, routed stdout/stderr framing, or unconditional `mcpbridge` reject path.
- Gap / Note: The direct-command containment path is absent.

### REQ-014 HTTP MCP facade method coverage, auth, ID rewrite, and error preservation

- Proposal Source: §5.2 lines 573-595.
- Status: Missing
- Evidence Type: code
- Evidence:
  - Targeted search found no Xcode MCP HTTP facade module, per-lease bearer middleware, backend request ID rewrite, or brokered `tools/list` cache.
- Gap / Note: Existing `mcp-server/src/http.rs` is the northbound control-plane MCP surface, not the provider-facing Xcode MCP broker.

### REQ-015 Provider-session-owned lease lifecycle and cleanup

- Proposal Source: §5.3 lines 596-634; §8.3 lines 1090-1098.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/session.rs` has no lease id or release hook.
  - `control-plane/crates/acp/src/manager.rs:157-170` only removes and closes the live ACP session.
  - Targeted search found no lease states `reserved`, `active`, `closing`, `released`, or `orphaned`.
- Gap / Note: No P051 lease lifecycle exists.

### REQ-016 Xcode PID drift and no-PID fail-closed handling

- Proposal Source: §5.4 lines 636-653; §8.2 lines 1076-1088.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/mcp.rs:194-233` only injects current Xcode PID into direct stdio env when available.
  - Targeted search found no `pool_pid_drift`, stale pool invalidation, or `xcode_pid_changed`.
- Gap / Note: Current behavior does not enforce broker pool drift semantics.

### REQ-017 Simulator UUID selection and ambiguity failure

- Proposal Source: §5.4.1 lines 655-660; §10 lines 1163-1165.
- Status: Missing
- Evidence Type: code
- Evidence:
  - Targeted search found no simulator UUID resolver, ambiguity error, or selected simulator observation fields.
- Gap / Note: P051's CoreSimulator destination stability work is absent.

### REQ-018 Permission fingerprint and per-lease tool filtering

- Proposal Source: §5.5 lines 661-671; §9 lines 1126-1137.
- Status: Missing
- Evidence Type: code
- Evidence:
  - Targeted search found no Xcode broker policy fingerprint, per-lease `tools/list` filter, or HTTP lease authorization map.
  - Existing MCP resolver only validates registry/provider compatibility in `control-plane/crates/engine/src/mcp.rs:99-174`.
- Gap / Note: P051-specific policy isolation is absent.

### REQ-019 Session reuse compatibility includes broker lease identity and shim authority

- Proposal Source: §5.6 lines 673-707; §5.1.1 lines 463-471.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/session/fingerprint.rs:51-75` hashes requested MCP server names, not accepted HTTP transport or broker lease identity.
  - No `requires_xcode_host_execution` or broker lease identity exists in the fingerprint input.
- Gap / Note: Generic session reuse exists, but the P051 compatibility rule does not.

### REQ-020 Durable `actual_xcode_runtime_observation_json` column and append semantics

- Proposal Source: §5.7 lines 709-782; §7 Domain / DB lines 942-982.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/domain/src/agent.rs:40-69` has no `actual_xcode_runtime_observation_json`.
  - `control-plane/crates/db/src/repos/agent_executions.rs:8-15` omits the column from `SELECT_COLS`.
  - Migration list stops at `016_p058_runtime_facts_and_artifact_claims.sql`; no P051 migration exists.
  - Targeted search found no `append_xcode_runtime_observation`.
- Gap / Note: No durable observation envelope or append path exists.

### REQ-021 GraphQL and MCP report readback for Xcode runtime observation

- Proposal Source: §5.7 lines 780-782; §7 MCP server / GraphQL lines 989-1017.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/graphql-server/src/types/stage.rs:103-156` exposes MCP fields but no `actualXcodeRuntimeObservation`.
  - `control-plane/crates/mcp-server/src/tools/reports.rs:124-142` emits MCP fields but no `xcode_runtime_observation`.
- Gap / Note: Operators cannot read P051 evidence from either northbound surface.

### REQ-022 P051 failure semantics and failure-class observations

- Proposal Source: §8 lines 1046-1118.
- Status: Missing
- Evidence Type: code
- Evidence:
  - Targeted search found no `backend_failure_class`, `per_lease_backend`, `broker_infrastructure`, `host_env_unavailable` in P051 broker code.
  - `control-plane/crates/engine/src/failure_classifier.rs` has a P058 test mentioning P051-shaped observations, but no P051 observation producer exists.
- Gap / Note: Typed failure classifier references do not implement P051 broker behavior.

### REQ-023 Migrate agent catalog direct `xcodebuild` entries

- Proposal Source: §5.1.1 lines 566-571; §10 lines 1187-1190.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `examples/agents/agents.yaml:803-804` still allows direct `xcodebuild`.
  - `examples/agents/agents.yaml:1185-1186` still declares direct `xcodebuild` required tools.
  - No `requires_xcode_host_execution` entries were found.
- Gap / Note: Catalog migration did not happen.

### REQ-024 Preserve `MCP_XCODE_PID` targeting

- Proposal Source: §3 exclusions lines 75-85; §7 Engine line 922.
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/mcp.rs:194-233` defines `inject_xcode_mcpbridge_pid_env`.
  - `control-plane/crates/engine/src/mcp.rs:129-130` calls the injector for command-backed MCP entries.
- Gap / Note: This is pre-existing direct-stdio behavior, not broker behavior.

## Architecture Review

**Summary:** Weak

### ARCH-001 Core broker architecture is absent

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-003, REQ-006, REQ-007, REQ-008, REQ-014, REQ-015
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/manager.rs:17-21`
  - `control-plane/crates/acp/src/lib.rs:113-126`
  - `control-plane/crates/daemon/src/main.rs:256-264`
- Why It Matters: The proposal's value depends on moving Xcode MCP out of provider-owned stdio process spawning and into a Chainworks-owned broker boundary. The current architecture still has no broker route, broker state, HTTP transport, or lease lifecycle.
- Recommended Action: Implement the wire substrate first: `Http` transport variant, ACP wire emitter changes, daemon route mount, broker state, and lease allocation before adapter launch.

### ARCH-002 Adapter launch identity invariant is not enforceable

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-005
- Evidence Type: code
- Evidence:
  - `control-plane/crates/acp/src/adapters/mod.rs:15-29`
  - `control-plane/crates/acp/src/adapters/codex.rs:52-123`
- Why It Matters: Capability probing is only meaningful when the probed process shape is identical to the real provider session. The current adapter API still lets each adapter build env, args, and config privately at launch time.
- Recommended Action: Add `ProviderLaunchSpec`, `prepare_launch_spec`, `open_session_with_launch_spec`, and tests that fixture provider args/env match the spec exactly.

## Product Review

**Summary:** At Risk

### PROD-001 Primary user flow remains unavailable

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-003, REQ-006, REQ-008, REQ-010, REQ-013
- Evidence Type: code, tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-051` -> unknown gate.
  - Targeted search found no broker, shim, host executor, or Xcode runtime envelope symbols.
- Why It Matters: Operators still cannot run parallel Xcode-capable ACP sessions through a host-session-safe broker, and isolated fake-home agents still have no P051 direct-command guard.
- Recommended Action: Do not dogfood P051 workflows until the broker and shim paths exist and the fixture gate proves them.

## UI Review

**Summary:** Acceptable

No UI findings. P051 explicitly excludes Swift app UI changes, and the audited gaps are control-plane/runtime surfaces rather than screens, layout, or macOS window behavior.

## UX Review

**Summary:** At Risk

### UX-001 Promised operator evidence is not readable anywhere

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-020, REQ-021, REQ-022
- Evidence Type: code
- Evidence:
  - `control-plane/crates/graphql-server/src/types/stage.rs:103-156`
  - `control-plane/crates/mcp-server/src/tools/reports.rs:124-142`
  - `control-plane/crates/domain/src/agent.rs:40-69`
- Why It Matters: P051 is designed to replace log-pattern inference with structured evidence. Without the durable envelope and readback, operators cannot distinguish host-env failures, shim rejections, broker failures, lease reuse, or simulator selection.
- Recommended Action: Add the DB/domain envelope and northbound typed readback before treating broker behavior as operationally supportable.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Canonical P051 gate is missing

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-002, REQ-024
- Evidence Type: tests-run, code
- Evidence:
  - `./scripts/test-gate.sh proposal-051` exits with `error: Unknown gate: proposal-051`.
  - `./scripts/test-gate.sh list | rg -n "proposal-051|p051|051"` produced no P051 gate.
- Why It Matters: The proposal explicitly makes `proposal-051|p051` the proof boundary. Without it, there is no canonical same-tree signal for broker, shim, observation, or failure semantics.
- Recommended Action: Add the gate only after implementing fixture tests for core broker behavior, capability preflight, daemon route readiness, shim enforcement, and observation readback.

### READY-002 Dirty tree lowers handoff safety

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: All
- Evidence Type: code
- Evidence:
  - `git status --short` showed many modified, deleted, and untracked files before the audit report was written.
- Why It Matters: A dirty tree is auditable, but it makes handoff and reproduction harder, especially when the audited proposal is not implemented and adjacent proposal work is in flight.
- Recommended Action: Before implementation sign-off, isolate P051 changes into a reviewable branch or commit range and rerun the audit on that narrower tree.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Not Checked | Not required for non-green audit; no implementation surfaces exist to validate. |
| Core user flow runtime-validated | Fail | Broker/shim/host-executor flows are absent. |
| Empty/loading/error states covered | Not Applicable | No Swift UI in P051. |
| Accessibility risk acceptable | Not Applicable | No Swift UI in P051. |
| Localization risk acceptable | Not Applicable | No user-facing Swift strings in scope. |
| Critical tests executed | Fail | `./scripts/test-gate.sh proposal-051` is unknown. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not run because conformance/readiness are non-green. |
| Privacy/permissions/entitlements reviewed | Fail | P051 token, host-home, shim, and broker security surfaces are absent. |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/051-shared-xcode-mcp-bridge-pool.md`
- `git rev-parse HEAD`
- `git status --short`
- `date -Iseconds`
- `rg -n "xcode_mcp|XcodeMcp|xcode_host|XcodeHost|xcode_shim|XcodeShim|actual_xcode_runtime|XcodeRuntime|ProviderCapability|ProviderLaunchSpec|ProbeKey|mcpCapabilities|provider_http_mcp|Http \\{|headers|requires_xcode_host_execution|xcode_absolute_path_forbidden|proposal-051|p051" control-plane examples scripts docs/reference -g '!target'`
- `./scripts/test-gate.sh list | rg -n "proposal-051|p051|051|proposal-050|proposal-052"`
- `./scripts/test-gate.sh proposal-051`
- Focused file reads of `control-plane/crates/acp/src/lib.rs`, `control-plane/crates/acp/src/adapters/mod.rs`, `control-plane/crates/acp/src/manager.rs`, `control-plane/crates/acp/src/transport.rs`, `control-plane/crates/engine/src/mcp.rs`, `control-plane/crates/workflow/src/catalog.rs`, `control-plane/crates/workflow/src/plan.rs`, `control-plane/crates/engine/src/session/fingerprint.rs`, `control-plane/crates/domain/src/agent.rs`, `control-plane/crates/db/src/repos/agent_executions.rs`, `control-plane/crates/graphql-server/src/types/stage.rs`, `control-plane/crates/mcp-server/src/tools/reports.rs`, `scripts/test-gate.sh`, and `examples/agents/agents.yaml`.

## Recommended Next Actions

1. Implement the P051 wire substrate: `Http` transport, ACP `mcpServers` discriminated union, capability preflight inputs, and fail-closed unsupported-provider path.
2. Add the broker daemon integration: `/xcode-mcp/{lease_id}` route, shared broker state, route readiness gate, lease/token minting, and per-lease backend owner.
3. Add host-user Xcode execution modules: host env builder, per-PID initialize mutex, simulator UUID resolver, PID drift handling, and failure classes.
4. Add direct-command containment: catalog field propagation, lint, shim binaries, dispatch token state, peer credential audit, and host executor.
5. Add durable observation and readback: DB migration, domain types, append repository, GraphQL typed field, MCP report projection, and parity tests.
6. Register `proposal-051|p051` only after the focused fixture inventory exists and fails closed when any required proof is missing.
