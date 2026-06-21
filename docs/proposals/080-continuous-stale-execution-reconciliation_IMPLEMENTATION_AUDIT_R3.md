# Proposal 080 Implementation Audit R3

## Verdict

Overall Conformance: Not Implemented

Overall Implementation Readiness: Not Ready

Audit confidence: Medium-high. The audit covered the proposal contract, P080 Rust/SQLite/MCP/GraphQL implementation surfaces, rollout evidence, canonical gate wiring, mandatory helper scans, and the repo-owned `proposal-080` gate. Confidence is limited by the failing same-tree gate and the absence of live daemon/soak evidence.

The implemented work is a substantial Phase 1 diagnostics/readback slice, but the active proposal still promises continuous stale-execution reconciliation with safe repair, helper/process cleanup, retry/cooldown/permanent-hold behavior, phase promotion evidence, and full gate coverage. Those promised behaviors are either explicitly disabled, represented only as storage/schema, or not proven.

## Audit Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/080-continuous-stale-execution-reconciliation.md` |
| Report | `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R3.md` |
| Repo root | `/Users/user/Documents/Chainworks Forge` |
| Git HEAD | `0e6482c8` |
| Implementation target | Current dirty worktree; no PR/range was supplied |
| Proposal state | Active, `draft_refined_for_implementation_review` |
| Prior review reuse | Not reused: `discover_prior_review.py` found no prior proposal-review artifacts. Existing implementation audit reports were intentionally not used for reviewer selection. |

## Implementation Target

The P080 implementation surface is primarily:

- `control-plane/crates/domain/src/p080.rs`
- `control-plane/crates/db/migrations/086_p080_stale_execution_reconciliation.sql`
- `control-plane/crates/db/src/repos/p080.rs`
- `control-plane/crates/mcp-server/src/tools/p080.rs`
- `control-plane/crates/graphql-server/src/types/p080.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/daemon/src/main.rs`
- `control-plane/crates/engine/src/executor.rs`
- `docs/runbooks/p080-stale-execution-repair.md`
- `docs/evidence/rollout-contract/**/p080-*.json`
- `docs/evidence/rollout/p080/README.md`
- `scripts/test-gate.sh`

The worktree also contains adjacent P086, ACP, Swift, docs, and script changes. The mandatory helpers therefore triggered a wider surface than P080 itself. Findings below are scoped to P080 unless explicitly called out as same-tree readiness blockers.

## Proposal Contract Summary

P080 asks for a continuous reconciliation system that:

- continuously classifies stale executions without interrupting useful active prompts;
- persists durable execution ownership witnesses and live ownership truth;
- safely repairs retryable, non-side-effect stale work;
- fails closed for side effects unless P076 declares `retry_safe`;
- records provider sessions/helper processes before cleanup and reaps only owned helpers;
- exposes one stable `p080_readback_v1` across MCP, read-only GraphQL, run reports, and release receipts;
- persists cooldown, recurrence, repair idempotency, and operator dedup across restarts;
- ships behind strict rollout controls and the `proposal-080` gate.

The proposal explicitly excludes a Phase 1 SwiftUI/AppKit wrapper and manual `requested_action=hold`. Follow-up proposal P099 owns the future read-only diagnostics window, and P098 owns manual hold/clear-hold semantics. Those excluded areas are not counted as P080 misses.

## Reviewer Selection

Selected reviewers:

- `rust_arch_reviewer`: daemon, repository, state-machine, and ownership-boundary fit.
- `rust_reliability_reviewer`: stale classification, repair loop, backpressure, fail-closed behavior, and restart durability.
- `api_contract_reviewer`: MCP/GraphQL/readback schema fidelity and versioning.
- `observability_rollout_reviewer`: rollout controls, metrics, reports, receipts, gate coverage, and soak artifacts.
- `rust_security_reviewer`: required by `security_sensitive_diff.py` for auth, public ingress, parser/resource limits, redaction, and process-boundary surfaces.

Rejected close alternatives:

- `macos_ui_reviewer` / `apple_ux_reviewer`: P080 Phase 1 has no SwiftUI surface by proposal, and `rg` found no P080 references under `Chainworks Forge/` or the Xcode project. Future UI is owned by P099.
- `rust_performance_reviewer`: P080's implemented performance surface is limited to loop budgets, request limits, pagination, and count budgets; this was folded into reliability and observability. This scoped handling is not used to claim Ready.
- `product_reviewer`: no separate product-facing decision surface exists beyond the rollout/operator-readback contract.

Mandatory helper coverage:

- `security_sensitive_diff.py` triggered and required an independent security pass before any Ready verdict. The security pass was performed for the active P080 MCP/GraphQL/database surfaces and the promised process-control boundary.
- `implementation_surface_fingerprint.py` required `api-contract`, `apple-ui-ux`, `architecture`, `observability-rollout`, `performance`, `reliability`, and `security` lenses over the full dirty tree. P080-specific Apple UI/UX was a non-implementation static check; the full dirty tree remains not Ready regardless because the canonical gate fails.

## Track 1: Proposal Conformance

### REQ-001 Continuous Live Classification

Status: Partially Implemented

The daemon starts a P080 loop and applies per-tick interval/deadline limits in `control-plane/crates/engine/src/executor.rs:5485`. The tick reads `live_disable`, requires `detection_only`, classifies running executions, emits metrics, and retires terminal heartbeats in `control-plane/crates/engine/src/executor.rs:5550`.

The implementation is still diagnose-only. The code comments state that no ACP session reset or scheduler capacity reclamation happens until later rollout phases in `control-plane/crates/engine/src/executor.rs:5495` and `control-plane/crates/engine/src/executor.rs:5540`. This satisfies a detection/readback subset, not the continuous reconciliation behavior promised by the full proposal.

### REQ-002 Durable Ownership Witnesses

Status: Partially Implemented

The migration adds P080 helper lease, helper member, reconciliation event, heartbeat/readback, dedup, cursor, watchdog, and rollout-control tables. The repository has classifier and readback code. However, active classification is centered on running execution rows and readback projection; the proposal's full ownership model for helper processes, provider sessions, P037 prompt-stale delegation, and P076 side-effect state is not exercised as a complete repair decision graph.

### REQ-003 Safe Repair Of Retryable Non-Side-Effect Work

Status: Missing

The active worker records `repair_action=diagnose_only` and `decision=diagnosed`. It explicitly leaves `repair_idempotency_key` null and writes an operator message that no actual ACP reset occurred in `control-plane/crates/engine/src/executor.rs:5706`.

The MCP handler also declares Phase 1 behavior where `repair_if_safe` and `hold` remain rollout-disabled and `clear_permanent_hold` remains disabled until Phase 5 in `control-plane/crates/mcp-server/src/tools/p080.rs:1`. The runbook confirms Phase 1 does not perform ACP reset, scheduler capacity repair, helper reap, manual hold, or permanent-hold clear in `docs/runbooks/p080-stale-execution-repair.md:1`.

### REQ-004 Side-Effect Fail-Closed / P076 Delegation

Status: Partially Implemented

The current implementation does not retry side effects, which is the safe default. The runbook instructs operators not to retry release/publish/git/upload/distribution work unless P076 reports `retry_safe` in `docs/runbooks/p080-stale-execution-repair.md:11`.

The missing side is positive integration: there is no active P080 repair path that consumes P076 retry-safe state to perform a guarded repair or classify `release_side_effect_drift` through the promised automated decision flow.

### REQ-005 Provider/Helper Records And Owned Helper Reaping

Status: Missing

The migration provides helper lease/member storage, but the implementation does not perform owned helper process reaping. The proposal's process-control security requirements are therefore not implemented as behavior: no active P080 path proves parent/command-start identity checks, lease ownership revalidation, or positive PID kill/reap logic. The runbook states helper reap is unavailable in Phase 1.

### REQ-006 Stable `p080_readback_v1` Across Surfaces

Status: Implemented for active read-only surfaces

The domain, DB, MCP, GraphQL, run-report, and release-receipt surfaces expose a stable P080 readback shape. The MCP handlers register diagnostics/reconcile/clear tools in `control-plane/crates/mcp-server/src/tools/p080.rs:19`. GraphQL has P080 typed readback/query/subscription plumbing. Run reports and release receipts include P080 reconciliation sections.

This implemented readback is mostly a diagnostics projection. It should not be interpreted as evidence that the underlying repair behavior is complete.

### REQ-007 Authorization, Redaction, Parser Limits, Idempotency, And Dedup

Status: Partially Implemented

The active read-only and diagnose-only lanes have meaningful checks: action-level capability ordering, live-disable ordering, request-shape validation, cursor validation, page-size/count budgets, and redaction/write validation. The MCP handler calls out the required auth-before-rollout ordering in `control-plane/crates/mcp-server/src/tools/p080.rs:156`.

Dedup and repair idempotency storage exist, and tests target duplicate-key behavior. But mutating repair/hold/clear behavior is disabled, so the proposal's restart-stable repair idempotency, cooldown, recurrence, and operator replay semantics are not fully exercised in production paths.

### REQ-008 Rollout Controls And Fail-Closed Startup

Status: Partially Implemented

Daemon startup seeds and validates rollout control. The live loop reads `live_disable` on every tick and treats missing state as fail-closed. The MCP surface disables rollout-control mutation in Phase 1; `rollout_control.set` is not registered in `control-plane/crates/mcp-server/src/tools/p080.rs:8`.

This implements the early fail-closed posture, but not the full phase-promotion mechanism or the later class enablement behavior required by the proposal.

### REQ-009 GraphQL Read-Only Projection And Subscription

Status: Implemented for read-only diagnostics

The GraphQL schema exposes read-only P080 query/subscription surfaces with closed enums, auth checks, cursor handling, and projection mapping. No GraphQL mutating repair path was found, matching the proposal non-goal.

### REQ-010 Metrics, Reports, Receipts, And Operator Runbook

Status: Partially Implemented

Metrics are emitted from active code paths; run reports and release receipts include P080 sections; the runbook exists. The missing part is rollout evidence. `docs/evidence/rollout/p080/README.md` only describes expected per-phase soak reports. The directory contains no actual phase soak or readiness report beyond that README.

### REQ-011 Gate And Fixture Contract

Status: Missing as a completion proof

The repo has many P080 rollout-contract fixture files under `docs/evidence/rollout-contract/`, including process-control, phase-promotion, migration, GraphQL, MCP, redaction, and run-report fixtures. The `proposal-080` gate in `scripts/test-gate.sh:7200` validates the migration file and runs a curated Rust test list from `scripts/test-gate.sh:321`, but it does not enforce all named proposal fixture artifacts as first-class gate inputs.

More importantly, the canonical same-tree gate failed during this audit. A proposal cannot be marked Ready while its own gate fails.

## Track 2: Specialist Findings

### READY-001: Canonical `proposal-080` Gate Fails On The Current Tree

Severity: Critical

`./scripts/test-gate.sh proposal-080` failed. The failing test invocation reported a Rust compile error in the `engine` crate while compiling the MCP-server test dependency:

```text
error: recursion limit reached while expanding `$crate::json_internal!`
  --> crates/engine/src/executor.rs:7531:36
help: consider increasing recursion limit by adding #![recursion_limit = "256"] to engine
proposal-080: FAIL - mcp-server::p080_http_rejects_duplicate_keys_before_auth_and_with_escaped_method returned a non-zero exit
```

The same source area contains a large `serde_json::json!` receipt construction in `control-plane/crates/engine/src/executor.rs:7534`. This is a same-tree readiness blocker even if the compile failure is caused by adjacent P086 work rather than P080.

### REL-001: P080 Repair Remains Diagnosed-Only

Severity: Critical

The worker and MCP code repeatedly state that no actual ACP reset, scheduler capacity repair, helper reaping, permanent-hold clear, or repair action occurs in the implemented Phase 1 path. This is not a minor follow-up: safe repair and reconciliation are central goals of the proposal.

Evidence:

- `control-plane/crates/engine/src/executor.rs:5495` says no actual ACP reset or scheduler reclamation occurs until Phase 3+.
- `control-plane/crates/engine/src/executor.rs:5706` writes diagnose-only readback and no repair key.
- `control-plane/crates/mcp-server/src/tools/p080.rs:11` says `repair_if_safe` and `hold` remain disabled.
- `docs/runbooks/p080-stale-execution-repair.md:3` states Phase 1 is detection/readback only.

### SEC-001: Promised Helper Process-Control Boundary Is Not Implemented

Severity: Major

The proposal has security-sensitive process cleanup requirements around owned helper leases and safe process termination. The implementation includes tables and schemas, but active P080 code does not prove process identity, command start, parent lineage, or owned-process termination before cleanup. This is acceptable for a detection-only phase, but not for full proposal conformance.

### API-001: Schema/Readback Is Stronger Than Behavior

Severity: Major

The MCP and GraphQL schemas expose future-capable tools and fields (`repair_if_safe`, `hold`, clear permanent hold, idempotency keys, cooldown/backoff, side-effect statuses), but the active behavior returns rollout-disabled/action-disabled or diagnose-only paths. This is fine as versioned forward-compatible scaffolding, but the proposal cannot be closed as implemented based on schema presence alone.

### OBS-001: Phase Promotion And Soak Evidence Are Absent

Severity: Major

The proposal requires controlled phase promotion and rollout evidence. `docs/evidence/rollout/p080/README.md` describes what reports typically include, but `find docs/evidence/rollout/p080 -maxdepth 1 -type f` returns only the README. No canary metrics, false-positive review samples, soak-duration results, or readiness sign-off files were found.

### GATE-001: The Proposal Gate Does Not Enforce The Full Fixture Contract

Severity: Major

`scripts/test-gate.sh:321` lists DB, MCP, and GraphQL unit tests, and `scripts/test-gate.sh:7200` wires the `proposal-080` gate. The gate does not directly validate the named JSON fixture set under `docs/evidence/rollout-contract/negative/` and `docs/evidence/rollout-contract/operator-readback/`. This leaves a gap between proposal acceptance criteria and the durable gate.

### APPLE-001: No P080 Swift Surface Is Implemented, Which Matches Phase 1 Scope

Severity: Informational

Static search found no P080 references under `Chainworks Forge/` or `Chainworks Forge.xcodeproj`. Because the proposal excludes Phase 1 SwiftUI/AppKit UI and P099 owns the future diagnostics window, this is not a conformance gap for P080.

## Full Implementation Tail Gate

Explicitly excluded or follow-up-owned:

- Manual hold/clear-hold UI and semantics: excluded from P080 Phase 1 and owned by P098.
- Read-only macOS diagnostics window: future UI owned by P099.
- GraphQL mutation repair surface: explicit non-goal.
- Arbitrary process termination and blind side-effect retry: explicit non-goals.
- `acp_prompt_stale` repair: delegated to P037 by proposal.

Still promised by P080 and not owned by a concrete follow-up found during this audit:

- actual ACP startup stale repair/reset after diagnosis;
- scheduler ownership drift repair and capacity reclamation;
- helper orphan drift repair/reaping with process-control proof;
- release-side-effect drift integration with P076 `retry_safe` for guarded retry;
- recurrence/cooldown/permanent-hold repair ledger behavior in active paths;
- phase-promotion evidence and soak sign-off;
- gate enforcement of the full fixture contract.

These residual items block any Implemented/Ready closeout.

## Readiness Checklist

| Check | Result | Notes |
|---|---|---|
| Proposal contract mapped | Pass | Goals, non-goals, follow-ups, rollout phases, API contracts, and tests were reviewed. |
| Prior proposal-review artifacts | Pass | None found by helper; no reviewer reuse claimed. |
| Mandatory security helper | Pass with blockers | Security pass performed; process-control behavior remains unimplemented. |
| Mandatory surface helper | Pass with scoped blockers | Full dirty tree triggers many lenses; P080-specific UI is non-goal; same-tree gate still fails. |
| Static Swift P080 scan | Pass | No P080 Swift references found. |
| Canonical proposal gate | Fail | `./scripts/test-gate.sh proposal-080` fails with Rust compile recursion-limit error in `engine`. |
| Soak/phase rollout evidence | Fail | Only rollout README found; no per-phase reports. |
| Full implementation tail | Fail | Core repair phases remain missing or disabled. |

## Verification Log

- Read `/Users/user/.codex/skills/proposal-implementation-audit/SKILL.md`.
- Ran `report_path.py`; generated report path is `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R3.md`.
- Ran `discover_prior_review.py`; no prior proposal-review artifacts found.
- Checked repo state and HEAD (`0e6482c8`).
- Reviewed P080 proposal status, goals, non-goals, contracts, rollout phases, success criteria, and gate requirements.
- Reviewed P098/P099 for concrete follow-up ownership of manual hold and future diagnostics UI.
- Ran `security_sensitive_diff.py --root ... --json`; security-sensitive categories triggered and required security pass.
- Ran `implementation_surface_fingerprint.py --root ... --json`; required lenses were captured and scoped.
- Reviewed P080 domain, migration, DB repo, MCP tool, GraphQL, daemon startup, engine loop, reports/receipts, runbook, rollout evidence, and gate script surfaces.
- Ran static search for P080 references under Swift app/project; no matches.
- Ran `find docs/evidence/rollout/p080 -maxdepth 1 -type f`; only `README.md` was present.
- Ran `find docs/evidence -path '*p080*' -maxdepth 5 -type f`; rollout-contract fixture files exist.
- Ran `./scripts/test-gate.sh proposal-080`; gate failed with the compile recursion-limit error noted above.

## Recommended Closeout Path

1. Fix the same-tree `proposal-080` gate failure first; no readiness verdict can pass while the canonical gate does not compile.
2. Decide whether P080 is meant to close as Phase 1 diagnostics/readback only or as the full proposal. If Phase 1 only, revise/retire the proposal contract and create explicit follow-ups for Phase 2-5 repair, helper reaping, P076 retry-safe integration, cooldown/permanent-hold behavior, and phase promotion.
3. If full P080 remains in scope, implement and prove the missing repair paths, process-control boundary, and restart-stable repair ledger.
4. Wire the `proposal-080` gate to enforce the proposal's named fixture artifacts and add real per-phase rollout/soak evidence.
5. Re-run `./scripts/test-gate.sh proposal-080` and the relevant full gate before requesting closeout.
