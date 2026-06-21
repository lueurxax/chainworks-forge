# P080 Continuous Stale Execution Reconciliation - Implementation Audit R8

Proposal: `docs/proposals/080-continuous-stale-execution-reconciliation.md`
Proposal revision: `p080-refined-2026-06-02-r28`
Audit date: 2026-06-21
Repository HEAD: `0e6482c82b58`
Assessed state: current working tree
New report path: `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R8.md`

## Verdict

**Partially implemented. Not ready for implementation closeout.**

The current implementation has a real, passing phase-scoped gate for P080: DB migration/rollout seed shape, MCP diagnostics and admission checks, read-only GraphQL admission coverage, detection-only readback, durable operator-request dedup for the supported repair path, and Phase 2 `acp_startup_stale` requeue repair all passed under `./scripts/test-gate.sh proposal-080`.

That passing gate does not prove the full proposal. The repository documentation and rollout evidence both describe the shipped scope as detection plus a single `acp_startup_stale` repair path, while the proposal requires a broader ownership registry, full stale-class vocabulary, commit-time predicate and side-effect revalidation, atomic mutation/projection/dedup semantics, metrics, and phase-promotion evidence.

## Review Routing

Selected reviewers:

- `chainworks_execution_truth_reviewer` - required repo-local reviewer for durable run, stage, work item, execution, and readback truth.
- `rust_reliability_reviewer` - reconciliation loop, crash consistency, recurrence, cooldown, retry, and idempotency behavior.
- `api_contract_reviewer` - MCP, GraphQL, run-report, release-receipt, cursor, and readback contracts.
- `rust_security_reviewer` - authorization order, parser limits, dedup fences, redaction, and process-signaling safety.
- `observability_rollout_reviewer` - rollout controls, gates, dashboards, metrics, and promotion evidence.

Rejected close alternatives:

- `rust_arch_reviewer` - displaced by the repo-local execution-truth reviewer plus reliability/API/security coverage.
- `macos_ui_reviewer`, `apple_ux_reviewer`, `apple_arch_reviewer` - not selected because the proposal explicitly says Phase 1 has no new SwiftUI diagnostic visual surface.
- `rust_performance_reviewer` - parser/resource and backpressure risks are covered by reliability and security for this audit.
- `product_reviewer` and `docs_reviewer` - rollout, evidence, and runbook readiness are covered by `observability_rollout_reviewer` within the five-reviewer cap.

No prior implementation-review artifact was discovered for this proposal by the review discovery tool in this workspace state.

## Evidence Summary

Verified working behavior:

- `./scripts/test-gate.sh proposal-080` passed in this audit session.
- The P080 gate list covers DB, MCP, and one GraphQL authorization test in `scripts/test-gate.sh:323-374`.
- The stable gate documentation explicitly defines this as "phase-scoped detection plus Phase 2 `acp_startup_stale` repair proof" in `docs/reference/test-gates.md:2435-2447`, and confirms that scheduler ownership repair, helper reap, side-effect-adjacent repair, manual hold, and permanent-hold clear remain disabled in `docs/reference/test-gates.md:2488-2492`.
- The rollout proof records implemented classes as only `detection_only` and `acp_startup_stale`, with scheduler, helper, release side-effect, and permanent-hold clear disabled in `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:2-13`.
- The runbook states the same phase-scoped status in `docs/runbooks/p080-stale-execution-repair.md:1-3`.

## Findings

### P1 - The ownership registry and classifier do not implement the proposal-required stale-truth model

The proposal requires a single ownership registry that joins work items, agent executions, session generations, ACP provider session state, helper leases, runtime invocation rows, P037 prompt supervision, and the P076 side-effect ledger, then classifies rows into the full closed vocabulary including `acp_prompt_stale`, `helper_orphan_drift`, `release_side_effect_drift`, `ambiguous_owner`, and `unknown`.

The implementation only scans `agent_executions`, `stage_executions`, `session_generations`, and a `work_items` subquery. See `control-plane/crates/db/src/repos/p080.rs:189-210`. Its classifier emits only `acp_startup_stale`, `scheduler_ownership_drift`, `warmup_pending`, and `useful`; it never emits prompt, helper, release side-effect, ambiguous-owner, or unknown classifications. See `control-plane/crates/db/src/repos/p080.rs:239-254`. The generated readback also hard-codes `repair_action=diagnose_only`, `executor_reregistration_state=expected`, `rollout_disablement=phase_not_reached`, and `side_effect_status=not_applicable` in `control-plane/crates/db/src/repos/p080.rs:257-276`.

This matches the phase-scoped rollout evidence, not the full proposal. The disabled class list in `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:2-13` confirms the missing coverage is intentional current scope.

Impact: P080 cannot yet be treated as the continuous stale-execution reconciliation layer described by the proposal. It cannot safely diagnose or reconcile helper or release side-effect drift, cannot delegate prompt-stale truth to P037 in the classifier, and cannot provide the operator with complete `p080_readback_v1` truth across the declared stale-class vocabulary.

Required before closeout: extend the ownership registry to all proposal-owned sources, emit the full closed stale-class vocabulary, wire P037/P076 truth into classification and readback, and add gate coverage for each class and unknown/ambiguous cases.

### P1 - `repair_if_safe` does not enforce commit-time predicate, side-effect, and dedup atomicity

The proposal requires mutating repair to parse and authorize first, then perform dedup lookup, predicate/side-effect revalidation, event write, state mutation, projection write, and dedup response commit as one typed transition under the write gateway.

The handler includes `expected_predicate_hash` only in the request fingerprint at `control-plane/crates/mcp-server/src/tools/p080.rs:1285-1293`. It is not passed to the DB repair function at `control-plane/crates/mcp-server/src/tools/p080.rs:1331-1337`, and the DB repair function does not compare an expected predicate hash against current state. It only checks that existing readback has `running_truth = stale_suspected` in `control-plane/crates/db/src/repos/p080.rs:1766-1785`, then requeues the work item through `p080_requeue_running_invoke_agent_by_id_tx` in `control-plane/crates/db/src/repos/p080.rs:1787-1801`.

The actual requeue checks only work-item identity, run, stage, kind, and running status before setting the work item back to pending in `control-plane/crates/db/src/repos/work_items.rs:739-807`. It does not query the P076 side-effect ledger or require retry-safe side-effect truth. The repair readback itself records `side_effect_status = not_applicable` in `control-plane/crates/db/src/repos/p080.rs:1851-1870`.

The repair event and readback are committed inside one DB transaction in `control-plane/crates/db/src/repos/p080.rs:1875-1925`, but the operator-request dedup row is inserted afterward by the MCP handler in `control-plane/crates/mcp-server/src/tools/p080.rs:1360-1378`. A crash or process kill after the repair transaction but before that dedup insert would leave a repaired tuple without the proposal-required durable replay record for the operator request.

Impact: an operator can submit a request whose stale predicate changed after diagnosis and still attempt repair, because the expected predicate is only part of dedup fingerprinting rather than a commit-time guard. Replay semantics are also weaker than the proposal because the mutation and dedup response are not atomically committed together.

Required before closeout: move `repair_if_safe` into a single typed DB/write-gateway transaction that locks and revalidates the target tuple, compares `expected_predicate_hash` when present, checks P076 side-effect status for side-effect-adjacent work, writes the repair event, mutates scheduler/runtime state, updates projection, advances recurrence/cooldown, and stores the dedup response atomically.

### P1 - Rollout evidence, metrics, and the canonical gate prove a smaller contract than the proposal success criteria

The canonical gate and stable documentation now describe a narrower implementation than the proposal. `docs/reference/test-gates.md:2435-2447` frames `proposal-080` as a phase-scoped DB/MCP/GraphQL proof. `docs/reference/test-gates.md:2488-2492` explicitly says scheduler ownership, helper reap, side-effect-adjacent repair, manual hold, and permanent-hold clear remain disabled. The rollout evidence agrees: `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:10-13` lists only `detection_only` and `acp_startup_stale` as implemented.

The metrics vocabulary is declared in `control-plane/crates/db/src/metrics.rs:236-263`, but the implementation does not emit the full set. For example, repository search found `stale_execution_repaired_total`, `p080_recurrence_epoch_advanced_total`, `p080_permanent_hold_engaged_total`, `p080_permanent_hold_cleared_total`, and `helper_reap_signal_escalation_total` only in the required-metrics declaration, not in runtime emission code. The live loop emits detection metrics for only `acp_startup_stale` and `scheduler_ownership_drift` in `control-plane/crates/engine/src/executor.rs:5613-5627`, and projection metrics only for `lane=mcp,status=valid` in `control-plane/crates/engine/src/executor.rs:5767-5772`.

Impact: the green gate is useful for the shipped slice, but it cannot support the proposal's success criteria or closeout. The proposal asks for strict rollout-contract proof, phase promotion artifacts, metrics and alerts, runbook drills, and acceptance tests covering the full reconciliation lifecycle.

Required before closeout: either narrow the proposal to the implemented phase-scoped contract or implement and test the remaining phase contracts. For full closeout, add the missing metric emitters, dashboard evidence, phase soak/promote artifacts, crash cutpoint proofs, helper/process-control proofs, side-effect safety proofs, and expanded GraphQL/run-report/release-receipt parity tests.

## Requirement Matrix

| Proposal area | Status | Evidence |
|---|---|---|
| SQLite migration shape and rollout-control seed | Implemented for the phase-scoped slice | Gate passed; migration and rollout seed covered by `scripts/test-gate.sh:323-341` |
| Fail-closed `live_disable` and `detection_only` gating | Implemented | Live loop checks `live_disable` first and refuses missing/unreadable rows in `control-plane/crates/engine/src/executor.rs:5550-5581`; detection-only gates classifier writes in `control-plane/crates/engine/src/executor.rs:5584-5599` |
| Ownership registry and stale-class classifier | Partial | Classifier joins only execution/session/work-item state in `control-plane/crates/db/src/repos/p080.rs:189-210`; emits only a subset in `control-plane/crates/db/src/repos/p080.rs:239-254` |
| Live reconciliation loop | Partial | Loop runs every 30s with a 20s tick timeout and diagnoses `acp_startup_stale` rows in `control-plane/crates/engine/src/executor.rs:5502-5537` and `control-plane/crates/engine/src/executor.rs:5655-5730`; it does not perform helper, side-effect, or scheduler repair |
| MCP diagnostics and admission surface | Partial | Diagnostics, diagnose-only, auth, duplicate-key, cursor, and Phase 2 acp-startup tests pass; `hold`, `clear_permanent_hold`, and non-acp-startup repairs remain disabled in `control-plane/crates/mcp-server/src/tools/p080.rs:1118-1265` |
| Mutating repair safety | Partial/blocking | Requeues a running invoke-agent item but does not atomically commit operator dedup or enforce expected predicate/P076 side-effect safety; see Finding 2 |
| GraphQL read-only projection | Partial | Gate covers read-only operator policy only in `scripts/test-gate.sh:374`; broader GraphQL pagination/subscription/readback parity is not proven by the P080 gate |
| Run report and release receipt readback | Partial | Code paths exist, but the gate is phase-scoped and projection metrics are emitted only for the MCP lane |
| Redaction and parser/resource limits | Mostly implemented for MCP slice | Tests cover duplicate-key scanning, schema version, malformed cursor, nested-filter validation, and redaction in `scripts/test-gate.sh:332-372` |
| Metrics, alerts, dashboards, and rollout promotion | Partial/blocking | Required metric names are declared in `control-plane/crates/db/src/metrics.rs:236-263`, but several rollout-critical metrics have no runtime emitters; rollout proof is phase-scoped only |
| Helper process signaling/reaping | Not implemented in current phase | Runbook and rollout proof state helper process signaling is not enabled in `docs/runbooks/p080-stale-execution-repair.md:1-3` and `docs/evidence/rollout/p080/phase-scoped-same-tree-proof.json:40-48` |
| P037 prompt and P076 side-effect delegation | Not implemented in classifier/repair path | Classifier and repair code do not join P037 or P076 state; repair readback uses `side_effect_status=not_applicable` |

## Gate Result

Command:

```bash
./scripts/test-gate.sh proposal-080
```

Result: passed.

Important limitation: this is a passing gate for the documented phase-scoped implementation, not a full proposal closeout gate.

## Closeout Decision

Do not delete or retire the proposal yet. The implementation can be described as a validated phase-scoped subset, but full P080 remains open until the ownership registry, complete stale-class behavior, commit-time safety invariants, metrics, and rollout-promotion evidence are implemented and covered by the canonical gate.
