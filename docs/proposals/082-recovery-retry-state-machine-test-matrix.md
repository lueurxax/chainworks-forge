# Proposal 082: Recovery and Retry State-Machine Test Matrix

| Field | Value |
|---|---|
| Date | 2026-05-01 |
| Status | Draft |
| Author | Codex |
| Depends on | [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md), [065-operator-retry-instruction-contract.md](065-operator-retry-instruction-contract.md), [076-auto-retry-observation-ledger-and-recovery-policy.md](076-auto-retry-observation-ledger-and-recovery-policy.md), [080-continuous-stale-execution-reconciliation.md](080-continuous-stale-execution-reconciliation.md) |
| Related | P037, P064, [durable side-effect reconciliation](../reference/execution-truth-and-recovery.md#durable-side-effect-ledger-and-reconciliation), April 30 2026 orchestration recovery and ACP startup fixes |
| Scope | Create a reusable failure matrix and required DB/engine tests for restart, retry, stale execution, duplicate mediation, late output, and ACP startup recovery. |
| Goal | Turn recurring recovery fixes into one state-machine proof suite instead of one-off incident patches. |

---

## 1. Problem

Recent recovery work repeatedly touched the same fault zone:

- restart behavior while a command or provider startup is in flight;
- stale execution truth occupying scheduler capacity;
- duplicate mediation/session attempts;
- retry instructions that can target the wrong identifier shape;
- late output after a stage has been superseded;
- ACP startup rows marked running without useful provider/session truth.

P045, P065, P076, and P080 define pieces of the recovery model, but implementation still needs a shared failure matrix that every recovery change extends.

## 2. Decision

Add a canonical recovery/retry state-machine matrix before the next broad recovery implementation slice.

The matrix must require at least:

- one DB assertion per row;
- one engine integration assertion per row;
- an operator-facing readback expectation per row.

## 3. Canonical Matrix

Add canonical artifact:

```text
docs/reference/recovery-retry-state-machine-test-matrix.md
```

Initial rows:

| Scenario | Setup | Expected repair/reject | DB assertion | Engine assertion | Readback |
|---|---|---|---|---|---|
| restart mid command | command journal written, work item not settled | resume/requeue exactly once or block with drift | one active claim/work item | no duplicate stage/agent attempt | `resume_claim_status` or stale reason |
| reject non-manual stage retry | retry requested for non-retryable stage | reject before mutation | no new stage execution | command handler returns typed denial | `invalid_stage_for_retry` |
| late output after supersede | old agent emits after retry stage created | ignore/quarantine old output | artifact links stay on active attempt only | active stage not regressed | stale output event |
| duplicate session/startup | two startup claims for same work | keep one owner, fail/repair duplicate | one active session generation | capacity not double counted | duplicate owner reason |
| stale ACP startup | running work, no provider session/activity after grace | mark stale and requeue/repair | session invalidated or repaired | one replacement work item | `startup_stalled` |
| stale scheduler ownership | running work without live executor owner | safe repair or needs reconciliation | capacity freed only through transition | no blind retry of release side effects | `stale_repaired` or `needs_effect_reconciliation` |
| release side-effect drift | unresolved side-effect ledger exists | block retry, route to durable side-effect reconciliation | side-effect status unchanged | no duplicate push/upload | `requires_effect_reconciliation` |
| retry identifier mismatch | stage execution UUID used where workflow stage id required | reject with guidance | no retry mutation | typed MCP error | valid identifier guidance |

## 4. Required Behavior

### 4.1 Fail closed before mutation

Every recovery command must validate eligibility before it mutates:

- run status;
- stage status;
- active work items;
- side-effect ledger;
- retry budget;
- identifier shape;
- caller capability.

### 4.2 Repair through state transitions

Recovery must not patch rows directly when a domain transition exists. Repairs must preserve old evidence and create explicit repair reason codes.

### 4.3 One owner per active execution

At any time, each active work item must have at most one live executor owner and one durable session generation owner.

### 4.4 Late output quarantine

Output from superseded executions must not reattach to active artifacts, unblock active stages, or overwrite current projection truth.

## 5. Tests

Add proof gate:

```text
proposal-082|p082
```

Required tests:

- DB repository tests for each matrix row;
- engine integration tests for each matrix row;
- MCP readback test for each recovery reason code;
- regression fixture for ACP startup stale repair;
- regression fixture for retry identifier guidance.

## 6. Non-Goals

- Do not add blind automatic retry.
- Do not auto-resolve human approvals.
- Do not retry release side effects while the durable side-effect ledger reports unresolved effects.
- Do not replace P045/P065/P076/P080; this proposal supplies their shared proof matrix.

## 7. Acceptance Criteria

P082 is complete when:

1. the matrix is checked in under `docs/reference/`;
2. every row has at least one DB assertion and one engine integration assertion;
3. stale recovery readback uses typed reason codes;
4. future recovery proposals must add rows before changing recovery behavior.
