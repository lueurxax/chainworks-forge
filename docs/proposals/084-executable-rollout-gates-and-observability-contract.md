# Proposal 084: Executable Rollout Gates and Observability Contract

| Field | Value |
|---|---|
| Date | 2026-05-01 |
| Status | Draft |
| Author | Codex |
| Depends on | [077-bounded-implementation-closeout-readiness-gates.md](077-bounded-implementation-closeout-readiness-gates.md), [059-release-evidence-gates-and-approval-payload-contract.md](059-release-evidence-gates-and-approval-payload-contract.md), [test-gates.md](../reference/test-gates.md), [release-gate.md](../reference/release-gate.md) |
| Related | P017 audit follow-ups, `scripts/test-gate.sh`, release evidence and operator reports |
| Scope | Define the rollout gate before merge: migrations, metrics, projection/readback, operator reports, hold conditions, and rollback disposition for each implementation proposal. |
| Goal | Stop closeout drift where evidence, metrics, and gates are added after implementation rather than required before release. |

---

## 1. Problem

Recent closeouts repeatedly needed late follow-ups for:

- metric helpers;
- audit addenda;
- gate guards;
- proposal-specific `scripts/test-gate.sh` changes;
- release/report fields;
- rollout/hold criteria.

The pattern is not one missing gate. The pattern is that gates are often defined after implementation, when the code already shaped the evidence instead of the proposal shaping the evidence.

## 2. Decision

Every implementation proposal that changes control-plane behavior, UI actionability, recovery, persistence, or release behavior must declare an executable rollout contract before implementation starts.

The rollout contract is part of readiness, not an optional closeout appendix.

## 3. Rollout Contract

Add canonical artifact template:

```text
docs/reference/executable-rollout-gate-template.md
```

Every proposal using the template must fill:

| Section | Required content |
|---|---|
| migrations | required DB/schema migrations and rollback/forward-only policy |
| gates | `scripts/test-gate.sh` aliases and exact commands |
| metrics | emitted metric/event names, labels, and expected cardinality |
| projections | readback fields proving behavior is visible |
| operator report | report fields an operator can inspect without logs |
| hold conditions | conditions that block release/manual closeout |
| rollback disposition | how to disable, hold, or reconcile without data loss |
| degraded drill | if applicable, evidence for degraded/fail-closed behavior |

## 4. Required Behavior

### 4.1 Gate before merge

The proposal must name:

- canonical gate alias;
- fast/local subset;
- any migration/readback verification command;
- remote-only or manual gates.

If no gate is required, the proposal must say why.

### 4.2 Metrics before closeout

Metrics and logs must exist before the feature is considered complete when the behavior changes:

- recovery decisions;
- auth denials;
- mutation denials;
- retry/reconciliation outcomes;
- projection lag/freshness;
- release side-effect settlement.

### 4.3 Operator readback

Every new backend state must have one of:

- GraphQL readback for UI diagnostics;
- MCP readback for operator/automation diagnostics;
- report artifact field;
- explicit no-readback decision with justification.

### 4.4 Hold and rollback

The proposal must define what happens if:

- migration succeeds but code fails;
- code lands but projection/readback is missing;
- metrics are absent;
- recovery/side-effect reconciliation detects unresolved state;
- UI and backend actionability disagree.

## 5. Tests

Add proof gate:

```text
proposal-084|p084
```

Required tests:

- template lint: proposals with implementation scope declare gate, metrics, readback, and hold criteria;
- `scripts/test-gate.sh list` includes registered aliases;
- gate command exists and returns non-zero on missing prerequisites;
- operator report/readback fixture includes required fields.

## 6. Non-Goals

- Do not replace P077 closeout readiness.
- Do not require all GitHub PR comments to be dispositioned by the orchestrator.
- Do not require production deployment automation.
- Do not define one universal gate for every proposal; each proposal owns its own executable contract.

## 7. Acceptance Criteria

P084 is complete when:

1. the rollout template exists under `docs/reference/`;
2. implementation proposals can be linted for gate/readback/metrics completeness;
3. P017-style late metric/gate additions would be caught before closeout;
4. operator-facing reports expose enough state to decide release/hold without reading raw logs.
