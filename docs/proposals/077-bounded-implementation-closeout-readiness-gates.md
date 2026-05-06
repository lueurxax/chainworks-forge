# Proposal 077: Bounded Implementation Closeout Readiness Gates

| Field | Value |
|---|---|
| Date | 2026-04-29 |
| Status | Implemented |
| Author | Andrey Khasanov |
| Depends on | [052-orchestrator-loop-budget-source-of-truth.md](052-orchestrator-loop-budget-source-of-truth.md), [059-release-evidence-gates-and-approval-payload-contract.md](059-release-evidence-gates-and-approval-payload-contract.md), [073-stability-freeze-regression-budget-and-refactor-plan.md](073-stability-freeze-regression-budget-and-refactor-plan.md), [output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md#implementation-self-assessment-and-handoff) |
| Scope | Add bounded, machine-readable readiness rules that decide whether an implementation run may leave implementation review for manual release, handoff, or completed stop-state. |
| Goal | Prevent runs from presenting incomplete proposal implementation as ready, without turning implementation review into an unbounded code/review loop. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-077|p077`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context and Motivation

The UI action boundary rollout exposed a workflow gap: a run could reach an apparently finished state even though a later implementation audit reported `Not Implemented / Not Ready`.

The concrete symptoms were:

- no canonical UI action boundary gate existed, so the run had no proposal-specific proof lane;
- implementation self-assessment and release transition truth were weaker than the proposal contract;
- a late audit could discover missing behavior after the run had already behaved as if implementation was complete;
- manual PR review remained separate, so GitHub/Copilot findings were not the cause of the orchestration gap.

The desired operating model is stricter than today's behavior but still bounded:

- runs should not claim implementation readiness while proposal-critical behavior is missing;
- code agents should not be sent into endless review/refine cycles after code-owned work is exhausted;
- final GitHub PR review remains a manual external quality gate for now, not an orchestrator-owned workflow blocker.

---

## 2. Problem Statement

### 2.1 `implementation_self_assessment_v2` is necessary but not sufficient

The existing self-assessment contract correctly separates code-owned tasks from non-code handoff, but a writer can still report `complete` while proposal-specific gate or audit evidence is absent.

That is not enough for implementation closeout. A proposal implementation run needs a second decision layer that evaluates proposal readiness from validated review artifacts, gates, and known handoff scope.

### 2.2 Focused gates are optional in practice

Many proposals require `./scripts/test-gate.sh proposal-XXX`, but the workflow can proceed even when:

- no proposal gate is registered;
- the registered gate was not run on the same tree;
- the gate is a contract/scaffold gate but not a closeout readiness gate;
- a stricter readiness gate exists but is not attached to the implementation workflow.

### 2.3 Audit verdicts are not transition authority

Implementation audits can report `Not Ready`, `Partial`, or `Ready with Risks`, but the workflow does not have a typed policy for how those verdicts affect transition from implementation review to release or handoff.

This creates two bad outcomes:

- optimistic closeout: `Not Ready` runs appear ready;
- churn: the system keeps asking code agents to fix non-code or release-owner decisions.

### 2.4 Manual PR review is intentionally outside this automation boundary

GitHub PR review, Copilot review, and operator review comments are still handled manually.

The orchestrator must not require all PR comments to have a disposition before release. That policy may be revisited later, but P077 deliberately excludes it to avoid premature automation and noisy review loops.

---

## 3. Scope

P077 includes:

- a typed implementation-closeout readiness decision model;
- rules for combining self-assessment, implementation audit, proposal gates, docs/security/prepush reports, and release evidence handoff;
- a bounded refinement policy that prevents infinite code/review loops;
- workflow transition changes for `state_9_implementation_reviewed`, `state_10_implementation_refined`, and `state_11_manual_release`;
- MCP/GraphQL/readback fields that explain why a run is release-ready, handoff-required, or blocked;
- a canonical `proposal-077|p077` proof gate.

P077 does not include:

- automating GitHub PR review, Copilot review, or PR comment disposition;
- auto-approving human release gates;
- replacing P059 release evidence gates;
- changing P052 hard loop budget semantics;
- treating non-code handoff as code-writer work;
- retroactively rewriting historical run artifacts.

---

## 4. Closeout Readiness Model

Add a typed contract, `implementation_closeout_readiness_v1`, with canonical path:

```text
review/implementation-closeout-readiness.json
```

Shape:

```json
{
  "schema_version": "implementation-closeout-readiness.v1",
  "run_id": "...",
  "proposal_id": "NNN",
  "status": "not_ready",
  "decision": "return_to_code_refine",
  "proposal_gate": {
    "required": true,
    "name": "proposal-NNN",
    "status": "missing"
  },
  "audit": {
    "status": "not_ready",
    "report_path": "review/implementation-audit.md"
  },
  "code_blockers": [
    {
      "id": "proposal-critical-code-blocker",
      "owner": "code_writer",
      "source": "audit_report",
      "summary": "GraphQL still exposes executable non-approval mutations."
    }
  ],
  "handoff_blockers": [],
  "known_risks": [],
  "loop_policy": {
    "refine_cycles_used": 1,
    "refine_cycles_max": 8,
    "soft_checkpoint_reached": false
  }
}
```

Allowed `status` values:

- `ready`
- `ready_with_risks`
- `handoff_required`
- `not_ready`
- `blocked`
- `invalid`
- `unknown`

Allowed `decision` values:

- `enter_manual_release`
- `await_non_code_handoff`
- `return_to_code_refine`
- `await_gate_definition`
- `await_operator_decision`
- `block_with_evidence`

---

## 5. Required Behavior

### 5.1 Proposal gate requirement

For proposal-backed implementation runs, the workflow must resolve a canonical proposal gate before closeout.

Resolution order:

1. explicit proposal metadata if present;
2. `proposal-XXX|pXXX` registered in `scripts/test-gate.sh`;
3. proposal-specific readiness gate if the proposal declares one;
4. `missing` if no gate is registered.

If the gate is missing, the run must not enter manual release as implementation-ready. It must produce `decision = await_gate_definition` unless an explicit proposal field marks the gate as intentionally not required.

### 5.2 Audit verdict policy

Implementation audit verdicts become transition inputs:

| Audit readiness | Closeout behavior |
|---|---|
| `Ready` | May enter manual release if gates and controlled reports are green. |
| `Ready with Risks` | May enter manual release only when every risk is classified as non-code, accepted handoff, follow-up, rollout constraint, or release-owner decision. |
| `Partial` | Cannot enter manual release unless all missing items are non-code handoff and code-owned blockers are zero. |
| `Not Ready` / `Not Implemented` | Cannot enter manual release. Return to code refine when code-owned blockers exist; otherwise await handoff/operator decision. |
| Missing/invalid audit | Fail closed with `decision = block_with_evidence` or `await_operator_decision`. |

The parser must tolerate existing audit prose during rollout, but new normalized readiness truth must be typed.

### 5.3 Bounded code/refine loop

The closeout decision must distinguish code-owned blockers from non-code blockers.

Rules:

- if code-owned blockers exist and implementation refine budget remains, transition to `state_10_implementation_refined`;
- if code-owned blockers exist and hard refine budget is exhausted, transition to `await_operator_decision` with the blocker list;
- if no code-owned blockers exist but required non-code handoff remains, transition to `handoff_required` / `await_non_code_handoff`;
- if the proposal gate is missing, transition to `await_gate_definition`, not code refine;
- if the same blocker recurs across repeated audit/refine cycles without meaningful diff or gate progress, mark a soft convergence checkpoint and ask for operator decision instead of looping silently.

This preserves P052's hard loop budget semantics and adds a closeout-specific soft checkpoint without pretending the hard workflow budget is exhausted.

### 5.4 Release transition guard

`state_9_implementation_reviewed` must not route directly to `state_11_manual_release` from `implementation_self_assessment_v2.blocking_remaining_code_tasks == 0` alone.

The transition must consult `implementation_closeout_readiness_v1`.

Minimum guard:

```text
implementation_closeout_readiness_v1.decision == 'enter_manual_release'
```

The existing self-assessment remains canonical for whether code-writer work should continue, but closeout readiness owns whether the run may present itself as release-ready.

### 5.5 Manual PR review remains external

P077 must not:

- inspect GitHub review comments as required transition truth;
- require Copilot comments to be fixed or dismissed before release;
- block workflow completion on PR review state;
- create automatic PR-review disposition artifacts.

The operator may still use PR review manually before merging. A future proposal may add PR-review integration after the manual process stabilizes.

---

## 6. Readback and Operator UX

Runs must expose closeout readiness as structured readback through MCP and GraphQL:

```json
{
  "implementation_closeout_readiness": {
    "status": "not_ready",
    "decision": "return_to_code_refine",
    "blocking_code_count": 2,
    "handoff_count": 0,
    "gate_status": "missing",
    "audit_status": "not_ready",
    "summary": "Implementation audit found proposal-critical code blockers and no UI action boundary gate exists."
  }
}
```

Operator surfaces should show:

- whether code work is still required;
- whether a proposal gate is missing or failing;
- whether audit evidence is current;
- whether remaining work is handoff/operator-owned;
- why the run is not being sent back to code if code blockers are zero.

The UI must not imply that a run is release-ready when closeout readiness is `not_ready`, `blocked`, `invalid`, or `unknown`.

---

## 7. Implementation Inventory

Expected areas:

| Area | Expected ownership |
|---|---|
| `control-plane/crates/domain` | `implementation_closeout_readiness_v1` typed model and status vocabulary |
| `control-plane/crates/engine/src/orchestrator.rs` | state 9/10/11 transition guard and bounded closeout routing |
| `control-plane/crates/engine/src/artifacts` or equivalent | audit/gate/self-assessment aggregation into active contract truth |
| `control-plane/crates/engine/tests` | state transition and bounded-loop regression tests |
| `control-plane/crates/graphql-server` | readback fields for closeout readiness |
| `control-plane/crates/mcp-server` | run detail/list closeout readiness readback |
| `examples/workflows/workflow.yaml` | transition guard updates |
| `examples/workflows/full-mvp-live.yaml` | transition guard updates |
| `examples/agents/agents.yaml` | lead/auditor aggregation prompt contract updates, if prompt-owned |
| `docs/reference/output-contracts-failure-evidence-and-recovery.md` | stable closeout readiness contract |
| `docs/reference/test-gates.md` | register `proposal-077|p077` |
| `scripts/test-gate.sh` | register `proposal-077|p077` |

---

## 8. Proof Gate

`./scripts/test-gate.sh proposal-077` must prove:

1. A run with `implementation_self_assessment_v2.complete` but missing proposal gate does not enter manual release; it records `await_gate_definition`.
2. A run with audit `Not Ready` and code-owned blockers returns to implementation refine while refine budget remains.
3. A run with audit `Not Ready` and no code-owned blockers does not call `code_writer`; it records handoff/operator decision.
4. A run with `Ready with Risks` enters manual release only when all risks are classified as accepted non-code handoff, rollout constraint, follow-up, or release-owner decision.
5. A run with green proposal gate, green controlled reports, and successful audit enters manual release.
6. Repeated identical blockers trigger a soft convergence checkpoint without claiming hard loop budget exhaustion.
7. MCP and GraphQL expose the same closeout readiness summary.
8. The gate registry includes `proposal-077|p077`.

The gate must not require live Xcode, GitHub PR review, Copilot review, daemon dogfood, or network access.

---

## 9. Rollout

1. Add typed model and parser with warning-only normalization for existing audit/report vocabulary.
2. Add readback without changing transitions.
3. Add transition guard behind a feature flag or workflow variable for one dogfood cycle.
4. Turn on fail-closed behavior for new proposal implementation runs.
5. Keep historical runs readable without rewriting old artifacts.

---

## 10. Acceptance Criteria

- A proposal implementation run cannot claim release readiness with a missing proposal gate.
- `Not Ready` and `Not Implemented` audits prevent manual-release transition unless an operator explicitly routes the run to handoff/decision outside code closeout.
- `Ready with Risks` has a bounded, typed path to release when remaining risks are non-code and explicitly accepted or deferred.
- Code agents are not invoked for missing gates, manual release evidence, PR review comments, or other non-code closeout tasks.
- Runs expose a concise closeout readiness explanation through MCP, GraphQL, and operator UI.
- `./scripts/test-gate.sh proposal-077` passes.

---

## 11. Implementation Refinements

The following details were refined during implementation (Stage 10):

### 11.1 Closeout Fingerprint
To prevent decision consistency issues between synthesizer execution and transaction commit, a **Closeout Fingerprint** was added to the readiness model. This fingerprint captures the immutable state of the run at evaluation time and is stored as `fingerprint_json` in the artifact.

### 11.2 Latency Budget
A **5,000ms latency budget** is enforced for fingerprint computation. If the budget is exceeded, the synthesizer fails closed with `status: unknown` and `decision: block_with_evidence` to avoid blocking the engine with expensive state scans.

### 11.3 Projection Rebuild After Closeout Transaction
After the state-9 closeout transaction commits the active gate/readiness pair, the orchestrator rebuilds run-state projections so downstream transition evaluation, GraphQL `runs.get`, and MCP `runs.get` see current P077 truth in the same `AdvanceRun` cycle. A rebuild failure is logged and retried on the next cycle rather than being treated as fatal: active SQLite truth remains the authority, and projections are eventually consistent.

The exported run-state projection includes a derived `fingerprint_hash` short hash for each P077 row, sourced from the readiness `fingerprint_json` via `CloseoutFingerprint::short_hash`. The hash is the operator-facing identifier used in tooltips, copy-to-clipboard, and VoiceOver announcements; the full fingerprint payload is available only through artifact readback. Rows without a fingerprint expose `fingerprint_hash: null`.

### 11.4 Proposal Gate Settlement Authorization
`SettleProposalGateCmd` validation hard-codes the canonical `ProposalGateSettle` capability literal rather than serializing the capability enum at runtime, eliminating a fail-open path if enum serialization ever produced an empty string. Empty caller capabilities are rejected with an explicit error, and the authority allow-list (`release_owner`, `control_plane_owner`, `proposal_owner`) is enforced before the command emits a `command_journal` row.

### 11.5 Advisory Mode Decision Capping (R14 Phase 1)
Advisory mode (and the `legacy_fallback` diagnostic variant) is implemented as a synthesizer-side cap rather than a transition-time bypass. When the resolved mode is not `enforcement`, `synthesize_implementation_closeout_readiness_for_state9` rewrites any `enter_manual_release` or `return_to_code_refine` decision to `await_operator_decision` and records a `diagnostic_reason` of the form `advisory_mode: <effective_mode> — diagnostic-only; no transition side effects until cutover to enforcement`. Status, blocker counts, gate status, audit status, and other observability fields are preserved verbatim so operators see what the full decision matrix would have produced under enforcement.

### 11.6 Settlement Action Surface Hardening
`ProposalGateSettlementAction::Execute` no longer auto-builds a `Passed` gate result without proof: when a receipt is supplied it is routed to `ImportReceipt`, and when no receipt is present the command bails with an explicit error directing the operator to run `./scripts/test-gate.sh proposal-077` and supply the receipt via `action=import_receipt`. The MCP `runs.settle_proposal_gate` tool drops `record_settlement` from its action enum entirely (callers passing it receive a hard error pointing at `import_receipt`), and the tool now requires `action` to be specified explicitly when `receipt_json` is absent rather than silently defaulting to a settlement that lacks proof.

