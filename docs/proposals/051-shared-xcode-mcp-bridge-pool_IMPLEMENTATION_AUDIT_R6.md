# Proposal 051 Implementation Audit R6

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` |
| Proposal revision | `p051-r30` |
| Audit timestamp | `2026-04-25T10:46:09Z` |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Audited git HEAD | `9c59df8045512fae6e5c26f0ca45cc4ef616f8ee` |
| Proposal checksum | `md5:d4e234d31c18969a068fc62e3c9cc340` |
| Implementation target | Current dirty worktree, implicit compare base |
| Report path | `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R6.md` |

The audit includes uncommitted current-worktree files. It does not claim committed repository truth until these changes are committed or otherwise made durable.

## Direct Verdict

| Dimension | Verdict |
|---|---|
| Overall Conformance | **Partial** |
| Fixture/readback readiness | **Ready with Risks** |
| Full P051 / broad `shim_enforced` readiness | **Not Ready** |
| Reviewer-selection reuse | **Partially reused** |
| Audit confidence | **High** for fixture/readback; **medium** for broad rollout |

P051's fixture/readback implementation is materially aligned with the current proposal and the canonical `proposal-051` gate passed on this tree. Full P051 closeout remains blocked because live parallel Xcode dogfood, token-leakage evidence, observation-pressure evidence, and explicit operator/release-owner sign-off are still absent.

## Prior Review Reuse

Discovered prior artifacts:

- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md`
- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`

Reuse classification: **Partially reused**. The prior review/evidence context still matches P051, and the proposal's reviewer checks were used as routing input. No explicit checked-in reviewer-selection artifact was found, so implementation routing was reconstructed for the actual Rust/GraphQL/MCP/Swift/readiness surface.

Selected implementation reviewers:

- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `rust_security_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`

Rejected close alternatives:

- `macos_ui_reviewer`: the Swift surface is narrow readback; current gate covers `RunTimelineInspectorViewTests` and `DaemonLifecycleClientTests`, and no broad UI redesign is in scope.
- `apple_ux_reviewer`: recovery/status strings are fixture-tested; the remaining UX evidence is live dogfood, captured under readiness/rollout.
- `product_reviewer`: no new product decision surface is implemented here; product acceptance is the existing dogfood/sign-off stop sign.
- Go/iOS/performance reviewers: no Go or iOS target exists in this proposal, and no benchmark-level performance claim was audited beyond fixture latency/readiness gates.

## Proposal State And Contract

Proposal state: **Active**. The checked-in proposal declares fixture/readback implementation schedulable and broad `shim_enforced` rollout blocked until live dogfood/sign-off evidence is attached (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1-5`).

Scope:

- Rust control-plane ACP runtime, daemon HTTP route, engine sink, DB/domain observation storage, workflow/catalog scanning, GraphQL/MCP readback, health/rollout gates.
- Minimal macOS SwiftUI readback surfaces for Xcode Runtime rows, broker health, policy warnings, and friendly failure labels.
- Live dogfood and broad rollout evidence remain separate from reproducible fixture gates.

Primary service/user flows audited:

1. ACP provider requests Xcode MCP and receives a brokered HTTP lease, or fails closed before session allocation when HTTP MCP is unsupported.
2. Xcode MCP bridge pool reserves, authorizes, queues, initializes, isolates, and releases per-provider leases/backends.
3. Direct Xcode shell commands are declared, shimmed, routed, denied, or warned without direct `mcpbridge` bypass.
4. Broker/shim/host-executor events append to durable observations and refresh GraphQL/MCP/Swift readback.
5. Fixture gates, security evidence, dependency posture, rollout follow-ups, and live dogfood stop signs control handoff vs full closeout.

## Fidelity Inventory

Matches:

- The source proposal now explicitly distinguishes fixture/readback schedulability from broad rollout readiness (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1-5`).
- P025/P026 historical proposal-file absence is no longer a fixture blocker; current reference/gate truth is documented instead (`docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md:1-36`).
- The staged gates are registered and include Rust fixture checks plus Swift readback checks (`scripts/test-gate.sh:1647-1648`, `scripts/test-gate.sh:2447-2536`).
- Observation ownership and late append refresh are implemented through the engine sink and stage-status invalidation event (`control-plane/crates/engine/src/executor.rs:68-138`, `control-plane/crates/engine/tests/integration.rs:3821-3837`).
- Observation persistence failure degrades broker health and emits the required metric/warning markers without recursive append (`control-plane/crates/acp/src/xcode_broker.rs:554-608`, `control-plane/crates/acp/src/xcode_broker.rs:1101-1127`).
- Shutdown drains live Xcode lease cleanup during session close and global close-all (`control-plane/crates/acp/src/manager.rs:456-503`).
- Follow-up triggers are now concrete P051 follow-up records with owner, scope, and acceptance criteria (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1955-1970`).

Divergences:

- Full P051 pre-ship dogfood is intentionally not satisfied. The dogfood table still records live run id, modal count, fake-home boundary, observation completeness, token leakage review, pressure metrics, and operator decision as not run/recorded/signed (`docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:33-56`).
- Broad rollout remains sequenced behind P037/P049 assumptions for production-like rollout/adaptive behavior (`docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md:19-26`).
- The evidence includes untracked current-worktree artifacts, including `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md`; this is valid for worktree audit but not committed truth.

Ambiguities / Evidence Gaps:

- No live parallel Gemini/Xcode-capable run was executed in this audit.
- No screenshot/runtime UI inspection was captured; UI claims are backed by Swift unit tests and reference docs.
- Token redaction is fixture-reviewed, but not proven against real daemon logs/reports/UI from dogfood.
- Append pressure and normalized-table follow-up triggers cannot be evaluated without real dogfood volume.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | Checked-in P051 is canonical source and stale contrary guidance is gated | Implemented |
| REQ-002 | Dependency audit distinguishes fixture blockers from rollout blockers | Implemented |
| REQ-003 | Follow-up blockers have concrete owner/scope/acceptance | Implemented |
| REQ-004 | Staged `p051-scaffold` and `proposal-051|p051` gates exist | Implemented |
| REQ-005 | Brokered HTTP Xcode MCP replaces provider-owned stdio fallback | Implemented |
| REQ-006 | Unsupported HTTP MCP providers fail closed before allocation | Implemented |
| REQ-007 | Pool capacity, per-PID initialize serialization, and lease isolation exist | Implemented |
| REQ-008 | Xcode target snapshot/resolver boundary fails closed | Implemented |
| REQ-009 | Broker MCP policy filters/denies without sibling leakage | Implemented |
| REQ-010 | Direct-command scanner and catalog Xcode signals cover structured/raw commands | Implemented |
| REQ-011 | PATH shim/host executor keeps `mcpbridge` broker-only | Implemented |
| REQ-012 | Durable observation schema, bounds, redaction, GraphQL/MCP/Swift readback exist | Implemented |
| REQ-013 | Late append notification refreshes readback | Implemented |
| REQ-014 | Observation persistence failure emits markers and degrades broker health | Implemented |
| REQ-015 | Broker health is subsystem-scoped, not global daemon readiness | Implemented |
| REQ-016 | Minimum Swift readback surfaces exist and are tested | Implemented |
| REQ-017 | Targeted fixture security review exists | Implemented for fixture/readback |
| REQ-018 | Live dogfood and operator sign-off are attached | Partially Implemented |

## Detailed Requirement Audit

| ID | Proposal source | Evidence | Implementation mapping / note |
|---|---|---|---|
| REQ-001 | Canonical source and stale guidance gate (`docs/proposals/...md:39-44`, `1710-1711`) | `scripts/test-gate.sh:2447-2502`; tests-run | The static scaffold gate scans for stale no-UI/debug-assert/drop-on-corrupt/pgrep/same-uid-only guidance. |
| REQ-002 | Dependency audit precondition and metric (`1654-1657`, `1713-1715`) | `docs/proposals/...review/dependency-audit.md:1-36` | P025/P026/P029 are reconciled for fixture/readback; P037/P049 remain rollout sequencing dependencies. |
| REQ-003 | Follow-up blockers (`1734-1751`, `1955-1970`) | proposal | P051-FU-01 and P051-FU-02 now define trigger, owner, scope, and acceptance. |
| REQ-004 | Staged gates (`72`, `1660-1668`) | `scripts/test-gate.sh:1647-1648`, `2447-2536`; tests-run | Fresh `./scripts/test-gate.sh proposal-051` passed. |
| REQ-005 | Brokered HTTP MCP / no stdio fallback (`61-76`) | reference, tests-run | Reference defines HTTP lease entry and no direct stdio fallback (`docs/reference/xcode-mcp-bridge-pool.md:22-53`). |
| REQ-006 | Unsupported provider fail-closed (`66`, `1673-1675`) | `control-plane/crates/engine/tests/integration.rs:3816-3861`; tests-run | Engine integration proves unsupported provider failure persists a fail-closed observation without session allocation success. |
| REQ-007 | Pool lease/backpressure/init isolation (`63-64`, `1678-1679`) | `scripts/test-gate.sh:2509-2512`, `2528-2529`; tests-run | ACP fixture suite covers capacity, queue timeout, per-PID initialize serialization, first-connect timeout, drift, backend crash, and lease isolation. |
| REQ-008 | Target resolver boundary (`1746-1747`, `1852-1853`) | tests-run, reference | ACP fixtures and reference cover immutable `XcodeTargetSnapshot` and fail-closed ambiguity/no-match behavior. |
| REQ-009 | Broker MCP policy (`1718-1719`, `1853`) | tests-run | ACP fixture suite covers tools/list filtering, tools/call denial, denied observation, and sibling lease isolation. |
| REQ-010 | Scanner/catalog signals (`67`, `1738-1739`, `1861`) | `scripts/test-gate.sh:2507`, `2526`; tests-run | Workflow integration tests cover catalog/agent/workflow raw and structured Xcode command detection. |
| REQ-011 | PATH shim/host executor and broker-only `mcpbridge` (`68`, `1583`, `1855`) | tests-run, targeted security review | Shim/security tests and proposal gate cover direct `mcpbridge` rejection and host-executor routing boundaries. |
| REQ-012 | Durable observation schema/readback (`70-71`, `1412-1445`) | code, tests-run, reference | DB/domain/engine/API/readback path is covered by append bounds tests, GraphQL/MCP checks, Swift readback tests, and reference docs. |
| REQ-013 | Late append notification (`1443`) | `control-plane/crates/engine/src/executor.rs:68-138`; `control-plane/crates/engine/tests/integration.rs:3821-3837` | Successful append publishes `StageStatusChanged` with current stage status so subscribers re-read DB-backed execution rows. |
| REQ-014 | Persistence failure policy (`1426-1429`, `1440`) | `control-plane/crates/acp/src/xcode_broker.rs:554-608`, `1101-1127`, `1908-1940` | Broker increments failure count, emits metric/warning markers, and degrades health without recursive warning append. |
| REQ-015 | Broker health separation (`1742-1743`, `1862`) | reference, tests-run | Reference documents subsystem health and daemon status readback; Swift `DaemonLifecycleClientTests` passed. |
| REQ-016 | Minimal Swift UI/readback (`95-119`, `1865-1868`) | tests-run | `RunTimelineInspectorViewTests` passed for structured rows, progress status, policy-warning coalescing, friendly failure text, and catalog flags. |
| REQ-017 | Targeted security review (`1657`, `1860`) | `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md:1-44` | Fixture/readback security review exists and covers bearer lifecycle, shim replay/process binding, env/cwd/token redaction, and diagnostic bypasses. |
| REQ-018 | Live dogfood/sign-off (`1682-1707`, `1730-1751`, `1850`, `1943-1953`) | `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:33-56` | Artifact and stop sign exist, but live run id, modal count, fake-home evidence, observation completeness, token leakage review, pressure metrics, and human decision are absent. |

## Reviewer Scorecard

| Lens | Score | Top risk | Confidence |
|---|---:|---|---|
| Proposal conformance | 8/10 | Full dogfood/sign-off remains incomplete | High |
| Rust architecture | 9/10 | Worktree is dirty; implementation is not yet committed truth | High |
| Rust reliability | 9/10 | Live Xcode modal and append-pressure behavior not dogfooded | High for fixtures, medium for live |
| Rust security | 8/10 | Fixture security passed, live token-leakage review absent | Medium |
| API contract | 9/10 | No separate live API subscriber run; covered by compile/fixtures | High |
| Observability/rollout | 7/10 | Broad rollout is intentionally held by dogfood/sign-off | High |
| Readiness | 7/10 fixture / 3/10 full | Full closeout blocked | High |

## Routed Specialist Findings

### READY-001: Full P051 closeout is still blocked by live dogfood and sign-off

- Reviewer: `observability_rollout_reviewer`
- Severity: **Critical**
- Confidence: **High**
- Related requirements: REQ-018
- Evidence: proposal metrics (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1682-1707`, `1730-1751`), dogfood stop sign (`docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:33-56`)
- Why it matters: The fixture gate proves local mechanics, but the proposal explicitly requires live modal-count, fake-home, observation-completeness, token-leakage, pressure, and human sign-off evidence before full P051 completion or broad `shim_enforced` rollout.
- Recommended action: Run a real parallel Xcode-capable dogfood stage and fill every required evidence field with a `GO`/`HOLD` decision.
- Acceptance criteria: Dogfood artifact contains run id, provider/runtime, Xcode target, modal count, fake-home result, observation completeness, token leakage review, pressure metrics, signer, timestamp, and decision.

### SEC-001: Security evidence is fixture-complete but not live-rollout-complete

- Reviewer: `rust_security_reviewer`
- Severity: **Major**
- Confidence: **Medium**
- Related requirements: REQ-017, REQ-018
- Evidence: targeted review (`docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md:1-44`), missing dogfood token-leakage field (`docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md:47-50`)
- Why it matters: The code-level bearer/shim/redaction boundaries are fixture-reviewed, but broad rollout needs proof that real daemon logs, reports, GraphQL/MCP readback, and Swift UI do not leak broker or shim tokens.
- Recommended action: Include token-leakage inspection in the live dogfood package.
- Acceptance criteria: The live evidence explicitly checks logs, tracing, stored observations, GraphQL, MCP reports, and UI for raw bearer/shim token leakage.

### OPS-001: P037/P049 are not fixture blockers, but remain broad-rollout sequencing constraints

- Reviewer: `observability_rollout_reviewer`
- Severity: **Minor**
- Confidence: **High**
- Related requirements: REQ-002, REQ-018
- Evidence: dependency audit (`docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md:19-26`)
- Why it matters: Starting or closing fixture/readback work is no longer blocked by missing P025/P026 lineage artifacts, but broad rollout still depends on execution supervision/watchdog and context strategy assumptions.
- Recommended action: Keep P037/P049 out of fixture readiness, but attach either concrete readiness evidence or an explicit fixed-default rollout constraint before broad `shim_enforced`.
- Acceptance criteria: Broad rollout package states whether P037/P049 are satisfied, narrowed, or deferred with concrete fallback boundaries.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Canonical full/proposal gate on audited tree | **Passed** | `./scripts/test-gate.sh proposal-051` |
| Rust scaffold fixture gate | **Passed** | Gate output ended with `Proposal 051 scaffold gate passed` |
| Rust full fixture/readback gate | **Passed** | Gate output ended with `Proposal 051 gate passed` |
| Swift readback tests | **Passed** | `RunTimelineInspectorViewTests`: 7 tests passed; `DaemonLifecycleClientTests`: 15 tests passed |
| Result bundle | **Recorded** | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-134015.xcresult` |
| Live dogfood runtime | **Not run** | Required by dogfood stop sign |
| UI screenshot/manual runtime inspection | **Not run** | Swift tests only |
| Accessibility/localization runtime | **Not run** | No dedicated runtime pass in this audit |
| Privacy/token leakage live evidence | **Not ready** | Fixture security exists; live token leakage review absent |
| Full repo regression beyond P051 gate | **Not run** | Canonical P051 gate used as proposal regression evidence |

## Verification Log

| Command | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py .../docs/proposals/051-shared-xcode-mcp-bridge-pool.md` | Produced `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R6.md` |
| `git rev-parse HEAD` | `9c59df8045512fae6e5c26f0ca45cc4ef616f8ee` |
| `git status --short` | Dirty worktree with P051 implementation/docs and unrelated P031/doc changes |
| `./scripts/test-gate.sh proposal-051` | **Passed**. Rust fixture/check lanes passed; Swift selected tests passed; result bundle recorded at `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-134015.xcresult` |

Observed non-blocking validation noise:

- Rust warnings for existing unused imports/functions during gate.
- macOS test launch emitted LaunchAgent code-sign registration warnings, but selected tests completed successfully and `xcodebuild` returned `TEST SUCCEEDED`.

## Final Verdict And Recommended Next Actions

Fixture/readback handoff can proceed from the audited worktree if the dirty changes are made durable. The prior P051 review findings about P025/P026 dependency blocking and unnamed future follow-ups are addressed for fixture scheduling: the dependency audit now uses current reference/gate truth, and P051-FU-01/P051-FU-02 have concrete triggers, owners, scope, and acceptance criteria.

Do not close P051 as fully implemented, release-ready, or broad `shim_enforced` ready yet. The next required action is a real parallel Xcode-capable dogfood run with completed modal/fake-home/observation/token/pressure evidence and explicit operator or release-owner `GO`/`HOLD` sign-off.
