# P051 Implementation Audit R4

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` |
| Proposal revision | `p051-r30` |
| Proposal hash | `6f5383f057d35ec90f99083c884a53f7` |
| Proposal state | Active; still says not ready for implementation scheduling while P025/P026 readiness is unresolved |
| Audit mode | `auto` via `proposal-implementation-audit` |
| Audit timestamp | `2026-04-25T08:43:06Z` |
| Implementation target | Current worktree |
| Git HEAD | `9c59df8045512fae6e5c26f0ca45cc4ef616f8ee` |
| Branch | `main` |
| Compare base | Implicit current tree, no PR/range supplied |
| Working tree | Dirty before this report; audit wrote only this R4 report |
| Report path | `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R4.md` |

## Prior Review Reuse

Reviewer-selection reuse: **Partially reused**.

Durable local artifacts discovered:

- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md`
- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`

No durable final proposal-review artifact with selected reviewers was found. Reviewer selection was reconstructed from the proposal surface, repo-local routing expectations, review findings supplied by the user, and current implementation evidence.

Selected reviewers:

- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`
- `chainworks_execution_truth_reviewer`

Rejected close alternatives:

- `macos_ui_reviewer`: Swift UI is a thin readback surface, not the main conformance risk.
- `apple_arch_reviewer`: current blocking issues are in Rust runtime/API/rollout contracts.
- `rust_security_reviewer`: P051 still requires targeted security review before merge/rollout; this audit records that as readiness debt.
- `product_reviewer`: metrics are preserved below, but the blockers are implementation and rollout-readiness defects.

## Review Finding Disposition

| Finding | R4 disposition |
|---|---|
| P1: P051 remains dependency-blocked | **Still open.** The dependency audit still marks P025/P026 missing canonical checked-in artifacts and blocks work outside the narrow PR1 exception. Current implementation includes runtime-control code, so this remains a readiness blocker. |
| P2: Follow-up blockers need draft proposals | **Addressed in current proposal text.** P051 now has `rollout_followups` with `P051-FU-01` and `P051-FU-02`, including trigger, owner, scope, and acceptance criteria at lines 1955-1969. No separate draft proposal files were found, but the review finding allowed making fallback work explicit P051 scope. |

## Proposal State And Contract Summary

P051 commits to a Chainworks-owned Xcode MCP bridge pool for ACP providers:

- Providers receive Xcode MCP through Chainworks-owned HTTP endpoints, not provider stdio `xcrun mcpbridge`.
- The broker spawns one backend `xcrun mcpbridge` subprocess per provider HTTP lease under host-user Xcode environment.
- Backend spawn and MCP `initialize` are serialized per Xcode PID; `tools/*` may run in parallel across independent leases/backends.
- Provider fake-home isolation is preserved.
- Unsupported provider HTTP MCP capability fails before lease/token/backend/shim/session-new.
- Direct Xcode shell commands are guarded by catalog lint and PATH shim dispatch.
- Typed Xcode runtime observations persist through DB/domain and are exposed through GraphQL, MCP reports, and minimum Swift UI.
- `p051-scaffold` and `proposal-051|p051` gates are registered.
- Runtime rollout remains blocked on dependency readiness, dogfood/sign-off evidence, and security/rollout gates.

## Platform And Product Scope

Apple scope: **macOS**.

Backend/service scope: **cross-stack Rust control-plane service, worker/runtime, API, data, rollout, and thin macOS readback**.

Leading metric: Xcode MCP startup latency and dogfood latency evidence for parallel Xcode sessions.

Guardrail metric: no token leakage, no provider stdio fallback, no fake-home failures, and non-Xcode workflows remain healthy under broker disabled/degraded states.

Decision checkpoint: do not schedule broad runtime-control work until P025/P026 readiness is resolved or scope is narrowed to the dependency audit PR1 exception; do not enable broad `shim_enforced` until dogfood/security/follow-up gates pass.

## Primary Implementation Flows

1. Workflow/catalog compilation detects Xcode MCP and direct Xcode command intent, then records broker/shim signals.
2. Engine resolves canonical Xcode MCP registry entries into `BrokeredXcodeMcpIntent`; ACP probes HTTP MCP support, reserves broker leases, and sends HTTP MCP `session/new`.
3. Daemon exposes `/xcode-mcp/{lease_id}`, authorizes bearer tokens, activates leases, enforces broker MCP policy, and forwards JSON-RPC to a per-lease backend `xcrun mcpbridge`.
4. PATH shim authorizes `xcodebuild`/`simctl` host execution, rejects direct `mcpbridge`, binds dispatch to the launched provider process/descendant, and emits runtime observations.
5. DB/domain/GraphQL/MCP/Swift surfaces expose typed runtime observations, friendly failure strings, broker health, catalog flags, and rollout evidence.

## Fidelity And Divergence Inventory

### Matches

- ACP HTTP MCP transport, provider capability preflight/cache, `ProbeKey`, and launch fingerprinting exist.
- Engine registry resolution migrates canonical Xcode MCP to broker intent and rejects stale/direct bypass cases.
- ACP manager attaches broker leases before `session/new` and releases them on failure/close paths.
- Daemon pool construction now uses `new_with_sink_and_process_backend`, and a daemon unit test proves `pool.has_backend()`.
- Lease pool, bearer authorization, capacity queueing, first-connect timeout, per-PID initialize lock, broker MCP policy, and observation emission exist.
- Target resolver and shim process/descendant authorization checks exist.
- Durable observation storage, bounds, corrupt recovery, redaction, GraphQL/MCP readback, and Swift readback surfaces exist.
- P051 gates are registered in both `scripts/test-gate.sh` and `docs/reference/test-gates.md`.
- P051 now names `P051-FU-01` and `P051-FU-02` with owner, scope, trigger, and acceptance criteria.

### Divergences

- Backend spawn still uses `CHAINWORKS_XCODE_PID`; proposal requires `MCP_XCODE_PID` and no `CHAINWORKS_*` backend env.
- Shim and catalog xcrun parsing still reject proposal-allowed non-consuming flags such as `--verbose`, `--log`, `--no-cache`, `--kill-cache`, `--help`, `-h`, and `--version`.
- `XcodeBrokerHealthSnapshot` still lacks proposal-required fields `reason_code`, `can_acquire_new_xcode_leases`, `last_transition_at`, and `operator_message`; it added `backend_available` and `observation_persistence_failures`, but those are not substitutes for the promised fields.
- P025/P026 dependency readiness remains unresolved.
- Dogfood, targeted security review, broad rollout evidence, and full canonical proposal gate evidence are still missing.

### Ambiguities / Evidence Gaps

- No live daemon/Xcode dogfood run was executed in this audit.
- Late observation append readback refresh/event behavior remains unproven by direct evidence.
- Full `proposal-051` was not rerun in R4 because conformance is already blocked by explicit missing requirements; R3 previously attempted it and hit local disk exhaustion while compiling `icu_properties`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 5 |
| Missing | 3 |
| Not Verifiable | 1 |
| Out of Scope | 0 |

Overall conformance: **Not Implemented** because in-scope committed requirements remain missing.

## Detailed Requirement Audit

| ID | Requirement | Source | Status | Evidence | Mapping / Gap |
|---|---|---|---|---|---|
| REQ-001 | Source/research artifact, dependency audit artifact, and static stale guidance exist | P051 preconditions and dependency audit | Implemented | proposal, prior-review, tests-run | Dependency audit exists and `p051-scaffold` static stale check passed. |
| REQ-002 | P025/P026 dependency readiness resolved or implementation narrowed to PR1 exception | P051 status line 5; dependency audit lines 14-24 | Missing | proposal, prior-review | P025/P026 remain missing canonical checked-in artifacts; implementation contains runtime-control changes outside the narrow PR1 exception. |
| REQ-003 | Rollout follow-up blockers have concrete IDs, owners, scope, and acceptance | P051 lines 1955-1969 | Implemented | proposal | `P051-FU-01` and `P051-FU-02` are explicitly defined inside P051. |
| REQ-004 | Canonical brokered Xcode MCP registry/fail-closed resolution | P051 brokered resolution | Implemented | code, tests-run | Engine/ACP paths resolve broker intent and fail-closed unsupported/stale cases. |
| REQ-005 | ACP HTTP MCP transport and provider capability preflight/cache | P051 HTTP transport and capability preflight | Implemented | code, tests-run | HTTP transport and capability cache are covered by `p051-scaffold`. |
| REQ-006 | Seven-phase executor flow converts broker intent before `session/new` and rolls back leases | P051 executor flow | Implemented | code, tests-run | `runtime_manager_attaches_brokered_xcode_http_lease_before_session_new` passed. |
| REQ-007 | Daemon broker route has process backend wiring | P051 daemon/broker backend model | Implemented | code, tests-run | `new_daemon_xcode_broker_pool` calls `new_with_sink_and_process_backend`; `daemon_xcode_broker_pool_has_process_backend` passed. |
| REQ-008 | One backend process per lease, per-PID initialize serialization, per-lease ordered pump | P051 lines 1199-1203 | Implemented | code, tests-run | ACP pool/process backend tests passed in `p051-scaffold`. |
| REQ-009 | Backend host env includes `MCP_XCODE_PID` and excludes `CHAINWORKS_*` | P051 lines 1203-1210 | Missing | proposal, code | Implementation sets `CHAINWORKS_XCODE_PID`; no `MCP_XCODE_PID` implementation was found. |
| REQ-010 | Capacity/backpressure and broker subsystem health contract | P051 lines 1236-1254 | Partially Implemented | code | Capacity/backpressure exist, but health snapshot lacks required fields. |
| REQ-011 | Xcode target resolver fail-closes ambiguous/wrong-user/missing-host-env targets | P051 target resolver criteria | Implemented | code, tests-found | `xcode_target.rs` owns immutable target snapshots and fail-closed cases. |
| REQ-012 | Broker MCP policy filters/denies/persists/isolates per lease | P051 BrokerMcpPolicy criteria | Implemented | code, tests-run | Pool tests cover tool filtering and denied tool behavior. |
| REQ-013 | Direct command scanner/catalog lint cover raw YAML, required tools, permission shell allow, and direct mcpbridge | P051 scanner criteria | Partially Implemented | code, tests-run | Scanner coverage exists, but xcrun allowed-flag contract is incomplete. |
| REQ-014 | PATH shim dispatch, host executor, cwd/env allowlist, simulator UUID rewrite | P051 shim/host executor criteria | Partially Implemented | code, tests-found | Core behavior exists; xcrun flag parser rejects proposal-allowed flags. |
| REQ-015 | Durable typed Xcode runtime observation schema with append bounds/retries/corrupt recovery/truncation/late refresh | P051 durable observation criteria | Partially Implemented | migration, code, tests-run | Storage/bounds/retry/corrupt recovery are implemented; late refresh/readback event remains unproven. |
| REQ-016 | GraphQL/MCP/Swift readback, friendly failure strings, progress states, broker health, policy warnings | P051 readback/UI criteria | Implemented | code, tests-found | Swift and server readback surfaces exist, including progress labels and broker health consumption. |
| REQ-017 | Session reuse, capability fingerprints, and shim-required fresh sessions | P051 session reuse criteria | Implemented | code, tests-found | Fingerprints and reuse suppression paths exist. |
| REQ-018 | P051 gates registered in script and reference docs | P051 gate criteria | Implemented | docs, tests-run | `scripts/test-gate.sh` and `docs/reference/test-gates.md` both document/register P051 gates. |
| REQ-019 | Dogfood pass, targeted security review, rollout evidence, and sign-off before broad rollout | P051 dogfood/security/rollout criteria | Missing | docs search, tests-run | Fixture gates pass, but live dogfood/sign-off and targeted security evidence were not found. |
| REQ-020 | Full same-tree canonical proposal gate passes for successful readiness | Audit success rule and P051 gate | Not Verifiable | tests-run | `p051-scaffold` passed; full `proposal-051` was not rerun in R4 due already-blocking conformance gaps. |

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Not Implemented | Missing backend env contract and unresolved dependency readiness | High |
| `rust_arch_reviewer` | Partial | R3 daemon backend blocker is fixed; exact runtime contract gaps remain | High |
| `rust_reliability_reviewer` | Fail | xcrun parsing and late readback refresh gaps remain | Medium |
| `api_contract_reviewer` | Fail | Backend env and health schema diverge from proposal | High |
| `observability_rollout_reviewer` | Fail | Dependency, security, dogfood, and full-gate readiness remain incomplete | High |
| `chainworks_execution_truth_reviewer` | Partial | Core observation truth exists, but late refresh and rollout proof are incomplete | Medium |
| Implementation readiness | Not Ready | Readiness blockers and missing full-gate/dogfood evidence | High |

## Routed Specialist Findings

### API-001: Backend host env still violates `MCP_XCODE_PID`

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-009
- Evidence types: proposal, code
- Evidence references: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1203-1210`, `control-plane/crates/acp/src/xcode_broker.rs:269-278`, `control-plane/crates/acp/tests/integration.rs:2319`
- Why it matters: The proposal requires `MCP_XCODE_PID=<target Xcode PID>` and explicitly excludes `CHAINWORKS_*` from backend env. Current code and tests use `CHAINWORKS_XCODE_PID`, so tests preserve the wrong contract.
- Recommended action: Replace the backend env key with `MCP_XCODE_PID`, add explicit host env allowlist coverage for `USER`, `LOGNAME`, and `PATH`, and assert no `CHAINWORKS_*` key reaches backend spawn.
- Acceptance criteria: Backend process tests fail if `MCP_XCODE_PID` is absent or any `CHAINWORKS_*` env key is present.

### API-002: xcrun parser still rejects proposal-allowed flags

- Reviewer: `api_contract_reviewer`, `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-013, REQ-014
- Evidence types: proposal, code
- Evidence references: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1322-1328`, `control-plane/crates/acp/src/xcode_shim.rs:1169-1246`, `control-plane/crates/workflow/src/direct_command.rs:477-512`
- Why it matters: P051 allows `-l`, `--log`, `--verbose`, `--no-cache`, `--kill-cache`, `--help`, `-h`, and `--version` as non-consuming xcrun flags. Both shim routing and catalog lint still treat any unrecognized dash token as unknown, so proposal-valid commands can fail closed.
- Recommended action: Share or align xcrun parsing between workflow lint and shim routing, adding the proposal's non-consuming allowlist.
- Acceptance criteria: Tests cover each allowed flag in both parsers while preserving fail-closed behavior for unknown flags.

### OPS-001: Broker health snapshot is still not the promised contract

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-010
- Evidence types: proposal, code
- Evidence references: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1236-1254`, `control-plane/crates/acp/src/xcode_broker.rs:142-150`, `control-plane/crates/domain/src/lifecycle.rs:180-189`, `Chainworks Forge/Support/DaemonLifecycleClient.swift:106-116`
- Why it matters: Current snapshots include counts, `backend_available`, and persistence failure count, but still omit `reason_code`, `can_acquire_new_xcode_leases`, `last_transition_at`, and `operator_message`. Operators cannot rely on the promised subsystem-health contract to distinguish degraded/disabled/fail-closed causes.
- Recommended action: Add the missing fields across ACP/domain/daemon/Swift readback and test Disabled/Degraded/Failed transitions.
- Acceptance criteria: Lifecycle/API/Swift readback exposes all proposal fields, and tests prove broker health gates only Xcode lease acquisition without failing global daemon readiness.

### REL-001: Late observation append readback refresh remains unproven

- Reviewer: `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-015
- Evidence types: code, tests-run
- Evidence references: `control-plane/crates/engine/src/executor.rs:68-81`, `control-plane/crates/db/src/repos/agent_executions.rs:180-258`
- Why it matters: Observation append storage is implemented and tested, but the inspected sink only writes the DB row. The audit did not find direct evidence that a late broker/shim append triggers northbound projection/readback refresh after the normal stage projection rebuild.
- Recommended action: Publish or enqueue readback/projection refresh after append, or document and test the existing mechanism if one exists.
- Acceptance criteria: A test appends an observation after normal projection rebuild and proves GraphQL/MCP/Swift-facing readback sees it without unrelated run activity.

### READY-001: P051 remains not ready for implementation closeout

- Reviewer: `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-002, REQ-019, REQ-020
- Evidence types: prior-review, proposal, tests-run
- Evidence references: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:5`, `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md:14-24`
- Why it matters: The proposal itself remains dependency-blocked by P025/P026, targeted security/dogfood evidence was not found, and no full same-tree `proposal-051` gate pass was recorded for R4. Even with scaffold passing and the R3 daemon-backend blocker fixed, this cannot be considered closeout-ready.
- Recommended action: Resolve/link P025/P026, finish backend env/xcrun/health/readback gaps, attach security and dogfood evidence, and run `./scripts/test-gate.sh proposal-051` on the final audited tree.
- Acceptance criteria: No missing/partial REQ items remain, targeted security/dogfood receipts are checked in, and the full canonical P051 gate passes.

## Readiness Checklist

| Item | Status | Notes |
|---|---|---|
| Build or canonical gate status | Partial | `p051-scaffold` passed; daemon backend unit test passed; full `proposal-051` not run in R4. |
| Core service flow integration validation | Partial | Daemon process backend wiring is now proven by unit test, but no live route-to-Xcode dogfood run was executed. |
| Runtime dogfood | Missing | No live Xcode/daemon dogfood sign-off evidence found. |
| API/schema contract | Not ready | Backend env and health snapshot still diverge from P051. |
| Data/persistence | Partial | Observation storage works; late readback refresh remains unproven. |
| UI/UX states | Partial evidence | Swift readback surfaces/tests exist; no runtime UI smoke or screenshot was run. |
| Security/privacy/permissions | Not ready | Targeted security review/sign-off is still missing. |
| Critical tests executed | Partial | Scaffold and daemon backend unit test passed. |
| Full regression or canonical full proposal gate | Not passed | Required for any successful readiness verdict. |

## Verification Log

Commands run from `/Users/user/Documents/Chainworks Forge`:

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...`: selected R4 path.
- `git status --short`: dirty tree before this report; notable P051 files include modified proposal and dependency audit, plus untracked R3.
- `git rev-parse HEAD`: `9c59df8045512fae6e5c26f0ca45cc4ef616f8ee`.
- `md5 -q docs/proposals/051-shared-xcode-mcp-bridge-pool.md`: `6f5383f057d35ec90f99083c884a53f7`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...`: found dependency audit and HTTP streaming feasibility artifact.
- Focused `rg`/`nl` inspections over P051 proposal, dependency audit, `scripts/test-gate.sh`, `docs/reference/test-gates.md`, ACP broker/shim code, workflow direct command scanner, daemon startup, domain lifecycle, and Swift lifecycle client.
- `./scripts/test-gate.sh p051-scaffold`: **passed**.
- `cd control-plane && CARGO_TARGET_DIR=target/proposal-051-scaffold-gate CARGO_BUILD_JOBS=1 cargo test -p daemon daemon_xcode_broker_pool_has_process_backend -- --exact --nocapture`: compiled but ran 0 tests because the exact filter did not match the module-qualified test name; not counted as pass evidence.
- `cd control-plane && CARGO_TARGET_DIR=target/proposal-051-scaffold-gate CARGO_BUILD_JOBS=1 cargo test -p daemon daemon_xcode_broker_pool_has_process_backend -- --nocapture`: **passed**, running `tests::daemon_xcode_broker_pool_has_process_backend`.

## Final Verdict

Overall conformance: **Not Implemented**.

Overall implementation readiness: **Not Ready**.

Audit confidence: **High** for dependency readiness, backend env, xcrun parser, health schema, gate registration, and daemon backend wiring status; **Medium** for late readback refresh because this was code-inspected but not runtime-proven.

Recommended next actions:

1. Resolve or link P025/P026 canonical artifacts, or reduce implementation scope to the dependency-audit PR1 exception.
2. Change backend env from `CHAINWORKS_XCODE_PID` to `MCP_XCODE_PID` and add no-`CHAINWORKS_*` backend env tests.
3. Align workflow and shim xcrun parsers with the proposal's non-consuming flag allowlist.
4. Complete `XcodeBrokerHealthSnapshot` fields across ACP/domain/daemon/Swift.
5. Add proof for late observation append readback refresh.
6. Attach targeted security and dogfood evidence, then run `./scripts/test-gate.sh proposal-051` on the final tree.
