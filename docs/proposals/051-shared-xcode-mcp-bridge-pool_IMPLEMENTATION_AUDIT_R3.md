# P051 Implementation Audit R3

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` |
| Proposal revision | `p051-r30` |
| Proposal hash | `6f5383f057d35ec90f99083c884a53f7` |
| Proposal state | Active, but explicitly not ready for implementation scheduling while P025/P026 dependency evidence remains unresolved |
| Audit mode | `auto` via `proposal-implementation-audit` |
| Audit timestamp | `2026-04-25T08:25:39Z` |
| Implementation target | Current worktree |
| Git HEAD | `e12cca57569310f317062cea31a6b2d3a23f5080` |
| Branch | `main` |
| Compare base | Implicit current tree, no PR/range supplied |
| Working tree | Dirty before this report; audit only wrote this R3 report |
| Report path | `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R3.md` |

## Prior Review Reuse

Reviewer-selection reuse: **Partially reused**.

Durable local artifacts found under `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/`:

- `dependency-audit.md`
- `http-streaming-feasibility.md`

No durable final proposal-review artifact with a complete reviewer-selection table was found beside the proposal. Reviewer selection was therefore reconstructed from the proposal surface, repo-local routing rules, the dependency audit, and the current conversation context.

Selected reviewers:

- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`
- `chainworks_execution_truth_reviewer`

Rejected close alternatives:

- `macos_ui_reviewer`: macOS UI readback is present, but not the primary implementation risk.
- `apple_arch_reviewer`: Swift surfaces are thin presentation/readback, not state ownership or provider architecture.
- `rust_security_reviewer`: security review is required by P051 before broad host-executor merge, but this audit focused on implementation conformance and records missing security evidence as readiness debt.
- `product_reviewer`: product metrics are preserved below, but implementation risk is primarily runtime/API/rollout.

## Proposal State And Contract Summary

P051 commits to a Chainworks-owned Xcode MCP bridge pool for ACP providers:

- Serve Xcode MCP to providers through Chainworks-owned HTTP streaming endpoints, not provider stdio `xcrun mcpbridge`.
- Spawn one backend `xcrun mcpbridge` subprocess per provider HTTP lease under the host user's Xcode environment.
- Serialize backend spawn plus MCP `initialize` per Xcode PID, while allowing independent `tools/*` parallelism across leases/backends.
- Preserve fake-home provider isolation.
- Fail closed before lease/token/backend/shim/session-new when HTTP MCP capability is absent.
- Guard direct Xcode shell commands through PATH shims and catalog lint.
- Keep `mcpbridge` broker-only.
- Persist typed Xcode runtime observations and expose them through DB/domain/GraphQL/MCP/minimum Swift UI.
- Register `p051-scaffold` and `proposal-051|p051` gates.
- Keep rollout blocked on dependency readiness, dogfood evidence, and follow-up triggers.

Explicit non-goals include provider stdio proxy fallback and OS-level prevention of every prompt-time absolute path.

## Platform And Product Scope

Apple scope: **macOS**.

Backend/service scope: **cross-stack Rust control-plane service, worker/runtime, API, data, rollout, and thin macOS readback**.

Leading metric: first Xcode MCP session startup latency and dogfood latency evidence for parallel Xcode sessions.

Guardrail metric: no token leakage, no provider stdio fallback, no fake-home failures, and non-Xcode workflows remain healthy under broker disabled/degraded states.

Decision checkpoint: do not schedule broad runtime-control implementation outside the dependency-audit PR1 narrow exception until P025/P026 canonical evidence is recovered; do not broadly enable `shim_enforced` until dogfood/security/follow-up gates pass.

## Primary Implementation Flows

1. Workflow/catalog compilation detects Xcode MCP and direct Xcode command intent, then records broker/shim signals.
2. Engine resolves canonical Xcode MCP registry entries into `BrokeredXcodeMcpIntent`, ACP probes provider HTTP MCP support, reserves broker leases, and sends HTTP MCP `session/new`.
3. Daemon exposes `/xcode-mcp/{lease_id}`, authorizes bearer tokens, activates leases, enforces broker MCP policy, and forwards JSON-RPC to a per-lease backend `xcrun mcpbridge`.
4. PATH shim authorizes `xcodebuild`/`simctl` host execution, rejects direct `mcpbridge`, binds dispatch to the launched provider process/descendant, and emits runtime observations.
5. DB/domain/GraphQL/MCP/Swift surfaces expose typed runtime observations, friendly failure strings, broker health, catalog flags, and rollout evidence.

## Fidelity And Divergence Inventory

### Matches

- ACP transport has a canonical HTTP MCP `session/new` shape and rejects unresolved `XcodeBrokerIntent` before provider startup.
- Provider capability preflight/cache, `ProbeKey`, launch fingerprinting, and fail-closed unsupported-provider paths are implemented.
- Engine-side Xcode MCP registry resolution migrates the canonical `xcrun mcpbridge` case to broker intent and fail-closes stale/non-canonical bypasses.
- Lease pool, bearer authorization, capacity queueing, first-connect timeout, per-PID initialize lock, broker MCP policy, and observation emission are implemented in the ACP crate.
- Target resolver and shim dispatch ownership checks exist.
- Durable `actual_xcode_runtime_observation_json` storage, redaction, bounds, corrupt recovery, GraphQL/MCP readback, and Swift presentation surfaces exist.
- `p051-scaffold` and `proposal-051|p051` are registered in `scripts/test-gate.sh`.

### Divergences

- Live daemon startup creates the Xcode broker pool without attaching `XcodeMcpProcessBackend`; the HTTP route can authorize a lease but backend forwarding fails with `xcode_mcp_backend_unavailable`.
- Backend spawn uses `CHAINWORKS_XCODE_PID` instead of proposal-required `MCP_XCODE_PID`, and therefore also violates the proposal's host-env promise of no `CHAINWORKS_*` backend variables.
- Shim and catalog xcrun parsing reject proposal-allowed non-consuming flags such as `--verbose`, `--log`, `--no-cache`, `--kill-cache`, `--help`, `-h`, and `--version`.
- `XcodeBrokerHealthSnapshot` lacks proposal-required fields `reason_code`, `can_acquire_new_xcode_leases`, `last_transition_at`, and `operator_message`.
- `docs/reference/test-gates.md` does not register P051 gates even though `scripts/test-gate.sh` does.
- Dependency audit still blocks runtime-control implementation outside the narrow PR1 exception because P025/P026 canonical artifacts are missing.
- Dogfood, targeted security review, broad rollout evidence, and checked-in follow-up proposal artifacts for `P051-FU-01`/`P051-FU-02` were not found.

### Ambiguities / Evidence Gaps

- The full `proposal-051` gate could not complete because the local build ran out of disk space while compiling a dependency in `target/proposal-051-gate`; this is environment-blocked evidence, not a test assertion failure.
- No live daemon/Xcode runtime dogfood was executed in this audit.
- Late observation append readback refresh/event behavior is not proven. The DB append path updates the hot row, but the inspected sink does not publish a refresh/event after late appends.
- P029 call-surface security-owner verification is referenced by dependency audit but no dedicated checked-in review artifact was found.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 7 |
| Missing | 4 |
| Not Verifiable | 1 |
| Out of Scope | 0 |

Overall conformance: **Not Implemented** because in-scope committed requirements are missing.

## Detailed Requirement Audit

| ID | Requirement | Source | Status | Evidence | Mapping / Gap |
|---|---|---|---|---|---|
| REQ-001 | Research/source reconciliation and static stale guidance exist before implementation scheduling | P051 acceptance criteria, dependency audit | Implemented | code, tests-run | `p051-scaffold` performs source/stale checks and passed. |
| REQ-002 | Dependency readiness for P025/P026 or explicit narrow PR1 exception before runtime-control work | Dependency audit lines 12-24 | Missing | prior-review, proposal | P025/P026 canonical artifacts are missing; runtime-control implementation exceeds the narrow non-runtime PR1 exception. |
| REQ-003 | Canonical brokered Xcode MCP registry/fail-closed resolution | P051 registry/brokered resolution | Implemented | code, tests-found | `engine/src/mcp.rs` resolves canonical Xcode MCP to broker intent and rejects stale/direct bypass cases. |
| REQ-004 | ACP HTTP MCP transport and provider capability preflight/cache | P051 HTTP transport and capability preflight | Implemented | code, tests-run | `ResolvedMcpServerTransport::Http`, `ProviderCapabilityCache`, `ProbeKey`, and `CapabilitySliceFingerprint` exist; scaffold gate covered capability paths. |
| REQ-005 | Seven-phase executor flow with lease conversion before session-new and rollback on failure | P051 executor flow | Implemented | code, tests-run | ACP manager attaches broker leases before `session/new`, commits launch resources only after conversion, and releases on failures. |
| REQ-006 | Daemon HTTP broker route authorizes leases and forwards JSON-RPC to backend | P051 HTTP broker contract | Partially Implemented | code | HTTP route and bearer activation exist, but daemon does not attach a process backend, so live forwarding fails. |
| REQ-007 | One backend `xcrun mcpbridge` process per HTTP lease, initialize serialized per Xcode PID, tools parallel across leases | P051 lines 1199-1203 | Partially Implemented | code, tests-run | Backend model and tests exist, but the live daemon path does not use the process backend. |
| REQ-008 | Backend host env includes `MCP_XCODE_PID` and no `CHAINWORKS_*` variables | P051 lines 1203-1210 | Missing | code | Backend sets `CHAINWORKS_XCODE_PID`; no `MCP_XCODE_PID` implementation was found. |
| REQ-009 | Capacity/backpressure, broker disabled/degraded states, and subsystem health fields | P051 lines 1236-1252 | Partially Implemented | code | Capacity and disabled behavior exist, but health snapshot fields omit `reason_code`, `can_acquire_new_xcode_leases`, `last_transition_at`, and `operator_message`. |
| REQ-010 | Xcode target resolver replaces pgrep-style selection and fail-closes ambiguous/wrong-user targets | P051 target resolver criteria | Implemented | code, tests-found | `xcode_target.rs` resolves explicit PID/workspace candidates and rejects ambiguous, wrong UID, and missing host env cases. |
| REQ-011 | Broker MCP policy filters tools, denies unsafe tools, persists policy, and isolates per lease | P051 BrokerMcpPolicy criteria | Implemented | code, tests-run | Lease policy exists and pool tests cover `tools/list` filtering and denied tool behavior. |
| REQ-012 | Direct command scanner and catalog lint detect raw YAML, required tools, permission shell allow entries, direct mcpbridge, and Xcode host-execution signals | P051 scanner criteria | Partially Implemented | code, tests-run | Scanner coverage exists and scaffold tests passed, but xcrun parser rejects proposal-allowed non-consuming flags. |
| REQ-013 | PATH shim dispatch, process/descendant binding, host executor, cwd/env allowlist, and simulator UUID rewrite | P051 shim dispatch and host executor criteria | Partially Implemented | code, tests-found | Core shim/host executor behavior exists; xcrun flag contract is incomplete. |
| REQ-014 | Durable typed Xcode runtime observation schema with append bounds, retries, corrupt recovery, truncation, and late readback refresh | P051 durable observation criteria | Partially Implemented | migration, code, tests-run | Storage, bounds, retries, corrupt recovery, truncation, and readback exist; late refresh/event behavior is not proven. |
| REQ-015 | GraphQL, MCP reports, Swift UI, friendly failure strings, broker states, policy warnings, and catalog flags | P051 readback/UI criteria | Implemented | code, tests-found | GraphQL/MCP fields and Swift `RunTimelineInspectorView`, `WorkflowMapProjection`, `DaemonLifecycleSurface`, and `AgentCatalogView` surfaces exist. |
| REQ-016 | Session reuse, capability fingerprints, and shim-required fresh-session behavior | P051 session reuse criteria | Implemented | code, tests-found | Engine and ACP paths record fingerprints and suppress reuse when shim runtime is required. |
| REQ-017 | Canonical gates registered in script and reference docs | P051 test gate criteria | Partially Implemented | code, docs | `scripts/test-gate.sh` registers `p051-scaffold` and `proposal-051|p051`; `docs/reference/test-gates.md` has no P051 gate registration. |
| REQ-018 | Dogfood pass, targeted security review, rollout evidence, and pressure metrics before broad rollout | P051 dogfood/security/rollout criteria | Missing | code search, docs | No checked-in dogfood sign-off, security review, or rollout evidence was found. |
| REQ-019 | Draft follow-up proposal artifacts for rollout blockers `P051-FU-01` and `P051-FU-02` | P051 lines 1734-1751 | Missing | proposal, docs search | Proposal names triggers, but no checked-in draft proposal artifacts or acceptance criteria files were found. |
| REQ-020 | Full same-tree canonical proposal gate passes for successful readiness | Audit success rule and P051 gate | Not Verifiable | tests-run | `p051-scaffold` passed; full `proposal-051` was environment-blocked by no disk space while building `icu_properties`. |

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Not Implemented | Missing live backend wiring and exact backend env contract | High |
| `rust_arch_reviewer` | Fail | Daemon route cannot reach process backend in live composition | High |
| `rust_reliability_reviewer` | Fail | Runtime tests cover fixture backend, not live daemon backend, and late readback refresh is unproven | Medium |
| `api_contract_reviewer` | Fail | Backend env and xcrun parser diverge from explicit contract | High |
| `observability_rollout_reviewer` | Fail | Dependency, health, dogfood, security, and follow-up rollout evidence remain incomplete | High |
| `chainworks_execution_truth_reviewer` | Fail | Durable observation path exists, but live runtime ownership cannot complete brokered MCP request flow | High |
| Implementation readiness | Not Ready | Critical primary service flow blocked | High |

## Routed Specialist Findings

### ARCH-001: Live daemon broker route has no process backend

- Reviewer: `rust_arch_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-006, REQ-007
- Evidence types: code
- Evidence references: `control-plane/crates/daemon/src/main.rs:333-343`, `control-plane/crates/acp/src/xcode_broker.rs:648-655`
- Why it matters: P051's central service flow is provider HTTP MCP lease -> daemon route -> per-lease backend `xcrun mcpbridge`. The daemon constructs `XcodeMcpBridgePool::new_with_sink(...)` without `XcodeMcpProcessBackend`, while `forward_json_rpc_request` fail-closes when `self.backend` is absent. A real `/xcode-mcp/{lease_id}` request can therefore authorize and activate a lease but cannot reach the backend process.
- Recommended action: Wire `XcodeMcpProcessBackend::new(XcodeMcpProcessBackendConfig::default())` or equivalent configured backend into daemon pool construction, then add a daemon-level integration test that exercises the real route through backend forwarding.
- Acceptance criteria: A daemon-composition test proves `/xcode-mcp/{lease_id}` forwards at least `initialize` and one allowed `tools/*` request to a backend fixture without hitting `xcode_mcp_backend_unavailable`.

### API-001: Backend host env violates the explicit `MCP_XCODE_PID` contract

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-008
- Evidence types: proposal, code
- Evidence references: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1203-1210`, `control-plane/crates/acp/src/xcode_broker.rs:266-275`
- Why it matters: P051 explicitly requires `MCP_XCODE_PID=<target Xcode PID>` and "No `CHAINWORKS_*`" in backend host env. The implementation sets `CHAINWORKS_XCODE_PID` and no `MCP_XCODE_PID` occurrence was found. This can break bridge target selection and also leaks an internal Chainworks-specific variable into the backend environment contrary to the isolation contract.
- Recommended action: Replace the env key with `MCP_XCODE_PID`, add `USER`, `LOGNAME`, and explicit `PATH` handling per proposal, and add a regression test that rejects any backend env key beginning with `CHAINWORKS_`.
- Acceptance criteria: Process backend tests assert `MCP_XCODE_PID` is present, `CHAINWORKS_*` is absent, and the exact host-env allowlist is enforced.

### API-002: xcrun parsers reject proposal-allowed non-consuming flags

- Reviewer: `api_contract_reviewer`, `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-012, REQ-013
- Evidence types: proposal, code
- Evidence references: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1322-1328`, `control-plane/crates/acp/src/xcode_shim.rs:1169-1246`, `control-plane/crates/workflow/src/direct_command.rs:500-512`
- Why it matters: P051 explicitly allows `-l`, `--log`, `--verbose`, `--no-cache`, `--kill-cache`, `--help`, `-h`, and `--version` as non-consuming xcrun flags. Both shim routing and catalog lint currently treat unrecognized `-` tokens as unknown flags. Valid proposal-permitted commands can therefore fail closed unexpectedly.
- Recommended action: Add a shared xcrun option parser or update both parsers to honor the proposal's non-consuming flag allowlist.
- Acceptance criteria: Unit/integration tests cover each proposal-allowed non-consuming flag in both shim routing and catalog lint, while unknown flags still fail closed.

### OPS-001: Broker health snapshot omits required operational fields

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-009
- Evidence types: proposal, code
- Evidence references: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1236-1249`, `control-plane/crates/acp/src/xcode_broker.rs:140-149`, `control-plane/crates/domain/src/lifecycle.rs:178-188`, `Chainworks Forge/Support/DaemonLifecycleClient.swift:106-113`
- Why it matters: The proposal's health contract includes `reason_code`, `can_acquire_new_xcode_leases`, `last_transition_at`, and `operator_message`. Current ACP/domain/Swift snapshots expose counts and disabled state only. Operators cannot distinguish capacity, disabled, degraded, and failed acquisition causes through the promised health contract.
- Recommended action: Extend the ACP/domain/daemon/Swift health schema to the proposal fields and add tests for Disabled/Degraded/Failed transitions.
- Acceptance criteria: GraphQL/lifecycle/Swift readback includes all required fields, and tests prove Xcode broker degraded/disabled changes lease acquisition without failing global daemon readiness.

### REL-001: Late observation append readback refresh is not proven

- Reviewer: `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-014
- Evidence types: code, tests-run
- Evidence references: `control-plane/crates/engine/src/executor.rs:68-81`, `control-plane/crates/db/src/repos/agent_executions.rs:180-258`, `control-plane/crates/engine/src/executor.rs:3191-3192`
- Why it matters: The storage path appends observations with optimistic retries and bounds, but the inspected sink only writes the DB row. The projection rebuild observed in executor settlement is part of the normal stage flow, not an event emitted after late broker/shim observation appends. Late backend/shim facts can be durable but invisible to northbound readback until some unrelated refresh happens.
- Recommended action: Publish or enqueue a projection/readback refresh after observation append, or document and test the exact readback refresh mechanism if it already exists elsewhere.
- Acceptance criteria: A test appends an observation after the normal stage projection rebuild and proves GraphQL/MCP/Swift-facing readback sees the update without unrelated run activity.

### READY-001: P051 is not schedulable or closeable as implemented

- Reviewer: `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-002, REQ-017, REQ-018, REQ-019, REQ-020
- Evidence types: prior-review, docs, tests-run
- Evidence references: `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md:12-24`, `docs/reference/test-gates.md`, `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1734-1751`
- Why it matters: The dependency audit blocks runtime-control work outside the narrow PR1 exception; P051 reference gate registration is incomplete; dogfood/security evidence is absent; follow-up trigger artifacts are not checked in; and the full canonical `proposal-051` gate did not complete on this machine because the build ran out of disk space.
- Recommended action: Recover/link P025/P026 or narrow implementation scope, register P051 in reference gate docs, create checked-in follow-up proposal drafts for `P051-FU-01` and `P051-FU-02`, run targeted security/dogfood evidence, free disk or reuse a clean target cache, and rerun the full gate.
- Acceptance criteria: Dependency readiness is resolved, reference docs match gate scripts, follow-up artifacts exist, dogfood/security receipts are checked in, and `./scripts/test-gate.sh proposal-051` passes on the audited tree.

## Readiness Checklist

| Item | Status | Notes |
|---|---|---|
| Build or canonical gate status | Not complete | `p051-scaffold` passed; full `proposal-051` was environment-blocked by no disk space. |
| Core service flow integration validation | Failing by inspection | Live daemon route cannot forward without attached backend. |
| Runtime dogfood | Missing | No live Xcode/daemon dogfood run or sign-off evidence found. |
| API/schema contract | Not ready | Backend env and health snapshot diverge from proposal contract. |
| Data/persistence | Partial | Observation storage implemented; late readback refresh not proven. |
| UI/UX states | Partial evidence | Swift readback views/tests exist; no runtime UI screenshot or UI smoke was run. |
| Accessibility/localization/privacy/permissions | Not fully assessed | UI is thin; privacy/token leakage guarded in code but no security sign-off found. |
| Critical tests executed | Partial | `p051-scaffold` passed; full gate blocked by environment. |
| Full regression or canonical full proposal gate | Not passed | Required for any successful readiness verdict. |

## Verification Log

Commands run from `/Users/user/Documents/Chainworks Forge`:

- `git status --short`: dirty tree with pre-existing Swift, Rust, proposal, and review-artifact modifications; this audit adds only R3.
- `md5 -q docs/proposals/051-shared-xcode-mcp-bridge-pool.md`: `6f5383f057d35ec90f99083c884a53f7`.
- `rg`/`nl` inspections over `control-plane/crates/acp`, `control-plane/crates/daemon`, `control-plane/crates/engine`, `control-plane/crates/db`, `control-plane/crates/domain`, Swift app files, `scripts/test-gate.sh`, and `docs/reference/test-gates.md`.
- `./scripts/test-gate.sh p051-scaffold`: **passed**. Covered workflow catalog lint, DB observation append bounds/corrupt recovery, ACP capability and bridge pool tests, engine observation sink persistence, and Rust checks for GraphQL/MCP servers.
- `./scripts/test-gate.sh proposal-051`: **inconclusive/environment-blocked**. It passed the nested scaffold section, passed domain artifact contract tests and workflow P051 tests in `target/proposal-051-gate`, then failed building `icu_properties` with `No space left on device (os error 28)`.

## Final Verdict

Overall conformance: **Not Implemented**.

Overall implementation readiness: **Not Ready**.

Audit confidence: **High** for the live daemon backend, backend env, xcrun parser, health schema, dependency/readiness, and reference gate gaps; **Medium** for late readback refresh because the audit used code search and did not execute live late-append runtime validation.

Recommended next actions:

1. Fix live daemon backend attachment and add a daemon-level route-to-backend integration test.
2. Correct backend env to `MCP_XCODE_PID` with no `CHAINWORKS_*` leakage and test the exact allowlist.
3. Align both xcrun parsers with the proposal's allowed non-consuming flags.
4. Complete broker health snapshot fields across ACP/domain/daemon/Swift.
5. Resolve P025/P026 dependency readiness or explicitly narrow implementation scope; add reference gate docs and checked-in `P051-FU-01`/`P051-FU-02` draft artifacts.
6. Free disk or clean the proposal target cache, rerun `./scripts/test-gate.sh proposal-051`, then rerun this audit or a closeout audit.
