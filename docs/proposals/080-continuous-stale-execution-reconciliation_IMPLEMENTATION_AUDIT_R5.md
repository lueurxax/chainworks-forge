# Implementation Audit R5: Proposal 080 Continuous Stale Execution Reconciliation

Audit timestamp: 2026-06-20T17:06:18+03:00
Auditor: Codex
Proposal: `docs/proposals/080-continuous-stale-execution-reconciliation.md`
Proposal revision: `p080-refined-2026-06-02-r28`
Proposal status: `draft_refined_for_implementation_review`
Implementation target: current dirty worktree at HEAD `0e6482c8`
Report path: `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R5.md`

Overall Conformance: Not Implemented

Overall Implementation Readiness: Not Ready

Audit confidence: High for the current repository state. The audit covered the proposal contract, required skill routing, current P080 Rust implementation surfaces, MCP and GraphQL handlers, reference docs, runbooks, rollout evidence, canonical gate behavior, focused build/tests, and security-sensitive surface classification. Confidence is limited only for live production behavior because no daemon soak or real rollout evidence exists.

## Executive Verdict

The current tree has a meaningful P080 Phase 1 diagnostics/readback slice: additive SQLite schema, P080 domain enums, rollout-control seed/fail-closed checks, a live diagnose-only classifier loop, MCP diagnostics and diagnose-only admission, read-only GraphQL projection/subscription code, run-report and release-receipt readback builders, and focused DB/MCP/GraphQL unit coverage.

It is not the full proposal. P080 still does not perform active stale execution repair: no ACP startup reset, scheduler lease/capacity reclaim, P076-backed side-effect-safe repair, owned helper reap, permanent-hold clear, or phase promotion evidence. The canonical `proposal-080` gate fails before Rust tests because required rollout JSON is missing and the rollout-contract fixtures still contain placeholder evidence markers.

The implementation should remain classified as Phase 1 scaffolding only. It is not ready for closeout or reference-doc retirement.

## Prior Review Reuse

`discover_prior_review.py` returned no prior review artifacts for reviewer-selection reuse.

Prior `*_IMPLEMENTATION_AUDIT_R*.md` files were not used to select reviewers. R4 was checked only as background after reviewer routing, and the R5 verdict is based on current proposal text, code, gates, and evidence.

## Reviewer Selection

The implementation surface fingerprint required these lenses: `api-contract`, `apple-ui-ux`, `architecture`, `observability-rollout`, `performance`, `reliability`, and `security`.

Selected reviewer lenses:

- Rust control-plane architecture and DB persistence: migrations, repositories, engine loop, transaction boundaries, and restart durability.
- API contract: MCP schemas/handlers and GraphQL read-only projection/subscription.
- Reliability and recovery: live loop admission, fail-closed gates, scheduler/session repair absence, and terminal heartbeat retirement.
- Security: public ingress, auth/run-scope ordering, parser limits, secret redaction, process-control safety, and disabled mutation paths.
- Observability and rollout: metrics, run reports, release receipts, rollout-control evidence, and canonical gate proof.

Rejected or reduced lenses:

- Apple UI/UX: reduced to a static boundary scan because P080 Phase 1 explicitly has no SwiftUI/AppKit visual surface and P099 owns the future diagnostics window.
- Performance: folded into reliability/rollout because the implemented slice has page limits, tick limits, and subscription rate shedding but no active repair throughput behavior.
- Product/design review: not selected because the current deliverable is daemon diagnostics and operator readback, not a user-facing feature slice.

Specialist coverage is sufficient for the implemented surface. The missing active repair phases will need renewed reliability, security, process-control, and rollout review when implemented.

## Rejected Completion Alternatives

- Rejected treating Phase 1 diagnostics as full P080 completion. The proposal's goals and rollout plan include active repair phases beyond diagnose-only readback.
- Rejected accepting placeholder rollout-contract fixtures as evidence. The gate explicitly rejects placeholder markers.
- Rejected counting P098 or P099 as P080 completion. P098 owns future manual hold semantics; P099 owns a future read-only macOS diagnostics window. Neither owns P080's ACP startup, scheduler, helper, or permanent-hold repair phases.
- Rejected relying on docs assertions without the canonical gate. The proposal requires `proposal-080` / `p080` gate proof.

## Proposal Contract Summary

P080 requires a daemon-owned reconciliation system that continuously distinguishes useful running work from stale running truth, repairs retryable non-side-effect work through a shared typed transition, delegates ambiguous side-effect work to authoritative subsystems, owns provider/helper process records before cleanup, exposes stable `p080_readback_v1` across MCP/GraphQL/reports/receipts, persists cooldown/recurrence/dedup state across restarts, and proves rollout safety through fixtures, gates, metrics, and phase evidence.

Explicit non-goals include SwiftUI repair controls, GraphQL repair mutations, blind side-effect retry, arbitrary process termination, enabled manual `requested_action=hold`, and P080-owned `acp_prompt_stale` repair. Manual hold is deferred to P098; the macOS diagnostics window is deferred to P099.

## Platform And Product Scope

Implemented scope is Rust control-plane daemon Phase 1. SwiftUI remains a read-only operator shell and currently contains no P080 implementation. A static search found no `P080`/`p080` references under `Chainworks Forge/`, which matches the Phase 1 UI non-goal.

Product-visible operator behavior today is MCP/GraphQL/report readback only. Operators cannot safely rely on P080 to fix stale execution state yet.

## Flow Audit

| Flow | Status | Evidence |
|---|---|---|
| Rollout-control seed and fail-closed defaults | Partial | Migration and repo seed rows exist for repair classes, `detection_only`, and `live_disable`; unit tests pass. Canonical evidence fixtures are still placeholders. |
| Live classification loop | Partial | `run_p080_stale_execution_reconciliation_loop` runs every 30 seconds with a 20 second tick deadline and writes diagnose-only readback when `detection_only` is enabled. It explicitly performs no ACP reset or scheduler reclaim. |
| MCP `p080.diagnostics.get.v1` | Partial | Handler enforces schema/version/resource/auth/gate checks and reads `p080_readback_heartbeats_v1`; focused MCP tests pass. |
| MCP `p080.reconcile.request.v1 diagnose_only` | Partial | Diagnose-only returns readback and writes no dedup row; focused tests pass. This is not repair. |
| MCP `repair_if_safe` | Missing | Handler returns `class_disabled` or `rollout_disabled`; no scheduler/session/lease mutation path exists. |
| MCP `hold` / `clear_permanent_hold` | Deferred/Missing | `hold` is disabled in P080 and future-owned by P098. `clear_permanent_hold` remains Phase 5-only and disabled. |
| GraphQL `p080Diagnostics` and subscription | Partial | Read-only types/resolvers/subscription code exist with auth and gate checks. Focused GraphQL P080 tests pass, but full SDL/fixture evidence remains placeholder-gated. |
| Run-report and release-receipt readback | Partial | DB repo builds P080 sections and redacts rows; focused DB tests cover report redaction and summary behavior. Gate evidence is missing. |
| Phase promotion and soak evidence | Missing | `docs/evidence/rollout/p080/` contains no JSON evidence, only `README.md`. |

## Fidelity And Divergence

High-fidelity areas:

- The Phase 1 fail-closed posture is faithfully represented in code, docs, and tests.
- P080 readback vocabulary is present across domain enums, DB rows, MCP, GraphQL, reports, and receipts.
- ReadOnlyOperator and scoped run access are handled conservatively before rollout-state disclosure.
- SwiftUI repair/control UI is absent, matching the Phase 1 non-goal.

Divergences:

- The proposal's full reconciliation promise is not met because active repair phases are missing.
- Rollout-contract fixtures are still placeholders, so declared contracts are not proven.
- Phase promotion requires soak/readiness artifacts that do not exist.
- Helper/process-control safety remains a design/schema contract, not executable behavior.

## Requirement Conformance

| Requirement | Verdict | Notes |
|---|---|---|
| Continuous stale execution detection | Partial | Live loop and classifier exist for Phase 1. The loop is diagnose-only and does not complete stale reconciliation. |
| Ownership registry joins runtime witnesses | Partial | Schema and some classifier inputs exist. Full provider/session/helper/P076 witness graph is not used for active repair decisions. |
| Shared typed repair transition | Missing | No active P080 path performs ACP reset, scheduler reclaim, helper reap, side-effect-safe repair, recurrence advancement for real repair, or permanent hold clear. |
| P076 side-effect safety | Missing | Current implementation does not repair side-effect-adjacent scheduler/release drift based on P076 retry-safe state. |
| Helper/process ownership and cleanup | Missing | Helper lease tables exist, but no active P080 path verifies Darwin process identity or signals owned helper processes. This is safe fail-closed behavior, not implementation. |
| Stable readback across MCP/GraphQL/reports/receipts | Partial | Meaningful readback contracts exist and focused tests pass. They reflect diagnose-only behavior and lack non-placeholder rollout evidence. |
| Auth, parser limits, redaction, run scope | Partial | MCP and GraphQL focused tests cover key Phase 1 ordering and redaction behavior. Canonical negative fixtures remain placeholders. |
| Dedup/idempotency across repair requests | Partial | DB primitives exist. Active repair dedup replay/fingerprint conflict behavior is phase-gated and not exercised by the canonical gate. |
| Rollout controls and phase promotion | Partial/Fail | Rollout-control rows and fail-closed checks exist. Promotion artifacts, soak evidence, and fixture proof are missing. |
| Proposal gate proof | Fail | `./scripts/test-gate.sh proposal-080` fails at evidence preflight. |

## Findings

### READY-001: Canonical P080 gate fails at fixture/evidence preflight

Severity: Blocker

`./scripts/test-gate.sh proposal-080` fails before build/tests. The failure reports:

```text
proposal-080: FAIL - missing P080 rollout evidence JSON under docs/evidence/rollout/p080
proposal-080: FAIL - fixture still contains placeholder evidence: docs/evidence/rollout-contract/operator-readback/p080-full-surface.fixture.json
proposal-080: FAIL - fixture still contains placeholder evidence: docs/evidence/rollout-contract/negative/p080-acp-prompt-stale-delegated-to-p037.json
...
proposal-080: FAIL - fixture still contains placeholder evidence: docs/evidence/rollout-contract/negative/p080-version-negotiation-compatibility.json
```

The gate is correct to fail. P080 cannot be marked Ready while release-critical fixtures are placeholders and rollout evidence JSON is absent.

Required action: replace placeholder fixtures with concrete positive/negative evidence, add the required rollout JSON under `docs/evidence/rollout/p080/`, and rerun the canonical gate.

### REL-001: P080 still does not repair stale executions

Severity: Blocker

The implemented live loop writes `decision=diagnosed` and `repair_action=diagnose_only`. The code and runbook both state Phase 1 performs no ACP reset, scheduler capacity repair, helper reap, manual hold, or permanent-hold clear. The MCP `repair_if_safe` branch returns disabled/phase-gated errors instead of invoking a shared typed repair transition.

This leaves the proposal's central behavior incomplete: retryable non-side-effect running rows with missing live ownership are not repaired within two healthy reconciliation intervals.

Required action: implement and gate the active repair phases or explicitly split them into follow-up proposals with accepted ownership. Today, they remain P080-owned.

### REL-002: Ownership and process-control repair security is schema-only

Severity: High

The migration creates helper lease and helper lease member tables, and the proposal defines strict Darwin process-control rules. No active P080 implementation uses those tables to reverify PID/start time/parent chain, enforce signal phases, or terminate owned helper processes. The placeholder `p080-process-control-security` fixture is still not real evidence.

The current behavior is safe because it does not signal processes. It is still not implementation of the process-control security contract.

Required action: implement helper ownership verification and signal handling only behind the Phase 4 gate, with concrete negative evidence for PID reuse, partial enumeration, permission failures, parent-chain mismatch, and signal verification failure.

### OBS-001: Rollout and soak evidence is absent

Severity: Blocker

The proposal requires phase readiness artifacts such as `docs/evidence/rollout/p080/phase-1-soak-report.json`. The directory contains no JSON evidence. There is no canary/soak window, no provider/work-kind coverage, no false-positive review sample, no operator acknowledgement, and no metric-backed promotion evidence.

Required action: produce the Phase 1 soak evidence and bind it to the rollout-contract fixture/gate inventory.

### API-001: Mutating API shape exists before mutating behavior

Severity: Medium

MCP exposes `repair_if_safe`, `hold`, and `clear_permanent_hold` request shapes, while the current implementation returns disabled/diagnose-only responses. That is acceptable for Phase 1 scaffolding, but it is not completion evidence and can confuse closeout status if documentation does not keep the distinction visible.

Required action: keep reference docs/runbooks explicit that these are listed but disabled until the corresponding phases pass, and avoid claiming P080 repair availability.

## Security Scan

The mandatory security-sensitive diff helper triggered review categories:

- `auth`
- `dos_resource_limits`
- `filesystem_subprocess_boundary`
- `parser_boundary`
- `public_ingress`
- `secrets_redaction_privacy`
- `unsafe_crypto_dependency`

Security-sensitive areas reviewed:

- MCP parser/resource limit ordering and duplicate-key pre-parse checks.
- Principal class and run-scope checks before rollout-state disclosure.
- ReadOnlyOperator restrictions for diagnose-only versus repair.
- Secret-like redaction for MCP, GraphQL, run-report, and release-receipt readback.
- P080 repair idempotency key and evidence marker public-safety handling.
- Helper/process-control safety boundaries.

No active P080 Phase 1 exploit was identified in static inspection. The security blocker is evidence and behavior completeness: active repair and helper signaling are not implemented, and the relevant negative fixtures are placeholders.

## Residual Follow-Up Ownership

- P080 remains owner of Phase 1 rollout evidence, canonical gate proof, active ACP startup stale repair, scheduler ownership repair, helper orphan reaping, permanent-hold clear, and phase promotion artifacts.
- P098 owns future manual operator hold and clear-hold semantics. It is Draft and explicitly does not replace P080's automatic stale classification or repair phases.
- P099 owns the future read-only macOS diagnostics window. It is Draft and explicitly must not start until P080 Phase 1 detection-only evidence exists.
- P037 remains owner of `acp_prompt_stale` supervision. P080 should continue to delegate this class.
- P076 remains owner of side-effect retry safety. P080 must consume P076 retry-safe truth before any side-effect-adjacent repair.

## Scorecard

| Area | Score | Rationale |
|---|---:|---|
| Proposal coverage | 35% | Phase 1 scaffolding exists; active repair phases and rollout evidence are missing. |
| API contract | 65% | MCP/GraphQL readback/admission surfaces exist and focused tests pass; full fixture evidence fails. |
| Reliability | 35% | Diagnose-only loop exists; no actual stale execution repair or scheduler capacity reclaim. |
| Security | 55% | Auth/redaction/parser checks are present; process-control repair remains unimplemented and unevidenced. |
| Observability/rollout | 30% | Metrics/docs/runbooks exist, but soak JSON and non-placeholder fixtures are absent. |
| UI boundary | 90% | No Swift P080 mutation/UI surface exists, which matches Phase 1. Future UI is P099-owned. |
| Gate readiness | 0% | Canonical `proposal-080` gate fails. |

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Proposal contract mapped | Pass | Goals, non-goals, APIs, rollout phases, metrics, and gates reviewed. |
| Prior review discovery | Pass | `discover_prior_review.py` returned no review artifacts. |
| Security-sensitive scan | Pass with blockers | Required categories reviewed; no active exploit found, but active repair security evidence is missing. |
| Build P080 Rust crates | Pass | `cargo build -p db -p mcp-server -p graphql-server` passed with warnings only. |
| Focused DB P080 tests | Pass | `cargo test -p db p080_ -- --nocapture` passed: 31 passed, 0 failed. |
| Focused MCP P080 tests | Pass | `cargo test -p mcp-server p080_ -- --nocapture` passed: 49 passed, 0 failed. |
| Focused GraphQL P080 tests | Pass | `cargo test -p graphql-server p080_ -- --nocapture` passed: 5 passed, 0 failed. |
| Canonical P080 gate | Fail | Missing rollout JSON and placeholder fixture evidence. |
| Active repair behavior | Fail | `repair_if_safe`, helper reap, scheduler reclaim, and permanent-hold clear are disabled/missing. |
| Phase promotion evidence | Fail | No JSON evidence under `docs/evidence/rollout/p080/`. |
| Swift UI boundary | Pass | No P080 references found under `Chainworks Forge/`; matches Phase 1 non-goal. |

## Verification Log

- Read `/Users/user/.codex/skills/proposal-implementation-audit/SKILL.md` completely and followed read-only audit mode.
- Ran `report_path.py` for P080; selected this R5 report path.
- Ran `discover_prior_review.py`; no prior review artifacts found.
- Ran `security_sensitive_diff.py`; security review categories triggered.
- Ran `implementation_surface_fingerprint.py`; required lenses recorded above.
- Parsed P080 proposal metadata and `proposal_markdown` sections for goals, non-goals, architecture, API contracts, rollout controls, metrics, evidence, and gates.
- Reviewed `scripts/test-gate.sh` P080 block and `docs/reference/test-gates.md` P080 gate documentation.
- Reviewed P080 implementation surfaces in `control-plane/crates/domain/src/p080.rs`, `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql`, `control-plane/crates/db/src/repos/p080.rs`, `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/mcp-server/src/tools/p080.rs`, `control-plane/crates/graphql-server/src/types/p080.rs`, and `control-plane/crates/graphql-server/src/schema.rs`.
- Reviewed reference/runbook statements in `docs/reference/rust-control-plane.md`, `docs/runbooks/p080-stale-execution-repair.md`, and `docs/runbooks/p080-reconciliation-loop-termination.md`.
- Reviewed follow-up ownership in `docs/proposals/098-p080-manual-operator-hold-and-clear-hold-semantics.md` and `docs/proposals/099-p080-read-only-diagnostics-window.md`.
- Ran `./scripts/test-gate.sh proposal-080 > /tmp/p080-audit-r5-gate.log 2>&1`; failed at rollout evidence/placeholder fixture preflight.
- Ran `cargo build -p db -p mcp-server -p graphql-server > /tmp/p080-audit-r5-cargo-build.log 2>&1`; passed with warnings.
- Ran `cargo test -p db p080_ -- --nocapture > /tmp/p080-audit-r5-db-tests.log 2>&1`; passed, 31 tests.
- Ran `cargo test -p mcp-server p080_ -- --nocapture > /tmp/p080-audit-r5-mcp-tests.log 2>&1`; passed, 49 tests.
- Ran `cargo test -p graphql-server p080_ -- --nocapture > /tmp/p080-audit-r5-graphql-tests.log 2>&1`; passed, 5 tests.
- Ran static search for P080 Swift references under `Chainworks Forge/`; none found.

## Final Actions Required

1. Replace placeholder rollout-contract fixtures with concrete P080 positive and negative evidence.
2. Add required rollout evidence JSON under `docs/evidence/rollout/p080/`, beginning with Phase 1 soak evidence.
3. Implement or formally split the active repair phases: ACP startup repair, scheduler ownership repair, helper orphan reaping, and permanent-hold clear.
4. Re-run `./scripts/test-gate.sh proposal-080` and do not move to closeout until it passes.

Final verdict: P080 is a useful Phase 1 detection/readback implementation, but the proposal is not implemented and is not ready.
