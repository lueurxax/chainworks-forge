# Implementation Closeout Readiness

Implementation closeout readiness is the active authority for moving a proposal-backed run from implementation review toward manual release, code refinement, non-code handoff, or operator decision. It prevents a run from looking release-ready only because code self-assessment has no remaining blocking code tasks.

## Authority

`implementation_closeout_readiness_v1` is the state-9 manual-release decision contract. Transition evaluation reads the active SQLite contract rows and never treats exported JSON, GraphQL projections, MCP projections, or raw artifact files as transition authority.

The active contracts are:

| contract | purpose | path |
| --- | --- | --- |
| `proposal_gate_result_v1` | Current governed proposal-gate result or waiver | `review/proposal-gate-result.json` |
| `implementation_closeout_readiness_v1` | Current closeout status and routing decision | `review/implementation-closeout-readiness.json` |
| `implementation_closeout_inputs_v1` | Derived diagnostic inputs for operator readback | `review/implementation-closeout-inputs.json` |
| `closeout_handoff_status_v1` | Derived owner and handoff projection | `review/closeout-handoff-status.json` |

`implementation_review_summary_v1.status` is an input to the closeout decision. It is not direct transition authority.

## Decisions

Readiness status values are `ready`, `ready_with_risks`, `handoff_required`, `not_ready`, `blocked`, `invalid`, and `unknown`.

Routing decisions are `enter_manual_release`, `return_to_code_refine`, `await_non_code_handoff`, `await_gate_definition`, `await_operator_decision`, and `block_with_evidence`.

The decision matrix separates code-owned blockers from non-code blockers:

- Code blockers with remaining refine budget route to code refinement.
- Repeated identical code blockers can trigger a soft convergence checkpoint and route to operator decision instead of looping silently.
- Code blockers with exhausted budget route to operator decision.
- Handoff, release-owner, waiver, rollout, and risk-settlement work routes to the owner surface instead of invoking `code_writer`.
- Missing gate definitions await gate definition.
- Malformed, unauthorized, stale, superseded, unavailable, or fingerprint-mismatched inputs fail closed with diagnostic evidence.

## Modes

Each run freezes a closeout readiness mode at admission:

| mode | behavior |
| --- | --- |
| `advisory` | Synthesizes and exposes readiness, but caps transition-causing decisions to `await_operator_decision` with a diagnostic reason. |
| `enforcement` | Requires `implementation_closeout_readiness_v1.decision == "enter_manual_release"` before manual release. |
| `legacy_fallback` | Diagnostic compatibility mode for legacy snapshots that lack explicit metadata. |

Unknown, malformed, or conflicting mode values remain visible as diagnostic states and cannot enter manual release without an explicit valid decision.

## Gate Settlement

Operators settle the proposal gate through a governed action command with `execute`, `import_receipt`, and `waive` actions. The managed executor records lineage, capability, journal identity, worktree and workflow fingerprints, source generation ids, timing, exit code, executor version, stdout/stderr digests, and validation status.

Unmanaged file-only receipts are rejected as invalid or unauthorized active inputs.

## State-9 Transaction

State-9 closeout uses a single transaction helper that activates the gate generation and readiness generation together, persists summary rows, rebuilds derived projections, commits, and only then returns data to transition evaluation.

Crash behavior is fail-closed:

- A crash before commit leaves the previous active truth authoritative.
- A crash after commit exposes a coherent gate/readiness pair.
- Projection rebuild failures are logged and retried from active SQLite truth.

## Fingerprint And Risk Lineage

The closeout fingerprint captures proposal/freeze digest, run and stage identity, workflow digest, worktree head, dirty or changed-file digest, source generation ids, and contract version. If fingerprint computation exceeds the 5,000 ms latency budget, readiness fails closed with an unavailable fingerprint diagnostic.

`ready_with_risks` can enter manual release only when each risk has typed accepted lineage or governed settlement. Free-form risk text alone is never enough.

Accepted lineage sources are controlled risk rows, release-owner decisions, governed waivers, or governed settlements.

## Readback

All readers use the same closeout readiness accessor:

- Transition evaluation reads active SQLite truth.
- GraphQL exposes `implementationCloseoutReadinessSummary` and compatibility `closeoutReadinessSummaryJson`.
- MCP run detail/list/report readbacks expose `implementation_closeout_readiness_summary` and compatibility `closeout_readiness_summary`.
- Run-state projections include the current summary and an operator-facing `fingerprint_hash`.
- The macOS run surface renders a read-only closeout readiness card with copy, diagnostics/readback, backlink, focus-return, recovery guidance, and accessibility announcements. It does not perform receipt import, waiver, settlement, or recovery writes.

## Rollout And Evidence

Rollout remains advisory until release-owner cutover criteria pass. The dependency checklist, metric ledger, go/no-go decision payload, rollback rule, and UI runtime proof are maintained as reference evidence:

- [p077-rollout-dependency-evidence.md](p077-rollout-dependency-evidence.md) is a retained historical alias evidence file for the rollout store, dependency checklist, metrics, and rollback rule.
- [p077-closeout-readiness-ui-evidence.md](p077-closeout-readiness-ui-evidence.md) is a retained historical alias evidence file for the read-only macOS surface, tokens, contrast, focus, recovery, and accessibility proof.

The rollout store uses retained historical alias tables `p077_rollout_metric_events`, `p077_rollout_decisions`, and `p077_rollout_advisory_migrations`.

## Validation

The retained historical alias gate `proposal-077|p077` validates the Rust domain, DB, engine, GraphQL, MCP, proof-gate, rollout-store, and static evidence checks.

The retained historical alias gate `proposal-077-ui|p077-ui` is the remote macOS runtime proof for the read-only UI surface.

Use retained historical alias `./scripts/test-gate.sh proposal-077` for local closeout readiness changes. Remote UI proof remains remote-host only by repository policy.
