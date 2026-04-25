# Proposal 051 Implementation Audit R7

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` |
| Proposal revision | `p051-r30` |
| Audit timestamp | `2026-04-25T12:11:54Z` |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Branch / HEAD | `main` / `5bd8c0c12b0d736d28c6f06b0ccd50dd97a4087c` |
| Commit summary | `5bd8c0c1 Complete P051 fixture readback layer` |
| Proposal checksum | `md5:d4e234d31c18969a068fc62e3c9cc340` |
| Implementation target | Committed P051 fixture/readback layer at `5bd8c0c1`, plus current worktree runtime verification |
| Report path | `docs/proposals/051-shared-xcode-mcp-bridge-pool_IMPLEMENTATION_AUDIT_R7.md` |

Current worktree note: the repository has unrelated dirty changes after `5bd8c0c1`, but the P051 proposal, P051 evidence/reference docs, P051 gate script, ACP/daemon/GraphQL/MCP/workflow/domain P051 surfaces, and Swift P051 readback tests are clean relative to HEAD. The current dirty diffs include later DB/engine recovery work; the canonical P051 gate was rerun on the current tree and still passed.

## Direct Verdict

| Dimension | Verdict |
|---|---|
| Overall Conformance | **Partial** |
| Fixture/readback readiness | **Ready with Risks** |
| Dogfood-start readiness | **Ready with Risks** via temporary dev daemon |
| Full P051 / broad `shim_enforced` readiness | **Not Ready** |
| Reviewer-selection reuse | **Partially reused** |
| Audit confidence | **High** for fixture/readback; **medium** for release rollout |

P051's fixture/readback layer is now durable in `main`, the canonical `proposal-051` gate passes, and the dev daemon is live on `127.0.0.1:4000` with `build_sha=5bd8c0c1` and healthy broker state. Full P051 closeout remains blocked because live parallel Xcode dogfood and explicit `GO/HOLD` sign-off are still intentionally absent. Production launch readiness also remains constrained by the unresolved primary SMAppService daemon path.

## Prior Review Reuse

Discovered prior artifacts:

- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md`
- `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md`

Reuse classification: **Partially reused**. The prior evidence still applies, but no checked-in explicit reviewer-selection artifact was detected. Routing was reconstructed from the committed implementation surface and the new runtime/launchd evidence.

Selected implementation reviewers:

- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `rust_security_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`

Rejected close alternatives:

- `macos_ui_reviewer`: P051 has narrow Swift readback tests, not a broad UI implementation.
- `apple_ux_reviewer`: the remaining UX evidence is live dogfood/operator observation, covered under readiness.
- `product_reviewer`: no new product decision surface was implemented; dogfood/sign-off remains the product checkpoint.
- Go/iOS/performance reviewers: no matching implementation surface or benchmark commitment is in scope.

## Proposal State And Contract

Proposal state: **Active**. The proposal explicitly says fixture/readback implementation is schedulable, while broad `shim_enforced` rollout remains blocked until live dogfood/sign-off evidence is attached (`docs/proposals/051-shared-xcode-mcp-bridge-pool.md:1-5`).

Scope:

- Rust ACP runtime, daemon HTTP route, engine observation sink, DB/domain observation storage, workflow/catalog scanning, GraphQL/MCP readback, broker health, and P051 gates.
- Minimal macOS readback through `RunTimelineInspectorView` and daemon lifecycle broker health decoding.
- Rollout evidence: dependency audit, targeted fixture security review, temporary daemon runtime health, live dogfood stop sign.

Primary audited flows:

1. ACP provider receives brokered HTTP Xcode MCP lease or fails closed before allocation.
2. Bridge pool reserves, authorizes, queues, initializes, isolates, observes, and releases leases.
3. Direct Xcode shell paths are scanned/shimmed/routed/denied without direct `mcpbridge` bypass.
4. Broker/shim/host-executor observations persist and refresh GraphQL/MCP/Swift readback.
5. P051 fixture gate and live daemon health establish dogfood-start readiness, while sign-off controls full rollout.

## Fidelity Inventory

Matches:

- P051 fixture/readback changes are committed in `main` at `5bd8c0c1`.
- `./scripts/test-gate.sh proposal-051` passed again during this audit on the current tree.
- Dev daemon health is live: `state=ready`, `build_sha=5bd8c0c1`, broker `state=healthy`, `can_acquire_new_xcode_leases=true`, zero active/queued leases, zero observation persistence failures.
- `/xcode-mcp/health` returns healthy broker state independently of global health.
- The temporary launchd job `com.chainworks.forge.daemon.manual.p051` is running and points at the debug app's bundled `chainworks-forge-daemon`.
- Prior P025/P026 dependency-blocker wording is resolved for fixture/readback by current reference/gate truth.
- P051-FU-01 and P051-FU-02 have concrete trigger, owner, scope, and acceptance criteria.

Divergences:

- Full dogfood/sign-off remains absent by design. No live parallel Xcode-capable stage, modal count, fake-home proof, observation completeness proof, token-leakage review, append-pressure data, or `GO/HOLD` decision was attached.
- The primary SMAppService label `com.chainworks.forge.daemon` is not validated as healthy. The operator reports it still fails with exit code 78 / `EX_CONFIG` and needs LWCR update; local `launchctl print gui/501/com.chainworks.forge.daemon` did not find a running service, while the manual P051 job is running.
- Runtime evidence is from a temporary dev launchd job, not the final production service registration path.

Ambiguities / Evidence Gaps:

- No live dogfood run was executed in this audit.
- No screenshots or manual UI inspection were captured; Swift readback is covered by tests.
- Token redaction is fixture/security-reviewed but not proven against real dogfood logs/reports/UI.
- Observation pressure thresholds cannot be evaluated before dogfood volume.
- Main SMAppService failure details are operator-supplied; this audit verified absence of the service from `launchctl print` and verified the manual job instead.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | Checked-in P051 is canonical and stale guidance is gated | Implemented |
| REQ-002 | Dependency audit distinguishes fixture blockers from rollout blockers | Implemented |
| REQ-003 | Rollout follow-up blockers have owner/scope/acceptance | Implemented |
| REQ-004 | `p051-scaffold` and `proposal-051|p051` gates exist and pass | Implemented |
| REQ-005 | Brokered HTTP Xcode MCP replaces provider-owned stdio fallback | Implemented |
| REQ-006 | Unsupported HTTP MCP providers fail closed before allocation | Implemented |
| REQ-007 | Pool capacity, per-PID initialize serialization, and lease isolation exist | Implemented |
| REQ-008 | Target resolver boundary fails closed on ambiguity/drift | Implemented |
| REQ-009 | Broker MCP policy filters and denies without sibling leakage | Implemented |
| REQ-010 | Direct-command scanner and catalog signals cover structured/raw Xcode commands | Implemented |
| REQ-011 | PATH shim/host executor keeps `mcpbridge` broker-only | Implemented |
| REQ-012 | Durable observations, bounds, redaction, GraphQL/MCP/Swift readback exist | Implemented |
| REQ-013 | Late append notification refreshes readback | Implemented |
| REQ-014 | Observation persistence failure emits markers and degrades broker health | Implemented |
| REQ-015 | Broker health is subsystem-scoped and daemon-readable | Implemented |
| REQ-016 | Minimum Swift readback surfaces exist and pass tests | Implemented |
| REQ-017 | Targeted fixture security review exists | Implemented for fixture/readback |
| REQ-018 | Live dogfood and operator/release-owner sign-off are attached | Partially Implemented |

## Detailed Requirement Audit

| ID | Proposal source | Evidence | Implementation mapping / note |
|---|---|---|---|
| REQ-001 | Canonical source and stale-guidance gate (`docs/proposals/...md:39-44`, `1710-1711`) | `scripts/test-gate.sh:2447-2502`; tests-run | Static gate rejects stale no-UI/debug-assert/drop-on-corrupt/pgrep/same-uid-only guidance. |
| REQ-002 | Dependency audit precondition (`1654-1657`, `1713-1715`) | `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md` | P025/P026/P029 are fixture-satisfied; P037/P049 remain rollout sequencing constraints. |
| REQ-003 | Follow-up blockers (`1734-1751`, `1955-1970`) | proposal | P051-FU-01/P051-FU-02 are named, owned, scoped, and have acceptance criteria. |
| REQ-004 | Staged gates (`72`, `1660-1668`) | tests-run | `./scripts/test-gate.sh proposal-051` passed during this audit. |
| REQ-005 | Brokered HTTP MCP and no stdio fallback (`61-76`) | code, reference, tests-run | HTTP lease path and no direct stdio fallback are covered by ACP fixtures and reference docs. |
| REQ-006 | Unsupported provider fail-closed (`66`, `1673-1675`) | tests-run | Engine integration proves fail-closed observation is persisted before unsupported provider allocation. |
| REQ-007 | Lease/backpressure/init isolation (`63-64`, `1678-1679`) | tests-run | ACP fixture suite covers capacity, queue timeout, per-PID initialize serialization, first-connect timeout, drift, crash, and lease isolation. |
| REQ-008 | Target resolver boundary (`1746-1747`, `1852-1853`) | tests-run, reference | Fixtures cover immutable `XcodeTargetSnapshot` and fail-closed ambiguity/no-match behavior. |
| REQ-009 | Broker MCP policy (`1718-1719`, `1853`) | tests-run | Fixtures cover tools/list filtering, tools/call denial, denied observation, and sibling isolation. |
| REQ-010 | Scanner/catalog signals (`67`, `1738-1739`, `1861`) | tests-run | Workflow/catalog integration tests cover structured/raw command detection and direct `mcpbridge` rejection. |
| REQ-011 | Shim/host executor broker-only `mcpbridge` (`68`, `1583`, `1855`) | tests-run, security review | P051 gate and targeted security review cover direct `mcpbridge` containment and host execution boundaries. |
| REQ-012 | Durable observation/readback (`70-71`, `1412-1445`) | code, tests-run, runtime | DB/domain/engine/API/Swift readback passed; daemon health exposes broker state at runtime. |
| REQ-013 | Late append notification (`1443`) | tests-run | Engine integration covers stage invalidation after late Xcode observation append. |
| REQ-014 | Persistence failure policy (`1426-1429`, `1440`) | tests-run, code | Broker increments failure count, emits required markers, and degrades health without recursive warning append. |
| REQ-015 | Broker health separation (`1742-1743`, `1862`) | tests-run, runtime | `GET /health` and `GET /xcode-mcp/health` both show healthy broker state independent of global readiness. |
| REQ-016 | Swift readback (`95-119`, `1865-1868`) | tests-run | `RunTimelineInspectorViewTests` and `DaemonLifecycleClientTests` passed. |
| REQ-017 | Security review (`1657`, `1860`) | evidence | `docs/evidence/051-shared-xcode-mcp-bridge-pool/security-review.md` is committed in `5bd8c0c1`. |
| REQ-018 | Dogfood/sign-off (`1682-1707`, `1730-1751`, `1850`, `1943-1953`) | evidence, runtime | Dev daemon is ready for dogfood, but required live dogfood table and explicit sign-off remain incomplete. |

## Reviewer Scorecard

| Lens | Score | Top risk | Confidence |
|---|---:|---|---|
| Proposal conformance | 8/10 | Live dogfood/sign-off still incomplete | High |
| Rust architecture | 9/10 | Dirty post-commit worktree should not be confused with committed P051 truth | High |
| Rust reliability | 9/10 fixture / 6/10 rollout | Temporary dev job works, primary service path unresolved | Medium-high |
| Rust security | 8/10 | Live token leakage review absent | Medium |
| API contract | 9/10 | Runtime health verified; live dogfood report readback not yet exercised | High |
| Observability/rollout | 7/10 | SMAppService path and dogfood sign-off block release | High |
| Readiness | 8/10 dogfood-start / 3/10 full closeout | No `GO/HOLD` evidence | High |

## Routed Specialist Findings

### READY-001: Full P051 closeout remains blocked by live dogfood and sign-off

- Reviewer: `observability_rollout_reviewer`
- Severity: **Critical**
- Confidence: **High**
- Related requirements: REQ-018
- Evidence: proposal dogfood metrics, `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md`, runtime daemon health
- Why it matters: The committed fixture/readback layer and live dev daemon prove readiness to start dogfood, but the proposal requires real modal-count, fake-home, observation-completeness, token-leakage, pressure, and human decision evidence before full closeout or broad `shim_enforced`.
- Recommended action: Run the real parallel Xcode-capable dogfood stage using the verified temporary daemon path, then fill the dogfood artifact with real data and explicit `GO/HOLD`.
- Acceptance criteria: Dogfood artifact records run id, workflow/stage, provider/runtime, Xcode target, modal count, fake-home result, observation completeness, token leakage review, pressure metrics, signer, timestamp, and decision.

### OPS-001: Production SMAppService daemon path is still not release-ready

- Reviewer: `observability_rollout_reviewer`
- Severity: **Major**
- Confidence: **Medium**
- Related requirements: REQ-015, REQ-018
- Evidence: runtime health from temporary `com.chainworks.forge.daemon.manual.p051`; operator-reported main label `EX_CONFIG` / LWCR blocker; local `launchctl print gui/501/com.chainworks.forge.daemon` did not find a running service.
- Why it matters: The temporary dev launchd job is enough to continue dogfood, but broad rollout should not depend on a manual label while the primary app-managed daemon route remains broken.
- Recommended action: Treat the dev job as dogfood-only infrastructure and keep production rollout held until the SMAppService/LWCR path starts cleanly and reports the same `build_sha` and broker health.
- Acceptance criteria: `com.chainworks.forge.daemon` starts through the normal app-managed path, `GET /health` reports `ready`, `build_sha=5bd8c0c1` or newer approved commit, broker `healthy`, and no launchd `EX_CONFIG`/LWCR blocker remains.

### SEC-001: Live token-leakage evidence is still missing

- Reviewer: `rust_security_reviewer`
- Severity: **Major**
- Confidence: **Medium**
- Related requirements: REQ-017, REQ-018
- Evidence: targeted fixture security review, missing dogfood token-leakage field
- Why it matters: Fixture coverage verifies bearer/shim/redaction mechanics, but broad rollout needs evidence from real daemon logs, GraphQL/MCP reports, stored observations, and Swift readback.
- Recommended action: Include explicit token-leakage inspection in the dogfood sign-off package.
- Acceptance criteria: Live evidence states that no raw MCP bearer or shim token appears in logs, tracing, stored observations, GraphQL, MCP reports, or UI.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| P051 commit on `main` | **Passed** | `5bd8c0c1 Complete P051 fixture readback layer` |
| Current P051 proposal/reference/evidence/gate files clean vs HEAD | **Passed** | `git status --short` for P051 paths returned no changes |
| Canonical proposal gate | **Passed** | `./scripts/test-gate.sh proposal-051` |
| Rust fixture/readback lane | **Passed** | `p051-scaffold` plus full Rust P051 tests/checks passed |
| Swift readback lane | **Passed** | `DaemonLifecycleClientTests`: 15 passed; `RunTimelineInspectorViewTests`: 7 passed |
| Swift result bundle | **Recorded** | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-150920.xcresult` |
| Dev daemon runtime health | **Passed** | `GET /health` ready, `build_sha=5bd8c0c1`, PID `77754`, broker healthy |
| Broker route health | **Passed** | `GET /xcode-mcp/health` healthy |
| Main SMAppService route | **Not Ready** | Main label not running in local launchctl check; operator reports `EX_CONFIG`/LWCR blocker |
| Live dogfood run | **Not run** | Dogfood artifact remains unsigned/incomplete |
| Token-leakage live review | **Not run** | Required in dogfood sign-off |
| UI screenshot/manual runtime inspection | **Not run** | Swift tests only |

## Verification Log

| Command / check | Result |
|---|---|
| `git rev-parse HEAD` | `5bd8c0c12b0d736d28c6f06b0ccd50dd97a4087c` |
| `git show --no-patch --format='%h %s%n%ci' 5bd8c0c1` | `5bd8c0c1 Complete P051 fixture readback layer`, committed `2026-04-25 14:47:25 +0300` |
| `git status --short` | Dirty worktree has unrelated post-commit changes; P051 checked paths are clean |
| `./scripts/test-gate.sh proposal-051` | **Passed**; result bundle `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-051-swift-20260425-150920.xcresult` |
| `curl -fsS http://127.0.0.1:4000/health` | `state=ready`, `build_sha=5bd8c0c1`, broker `healthy`, PID `77754` |
| `curl -fsS http://127.0.0.1:4000/xcode-mcp/health` | broker `healthy`, can acquire leases, zero persistence failures |
| `launchctl print gui/501/com.chainworks.forge.daemon.manual.p051` | manual P051 job running |
| `launchctl print gui/501/com.chainworks.forge.daemon` | no running service found in local check |

Observed non-blocking validation noise:

- Rust warnings for existing unused imports/functions.
- macOS test launch emitted expected local connection-refused logs for a negative lifecycle test.

## Final Verdict And Recommended Next Actions

The committed P051 fixture/readback layer is ready for dogfood continuation. The temporary dev daemon is a sufficient dogfood substrate because it is running the committed `5bd8c0c1` build and exposes ready/healthy broker state on `127.0.0.1:4000`.

Do not mark P051 fully complete or broad `shim_enforced` ready yet. Next steps are: run live parallel Xcode dogfood against the dev daemon, attach real evidence and `GO/HOLD`, then separately fix or explicitly scope the primary SMAppService/LWCR launch path before production rollout.
