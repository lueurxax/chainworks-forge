# Proposal 051 Implementation Audit R8

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` |
| Report | `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R8.md` |
| Audit timestamp | 2026-04-25 23:59:46 +0300 |
| Audit mode | `proposal-implementation-audit` auto mode |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current worktree |
| Compare base | Implicit current branch/worktree audit |
| HEAD | `490e79343953903a0680253d771cf4785306258e` (`490e7934 Use current macOS runner for CI`) |
| Worktree state | Dirty; P051 code/evidence changes and dogfood YAML are not fully committed |
| Proposal state | Active implementation contract; fixture/readback schedulable, broad rollout gated by dogfood/sign-off |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Partially reused with delta |
| Audit confidence | High for fixture/readback mechanics; Medium for release readiness |

## Implementation Target

This audit evaluates the current dirty worktree, not only committed `main`.
The runtime currently listening on `127.0.0.1:4000` reports:

- global health `state=ready`
- `build_sha=490e7934-p051-sleep1`
- daemon PID `46760`
- broker health `state=healthy`
- `backend_available=true`
- `can_acquire_new_xcode_leases=true`
- `active_lease_count=0`
- `observation_persistence_failures=0`

Relevant P051 dirty/untracked surfaces inspected:

- `control-plane/crates/acp/src/xcode_broker.rs`
- `control-plane/crates/acp/src/manager.rs`
- `control-plane/crates/acp/src/adapters/mod.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/acp/tests/integration.rs`
- `control-plane/crates/daemon/src/main.rs`
- `control-plane/crates/daemon/src/host_interruption_sources.rs`
- `control-plane/crates/workflow/tests/integration.rs`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-agents.yaml`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-workflow.yaml`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-mcp-config.yaml`

Unrelated dirty files exist in P031/P053/P058/P061/P063/P068/P069/P070 areas and were not treated as P051 evidence except where the P051 gate compiled or tested through them.

## Prior Review Reuse

Prior artifacts found:

- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`
- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/dependency-audit.md`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md`
- embedded proposal reviewer checks for architect, product owner, UX designer, and UI designer

Reuse classification: **Partially reused with delta**. The proposal's original reviewer concerns remain relevant, but the implementation now adds concrete Rust runtime, cancellation, live dogfood, and rollout risks. Prior implementation audit reports were not used for reviewer selection.

Selected reviewers/lenses:

- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`
- `macos_ui_reviewer`

Rejected close alternatives:

- `rust_security_reviewer`: targeted security review evidence exists and no new unreviewed bearer/shim surface was found in this audit. Security remains a rollout checklist item.
- `product_reviewer`: product concerns are represented here through dogfood acceptance, sign-off, and rollout readiness.
- `performance_reviewer`: latency evidence was inspected, but no benchmark-backed performance acceptance target beyond dogfood startup latency was introduced.

## Contract Summary

P051 commits to a Chainworks-owned Xcode MCP broker boundary that preserves provider fake-home isolation while exposing Xcode MCP and selected Xcode shell work through daemon-owned host-user services. The explicit contract includes:

- brokered HTTP MCP lease attachment before provider `session/new`
- provider capability fail-closed behavior before lease/token/backend allocation
- per-lease bearer authorization and redacted readback
- backend `mcpbridge` lifecycle under host-user Xcode environment
- initialize serialization per Xcode PID
- cross-lease tools parallelism across independent leases/backends
- per-lease MCP policy filtering/denial
- direct command scanner, PATH shims, and host executor routing
- durable `actual_xcode_runtime_observation_json`
- GraphQL/MCP/Swift readback of runtime observations and broker health
- staged gates `p051-scaffold`, `proposal-051`, and `p051`
- parallel Gemini Xcode dogfood with modal-count, fake-home, observation, token, pressure, and approver evidence

Platform/product scope:

- Apple: macOS operator app read-only surfaces only
- Backend/service: Rust control-plane daemon, ACP runtime, workflow compiler, DB/readback, MCP/GraphQL API, rollout/observability
- Cross-stack: ACP provider launch payload, broker HTTP facade, report/readback surfaces, operator dogfood evidence

## Primary Flows

1. Brokered Xcode MCP provider launch: catalog intent -> capability probe -> lease reservation -> HTTP MCP payload -> provider `session/new`.
2. Xcode MCP runtime request path: authorized HTTP lease -> backend `mcpbridge` -> JSON-RPC id mapping -> tools/list and tools/call readback.
3. Failure and cleanup path: provider failure/cancel/session close -> ACP runtime cleanup -> broker lease/backend release -> durable observations.
4. Operator readback path: stored observations and broker health -> GraphQL/MCP reports -> Swift timeline/daemon surfaces.
5. Rollout path: dependency audit -> fixture gate -> live Gemini dogfood -> token/pressure/modal evidence -> release-owner decision.

## Fidelity Inventory

### Matches

- Dependency blocker posture is reconciled for fixture/readback work. P025/P026 historical proposal files are absent, but current references and gates are now treated as implemented-system truth.
- `./scripts/test-gate.sh proposal-051` passed on the audited tree.
- The dogfood catalog/workflow compiles and asserts two parallel Gemini agents requesting only brokered `xcode` MCP.
- Runtime health shows the broker is healthy with zero active leases and zero observation persistence failures.
- R9 dogfood evidence records a successful two-lane Gemini Xcode run with completed `tools/list`, follow-up `tools/call`, declared outputs, lease release, and no broker persistence failures.
- R8 cancellation cleanup gap is addressed by `AcpRuntimeManager::close_session()` releasing orphaned Xcode lease cleanup even when the live ACP session is already missing.
- Daemon composition now starts host-interruption monitors and wires ACP runtime cleanup into host interruption recovery.

### Divergences

- The proposal explicitly says one backend `xcrun mcpbridge` subprocess per active provider HTTP lease and cross-lease tools parallelism across independent backend processes. The current implementation shares one initialized backend per `run_id + Xcode pid + developer_dir`.
- Because the shared backend is guarded by a single session mutex, sibling lease requests to the same backend are serialized at the stdio process boundary. This is not the same concurrency model the proposal describes.
- Live dogfood uses a temporary dev launchd job, not the production `com.chainworks.forge.daemon` SMAppService path.
- The dogfood artifact records `HOLD` and explicitly says it does not claim release-owner sign-off.
- The newest P051 changes and dogfood YAML are still dirty/untracked, so the current behavior is not yet durable checked-in repo truth.

### Ambiguities / Evidence Gaps

- Modal evidence is sufficient for the observed R9 start, but a later modal was seen during restart/debug activity and was not attributed to the same run.
- Token leakage review is scoped artifact/evidence search; it is not a full release log/UI sweep.
- No live production SMAppService dogfood evidence was attached.
- Follow-up IDs `P051-FU-01` and `P051-FU-02` are named in the proposal, but no draft proposal files are linked.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | HTTP streaming research and scoped architecture evidence | Implemented |
| REQ-002 | Canonical checked-in source proposal and stale guidance gate | Implemented |
| REQ-003 | Dependency audit before scheduling | Implemented |
| REQ-004 | Provider HTTP MCP capability probe and fail-closed allocation boundary | Implemented |
| REQ-005 | Brokered Xcode MCP intent and HTTP lease attachment | Implemented |
| REQ-006 | Backend process model, initialize serialization, and tools parallelism | Partially Implemented |
| REQ-007 | Per-lease broker MCP policy enforcement | Implemented |
| REQ-008 | Broker capacity, health, disabled state, and rollback switch | Implemented |
| REQ-009 | Deterministic Xcode target resolver and immutable target snapshot | Implemented |
| REQ-010 | Host-user Xcode environment with provider fake-home isolation | Implemented |
| REQ-011 | Direct command scanner, shim dispatch, and host executor boundary | Implemented |
| REQ-012 | Durable observation persistence and GraphQL/MCP readback | Implemented |
| REQ-013 | Minimum Swift readback surface and friendly failure mapping | Implemented |
| REQ-014 | Registered and passing P051 fixture gates | Implemented |
| REQ-015 | Parallel Gemini dogfood with successful brokered Xcode tools | Partially Implemented |
| REQ-016 | Modal dedup dogfood evidence | Implemented with caveat |
| REQ-017 | Token redaction/leakage evidence | Implemented with caveat |
| REQ-018 | Observation pressure evidence and follow-up trigger | Implemented |
| REQ-019 | Targeted security review before host executor/shim slice | Implemented |
| REQ-020 | Named operator/release-owner approval | Partially Implemented |

## Detailed Requirement Audit

### REQ-001: HTTP Streaming Research

- Proposal source: research artifact and scoped architecture status, proposal lines 45-46.
- Status: Implemented.
- Evidence types: proposal, prior-review.
- Evidence: `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`.
- Mapping: research verdict is present and proposal scope follows the HTTP broker approach.
- Gap/note: no gap for fixture/readback.

### REQ-002: Canonical Source And Stale Guidance Gate

- Proposal source: lines 1710-1711, 1851, 1869.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence: `p051-scaffold` static source check in `scripts/test-gate.sh`; `./scripts/test-gate.sh proposal-051` passed.
- Mapping: source proposal exists and stale contrary guidance check runs before P051 tests.
- Gap/note: no stale-source failure found in this audit.

### REQ-003: Dependency Audit

- Proposal source: lines 1655, 1714-1715, 1870.
- Status: Implemented.
- Evidence types: proposal, docs.
- Evidence: `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md`, `docs/evidence/051-shared-xcode-mcp-bridge-pool/dependency-audit.md`.
- Mapping: current reference/gate truth resolves P025/P026 as not current fixture blockers; broad rollout still depends on dogfood/sign-off.
- Gap/note: earlier missing historical P025/P026 proposal files are no longer current fixture blockers.

### REQ-004: Provider Capability Probe And Fail-Closed Boundary

- Proposal source: lines 66, 1674-1675, 1832.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence: adapter-specific probe timeout in `control-plane/crates/acp/src/adapters/mod.rs`; public probe helper in `control-plane/crates/acp/src/transport.rs`; P051 gate test `brokered_xcode_probe_accepts_http_but_requires_lease_conversion`.
- Mapping: Gemini capability probe timeout is extended, and fixture proves no lease conversion without HTTP capability.
- Gap/note: Auggie/Junie remain out of P051 launch scope until separate capability proof.

### REQ-005: Brokered Intent And HTTP Lease Attachment

- Proposal source: lines 62, 1173, 1831.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence: `XcodeMcpBridgePool::attach_brokered_xcode_leases()` at `control-plane/crates/acp/src/xcode_broker.rs:1511`; P051 integration tests and gate.
- Mapping: broker intent is converted to an HTTP MCP payload with bearer header before provider launch.
- Gap/note: bearer is hashed in broker state and redacted by readback surfaces.

### REQ-006: Backend Process Model, Initialize Serialization, And Tools Parallelism

- Proposal source: lines 63-64, 1200-1202, 1678-1679, 1833-1834.
- Status: Partially Implemented.
- Evidence types: proposal, code, tests-run, runtime.
- Evidence: current backend registry/session key at `control-plane/crates/acp/src/xcode_broker.rs:124-143`, `214-239`, `1893-1910`; shared-backend test at `control-plane/crates/acp/tests/integration.rs:2501-2665`; R9 dogfood shared backend evidence at `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:157-186`.
- Mapping: initialize serialization works and R9 dogfood proves successful tools/list and tools/call through the broker.
- Gap/note: implementation shares one initialized backend per `run_id + Xcode pid + developer_dir`, while the proposal still requires one backend per active provider HTTP lease and cross-lease tools parallelism across independent backend processes.

### REQ-007: Broker MCP Policy

- Proposal source: lines 1718-1719, 1853.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence: `BrokerMcpPolicy` and `authorize_json_rpc_request()` in `control-plane/crates/acp/src/xcode_broker.rs`; P051 gate test `xcode_mcp_bridge_pool_enforces_per_lease_tool_policy`.
- Mapping: tools/list filtering and tools/call denial happen before backend forwarding with denied observations.
- Gap/note: policy remains per lease even when the backend process is shared.

### REQ-008: Broker Capacity, Health, Disabled State, Rollback

- Proposal source: lines 1216-1230, 1521, 1593-1604, 1653, 1835-1836, 1862.
- Status: Implemented.
- Evidence types: code, tests-run, runtime.
- Evidence: health snapshot fields in `control-plane/crates/acp/src/xcode_broker.rs:146-172`; current `/health` and `/xcode-mcp/health` runtime checks; P051 gate capacity/disabled tests.
- Mapping: broker health is subsystem-scoped and does not collapse global daemon readiness.
- Gap/note: production SMAppService path is not yet validated.

### REQ-009: Xcode Target Resolver

- Proposal source: lines 1453-1478, 1746-1747, 1852, 1863.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence: target snapshot resolution in `control-plane/crates/acp/src/xcode_broker.rs:1467-1505`; P051 gate target resolver tests.
- Mapping: resolver receives engine/catalog selection inputs and records immutable target evidence.
- Gap/note: no newest-process `pgrep` path was found in brokered P051 path evidence.

### REQ-010: Host Env And Fake-Home Isolation

- Proposal source: lines 65, 1203-1210, 1837.
- Status: Implemented.
- Evidence types: code, tests-run, runtime.
- Evidence: backend spawn environment in `control-plane/crates/acp/src/xcode_broker.rs`; test `xcode_mcp_bridge_pool_process_backend_spawns_with_target_env_and_rewrites_ids`; R9 fake-home boundary row.
- Mapping: backend process clears inherited env and restores host Xcode allowlist; provider state remains fake-home isolated.
- Gap/note: live dogfood evidence is from a dev daemon.

### REQ-011: Direct Command Scanner, Shim Dispatch, Host Executor

- Proposal source: lines 67-68, 1324-1360, 1387-1410, 1838-1843, 1854-1855, 1861.
- Status: Implemented.
- Evidence types: code, tests-run, docs.
- Evidence: P051 workflow/catalog lint tests in the gate; `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md`.
- Mapping: direct `mcpbridge` bypass is rejected; Xcode host execution intent must be explicit; security review records shim/process-binding controls.
- Gap/note: residual prompt-time absolute paths remain an accepted rollout risk.

### REQ-012: Durable Observations And GraphQL/MCP Readback

- Proposal source: lines 1413-1445, 1694-1695, 1726-1727, 1844-1845, 1856, 1864.
- Status: Implemented.
- Evidence types: code, tests-run, runtime.
- Evidence: DB observation append tests, engine sink test, GraphQL/MCP checks in `proposal-051`, runtime health `observation_persistence_failures=0`.
- Mapping: P051 gate proves append serialization, corrupt JSON recovery, event/byte bounds, and readback compile contracts.
- Gap/note: normalized event table remains a triggered follow-up, not current scope.

### REQ-013: Minimum Swift Readback Surface

- Proposal source: lines 98-120, 1698-1699, 1846, 1857-1859, 1865-1868.
- Status: Implemented.
- Evidence types: tests-run.
- Evidence: Swift tests `RunTimelineInspectorViewTests` and `DaemonLifecycleClientTests` in `./scripts/test-gate.sh proposal-051`.
- Mapping: Swift test suite verifies structured Xcode runtime rows, coalesced policy warnings, friendly failures, catalog flags, and broker health decoding.
- Gap/note: audit did not capture fresh screenshots or UI runtime recordings.

### REQ-014: Registered And Passing P051 Gates

- Proposal source: lines 72, 1662-1667, 1849.
- Status: Implemented.
- Evidence types: tests-run.
- Evidence: `./scripts/test-gate.sh proposal-051` passed on 2026-04-25 23:57 +0300; result bundle `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-235641.xcresult`.
- Mapping: full P051 fixture/readback gate passed on the audited tree.
- Gap/note: full repository regression was not run.

### REQ-015: Parallel Gemini Dogfood With Successful Xcode Tools

- Proposal source: lines 1651, 1678-1687, 1850, 1945-1953.
- Status: Partially Implemented.
- Evidence types: runtime, docs.
- Evidence: R7 and R9 dogfood sections in `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:100-186`.
- Mapping: R9 proves two Gemini lanes completed, acquired brokered leases, used Xcode tools, emitted outputs, and released leases.
- Gap/note: R9 proves successful cross-lease tools use, but because implementation now shares one backend and serializes stdio access, it does not prove the proposal's independent-backend cross-lease tools parallelism.

### REQ-016: Modal Dedup Evidence

- Proposal source: lines 1682-1683, 1850.
- Status: Implemented with caveat.
- Evidence types: runtime, docs.
- Evidence: R9 modal row at `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:181` and acceptance row at line 244.
- Mapping: operator saw one real Xcode consent modal when two brokered Gemini sessions started.
- Gap/note: a later modal was observed during restart/debug activity and was not attributed to the same run.

### REQ-017: Token Redaction / Leakage Evidence

- Proposal source: lines 1702-1703.
- Status: Implemented with caveat.
- Evidence types: docs, tests-run.
- Evidence: security review, R9 token leakage row, P051 gate.
- Mapping: scoped artifact/evidence search found no raw bearer/shim token matches; tests cover redaction controls.
- Gap/note: audit did not perform a full release-grade sweep of all live daemon logs, UI screenshots, and external artifacts.

### REQ-018: Observation Pressure Evidence

- Proposal source: lines 1750-1751, 1871.
- Status: Implemented.
- Evidence types: runtime, docs.
- Evidence: R9 acceptance table records no broker persistence failures and health reports `observation_persistence_failures=0`.
- Mapping: current dogfood evidence does not trigger the normalized event-table follow-up.
- Gap/note: follow-up draft artifact is not checked in.

### REQ-019: Targeted Security Review

- Proposal source: lines 1587, 1657, 1860, 1940.
- Status: Implemented.
- Evidence types: docs, tests-run.
- Evidence: `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md`.
- Mapping: targeted security review records bearer lifecycle, env allowlist, process binding, token redaction, and broker-only `mcpbridge` controls.
- Gap/note: broad rollout still needs live token-leakage evidence attached to final sign-off.

### REQ-020: Named Operator / Release-Owner Approval

- Proposal source: lines 1706-1707, 1951-1953.
- Status: Partially Implemented.
- Evidence types: docs.
- Evidence: `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:5-8`, `250`, `252-258`.
- Mapping: an explicit `HOLD` is recorded.
- Gap/note: no named release-owner `GO` is attached, and the artifact explicitly says it does not claim release-owner sign-off.

## Reviewer Scorecard

| Lens | Score | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | backend process model diverges from proposal | High |
| Rust architecture | Not Ready | shared backend contradicts per-lease backend contract | High |
| Rust reliability | Not Ready | sibling failure/serialization semantics changed without proposal-aligned acceptance | Medium |
| API contract | Ready with risks | readback contracts pass, but evidence docs still need final sign-off update | High |
| Observability/rollout | Not Ready | HOLD, dev daemon only, dirty tree | High |
| macOS UI | Ready with risks | Swift readback tests pass, but no fresh runtime screenshot/UI proof captured by audit | Medium |

## Routed Findings

### ARCH-001: Shared backend contradicts the P051 backend contract

- Reviewer: `rust_arch_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-006, REQ-015
- Evidence types: proposal, code, tests-run, runtime
- Evidence references:
  - Proposal requires one backend subprocess per provider HTTP lease at lines 63, 1200-1202, 1678-1679, 1833-1834.
  - Implementation shares backend sessions by `run_id + Xcode pid + developer_dir` at `control-plane/crates/acp/src/xcode_broker.rs:214-239` and `1901-1910`.
  - The shared-backend test asserts only one spawned backend for two leases at `control-plane/crates/acp/tests/integration.rs:2501-2665`.
  - R9 dogfood records one `mcpbridge` process for two Gemini ACP processes at `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:172-186`.
- Why it matters: the implementation may be a better modal-dedup design, but it is not the architecture P051 currently approves. It also changes cross-lease tools parallelism from independent backend processes to serialized access through one stdio process.
- Recommended action: choose one source of truth before closeout. Either update P051 and the reference docs to make shared initialized backend per run/Xcode target the approved architecture, with explicit isolation/failure/parallelism semantics, or revert implementation to the per-lease backend model.
- Acceptance criteria: proposal, reference, tests, and dogfood evidence all describe the same backend process model; REQ-006 can be marked Implemented without caveat.

### REL-001: Shared backend failure blast radius is not covered by proposal-aligned sibling tests

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-006, REQ-015
- Evidence types: proposal, code, tests-found
- Evidence references:
  - Proposal says a backend bridge crash should fail only that lease while siblings continue at lines 1536-1537.
  - `remove_session_after_failure()` removes every lease mapping for the shared backend session at `control-plane/crates/acp/src/xcode_broker.rs:242-257`.
  - Existing crash retry test covers one lease only at `control-plane/crates/acp/tests/integration.rs:2807-2913`.
- Why it matters: with shared backend sessions, one backend failure no longer maps cleanly to one lease. Sibling leases may recover on retry, but the proposal promises per-lease failure isolation and the current tests do not prove sibling behavior under a shared backend crash.
- Recommended action: if shared backend remains, add explicit tests for two leases sharing one backend where one backend request fails/crashes, then prove sibling lease behavior and observation semantics. Update proposal acceptance to the intended behavior.
- Acceptance criteria: a fixture proves sibling leases either continue without visible failure or fail/retry with explicitly accepted shared-backend semantics and durable observations.

### READY-001: Release-owner sign-off and production launch path remain HOLD

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-020
- Evidence types: docs, runtime
- Evidence references:
  - Dogfood sign-off status is `HOLD` at `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:5-8`.
  - Runtime substrate uses temporary label `com.chainworks.forge.daemon.manual.p051` and explicitly not the production SMAppService path at lines 36-46.
  - Stop sign requires production SMAppService validation or explicit scoping out, plus human `GO`, at lines 252-258.
- Why it matters: fixture and live dogfood mechanics can be green while release readiness remains false. P051 cannot be closed out as release-ready under its current sign-off artifact.
- Recommended action: either validate the production `com.chainworks.forge.daemon` SMAppService path and attach named release-owner `GO`, or explicitly scope production SMAppService out of P051 closeout with owner/acceptance criteria.
- Acceptance criteria: dogfood sign-off records named signer, timestamp, run id, provider versions, Xcode PID, session count, modal count, observation completeness, and either production SMAppService evidence or an accepted scoped exclusion.

### READY-002: Current P051 behavior is not yet durable checked-in repo truth

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-002, REQ-014, REQ-015, REQ-020
- Evidence types: git status, docs, code
- Evidence references:
  - Dirty P051 code in `control-plane/crates/acp/*`, `control-plane/crates/daemon/src/main.rs`, and `control-plane/crates/workflow/tests/integration.rs`.
  - Dirty evidence file `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md`.
  - Untracked dogfood files `dogfood-agents.yaml`, `dogfood-workflow.yaml`, and `dogfood-mcp-config.yaml`.
- Why it matters: the audit can evaluate the current worktree, but closeout needs the implementation and evidence to survive outside the local session. The current `main` commit alone does not contain the complete R8/R9 shared-backend, dogfood, and hardening truth.
- Recommended action: commit or otherwise preserve the P051 implementation/evidence slice before closeout, excluding unrelated dirty changes.
- Acceptance criteria: a committed branch/commit contains the P051 code, tests, reference/evidence artifacts, and dogfood YAML that this audit evaluated; `./scripts/test-gate.sh proposal-051` passes from that committed state.

### OPS-001: Follow-up IDs exist but are not draft proposal links

- Reviewer: `observability_rollout_reviewer`
- Severity: Minor
- Confidence: Medium
- Related requirements: REQ-018
- Evidence types: proposal, docs
- Evidence references:
  - `P051-FU-01` and `P051-FU-02` are named at proposal lines 1734-1735 and 1750-1751.
  - No corresponding draft proposal files were found in `docs/proposals/`.
- Why it matters: current dogfood does not trigger those follow-ups, so this is not a fixture blocker. If the thresholds trigger later, unmaterialized follow-ups can still turn into an unnamed future.
- Recommended action: create/link draft proposal files for `P051-FU-01` and `P051-FU-02`, or explicitly state that triggered mitigation remains in P051 scope with owner and acceptance criteria.
- Acceptance criteria: each trigger points to either a checked-in draft proposal or an explicit in-scope P051 remediation section.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Canonical P051 gate | Passed | `./scripts/test-gate.sh proposal-051` passed on audited tree |
| Swift result bundle | Present | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-235641.xcresult` |
| Runtime health | Passed dev-daemon check | `127.0.0.1:4000/health` ready; broker healthy |
| Live dogfood | Passed with caveats | R9 completed both Gemini lanes and released leases |
| Backend process model | Not ready | implementation/proposal mismatch |
| Cancellation cleanup regression | Passed | targeted ACP unit test passed |
| Sleep/wake hardening tests | Passed | targeted engine/daemon tests passed |
| UI readback tests | Passed | `RunTimelineInspectorViewTests`, `DaemonLifecycleClientTests` |
| Security review | Complete for fixture/readback | targeted security review artifact |
| Production SMAppService | Not verified | dogfood uses dev launchd job |
| Release-owner GO | Missing | sign-off artifact records HOLD |
| Durable repo truth | Not ready | P051 changes are dirty/untracked |
| Full repo regression | Not run | only canonical P051 gate and targeted tests were run |

## Verification Log

Commands run during this audit:

- `./scripts/test-gate.sh proposal-051`
  - Result: passed.
  - Swift result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-235641.xcresult`.
- `cargo test -p acp close_session_releases_orphaned_xcode_lease_cleanup_when_live_session_is_missing -- --nocapture`
  - Result: passed.
- `cargo test -p engine host_interruption_records_epoch_cancels_execution_and_requeues_invoke_work -- --nocapture`
  - Result: passed.
- `cargo test -p engine host_interruption_requires_runtime_cleanup_before_retry_enqueue -- --nocapture`
  - Result: passed.
- `cargo test -p daemon native_event_bridge_maps_system_sleep_wake_event -- --nocapture`
  - Result: passed.
- `cargo test -p daemon native_event_bridge_maps_network_migration_event -- --nocapture`
  - Result: passed.
- `curl -fsS http://127.0.0.1:4000/health`
  - Result: passed; ready, build `490e7934-p051-sleep1`.
- `curl -fsS http://127.0.0.1:4000/xcode-mcp/health`
  - Result: passed; broker healthy, active leases `0`, persistence failures `0`.

Command mistakes corrected:

- Initial combined cargo filters for two test names failed because `cargo test` accepts one test filter argument. Tests were rerun individually and passed.

Not run:

- Full repository regression suite.
- Production `com.chainworks.forge.daemon` SMAppService dogfood.
- Fresh live Gemini dogfood rerun after this audit.
- Fresh UI screenshot/screen-recording proof.
- Release-grade token sweep across all live logs, reports, UI, and external artifacts.

## Final Verdict

P051 fixture/readback mechanics are substantially implemented and the canonical `proposal-051` gate passes on the audited tree. R9 live dogfood is meaningful positive evidence for brokered Gemini Xcode execution.

P051 is **not ready for closeout**. The top blocker is not the old P025/P026 dependency issue; that is reconciled for fixture work. The top blocker is that the implementation now uses a shared initialized backend process while the proposal still requires one backend process per lease and cross-lease tools parallelism across independent backends. In addition, release-owner sign-off is still `HOLD`, production SMAppService dogfood is not validated or scoped out, and the current P051 slice is not fully committed.

Recommended next actions:

1. Decide and align the backend process model: update P051/reference/tests to shared backend semantics or revert implementation to per-lease backends.
2. Add sibling-failure/shared-backend reliability fixture if shared backend remains.
3. Commit/preserve the P051 implementation and dogfood YAML as durable repo truth.
4. Validate or explicitly scope out production SMAppService.
5. Attach named release-owner `GO` only after the above are resolved.
