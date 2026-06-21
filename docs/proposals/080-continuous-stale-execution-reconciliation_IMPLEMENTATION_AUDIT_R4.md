# Proposal 080 Implementation Audit R4

## Verdict

Overall Conformance: Not Implemented

Overall Implementation Readiness: Not Ready

Audit confidence: Medium-high. The audit covered the active proposal contract, the current dirty Rust control-plane implementation surface, MCP and GraphQL readback behavior, rollout evidence, the P080 gate, mandatory helper routing, and a security-sensitive pass. Confidence is limited by the absence of live daemon and soak evidence.

The current tree contains a real Phase 1 diagnostics/readback implementation: additive SQLite schema, rollout-control seed and fail-closed checks, a live diagnose-only classifier loop, MCP diagnostics/reconcile admission, read-only GraphQL projection/subscription, and run-report/release-receipt readback sections. It does not implement the full proposal's safe repair, scheduler capacity reclamation, owned helper reaping, P076-backed side-effect repair, permanent-hold/cooldown repair ledger behavior, or phase-promotion evidence. The canonical `proposal-080` gate fails before Rust tests because required rollout evidence is missing and fixture files still contain placeholder markers.

## Audit Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/080-continuous-stale-execution-reconciliation.md` |
| Report | `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R4.md` |
| Repo root | `/Users/user/Documents/Chainworks Forge` |
| Git HEAD | `0e6482c8` |
| Implementation target | Current dirty worktree; no PR or compare range supplied |
| Proposal state | Active, `draft_refined_for_implementation_review` |
| Prior review reuse | Not reused: `discover_prior_review.py` found no proposal-review artifacts. Prior implementation audits were historical context only. |

## Implementation Surface

P080-owned or P080-relevant surfaces reviewed:

- `control-plane/crates/domain/src/p080.rs`
- `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql`
- `control-plane/crates/db/src/repos/p080.rs`
- `control-plane/crates/daemon/src/main.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/mcp-server/src/tools/p080.rs`
- `control-plane/crates/graphql-server/src/types/p080.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `docs/runbooks/p080-stale-execution-repair.md`
- `docs/evidence/rollout/p080/`
- `docs/evidence/rollout-contract/**/p080-*.json`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`

The worktree is dirty with adjacent proposal work, including P079, P083, P086, Swift, documentation, and test-gate edits. Findings below are scoped to P080 unless explicitly called out as same-tree readiness blockers.

## Proposal Contract Summary

P080 requires a live-daemon reconciliation system that continuously distinguishes useful running work from stale truth, safely repairs retryable non-side-effect work, delegates side-effect ambiguity to P076, owns helper/provider process records before cleanup, exposes stable `p080_readback_v1` across MCP/GraphQL/reports/receipts, persists recurrence/cooldown/idempotency/dedup state across restarts, and proves rollout safety through the `proposal-080`/`p080` gates.

The proposal explicitly excludes Phase 1 SwiftUI/AppKit repair UI, enabled manual `requested_action=hold`, GraphQL repair mutations, blind side-effect retry, arbitrary process termination, and P080-owned `acp_prompt_stale` repair. Static search found no P080 references under `Chainworks Forge/` or the Xcode project, which matches the Phase 1 UI non-goal.

## Reviewer Routing

Selected reviewers:

- `rust_arch_reviewer`: daemon loop, repository boundaries, state-machine shape, and ownership model.
- `rust_reliability_reviewer`: live classification, fail-closed gates, backpressure, restart durability, and repair absence.
- `api_contract_reviewer`: MCP, GraphQL, versioning, cursor, and readback schema fidelity.
- `observability_rollout_reviewer`: metrics, reports, receipts, rollout controls, gates, and soak evidence.
- `rust_security_reviewer`: required by `security_sensitive_diff.py` because the touched surface includes auth, public ingress, parser/resource limits, redaction, filesystem/subprocess/process boundaries, and dependency/security-sensitive areas.

Rejected close alternatives:

- Apple UI/UX review is not a P080 Phase 1 implementation lane. The future read-only diagnostics window is follow-up-owned.
- Performance review is folded into reliability and rollout for this audit because the current P080 implementation exposes budgets, pagination, and loop limits but no active repair throughput behavior.

## Track 1: Requirement Conformance

### REQ-001 Continuous Live Classification

Status: Partially Implemented

The engine starts a P080 loop and runs a bounded tick every 30 seconds. The tick checks `live_disable`, requires `detection_only`, classifies running executions, emits detection metrics, and retires terminal heartbeats in `control-plane/crates/engine/src/executor.rs`.

This is live classification, not full reconciliation. The code explicitly says no ACP reset or scheduler reclamation happens until later phases, and the tick writes diagnose-only readback.

### REQ-002 Durable Ownership Witnesses

Status: Partially Implemented

Migration `086_p080_stale_execution_reconciliation.sql` adds helper leases, helper lease members, reconciliation events, recurrence epochs, dedup, deferral, iteration cursor, readback heartbeats, watchdog, and rollout-control tables. The repository uses running execution, stage, work-item, and session-generation state to classify some stale cases.

The full ownership witness model is incomplete as behavior. Helper lease/process identity tables exist, but active P080 code does not use them to prove helper ownership or perform safe cleanup. Provider session and side-effect witnesses are not wired into a complete repair decision graph.

### REQ-003 Safe Repair Of Retryable Non-Side-Effect Work

Status: Missing

The live loop records `repair_action=diagnose_only`, `decision=diagnosed`, and no `repair_idempotency_key`. The MCP handler returns readback for `diagnose_only`, returns `action_disabled_in_phase` for `hold`, and returns `class_disabled` or `rollout_disabled` for `repair_if_safe`.

The runbook states Phase 1 does not perform ACP reset, scheduler capacity repair, helper reap, manual hold, or permanent-hold clear. This is the central reason the full proposal remains Not Implemented.

### REQ-004 Side-Effect Fail-Closed And P076 Delegation

Status: Partially Implemented

The current implementation is fail-closed because it does not perform side-effect retries. The runbook tells operators not to retry release, publish, git, upload, or distribution work unless P076 reports `retry_safe`.

The positive P080 behavior is missing: no active P080 repair path consumes P076 retry-safe state to repair side-effect-adjacent scheduler ownership drift or `release_side_effect_drift`.

### REQ-005 Provider/Helper Records And Owned Helper Reaping

Status: Missing

Schema exists for helper leases and members, but P080 does not perform owned helper reaping. No active P080 path proves Darwin process identity, parent-chain evidence, command-start identity, PID-reuse prevention, signal escalation, or verified termination before cleanup.

### REQ-006 Stable `p080_readback_v1` Across Surfaces

Status: Implemented for active read-only and diagnose-only surfaces

The domain enum set, DB readback projection, MCP diagnostics/reconcile responses, GraphQL typed objects, run-report section, and release-receipt section all expose the P080 readback shape. This is meaningful contract work, but it is still a readback contract over diagnose-only behavior.

### REQ-007 Authorization, Redaction, Parser Limits, Idempotency, And Dedup

Status: Partially Implemented

The active MCP path enforces resource limits, schema versions, closed top-level/nested fields, run-scope authorization before rollout-state disclosure, live-disable checks, cursor binding, page-size/count budgets, and readback redaction. GraphQL performs read-only auth checks and revalidates subscription authorization and rollout gates.

Dedup and idempotency storage exists, and targeted tests cover pieces of the handler behavior. However, mutating repair, hold, clear, cooldown, recurrence, and replay semantics remain disabled, so the hard restart-stable idempotency contract is not fully proven in active paths.

### REQ-008 Rollout Controls And Fail-Closed Startup

Status: Partially Implemented

Daemon startup seeds and validates the rollout-control matrix and refuses startup on partial/missing required rows. The live loop checks `live_disable` each tick and requires `detection_only`. MCP does not register a rollout-control mutation tool in Phase 1.

Full phase promotion is not implemented or evidenced. The proposal requires readiness artifacts such as phase soak reports before moving beyond detection-only; those artifacts are absent.

### REQ-009 GraphQL Read-Only Projection And Subscription

Status: Implemented for diagnostics

GraphQL exposes read-only P080 diagnostics objects and a polling subscription with initial snapshot rows, row updates/removals, projection-rebuilt events, auth rechecks, rollout gate rechecks, and rate shedding. No GraphQL mutation repair path was found, which matches the proposal non-goal.

### REQ-010 Metrics, Reports, Receipts, And Runbook

Status: Partially Implemented

The code emits P080 detection, deferral, parser, authorization, disabled-action, projection, and subscription metrics on active paths. The DB repository builds run-report and release-receipt P080 sections, and the operator runbook exists.

Rollout evidence is missing. `docs/evidence/rollout/p080/` contains only a README and no JSON soak or readiness reports.

### REQ-011 Gate And Fixture Contract

Status: Missing as completion proof

The `proposal-080` gate is wired and now checks rollout-contract inventory before Rust tests. It fails on the current tree because no rollout evidence JSON exists and most P080 fixture files still contain placeholder markers. Since the proposal requires gate proof, P080 cannot be marked Ready.

## Track 2: Specialist Findings

### READY-001: The Canonical P080 Gate Fails At Fixture/Evidence Preflight

Severity: Critical

`bash ./scripts/test-gate.sh proposal-080` fails before Rust tests:

```text
proposal-080: FAIL - missing P080 rollout evidence JSON under docs/evidence/rollout/p080
proposal-080: FAIL - fixture still contains placeholder evidence: docs/evidence/rollout-contract/operator-readback/p080-full-surface.fixture.json
proposal-080: FAIL - fixture still contains placeholder evidence: docs/evidence/rollout-contract/negative/p080-acp-prompt-stale-delegated-to-p037.json
...
proposal-080: FAIL - fixture still contains placeholder evidence: docs/evidence/rollout-contract/negative/p080-version-negotiation-compatibility.json
```

This is a hard readiness blocker. The gate is now stricter than the prior R3 audit, and it correctly prevents a Ready verdict while rollout evidence and fixture proof remain placeholders.

### REL-001: P080 Still Does Not Repair Stale Executions

Severity: Critical

The implementation is explicitly diagnose-only. `repair_if_safe` does not reclaim scheduler capacity, reset ACP session startup, or settle a stale work item. The live loop writes `diagnosed` events and readback; it does not perform the shared typed repair transition required by the proposal.

### SEC-001: The Process-Control Boundary Is Schema-Only

Severity: Major

The proposal's helper reaping rules are security-sensitive because a bad implementation could signal unrelated user processes. The current tree has helper lease tables and process-control fixtures, but the fixtures are still placeholders and no active P080 code performs verified owned-helper termination. This remains fail-closed, which is safe, but it is not implemented.

### API-001: Future-Capable API Shape Overstates Current Behavior

Severity: Major

The MCP schemas expose `repair_if_safe`, `hold`, `operator_request_dedup_key`, and `p080.clear_permanent_hold.v1`; GraphQL exposes readback fields for repair action, hold reason, side-effect status, and retry/backoff. Current behavior returns disabled/diagnose-only responses for the mutating semantics. This is acceptable scaffolding but not completion evidence.

### OBS-001: Phase Promotion And Soak Evidence Are Absent

Severity: Major

The proposal requires phase readiness artifacts such as `docs/evidence/rollout/p080/phase-1-soak-report.json`. The rollout evidence directory contains only `README.md`. No canary metrics, operator acknowledgements, false-positive review samples, or phase sign-offs were found.

### GATE-001: Targeted Tests Pass, But They Are Not A Substitute For The Proposal Gate

Severity: Major

The following targeted checks passed:

- `CARGO_BUILD_JOBS=1 cargo test -p db p080_seed_rollout_control_inserts_all_classes -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test -p mcp-server p080_repair_if_safe_rollout_enabled_stale_returns_diagnosed -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test -p graphql-server p080_graphql_read_only_operator_surface_policy_query_denied -- --nocapture`

These prove selected DB/MCP/GraphQL slices still compile and behave as intended. They do not satisfy the proposal because the full `proposal-080` gate fails before reaching the Rust test list.

### APPLE-001: No P080 Swift Surface Exists, Which Matches Phase 1

Severity: Informational

`rg` found no P080 references under `Chainworks Forge/` or `Chainworks Forge.xcodeproj`. Because Phase 1 excludes a SwiftUI/AppKit wrapper and future UI is follow-up-owned, this is not a P080 defect.

## Full Implementation Tail Gate

Explicitly excluded or follow-up-owned:

- SwiftUI/AppKit diagnostics window: excluded from Phase 1 and future-owned.
- Manual `requested_action=hold`: explicitly disabled in P080.
- GraphQL repair mutation: explicit non-goal.
- Blind side-effect retry: explicit non-goal.
- Arbitrary process termination: explicit non-goal.
- P080-owned `acp_prompt_stale` repair: delegated to P037.

Still promised by P080 and not complete:

- ACP startup stale repair/reset after diagnosis.
- Scheduler ownership drift repair and capacity reclamation.
- P076-backed side-effect-safe repair admission.
- Owned helper reaping with process identity proof.
- Recurrence, cooldown, permanent-hold, and clear-hold behavior in active paths.
- Concrete rollout/soak evidence and phase promotion artifacts.
- Non-placeholder fixture proof under the canonical gate.

These tail items block closeout.

## Readiness Checklist

| Check | Result | Notes |
|---|---|---|
| Proposal contract mapped | Pass | Goals, non-goals, rollout phases, API contracts, and gates reviewed. |
| Prior review artifact discovery | Pass | Helper found no proposal-review artifacts. |
| Security-sensitive routing | Pass with blockers | Security pass performed; process-control behavior remains unimplemented. |
| Implementation-surface routing | Pass with blockers | P080 relevant lenses covered; dirty tree includes adjacent changes. |
| Swift/UI static scan | Pass | No P080 Swift references found; this matches Phase 1. |
| Canonical proposal gate | Fail | Missing rollout JSON and placeholder fixture markers. |
| Targeted DB/MCP/GraphQL tests | Pass | Selected slices passed; not enough for Ready. |
| Phase rollout evidence | Fail | Only README exists under `docs/evidence/rollout/p080/`. |
| Full implementation tail | Fail | Core repair phases remain disabled or schema-only. |

## Verification Log

- Read `/Users/user/.codex/skills/proposal-implementation-audit/SKILL.md`.
- Ran `report_path.py`; report path is `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R4.md`.
- Ran `discover_prior_review.py`; no prior proposal-review artifacts found.
- Ran `security_sensitive_diff.py`; security-sensitive surfaces were triggered.
- Ran `implementation_surface_fingerprint.py`; required lenses included API contract, architecture, observability/rollout, reliability, security, performance, and Apple UI/UX. Apple UI/UX was rejected for Phase 1 scope after static scan.
- Parsed `docs/proposals/080-continuous-stale-execution-reconciliation.md` proposal markdown and reviewed goals, non-goals, rollout controls, phase promotion, metrics, and test/gate sections.
- Reviewed P080 domain, DB migration/repo, daemon startup, engine loop, MCP handler, GraphQL types/schema, runbook, rollout evidence, and gate docs/scripts.
- Ran `./scripts/test-gate.sh proposal-080 > /tmp/p080-audit-r4-gate.log 2>&1`; first direct invocation exited 137 with an empty log.
- Re-ran `bash ./scripts/test-gate.sh proposal-080 > /tmp/p080-audit-r4-gate2.log 2>&1`; failed deterministically at P080 fixture/evidence preflight.
- Ran `CARGO_BUILD_JOBS=1 cargo test -p db p080_seed_rollout_control_inserts_all_classes -- --nocapture`; passed.
- Ran `CARGO_BUILD_JOBS=1 cargo test -p mcp-server p080_repair_if_safe_rollout_enabled_stale_returns_diagnosed -- --nocapture`; passed.
- Ran `CARGO_BUILD_JOBS=1 cargo test -p graphql-server p080_graphql_read_only_operator_surface_policy_query_denied -- --nocapture`; passed.
- Ran static search for P080 Swift/Xcode references; none found.
