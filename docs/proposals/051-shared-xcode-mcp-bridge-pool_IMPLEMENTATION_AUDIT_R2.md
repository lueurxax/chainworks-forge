# Proposal 051 Implementation Audit Report R2

## 0. Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` |
| Proposal state | `Active` with completed research gate; proposal metadata still says `Research-gated, scope revised`, and §3.1 says implementation may proceed after the scoped-architecture verdict |
| Implementation target | Current worktree / current branch |
| Compare base | Implicit; no PR/range supplied |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `e9cbc522c7d4b58f1e7d4cc7f09a0b0357bd60bc` |
| Working tree status | Dirty; many ACP/engine/daemon/workflow/domain/db/example/test-gate files modified, plus prior untracked P051 R1 audit |
| Audit timestamp | 2026-04-19 |
| Report path | `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R2.md` |
| Platform/product scope | `cross-stack`: Rust ACP/runtime/daemon/workflow/catalog/API/security/reliability/rollout |

## 1. Verdict

- Overall Conformance: `Not Implemented`
- Overall Implementation Readiness: `Not Ready`
- Reviewer Selection Reuse: `Not reused`
- Audit Confidence: `High`
- Same-tree full regression / canonical gate: `Not Run`
- Highest-risk blockers:
  1. The ACP adapter/runtime boundary remains `open_session(&ExecutionRequest)`; P051's capability preflight, `ProviderLaunchSpec`, `SessionNewSpec`, and `open_session_with_specs` contract are absent.
  2. MCP resolution and `session/new` serialization still model stdio/platform transports only; there is no Xcode HTTP broker lease path and current Xcode MCP remains direct `xcrun mcpbridge` stdio behavior.
  3. The host-user Xcode execution boundary, command shim/dispatch token, peer credential audit, simulator destination rewrite, and durable `actual_xcode_runtime_observation_json` are absent.

## 2. Prior Proposal-Review Reuse

- Prior artifacts found:
  - `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`
- Prior selected reviewers: none detected by the reuse helper.
- Prior rejected close alternatives: none detected.
- Prior stacks / surfaces / risks:
  - Rust ACP provider/session startup, MCP transport wire shape, daemon loopback HTTP routing, host-user Xcode boundary, simulator UUID stability, security token/lease handling.
- Prior required changes before implementation:
  - Revise to one `mcpbridge` backend per HTTP client session, preserve HTTP streaming provider path, serialize initialize per Xcode PID, and keep broker-owned host-user environment.
- Reuse decision: `Not reused`
- Delta from prior selection: Routed from proposal/current implementation evidence because no structured reviewer selection state was found.
- Reasoning: The only discovered proposal-local artifact is the research pack, not `reviewer-selection.yaml` or an equivalent proposal-review selection artifact. Per audit rules, the existing `IMPLEMENTATION_AUDIT_R1` is ignored for reviewer selection.

## 3. Current Reviewer Routing

| Reviewer | Discipline / Stack | Why Selected | Evidence IDs | Reused From Proposal Review? | Notes |
|---|---|---|---|---|---|
| `rust_arch_reviewer` | Rust ACP/engine/daemon architecture | P051 changes core adapter traits, runtime manager ownership, daemon router composition, workflow compiler/catalog chain | E-CODE-001, E-CODE-002, E-CODE-004, E-CODE-006 | No | Primary implementation gap owner |
| `rust_reliability_reviewer` | Lease lifecycle, reuse, cancellation, failure isolation | P051 requires provider-session-owned leases, per-lease backend cleanup, PID drift handling, reuse-compatible supersession | E-PROP-006, E-CODE-003, E-CODE-011 | No | Most lifecycle requirements missing |
| `rust_security_reviewer` | Token, shim dispatch, peer identity, host boundary | P051 defines bearer/session-id binding, Unix socket peer credential derivation, command allowlist, no raw `mcpbridge` bypass | E-PROP-004, E-PROP-005, E-PROP-008, E-CODE-010 | No | Selected due shell-capable agent boundary |
| `api_contract_reviewer` | ACP `session/new`, GraphQL/MCP/report contracts, catalog schema | P051 changes ACP MCP wire shape, `ResolvedAgent`, `ExecutionRequest`, DB/domain, GraphQL/MCP projection | E-PROP-002, E-PROP-009, E-CODE-002, E-CODE-006, E-CODE-007 | No | Critical contract gaps |
| `observability_rollout_reviewer` | Test gate, runtime observations, readiness/health, rollout proof | P051 requires canonical `proposal-051|p051` gate, daemon route readiness, structured observations | E-PROP-001, E-PROP-009, E-CODE-005, E-CODE-008 | No | Gate is absent |

### Rejected Close Alternatives

| Reviewer | Why Rejected | Evidence IDs |
|---|---|---|
| `macos_ui_reviewer` | Proposal explicitly excludes Swift app UI changes; audited gaps are Rust runtime/control-plane. | E-PROP-001 |
| `apple_arch_reviewer` | No Swift/AppKit/SwiftUI implementation slice required by P051. | E-PROP-001 |
| `rust_performance_reviewer` | Startup latency is observable, but no performance implementation exists to assess beyond missing broker/runtime. | E-CODE-004 |
| `product_reviewer` | No product metrics or decision checkpoint implementation is central to this audit. | E-PROP-001 |

## 4. Proposal Contract Summary

- In scope:
  - Chainworks-owned HTTP streaming Xcode MCP broker.
  - Provider capability preflight before MCP resolution and before per-lease resources.
  - One backend `xcrun mcpbridge` subprocess per HTTP client session.
  - Host-user environment for broker-owned Xcode/CoreSimulator processes.
  - Direct Xcode command containment through PATH shims plus catalog lint for executable fields.
  - Simulator UUID selection and `xcodebuild -destination` argv rewrite/reject behavior.
  - Session-lifetime bearer tokens bound to MCP session-id with single active stream.
  - Durable Xcode runtime observation envelope and GraphQL/MCP readback.
  - Canonical `proposal-051|p051` proof gate.
- Out of scope:
  - Sharing one language-model ACP session between reviewers.
  - Changing XcodeBuildMCP itself.
  - Stdio proxy compatibility as the P051 architecture.
  - Swift app UI changes.
  - General-purpose shell policy engine.
- Platform/product scope:
  - Cross-stack Rust control-plane plus ACP/MCP/API/catalog/observability/security surfaces. No iOS/macOS UI surface.
- Locked decisions:
  - HTTP streaming broker path, no stdio fallback.
  - Broker owns `xcrun mcpbridge`; agents must not directly run raw `mcpbridge`.
  - Provider capability preflight uses the same process launch capability slice as real startup.
  - `ProviderLaunchSpec` and `SessionNewSpec` are separate.
  - Tokens are session-lifetime, not single-use.
  - `requires_xcode_host_execution` must flow through catalog, compiler, request, lease, fingerprint, and reuse.
- Primary service implementation flows:
  1. Xcode MCP agent starts: prepare launch spec, preflight provider HTTP capability, resolve Xcode MCP to broker HTTP entry, attach credentials, send `session/new`, connect lease.
  2. Parallel Xcode MCP agents: each gets separate lease/backend, initialize serializes per Xcode PID, tool calls remain isolated/parallel.
  3. Direct `xcodebuild`/`simctl` from agent shell: shim dispatches to host executor or rejects with durable observation.
  4. Reused provider session: live session is reused only when accepted MCP set and frozen Xcode host-execution policy match; otherwise fresh session/lease/token.
  5. Failure path: provider HTTP unsupported, broker/host-env unavailable, PID drift, backend crash, or cancellation fails closed and records structured evidence.
- Acceptance criteria and test/evidence requirements:
  - Proposal §10 and §11 require unit/integration/static gate tests for provider capability preflight, HTTP broker, lease cleanup, direct command guard, simulator rewrite, catalog field propagation, reuse compatibility, observation readback, and gate registration.

## 5. Implementation Evidence Summary

- Changed files / modules / crates inspected:
  - `control-plane/crates/acp/src/{lib.rs,manager.rs,transport.rs,adapters/*.rs}`
  - `control-plane/crates/engine/src/{executor.rs,mcp.rs,session/fingerprint.rs}`
  - `control-plane/crates/workflow/src/{catalog.rs,compiler.rs,definition.rs,plan.rs}`
  - `control-plane/crates/domain/src/agent.rs`
  - `control-plane/crates/db/src/repos/agent_executions.rs`
  - `control-plane/crates/daemon/src/main.rs`
  - `examples/agents/{agents.yaml,agents_mcp_profiles_v2.yaml}`
  - `scripts/test-gate.sh`, `docs/reference/test-gates.md`
- Adjacent files inspected:
  - Proposal and research artifact.
- Tests found:
  - Existing ACP/engine/workflow/db tests are present, but no `proposal-051|p051` gate or P051-specific tests were found.
- Tests run:
  - None. Source inspection establishes missing required implementation surfaces; a successful verdict is not possible, so full regression was not required by the skill's successful-verdict rule.
- Runtime checks:
  - None.
- Benchmarks:
  - None.
- API/schema/migration checks:
  - Source inspection of ACP wire emitter, DB migrations, domain model, workflow catalog/schema, daemon routing, and gate docs.
- Evidence gaps:
  - No runtime proof of Xcode MCP broker behavior exists because the broker surface is absent.

## 6. Proposal Fidelity / Divergence

### Matches

- Research gate artifact exists and has an allowed verdict: "Proceed with scoped architecture" with Phase 0 probes complete.
- Existing code still injects `MCP_XCODE_PID` for direct stdio `xcrun mcpbridge` entries, which P051 explicitly says should not be removed. This is legacy support, not the P051 broker.
- Existing ACP session reuse machinery exists, but it does not include P051's accepted-MCP-set, broker lease, or shim-signal compatibility rules.

### Divergences

- The implementation still represents Xcode MCP as stdio `xcrun mcpbridge`, while P051 requires provider-facing HTTP entries and no stdio fallback.
- Adapter startup still privately derives process env/args/config inside `open_session(&ExecutionRequest)`, contrary to P051's trait split.
- The daemon mounts only `/mcp` on the shared listener; no `/xcode-mcp/{lease_id}` route or broker state is mounted.
- Catalog/compiler/runtime do not carry `requires_xcode_host_execution`.
- Direct `xcodebuild` allowlist entries remain in the sample agent catalogs without P051's explicit migration field.
- Durable Xcode runtime observation, GraphQL typed readback, and MCP report projection are absent.

### Ambiguities / Evidence Gaps

- Because the worktree is dirty and includes many P057/P058-era changes, some unrelated files may be in-flight. The P051-specific symbols and modules are absent across the audited tree, so this does not affect the P051 verdict.
- No structured reviewer-selection artifact exists for P051; audit routing was inferred from proposal and implementation evidence.

## 7. Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 2 |
| Partially Implemented | 2 |
| Missing | 14 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## 8. Requirement Audit

### REQ-001 Research gate is complete and allows scoped implementation
- Proposal Source: §3.1, lines 87-116; research artifact lines 12-18 and 37-51.
- Status: `Implemented`
- Implementation Mapping: Proposal-local research artifact exists at the referenced path.
- Evidence Type: `proposal`, `prior-review`
- Evidence: `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md:12` records "Proceed with scoped architecture"; lines 41-51 record concurrent `mcpbridge` probe success.
- Gap / Note: This is proposal/research readiness, not runtime implementation.

### REQ-002 Add provider capability preflight/cache before MCP resolution
- Proposal Source: §5.1.2, lines 233-239 and 292-303.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: `AcpRuntimeManager` owns only `adapters` and `live_sessions` (`control-plane/crates/acp/src/manager.rs:17-21`); no `ProviderCapabilityCache` or `ensure_provider_capabilities`. `engine::mcp::resolve_mcp_servers` receives only requested ids/profile/provider (`control-plane/crates/engine/src/mcp.rs:61-65`), not `AgentCapabilities`.
- Gap / Note: Provider HTTP capability cannot fail closed before lease/session allocation because there is no preflight path.

### REQ-003 Split adapter API into `ProviderLaunchSpec`, `SessionNewSpec`, and `open_session_with_specs`
- Proposal Source: §5.1.2, lines 307-345 and 348-403.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: `AcpAdapter` still exposes `open_session(&ExecutionRequest)` and default `execute` calls it directly (`control-plane/crates/acp/src/adapters/mod.rs:15-29`). Codex/Gemini/Claude adapters still build env/args/config privately inside `open_session` (`codex.rs:52-123`, `gemini.rs:54-102`, `claude.rs:57-103`).
- Gap / Note: The proposal's probe/real-session equivalence cannot be enforced.

### REQ-004 Resolve Xcode MCP to provider-facing HTTP streaming entries and reject unsupported providers before lease/session
- Proposal Source: §5.1.2, lines 407-412; §5.2, lines 783-805; research artifact lines 107-131.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: `ResolvedMcpServerTransport` supports only `Stdio` and `Platform` (`control-plane/crates/acp/src/lib.rs:113-126`). `mcp_servers_wire_value` emits stdio without a `type` discriminator and bails on `Platform` (`control-plane/crates/acp/src/transport.rs:148-176`). No HTTP/SSE transport variant exists.
- Gap / Note: Broker HTTP URL/header cannot reach `session/new`.

### REQ-005 Add Xcode MCP broker/pool/HTTP route on daemon listener
- Proposal Source: §5.2, lines 783-805; §7, lines 1129-1150.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: `daemon/src/main.rs` creates `AcpRuntimeManager::new()` without broker state (`control-plane/crates/daemon/src/main.rs:199-216`), mounts only `mcp_server::http::routes(mcp)` and logs `/mcp` (`main.rs:256-264`), then passes only `mcp_routes` into GraphQL serving (`main.rs:341-349`). No `xcode_mcp_broker.rs` or `/xcode-mcp` route was found.
- Gap / Note: Provider-facing broker endpoint and route readiness do not exist.

### REQ-006 Launch one backend `xcrun mcpbridge` per HTTP client session and serialize initialize per Xcode PID
- Proposal Source: Scope lines 64-72; §5.3, lines 830-837; test plan lines 1441-1444 and 1471-1475.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: No broker/pool/backend owner module exists. Current MCP resolver directly produces `ResolvedMcpServerTransport::Stdio { command, args, env }` (`control-plane/crates/engine/src/mcp.rs:124-135`), so providers still own direct backend launch.
- Gap / Note: No per-PID initialize mutex, backend subprocess ownership, or request-id correlation exists.

### REQ-007 Preserve host-user Xcode execution boundary while keeping provider fake-home isolation
- Proposal Source: §5.1.1, lines 424-436; §8.1.1, lines 1347-1354.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: Codex still constructs an isolated runtime home and environment for the provider (`control-plane/crates/acp/src/adapters/codex.rs:60-80`) and there is no `xcode_host_env.rs`, `xcode_host_executor.rs`, or broker-owned Xcode subprocess environment.
- Gap / Note: Provider fake-home isolation remains, but Xcode commands are not moved behind a host-user executor.

### REQ-008 Add direct Xcode command guard, shim dispatch, and absolute `mcpbridge` rejection
- Proposal Source: §5.1.1, lines 438-470, 767-774, and 706-720.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`, `config`
- Evidence: No `XcodeShimInjectionSignal`, no shim dispatch socket, no `XcodeShimDispatchLease`, no direct-command lint/guard modules were found. Example catalogs still allow direct `xcodebuild` commands (`examples/agents/agents.yaml:789-804`).
- Gap / Note: The diagnostic-mode hard ban for `mcpbridge` has no enforcement point.

### REQ-009 Derive Unix socket peer pid/uid for shim audit identity
- Proposal Source: §5.1.1, lines 706-720 and §5.7 envelope lines 1007-1021.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: No shim Unix socket listener or peer-credential derivation code found; no `derived_peer_pid`, `derived_peer_uid`, or `claimed_provider_pid` fields exist in implementation types.
- Gap / Note: This depends on REQ-008.

### REQ-010 Add `requires_xcode_host_execution` through catalog, compiler, request, fingerprint, and reuse
- Proposal Source: §7 "Catalog / compiler / engine chain", lines 1308-1317; §5.1.1 lines 442-448; §5.3 lines 673-681.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`, `config`
- Evidence: `AgentEntry` lacks `requires_xcode_host_execution` (`control-plane/crates/workflow/src/catalog.rs:102-122`); `ResolvedAgent` lacks it (`plan.rs:55-91`); compiler only threads existing session/worktree/MCP fields (`compiler.rs:263-352`); `BindingFingerprintInput` lacks the field (`engine/src/session/fingerprint.rs:23-44`); example catalogs contain direct `xcodebuild` entries but no field.
- Gap / Note: Changing only the host-execution policy cannot force fresh session/lease/token because it is not represented.

### REQ-011 Implement session-lifetime lease bearer with first-connect TTL, MCP session-id binding, single active stream, reconnect/replay rules
- Proposal Source: §5.3, lines 806-865; §9 lines 1416-1423.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: No lease state, token state, `first_connect_deadline`, `bound_mcp_session_id`, or `active_stream_count` implementation found. `AcpRuntimeManager` stores only live ACP sessions keyed by generation id (`control-plane/crates/acp/src/manager.rs:17-21`).
- Gap / Note: The broker security model is absent.

### REQ-012 Add P051 session reuse compatibility for accepted MCP set, live lease, and shim signal
- Proposal Source: §5.6, lines 932-963; §5.3 lines 673-681.
- Status: `Partially Implemented`
- Implementation Mapping: Generic session reuse exists, P051-specific compatibility does not.
- Evidence Type: `code`
- Evidence: Existing reuse policy hashes basic binding inputs including requested MCP server ids (`engine/src/session/fingerprint.rs:23-75`) and engine routes reusable sessions through `reuse_existing_session` (`engine/src/executor.rs:1512-1525`). There is no accepted MCP transport set, HTTP broker contract, lease liveness, Xcode PID, permission fingerprint, or shim-signal check in the fingerprint/reuse path.
- Gap / Note: Current reuse could treat a live session as reusable even though P051 would require fresh session or live-lease validation.

### REQ-013 Add durable `actual_xcode_runtime_observation_json` and append/readback semantics
- Proposal Source: §5.7, lines 974-1076; §7 domain/DB lines 1232-1272; GraphQL/MCP lines 1281-1305.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`, `migration`
- Evidence: Domain `AgentExecution` has MCP observation fields only (`control-plane/crates/domain/src/agent.rs:40-69`). DB `SELECT_COLS`, insert, and update paths include `actual_mcp_observation_json`, not Xcode runtime observation (`control-plane/crates/db/src/repos/agent_executions.rs:8-15`, `24-64`, `126-150`). Latest migration list ends at `017_p057_artifact_contract_dimensions.sql`; no P051 column migration exists.
- Gap / Note: GraphQL/MCP readback cannot expose absent durable state.

### REQ-014 Add simulator UUID destination parser/argv rewrite/reject contract
- Proposal Source: §5.4.1, lines 888-918; test plan lines 1457-1465.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: No `xcode_host_executor.rs` or destination parser/rewrite module found. Existing code only injects `MCP_XCODE_PID` for `xcrun mcpbridge` (`control-plane/crates/engine/src/mcp.rs:194-211`).
- Gap / Note: Name/OS simulator destinations would still reach `xcodebuild` unchanged if an agent runs them.

### REQ-015 Preserve `MCP_XCODE_PID` targeting for broker-owned `mcpbridge`
- Proposal Source: Scope exclusions lines 75-80; host-env lines 428-435.
- Status: `Partially Implemented`
- Implementation Mapping: Legacy stdio resolver injects the env var.
- Evidence Type: `code`
- Evidence: `inject_xcode_mcpbridge_pid_env` adds `MCP_XCODE_PID` when command is `xcrun` and first arg is `mcpbridge` (`control-plane/crates/engine/src/mcp.rs:194-211`).
- Gap / Note: This is on the old direct stdio path. P051 requires broker-owned launches to preserve this targeting after the HTTP broker is added.

### REQ-016 Enforce policy/tool filtering per lease and prevent lower-permission reuse of higher-permission tool sets
- Proposal Source: §5.5, lines 920-931; §9 lines 1416-1421.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`
- Evidence: No broker lease model, permission fingerprint in pool key, per-lease `tools/list` filtering, or HTTP bearer middleware exists.
- Gap / Note: Depends on REQ-005/REQ-011.

### REQ-017 Migrate example agent catalogs for explicit `requires_xcode_host_execution`
- Proposal Source: §5.1.1, lines 776-781; §7 lines 1318-1323.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `config`
- Evidence: `examples/agents/agents.yaml` still has direct `xcodebuild` allowlist entries (`lines 789-804`), and `rg requires_xcode_host_execution examples/agents/...` returned no matches.
- Gap / Note: The catalog cannot distinguish intended direct-host execution from commands that should be rejected.

### REQ-018 Add canonical `proposal-051|p051` proof gate and docs
- Proposal Source: Gate note lines 14-15; scope line 73; §7 gate ownership lines 1325-1333.
- Status: `Missing`
- Implementation Mapping: None found.
- Evidence Type: `code`, `docs`
- Evidence: `scripts/test-gate.sh` lists `proposal-050`, `proposal-057`, and `proposal-058`, but no `proposal-051|p051` case (`scripts/test-gate.sh:1469-1471`, `2079-2214`). `docs/reference/test-gates.md` references P051 only as consumed by P058, not as its own gate (`docs/reference/test-gates.md:931-979`).
- Gap / Note: No canonical same-tree P051 proof path exists.

## 9. Prior Review Finding Follow-Through

| Prior Finding / Required Change | Status | Evidence | Notes |
|---|---|---|---|
| HTTP streaming feasibility must complete before implementation | `Addressed` | Research artifact lines 12-18, 37-51, 95-131 | Proposal/research state only |
| Use per-lease backend subprocesses rather than one shared backend | `Not Addressed` | No broker/backend owner module found | Still direct stdio resolver |
| Provider `session/new` must receive HTTP MCP entries with `type` and headers | `Not Addressed` | `transport.rs:148-176` | No HTTP variant; stdio emitter omits `type` |
| Keep broker as host-user Xcode boundary | `Not Addressed` | No host env/executor modules | Provider fake-home remains the only modeled launch env |

## 10. Reviewer Scorecard

| Reviewer | Result | Confidence | Evidence Completeness | Critical | Major | Minor | Notes |
|---|---|---|---|---:|---:|---:|---|
| `rust_arch_reviewer` | `Fail` | High | High | 1 | 1 | 0 | Core architecture surface absent |
| `rust_reliability_reviewer` | `Fail` | High | High | 1 | 1 | 0 | Lease/reuse/failure semantics absent |
| `rust_security_reviewer` | `Fail` | High | High | 1 | 0 | 0 | Shim/token/host boundary absent |
| `api_contract_reviewer` | `Fail` | High | High | 1 | 1 | 0 | ACP, catalog, DB/API contracts absent |
| `observability_rollout_reviewer` | `Fail` | High | High | 0 | 2 | 0 | Gate/readiness/observation proof absent |

## 11. Routed Specialist Findings

### 11.1 Critical

#### ARCH-001
- Reviewer: `rust_arch_reviewer`
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-002, REQ-003, REQ-004
- Evidence Type: `code`
- Evidence: `control-plane/crates/acp/src/adapters/mod.rs:15-29`, `control-plane/crates/acp/src/adapters/codex.rs:52-123`, `control-plane/crates/acp/src/manager.rs:17-21`, `control-plane/crates/engine/src/mcp.rs:61-65`
- Why It Matters: P051's safety model depends on probing the same launch capability slice later used for the real provider session, then resolving MCP servers from the provider's advertised HTTP capabilities. The current API leaves launch construction inside adapters and resolves MCP without capabilities, so the proposal's preflight guarantee is structurally impossible.
- Recommended Action: Introduce `ProviderLaunchSpec`, `SessionNewSpec`, adapter `prepare_launch_spec`, `prepare_session_new_spec`, and `open_session_with_specs`; move env/args/config derivation into shared launch builders; add `AcpRuntimeManager::ensure_provider_capabilities`.
- Acceptance Criteria: Tests fail if Codex, Claude, Gemini, Auggie, or Junie launch env/args/config diverge between probe and real session capability slice; real `session/new` receives resolved MCP entries while probe sends `mcpServers: []`.

#### API-001
- Reviewer: `api_contract_reviewer`
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-004, REQ-013
- Evidence Type: `code`
- Evidence: `control-plane/crates/acp/src/lib.rs:113-126`, `control-plane/crates/acp/src/transport.rs:148-176`, `control-plane/crates/domain/src/agent.rs:40-69`, `control-plane/crates/db/src/repos/agent_executions.rs:8-15`
- Why It Matters: P051 requires the provider-facing ACP payload to use HTTP MCP entries with URL and bearer headers, and requires separate durable Xcode runtime observation. The current transport can only emit stdio payloads and the persistence model has no Xcode observation field.
- Recommended Action: Add HTTP/SSE MCP transport variants and wire serialization; add the DB migration/domain/repo/GraphQL/MCP projection for `actual_xcode_runtime_observation_json`.
- Acceptance Criteria: ACP fixture sees `session/new.params.mcpServers[]` with `type: "http"`, `url`, and `headers`; DB/GraphQL/MCP readback expose typed `mcp_broker_observations`, `xcode_shim_events`, and `xcode_host_executor_events`.

#### REL-001
- Reviewer: `rust_reliability_reviewer`
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-005, REQ-006, REQ-011, REQ-012
- Evidence Type: `code`
- Evidence: `control-plane/crates/acp/src/manager.rs:17-21`, `control-plane/crates/engine/src/executor.rs:1512-1525`, `control-plane/crates/engine/src/session/fingerprint.rs:23-75`
- Why It Matters: P051's reliability goals are the broker lease lifecycle, per-lease backend isolation, provider-session-owned token lifetime, and reuse-incompatible supersession. None of those states or checks exist, so cancellation, PID drift, replay/reconnect, and stale-session reuse cannot satisfy the proposal.
- Recommended Action: Implement `XcodeMcpBridgePool`/lease states, per-lease backend ownership, first-connect TTL, MCP session-id binding, single-active-stream handling, lease cleanup on every provider close/error/drop path, and P051 reuse compatibility/fingerprint inputs.
- Acceptance Criteria: Fixture tests prove one cancelled/backend-failed lease does not close sibling leases, PID drift closes all stale-PID leases, successful execution does not release a provider-session-owned lease, and changing only accepted Xcode MCP contract/host-execution policy forces fresh session.

#### SEC-001
- Reviewer: `rust_security_reviewer`
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-007, REQ-008, REQ-009, REQ-016
- Evidence Type: `code`, `config`
- Evidence: No shim/host executor modules found; `examples/agents/agents.yaml:789-804` still allows direct `xcodebuild`.
- Why It Matters: Without command shims, dispatch-token validation, Unix socket peer credentials, host-env allowlisting, and an absolute `mcpbridge` rejection path, shell-capable agents can continue running Xcode tools directly from the fake-home provider environment, preserving the CoreSimulator failure class and bypassing broker policy/observability.
- Recommended Action: Add PATH shims, `XcodeShimDispatchLease`, Unix-domain dispatch socket with peer credential derivation, host executor for only `xcodebuild`/`simctl`, and hard reject `mcpbridge`/`xcrun mcpbridge` even in diagnostic mode.
- Acceptance Criteria: Forged direct socket requests cannot spoof recorded peer pid/uid; fake-home `requires_xcode_host_execution: false` direct Xcode commands are rejected with structured observation; true host-execution commands route through host env; all `mcpbridge` direct invocations fail before provider launch or at shim boundary.

### 11.2 Major

#### OPS-001
- Reviewer: `observability_rollout_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-005, REQ-013, REQ-018
- Evidence Type: `code`, `docs`
- Evidence: `control-plane/crates/daemon/src/main.rs:256-264`, `scripts/test-gate.sh:1469-1471`, `scripts/test-gate.sh:2079-2214`, `docs/reference/test-gates.md:931-979`
- Why It Matters: The daemon has no `/xcode-mcp` route readiness/health integration, no P051 proof gate, and no durable observations. Operators and CI cannot prove the proposal's behavior or safely distinguish broker unavailable, provider HTTP unsupported, backend crash, PID drift, or host-env failure.
- Recommended Action: Mount broker router at `/xcode-mcp/{lease_id}` on the existing daemon listener, add readiness gating before lease URL/token emission, add `proposal-051|p051` to `scripts/test-gate.sh` and `docs/reference/test-gates.md`, and include observation readback tests.
- Acceptance Criteria: `./scripts/test-gate.sh proposal-051` exists and statically verifies the research artifact, route readiness, no lease URL/token before middleware is live, broker observations, and readback parity.

#### API-002
- Reviewer: `api_contract_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-010, REQ-017
- Evidence Type: `code`, `config`
- Evidence: `control-plane/crates/workflow/src/catalog.rs:102-122`, `control-plane/crates/workflow/src/plan.rs:55-91`, `control-plane/crates/workflow/src/compiler.rs:263-352`, `control-plane/crates/engine/src/session/fingerprint.rs:23-75`, `examples/agents/agents.yaml:789-804`
- Why It Matters: `requires_xcode_host_execution` is the policy bit that freezes shim authority and invalidates reuse. Dropping it from catalog/compiler/request/fingerprint means a live session can retain stale authority after a catalog policy change.
- Recommended Action: Add the field end-to-end, default false, reject invalid YAML types, include it in the binding fingerprint and reuse-compat checks, and migrate example agents.
- Acceptance Criteria: Changing only `requires_xcode_host_execution` forces `FreshSessionRequired` and mints a fresh shim token/lease; example catalogs explicitly annotate direct Xcode command agents.

#### REL-002
- Reviewer: `rust_reliability_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-014
- Evidence Type: `code`
- Evidence: No destination parser/rewriter module found; only existing Xcode helper is `MCP_XCODE_PID` injection in `control-plane/crates/engine/src/mcp.rs:194-211`.
- Why It Matters: The proposal explicitly says recording a selected simulator UUID is insufficient if spawned `xcodebuild` still receives ambiguous `name + OS` destinations. Without argv rewrite/reject logic, duplicate simulator devices can keep the implementation flaky.
- Recommended Action: Implement host-executor destination parser for `-destination` and `-destination=...`, pass through UUID/macOS forms, rewrite unique name/OS matches, and reject ambiguous/not-found/unparseable values before spawn.
- Acceptance Criteria: Fixture argv echo proves unique name/OS input is spawned as `id=<UUID>`, ambiguous input does not spawn `xcodebuild`, and observations record original/rewritten/rejected destination data.

#### READY-001
- Reviewer: `observability_rollout_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-018
- Evidence Type: `tests-found`
- Evidence: No P051 tests/gate were found in `scripts/test-gate.sh`; no P051 implementation modules exist to exercise.
- Why It Matters: Even after implementation lands, P051 requires a broad proof gate for transport, lease, security, simulator, observation, and reuse behavior. The current tree has no gate ownership, so implementation cannot be accepted.
- Recommended Action: Add focused Rust tests and the canonical gate before marking P051 ready.
- Acceptance Criteria: P051 gate runs in a Rust-only host policy and fails closed when any required proof inventory item is missing.

### 11.3 Minor

None.

### 11.4 Notes

- The legacy `MCP_XCODE_PID` injection is still present and should be reused by the future broker-owned `mcpbridge` launch path rather than deleted.

## 12. Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build or canonical gate status | `Not Run` | No successful verdict possible; `proposal-051|p051` gate is absent. |
| Proposal contract satisfied | `No` | 14 missing requirements. |
| Prior review blockers addressed | `No` | Research addressed; implementation blockers not addressed. |
| Tests cover committed behavior | `No` | P051-specific tests absent. |
| Critical tests executed | `No` | Not meaningful while required surfaces are absent. |
| Core user/service flow runtime or integration validated where needed | `No` | No broker path exists. |
| Empty/loading/error/offline/permission states covered where relevant | `Not Applicable` | No UI scope. |
| Accessibility and localization risk acceptable where relevant | `Not Applicable` | No UI scope. |
| API/schema compatibility acceptable | `No` | ACP HTTP MCP, catalog field, DB/API readback absent. |
| Migration/rollback path acceptable | `No` | Required DB migration absent. |
| Telemetry/observability sufficient | `No` | Required Xcode runtime observation absent. |
| Security/privacy risk acceptable | `No` | Shim/host boundary absent. |
| Privacy/permissions/entitlements reviewed where relevant | `Not Applicable` | No Swift entitlement change in P051. |
| Performance risk acceptable | `Not Verifiable` | No broker latency instrumentation exists. |
| Full regression suite or canonical full/proposal gate passed on audited tree/HEAD | `No` | Not run; P051 gate absent. |
| Release/handoff evidence sufficient | `No` | Missing implementation and proof gate. |

## 13. Product / Metrics Overlay

- Leading metric: Not selected.
- Guardrail metric: Not selected.
- Decision checkpoint: Research checkpoint completed; no implementation rollout checkpoint exists.
- Rollout recommendation: Do not hand off as implemented; complete core runtime/API/security/gate work first.
- Instrumentation gaps: Entire `actual_xcode_runtime_observation_json` envelope and broker tracing/readback are absent.

## 14. Verification Log

- Commands run:
  - `sed -n ... /Users/user/.codex/skills/proposal-implementation-audit/SKILL.md`
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...`
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...`
  - `git rev-parse HEAD`
  - `git status --short -- ...`
  - `git diff --name-only -- ...`
  - `rg -n 'Xcode|xcode|mcpbridge|ProviderLaunchSpec|SessionNewSpec|prepare_launch_spec|open_session_with_specs|ensure_provider_capabilities|ProviderCapability|mcpCapabilities|XcodeShim|ShimDispatch|xcode_mcp|xcode-mcp|actual_xcode_runtime_observation_json|requires_xcode_host_execution|proposal-051|p051|...' ...`
  - `nl -ba ... | sed -n ...` for proposal, research artifact, ACP, engine, daemon, workflow, domain, DB, example catalogs, and gate docs.
- Files inspected:
  - `docs/proposals/051-shared-xcode-mcp-bridge-pool.md`
  - `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`
  - `control-plane/crates/acp/src/adapters/mod.rs`
  - `control-plane/crates/acp/src/adapters/{codex.rs,gemini.rs,claude.rs}`
  - `control-plane/crates/acp/src/{lib.rs,manager.rs,transport.rs}`
  - `control-plane/crates/engine/src/{executor.rs,mcp.rs,session/fingerprint.rs}`
  - `control-plane/crates/workflow/src/{catalog.rs,compiler.rs,definition.rs,plan.rs}`
  - `control-plane/crates/domain/src/agent.rs`
  - `control-plane/crates/db/src/repos/agent_executions.rs`
  - `control-plane/crates/db/migrations/*`
  - `control-plane/crates/daemon/src/main.rs`
  - `examples/agents/{agents.yaml,agents_mcp_profiles_v2.yaml}`
  - `scripts/test-gate.sh`
  - `docs/reference/test-gates.md`
- Artifacts inspected:
  - Prior research artifact only; no structured reviewer-selection artifact found.
- Commands not run and why:
  - `./scripts/test-gate.sh proposal-051`: gate does not exist.
  - Full regression/canonical gate: not required because the verdict is non-successful and core implementation surfaces are absent.
  - Live Xcode/daemon/provider runtime checks: not run because no P051 broker implementation exists to exercise.

## 15. Recommended Next Actions

- MUST-01: Implement the ACP adapter/runtime contract split first: `ProviderLaunchSpec`, `SessionNewSpec`, provider capability preflight/cache, shared launch builder, and `open_session_with_specs`.
- MUST-02: Add the HTTP Xcode MCP broker path and daemon route: `/xcode-mcp/{lease_id}`, bearer middleware, per-lease backend `mcpbridge`, initialize mutex, lease lifecycle, cleanup, and readiness gating.
- MUST-03: Add the host-user Xcode boundary: env builder, PATH shims, shim dispatch socket with peer credentials, host executor for `xcodebuild`/`simctl`, hard rejection for all direct `mcpbridge` paths, and simulator destination rewrite/reject.
- MUST-04: Thread `requires_xcode_host_execution` through catalog, compiler, `ExecutionRequest`, binding fingerprint, lease freeze, and reuse compatibility; migrate example catalogs.
- MUST-05: Add durable `actual_xcode_runtime_observation_json` with DB/domain/GraphQL/MCP readback and append semantics.
- MUST-06: Add `proposal-051|p051` to `scripts/test-gate.sh` and `docs/reference/test-gates.md` with the P051 proof inventory.
- SHOULD-01: Preserve the current `MCP_XCODE_PID` targeting helper by moving/reusing it inside the broker-owned `mcpbridge` launch path.
