# Proposal 052: Orchestrator Loop Budget Source of Truth

| Field | Value |
|---|---|
| Date | 2026-04-18 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [023-loop-improvement-analytics-and-iteration-progression.md](023-loop-improvement-analytics-and-iteration-progression.md), [028-forced-advance-on-loop-budget-exhaustion.md](028-forced-advance-on-loop-budget-exhaustion.md), [037-acp-execution-supervision-and-idle-watchdog.md](037-acp-execution-supervision-and-idle-watchdog.md), [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md) |
| Scope | Define one source of truth for hard loop budgets and a separate contract for lead/steward soft checkpoints in workflow snapshots, run context, generated artifacts, and proof gates. |
| Goal | Prevent generated run artifacts from reporting a soft operator checkpoint as an exhausted hard workflow loop budget while keeping useful lead escalation heuristics. |

**Gate naming note:** this proposal owns the new canonical gate alias `proposal-052|p052`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md`; it must not reuse existing proposal gates.

---

## 1. Context and Motivation

Chainworks workflows already support looped states with a workflow-owned `loop.max`. In the full MVP workflow, proposal review refinement uses:

```yaml
variables:
  max_proposal_revision_cycles: 15
...
loop:
  while: proposal_review_status == "changes_requested"
  max: vars.max_proposal_revision_cycles
```

That value is the hard execution budget. It is compiled into the workflow plan, persisted in the run snapshot, and evaluated by the orchestrator when deciding whether a loop is exhausted.

During live recovery work on run `9318de0d-9c75-40ad-9d0a-74c3610b021d`, generated artifacts reported a different budget:

- `proposal_review_iterations: 3`
- `max_allowed: 3`
- `budget_exhausted: true`
- `Proposal review loop budget exhausted (3/3 iterations)`

The run was still executing `state_5_proposal_refined`, and both the source workflow and the run snapshot still had `max_proposal_revision_cycles: 15`. Therefore `3/3` was not the hard workflow limit. It was a lead/steward soft checkpoint that was written into artifacts with hard-budget language.

This is operationally dangerous. Operators, reviewers, recovery tools, and future UI projections can make the wrong decision if they cannot tell the difference between:

- hard workflow truth: the engine will not continue beyond this limit without the workflow exhaustion policy
- soft checkpoint: the lead recommends escalation or extra attention after this many cycles, but the engine can still continue

P052 fixes the contract without changing the hard limit from 15 and without removing the lead/steward optimization.

---

## 2. Problem Statement

### 2.1 Hard and soft loop concepts are currently conflated

Generated artifacts can invent or repeat a `max_allowed` value without declaring whether it came from the compiled workflow, the run snapshot, an agent prompt, an operator policy, or a lead heuristic.

The terms `budget_exhausted`, `loop budget exhausted`, and `max_allowed` currently imply hard engine truth. If the value is actually a soft checkpoint, the artifact lies about run semantics.

### 2.2 The lead does not have a required readback contract

Lead and proposal-writer agents receive run context and prior artifacts, then write summaries and run-state files. The proposal workflow does not explicitly require those agents to read the hard loop budget from the workflow snapshot or from an orchestrator-provided structured field.

As a result, an artifact can derive its apparent budget from a prior summary, a local convention, or a prompt-side heuristic.

### 2.3 Downstream recovery and review tools need machine-readable distinction

Recovery logic, MCP readback, proposal review triads, and operator summaries need to know whether a loop is:

- within hard budget and below soft checkpoint
- within hard budget but at or beyond soft checkpoint
- at hard budget exhaustion
- past hard budget due to recovery or forced-advance behavior

Those states require separate fields. A single boolean such as `budget_exhausted` is not enough unless its source and scope are explicit.

---

## 3. Scope

This proposal includes:

- a canonical hard loop budget source rule
- a separate soft checkpoint contract for lead/steward escalation heuristics
- structured loop-budget fields for generated run artifacts
- prompt and context requirements for lead, steward, proposal writer, and reviewers that summarize loop status
- validation rules that reject or flag artifacts that claim hard exhaustion without a workflow-backed hard limit
- tests proving a workflow hard limit of 15 can coexist with a soft checkpoint at 3
- canonical gate ownership for `proposal-052|p052`

This proposal does not include:

- changing `examples/workflows/full-mvp-live.yaml` from 15 to 3
- removing soft checkpoint behavior
- changing P051 Xcode MCP bridge pooling
- changing ACP provider retry/reuse semantics
- manually repairing historical artifacts for existing runs unless an operator explicitly requests it
- Swift app UI changes, except optional read-only projection if an implemented surface already shows loop budget fields

---

## 4. Core Rules

### 4.1 Hard loop budget source of truth

For every looped state, the hard loop budget is owned by the compiled workflow plan for the run.

The source order is:

1. the persisted run workflow snapshot, including resolved variables
2. the compiled workflow plan loaded from that snapshot
3. the source workflow YAML only before run creation or for static tests

Generated artifacts must not derive hard loop limits from prior summaries, agent memory, prompt prose, or soft checkpoint policy.

For the full MVP proposal refinement loop, the hard budget is:

```text
state: state_5_proposal_refined
loop.max expression: vars.max_proposal_revision_cycles
resolved hard limit: 15
source: run.workflow_snapshot_json.variables.max_proposal_revision_cycles
```

### 4.2 Hard exhaustion is engine-owned

Only engine-owned state can declare hard loop exhaustion.

A generated artifact may report hard exhaustion only when the orchestrator-provided loop context says the hard limit has been reached or exceeded. Agent-authored summaries must not compute hard exhaustion independently from stale counters.

Hard exhaustion language includes:

- `budget_exhausted: true`
- `loop budget exhausted`
- `max allowed iterations reached`
- `forced advance because budget exhausted`
- equivalent user-facing copy

Those phrases are reserved for hard workflow exhaustion.

### 4.3 Soft checkpoint policy

Lead/steward agents may still use a soft checkpoint to recommend escalation before the hard workflow limit.

A soft checkpoint must be represented as advisory policy, not as workflow exhaustion.

Required fields:

```json
{
  "soft_checkpoint_after_review_passes": 3,
  "soft_checkpoint_source": "steward_policy:proposal_review_quality_checkpoint",
  "soft_checkpoint_reached": true,
  "soft_checkpoint_reason": "Three review/refine passes have not converged.",
  "recommended_action": "Ask the operator or lead to decide whether to continue, narrow scope, or change reviewer instructions."
}
```

Allowed user-facing language:

```text
Soft checkpoint reached after 3 review passes. The workflow hard limit remains 15.
```

Disallowed user-facing language for a soft checkpoint:

```text
Proposal review loop budget exhausted (3/3 iterations).
```

### 4.4 Artifact loop status schema

Any generated artifact that reports proposal loop status must use the following structure or an equivalent typed model:

```json
{
  "loop_kind": "proposal_review_refinement",
  "state_id": "state_5_proposal_refined",
  "iterations_used": 3,
  "hard_limit": 15,
  "hard_limit_source": "workflow_snapshot_json.variables.max_proposal_revision_cycles",
  "hard_limit_exhausted": false,
  "soft_checkpoint_after_review_passes": 3,
  "soft_checkpoint_source": "steward_policy:proposal_review_quality_checkpoint",
  "soft_checkpoint_reached": true,
  "soft_checkpoint_reason": "Three review/refine passes have not converged.",
  "recommended_action": "Escalate for operator or lead decision before spending more reviewer cycles."
}
```

`max_allowed` is deprecated for new artifacts because it is ambiguous. If it remains for backward compatibility, it must mean the hard workflow limit and must match `hard_limit`.

### 4.5 Artifact locations covered by P052

At minimum, P052 applies to generated proposal-loop artifacts under a run meta root:

- `state/run-state.json`
- `reviews/proposal/summary.json`
- `reviews/proposal/reviewer-scope-plan.json`
- `summaries/orchestrator.md`
- `proposals/current/proposal.md`
- `proposals/current/revision-summary.md`

If a new artifact reports proposal loop status, it inherits this contract.

---

## 5. Required Behavior

### 5.1 Orchestrator context assembly

Before spawning a lead, steward, proposal writer, proposal reviewer, or recovery agent that may summarize loop state, the control-plane must provide a structured loop context:

```json
{
  "loop_kind": "proposal_review_refinement",
  "state_id": "state_5_proposal_refined",
  "iterations_used": 3,
  "hard_limit": 15,
  "hard_limit_source": "workflow_snapshot_json.variables.max_proposal_revision_cycles",
  "hard_limit_exhausted": false,
  "soft_checkpoint_after_review_passes": 3,
  "soft_checkpoint_source": "steward_policy:proposal_review_quality_checkpoint",
  "soft_checkpoint_reached": true
}
```

Agents may quote or summarize this context. They must not replace it with local arithmetic unless the context is absent, and absence must be treated as unknown rather than exhausted.

### 5.2 Lead and steward prompt contract

Prompt contracts must require:

- hard limit comes only from `hard_limit`
- soft checkpoint comes only from `soft_checkpoint_after_review_passes`
- soft checkpoint copy must include the remaining hard limit when it recommends escalation
- hard-exhaustion copy is forbidden unless `hard_limit_exhausted == true`
- artifact JSON must preserve both hard and soft fields when present

### 5.3 Validation on artifact import or normalization

The daemon should validate generated loop-status artifacts before treating them as authoritative.

Validation failures:

- artifact says `hard_limit_exhausted: true` but orchestrator context says false
- artifact says `budget_exhausted: true` with no hard-limit source
- artifact sets `max_allowed` to a value different from the run snapshot hard limit
- artifact uses hard-exhaustion language while reporting only a soft checkpoint

Initial implementation may warn and annotate during rollout, but the proof gate must include a fail-closed path before P052 is considered complete.

### 5.4 Readback and debug surfaces

Any readback surface that exposes loop status should show both values:

```text
proposal review passes: 3
soft checkpoint: reached at 3
hard limit: 15
hard limit exhausted: false
```

GraphQL, MCP, CLI, or debug JSON surfaces are not required to add new public fields unless they already expose the affected artifacts. If they do expose loop status, they must preserve the distinction.

---

## 6. Implementation Inventory

Expected Rust/control-plane files and areas:

- workflow definition and compiler types that resolve `loop.max` and keep source metadata
- orchestrator loop accounting and transition evaluation
- run context assembly for ACP agent spawns
- artifact normalization/import logic for proposal-loop artifacts
- agent prompt builders for lead, steward, proposal writer, and proposal reviewers
- MCP/GraphQL/debug readback models if they expose loop-status artifacts
- example workflow fixtures using `max_proposal_revision_cycles: 15`
- focused control-plane tests for loop context and artifact validation

Expected repository docs and gate files:

- `docs/reference/rust-control-plane.md`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`
- `examples/workflows/full-mvp-live.yaml` only if soft checkpoint policy becomes workflow-declared
- `examples/agents/agents.yaml` or adjacent agent contract files if prompt text is catalog-owned

The implementation must identify the exact source of the current soft checkpoint. If that policy is prompt-only today, P052 must move it into a named policy field, named prompt parameter, or workflow metadata before it can be used as machine-readable truth.

---

## 7. Proof Gate

The canonical gate is:

```bash
./scripts/test-gate.sh proposal-052
```

It must include Rust/control-plane proof for:

1. A workflow with `max_proposal_revision_cycles: 15` produces loop context with `hard_limit: 15`.
2. The same run can report `soft_checkpoint_after_review_passes: 3` without setting `hard_limit_exhausted`.
3. An artifact that reports `max_allowed: 3` for a run whose hard limit is 15 is rejected or annotated as invalid.
4. An artifact that says `budget_exhausted: true` while `hard_limit_exhausted: false` is rejected or annotated as invalid.
5. Lead/steward/proposal-writer prompt fixtures include both hard and soft budget fields.
6. `docs/reference/test-gates.md` and `scripts/test-gate.sh` register `proposal-052|p052`.

The gate does not require Swift UI tests.

---

## 8. Rollout and Compatibility

P052 should roll out in phases:

1. Add structured loop context and preserve it in prompts/artifacts.
2. Add warnings for ambiguous historical fields such as `max_allowed`.
3. Update generated artifacts to write `hard_limit` and `soft_checkpoint_*`.
4. Turn validation warnings into fail-closed behavior for new post-P052 runs.
5. Leave pre-P052 artifacts readable but mark ambiguous budget fields as legacy.

Existing run artifacts should not be rewritten automatically. If an operator wants to repair a specific active run's artifacts, that is a separate recovery action and must preserve the original evidence trail.

---

## 9. Risks and Constraints

- Overcorrecting by deleting the soft checkpoint would remove useful lead behavior. The proposal preserves it as advisory policy.
- Treating prompt text as source of truth would recreate the problem. The policy must be named and structured.
- Existing artifacts may contain `max_allowed` and `budget_exhausted` with old semantics. Readback must treat those fields as legacy unless they include a source.
- P028 hard-exhaustion behavior remains valid. P052 only tightens how generated artifacts distinguish hard exhaustion from advisory escalation.

---

## 10. Open Questions

1. Should the soft checkpoint policy live in workflow YAML, agent catalog metadata, or steward policy configuration?
2. Should reaching the soft checkpoint create a manual approval/checkpoint stage, or should it remain a lead recommendation inside artifacts?
3. Should recovery tools offer an explicit "continue past soft checkpoint" action, or is existing retry/resume enough?
4. Should post-P052 readback expose loop context as a first-class GraphQL/MCP field, or is artifact-level validation sufficient for now?
