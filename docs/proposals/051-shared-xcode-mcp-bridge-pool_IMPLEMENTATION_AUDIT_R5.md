# Proposal 051 Implementation Audit R5

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` |
| Proposal revision | `p051-r30` |
| Proposal checksum | `md5:6f5383f057d35ec90f99083c884a53f7` |
| Audit report | `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R5.md` |
| Audit timestamp | `2026-04-25T09:59:54Z` |
| Repository HEAD | `9c59df8045512fae6e5c26f0ca45cc4ef616f8ee` |
| Implementation target | Current dirty working tree |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready for full P051 / broad `shim_enforced`; fixture/readback lane is Ready with Risks |
| Reviewer selection reuse | Reused with delta |
| Audit confidence | High for fixture/readback implementation and gates; medium for live dogfood and runtime modal behavior |

## Scope Audited

This audit compares the current working-tree implementation against P051's checked-in implementation contract, not only against the stable reference. It includes Rust control-plane code, Swift readback surfaces, reference docs, evidence docs, and the registered `proposal-051` validation gate.

The worktree is dirty. The audit therefore treats current files as implementation evidence but does not claim they are committed repository truth. Relevant dirty files include:

- `docs/proposals/051-shared-xcode-mcp-bridge-pool.md`
- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md`

## Prior Review Reuse

Prior adjacent artifacts found and reused:

- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md`
- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/dependency-audit.md`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md`
- `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md`

Detected reviewer IDs were not explicitly recorded in prior artifacts, so this audit reuses the evidence and routes fresh implementation lenses by code surface.

Selected reviewers:

- `rust_arch_reviewer`: ACP manager, broker pool, target resolver, shim, daemon routes, ownership boundaries.
- `rust_reliability_reviewer`: lease lifecycle, shutdown cleanup, capacity/backpressure, retry/close semantics.
- `rust_security_reviewer`: bearer token handling, provider fake-home boundary, shim process binding, host command boundary.
- `api_contract_reviewer`: HTTP MCP session shape, GraphQL/MCP readback, durable observation envelope.
- `observability_rollout_reviewer`: staged gates, health snapshots, evidence docs, dogfood/sign-off, follow-up triggers.

Rejected close alternatives:

- `macos_ui_reviewer`: Swift scope is a narrow read-only readback surface, not a broad macOS UI proposal.
- `product_reviewer`: product acceptance is represented by rollout metrics and dogfood stop signs, not new product behavior.
- Go, iOS, and protobuf/OpenAPI reviewers: no active implementation surface.

## Proposal Contract Summary

P051 requires a Chainworks-owned Xcode MCP bridge pool that:

- converts brokered Xcode intents to HTTP MCP leases before provider `session/new`,
- keeps provider fake-home isolation while routing host Xcode work through daemon-owned broker/shim boundaries,
- serializes backend `xcrun mcpbridge` initialization per Xcode PID,
- fails closed when HTTP MCP or broker capability is unavailable,
- prevents direct `mcpbridge` execution outside the broker path,
- records durable, redacted Xcode runtime observations through a single DB append path,
- exposes GraphQL, MCP, daemon health, and Swift readback,
- defines staged fixture gates plus separate live dogfood/sign-off before broad rollout.

Primary audited flows:

1. Brokered Xcode MCP lease attachment from provider intent to HTTP MCP `session/new`.
2. Pool capacity, backend process startup, target resolution, authorization, and sibling lease isolation.
3. Direct Xcode command containment through scanner signals, shim grant authorization, host executor policy, and observation events.
4. Durable observation write/readback through domain, DB, engine sink, GraphQL, MCP, and Swift.
5. Rollout evidence: dependency audit, security review, dogfood stop sign, test gates, and reference docs.

## Implementation Matches

- Stable implemented reference exists at `docs/reference/xcode-mcp-bridge-pool.md`; it correctly limits its claim and says it does not provide live dogfood sign-off.
- Gate documentation exists in `docs/reference/test-gates.md:920-994` for `p051-scaffold` and `proposal-051|p051`, explicitly saying those gates are fixture/readback gates and not release-owner sign-off.
- The daemon constructs the Xcode broker pool after listener binding, injects it as the ACP lease attacher, publishes broker health, mounts `/xcode-mcp` routes, and configures the shim socket (`control-plane/crates/daemon/src/main.rs:333-451`).
- The ACP manager performs the ordered launch/session sequence: launch spec, capability check, lease attachment, shim runtime injection, unconverted-intent rejection, `session/new` prep, open session, and error cleanup (`control-plane/crates/acp/src/manager.rs:305-367`).
- Broker pool health degrades on queue/capacity pressure and observation persistence failures (`control-plane/crates/acp/src/xcode_broker.rs:554-608`).
- Broker bearer authorization and active-lease marking exist (`control-plane/crates/acp/src/xcode_broker.rs:611-659`).
- Broker observation persistence failure increments an internal failure counter and changes health state to degraded (`control-plane/crates/acp/src/xcode_broker.rs:1101-1125`).
- Xcode target selection is deterministic and fail-closed; it uses explicit PID/workspace inputs and host probing rather than selecting newest global `pgrep` output (`control-plane/crates/acp/src/xcode_target.rs`).
- Shim grants bind to provider process identity and active prompt state, reject direct `mcpbridge`, and route allowed Xcode commands through host executor policy (`control-plane/crates/acp/src/xcode_shim.rs`).
- Domain/DB observation code owns redaction, bounds, corrupt JSON recovery, truncation/drop counters, and optimistic append retry (`control-plane/crates/domain/src/xcode_runtime.rs`; `control-plane/crates/db/src/repos/agent_executions.rs`).
- GraphQL and MCP readback surfaces are present and included in the gate.
- Swift readback surfaces exist for run-timeline Xcode Runtime rows, policy warnings, friendly failures, Agent Catalog infrastructure flags, and daemon broker health (`Chainworks Forge/Views/RunTimelineInspectorView.swift`; `Chainworks Forge/Views/AgentCatalogView.swift`; `Chainworks Forge/Views/DaemonLifecycleSurface.swift`).
- P051 follow-up triggers `P051-FU-01` and `P051-FU-02` now have explicit trigger, owner, scope, and acceptance in P051 itself (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1955-1970`).

## Divergences And Evidence Gaps

- The proposal top-level status still says scheduling is blocked by unresolved P025/P026 readiness (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:5`), while the dependency audit says P025/P026 are not current fixture blockers and the release stop sign is live dogfood/sign-off (`docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md:3-10`, `:24-36`). This is a source-of-truth contradiction.
- Live dogfood and explicit operator/release-owner sign-off are missing. The evidence file intentionally records `Not run`, `Not recorded`, and `Not signed` for required pre-ship fields (`docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:33-54`).
- P051 requires late async observation appends to publish an execution update so GraphQL/MCP/UI can refresh without log scraping (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1443`). The current DB-backed sink only appends to the DB (`control-plane/crates/engine/src/executor.rs:68-81`), and `DomainEvent` has no Xcode observation or agent-execution update event (`control-plane/crates/domain/src/events.rs:9-67`).
- P051 requires deterministic cleanup of broker resources on close/crash/cancel. Per-session close releases leases, but `close_all_sessions()` drains only `live_sessions` and does not drain `live_xcode_leases`; daemon shutdown calls `close_all_sessions()` (`control-plane/crates/acp/src/manager.rs:456-497`, `control-plane/crates/daemon/src/main.rs:421-431`).
- Observation persistence failure handling is partial: it increments an internal pool counter and warns, but the named metric `xcode_observation_persist_failed_total`, tracing error severity, and best-effort execution warning path from the proposal are not evident (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1427-1429`; `control-plane/crates/acp/src/xcode_broker.rs:1101-1125`).
- The acceptance criterion says high-level `RunProgressView` shows bridge lock/start/action-required states, not only the timeline inspector (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1865`). The implementation evidence found this status in `RunTimelineInspectorView` and projection tests, but not in a distinct high-level `RunProgressView` surface.

## Requirement Audit

| ID | Requirement | Status | Evidence / Notes |
|---|---|---|---|
| REQ-001 | Canonical P051 source and stale-guidance scaffold check | Implemented | `p051-scaffold` is documented and included in `proposal-051`; gate passed. |
| REQ-002 | Dependency audit before scheduling with P025/P026/P029/P037/P049 posture | Partial | Audit artifacts now resolve fixture blockers, but proposal line 5 still contradicts them. |
| REQ-003 | P051-FU-01/P051-FU-02 follow-up blockers have concrete owner/scope/acceptance | Implemented | `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1955-1970`. |
| REQ-004 | Registered staged gates and stable reference docs | Implemented | `docs/reference/xcode-mcp-bridge-pool.md`; `docs/reference/test-gates.md:920-994`; `proposal-051` passed. |
| REQ-005 | Brokered Xcode MCP intent becomes HTTP MCP lease before `session/new`; no stdio fallback | Implemented | ACP manager and transport paths covered by gate. |
| REQ-006 | Provider HTTP MCP capability preflight, fingerprint/cache, fail-closed unsupported providers | Implemented | ACP capability checks and integration tests covered by `proposal-051`. |
| REQ-007 | Pool lease, backend, capacity, per-PID initialize serialization, route authorization | Implemented | Broker pool code and ACP integration tests covered by `proposal-051`. |
| REQ-008 | Deterministic `XcodeTargetResolver`, no newest global Xcode selection | Implemented | `control-plane/crates/acp/src/xcode_target.rs`; target tests covered by gate. |
| REQ-009 | Broker MCP policy filters tools and denies unauthorized calls with observations | Implemented | `BrokerMcpPolicy` and integration tests covered by gate. |
| REQ-010 | Direct command scanner covers raw/typed catalog and workflow declarations | Implemented | `control-plane/crates/workflow/src/direct_command.rs`; workflow P051 tests passed. |
| REQ-011 | Shim dispatch/host executor auth, process binding, cwd/env policy, simulator rewrite, `mcpbridge` rejection | Implemented | `control-plane/crates/acp/src/xcode_shim.rs`; gate coverage present. |
| REQ-012 | Durable observation schema, redaction, bounded DB append, corrupt recovery, GraphQL/MCP readback | Implemented | Domain/DB/API tests and compile checks passed. |
| REQ-013 | Observation persistence failure policy: tracing error, named metric, degraded health, execution warning | Partial | Health degradation and counter exist; named metric/error/warning path not evident. |
| REQ-014 | Late async append notification for GraphQL/MCP/UI refresh | Missing | DB sink appends only; no matching domain event found. |
| REQ-015 | Session close/crash/cancel cleanup releases broker leases | Partial | `close_session()` releases leases; `close_all_sessions()` shutdown path does not. |
| REQ-016 | Broker health subsystem and kill switch preserve non-Xcode daemon readiness | Implemented | Daemon health and lifecycle readback are present and tested. |
| REQ-017 | Minimum Swift read-only UI/readback surface | Partial | Timeline inspector, policy warnings, friendly failures, catalog, and daemon health exist; high-level `RunProgressView` propagation not proven. |
| REQ-018 | Targeted security review before host executor/shim merge | Implemented for fixture/readback | `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md`; broad rollout still has dogfood security holds. |
| REQ-019 | Live dogfood, modal count, fake-home boundary, token leakage review, operator GO/HOLD | Missing | Stop-sign artifact records required fields as not run/not recorded/not signed. |
| REQ-020 | Full registered `proposal-051` gate | Implemented | `./scripts/test-gate.sh proposal-051` passed on 2026-04-25. |

## Routed Specialist Findings

### READY-001: Full rollout is blocked by missing live dogfood/sign-off

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Evidence: `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:33-54`; `docs/reference/test-gates.md:976-994`

The fixture/readback gate is green, but P051 pre-ship acceptance still requires a real parallel Xcode-capable dogfood run, modal-count evidence, fake-home boundary evidence, observation completeness, token leakage review, pressure metrics, and explicit operator/release-owner GO/HOLD. The evidence file correctly prevents fabricating this proof. Do not close P051 as fully implemented or enable broad `shim_enforced` until that table is filled with real evidence.

Acceptance:

- Record a real dogfood run id, workflow/stage, provider/runtime, Xcode target, modal count, fake-home boundary result, observation completeness, token leakage review, observation pressure metrics, and signer/timestamp.
- Keep `proposal-051` as a fixture/readback gate, not a fake dogfood substitute.

### OPS-001: Proposal status contradicts dependency audit posture

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Evidence: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:5`; `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md:3-10`, `:24-36`

The dependency audit now says P025/P026 are not fixture blockers because implemented references and gates exist, but the proposal top-level status still says P051 is not ready for scheduling due to unresolved P025/P026 readiness. That leaves schedulability ambiguous and can cause closeout or implementation routing to follow stale guidance.

Acceptance:

- Align P051's top-level status with the dependency audit.
- Explicitly distinguish fixture/readback schedulability from broad rollout dogfood/sign-off.

### API-001: Late observation append refresh is not implemented

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Evidence: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1443`; `control-plane/crates/engine/src/executor.rs:68-81`; `control-plane/crates/domain/src/events.rs:9-67`

P051 requires late async Xcode observation appends to publish an execution update so GraphQL/MCP/UI readback can refresh without log scraping. The concrete engine sink writes to the DB only, and the domain event enum has no Xcode observation or agent-execution update event. This means persisted observations exist, but active UI/API consumers may not be notified after late shim/host-executor appends.

Acceptance:

- Publish an existing or new execution-update event after successful append.
- Add a test proving late append notification reaches the subscription/readback refresh path.

### REL-001: Shutdown close path can leave broker leases unreleased

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium-High
- Evidence: `control-plane/crates/acp/src/manager.rs:456-497`; `control-plane/crates/daemon/src/main.rs:421-431`

`close_session()` removes the matching `live_xcode_leases` entry and releases brokered leases. `close_all_sessions()` drains only live ACP sessions, and daemon shutdown uses `close_all_sessions()`. If a live session owns brokered Xcode leases during shutdown, the session close is attempted but the lease cleanup map is not drained or released through the attacher.

Acceptance:

- Drain `live_xcode_leases` during `close_all_sessions()` and release all associated leases, even when session close fails.
- Add a manager-level test with a fixture lease attacher proving shutdown releases all live lease ids.

### OBS-001: Observation persistence failure policy is only partially implemented

- Reviewer: `observability_rollout_reviewer`
- Severity: Medium
- Confidence: Medium
- Evidence: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1427-1429`; `control-plane/crates/acp/src/xcode_broker.rs:1101-1125`

Broker observation persistence failure increments an internal failure counter, logs a warning, and degrades broker health. The proposal contract is stronger: emit a tracing error, increment the named metric `xcode_observation_persist_failed_total`, and append a best-effort `observation_persistence_degraded` warning through normal execution failure evidence when the event belongs to an active execution.

Acceptance:

- Add the named metric or explicitly update the proposal/reference if the internal health counter is the chosen contract.
- Emit an error-level trace and persist best-effort degraded evidence for active executions.

### UI-001: High-level RunProgressView bridge state was not found

- Reviewer: `observability_rollout_reviewer`
- Severity: Medium
- Confidence: Medium
- Evidence: `docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1865`; `Chainworks Forge/Views/RunTimelineInspectorView.swift:78-103`; `Chainworks ForgeTests/RunTimelineInspectorViewTests.swift:179-211`

The bridge lock/action-required status is implemented and tested in the timeline inspector/projection path, but the proposal acceptance criterion explicitly says high-level `RunProgressView` should show those states and not only the timeline inspector. This may be a naming drift in the UI surface, but no distinct `RunProgressView` implementation evidence was found.

Acceptance:

- Either surface the same status in the real high-level run progress view or amend the proposal/reference to name the implemented surface.
- Add a focused Swift test for the actual high-level surface if it exists.

## User-Provided Review Findings Status

| Finding | Status after audit | Notes |
|---|---|---|
| Finding 1: P051 remains dependency-blocked | Partially addressed | Dependency audit artifacts now resolve fixture blockers, but proposal line 5 still says P025/P026 block scheduling. See OPS-001. |
| Finding 2: Follow-up blockers need draft proposals | Addressed in P051 scope | P051 now defines `P051-FU-01` and `P051-FU-02` with trigger, owner, scope, and acceptance. This satisfies the review option to keep fallback work explicit P051 scope. |

## Verification Log

Command run:

```bash
./scripts/test-gate.sh proposal-051
```

Result: passed.

Gate coverage observed:

- `p051-scaffold` passed.
- Workflow P051 integration tests passed.
- DB Xcode runtime observation tests passed.
- ACP brokered probe and bridge pool integration tests passed.
- ACP runtime manager lease attach test passed.
- Engine observation sink persistence test passed.
- GraphQL and MCP server compile checks passed.
- Domain artifact contract tests passed.
- Focused Swift tests passed:
  - `Chainworks ForgeTests/DaemonLifecycleClientTests`
  - `Chainworks ForgeTests/RunTimelineInspectorViewTests`

Not run:

- Live Xcode dogfood run.
- Release-owner/operator sign-off.
- Remote UI smoke tests, per repository policy.

## Final Verdict

P051 has a substantial and coherent fixture/readback implementation. The brokered MCP path, pool mechanics, target resolver, shim/host executor boundary, durable observation envelope, GraphQL/MCP readback, Swift readback, reference docs, evidence docs, and `proposal-051` gate are all in place enough to treat the fixture/readback lane as Ready with Risks.

P051 is not ready for full implementation closeout or broad `shim_enforced` rollout. The remaining blockers are concrete:

1. Fill the live dogfood/sign-off evidence and keep it separate from fixture gates.
2. Align the proposal top-level status with the updated dependency audit.
3. Implement late observation append notification for GraphQL/MCP/UI refresh.
4. Release broker leases in `close_all_sessions()` shutdown cleanup.
5. Resolve the high-level `RunProgressView` acceptance mismatch.
6. Either implement the exact observation persistence failure metric/error/warning contract or update P051/reference docs to match the chosen implementation.

Recommended next step: fix OPS-001 first because it is documentation truth only and removes scheduling ambiguity, then handle API-001 and REL-001 as small implementation patches before any closeout claim. READY-001 remains the release stop sign.
