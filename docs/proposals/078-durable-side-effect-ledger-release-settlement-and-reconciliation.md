# Proposal 078: Durable Side-Effect Ledger, Release Settlement, and Reconciliation

| Field | Value |
|---|---|
| Date | 2026-04-29 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | P075 Local Persistence Write Budget, P072 Operator Action Routing, P038 Compaction, P045/P065 Recovery MCP Tools, Rust control plane |
| Related | P066 Provider Toolchain Cache Mapping, P076 Auto-Retry Observation Ledger |
| Scope | Introduce a durable side-effect lifecycle for irreversible or externally visible operations so release/recovery code can reconcile incomplete settlements without repeating side effects. |
| Goal | Prevent duplicate git pushes, duplicate uploads, repeated release operations, and unsafe retries by recording side-effect intent before execution, settling verified effects atomically, and routing incomplete settlement states to MCP reconciliation instead of blind retry. |

---

## 1. Why this proposal exists

The local control plane intentionally avoids Temporal and other external durable workflow engines.

That is acceptable only if the system implements its own discipline for external side effects.

The most dangerous failure class is:

> an external side effect already happened, but the control-plane database did not complete its durable settlement.

Example:

1. release stage starts;
2. agent or release service performs `git push`;
3. some artifacts/receipts are partially created;
4. daemon crashes before AgentExecution, StageExecution, runtime facts, delivery receipt, projection invalidation, and workflow cursor are updated;
5. after restart, the database does not know whether the side effect completed;
6. a normal retry may perform the push/upload again.

This is not a normal retry problem.
It is a durable side-effect lifecycle problem.

---

## 2. Core decision

Every irreversible or externally visible operation must be represented by a durable side-effect record.

The system must create a side-effect intent **before** executing the external operation.

If the process dies after the side effect but before settlement, startup recovery must classify the record as requiring reconciliation.

Normal retry is forbidden while unresolved side effects exist.

---

## 3. Current repo truth

This proposal is grounded in the current Rust control-plane implementation, not only in the desired architecture.

### 3.1 P072 boundary is related but not sufficient

P072 is partially implemented:

- `auth` has `surface_policies`;
- default bootstrapped operator principals allow only `approveApproval` and `rejectApproval` as GraphQL mutations;
- v2 principal validation requires `default-operator` and `ui_operator` to expose exactly those two approval mutations.

However, the GraphQL schema still exposes executable legacy/control mutations including `startRun`, `approveStage`, `rejectStage`, `retryStage`, and `cancelRun`. Those resolvers are currently gated through `mutation_allowed`, and tests/fixtures still contain broad mutation-capability principals for non-production compatibility.

P078 must therefore not rely on GraphQL removal as the only safety boundary. Release retry safety must live in the command/release execution path itself. P072 remains the northbound operator-action boundary; P078 owns durable side-effect reconciliation and retry blocking.

### 3.2 `RetryStage` currently has no side-effect preflight

`CommandHandler::RetryStage` currently performs a large transactional rewrite:

- records the command journal entry;
- finds the latest stage attempt;
- validates retry eligibility from stage/run status;
- applies retry-budget handling;
- cancels running agent executions and pending/running work items;
- settles the old stage as skipped;
- creates a new stage attempt;
- updates run status/current state;
- supersedes workflow conflicts and active artifact claims;
- enqueues `AdvanceRun`;
- rebuilds projections after commit.

That flow is correct for ordinary retry but unsafe for stages that may already have external side effects. P078 must insert the unresolved-side-effect check before the old attempt is skipped, before pending/running work is cancelled, before artifact claims are superseded, and before the new attempt is inserted.

If unresolved side effects exist for the run, stage execution, or release agent execution, `RetryStage` must fail closed with `requires_effect_reconciliation` and must leave the existing stage/work-item/evidence state intact for inspection.

### 3.3 Release execution currently settles after external effects

The Rust release path already bypasses ACP for release agents, but it is still post-fact from a durability perspective:

- `process_release_agent` calls `GitReleaseService::commit_and_push(...)`;
- `GitReleaseService::commit_and_push(...)` performs `git commit` and then `git push`;
- only after successful push does the executor persist `release_manifest` and `git_push_receipt`;
- `build_archive_and_push_connect` similarly calls `ConnectPublishService::build_and_distribute(...)` and persists `release_bundle_manifest` / `connect_upload_receipt` only after the external operation returns.

The older Swift `ReleaseOpsCoordinator` has the same shape: it calls commit/push and build/distribute first, then returns `ReleaseResult` and receipts.

This is the exact crash window P078 must close: external side effect completed, but durable settlement did not.

---

## 4. What counts as a side effect

Initial side-effect kinds:

- `git_commit`
- `git_push`
- `build_archive`
- `connect_upload`
- `tag_create`
- `artifact_publish`

Future kinds may include:

- issue mutation,
- PR mutation,
- deploy/publish action,
- remote cleanup,
- destructive filesystem action.

---

## 5. Lifecycle

Canonical lifecycle:

```text
planned
→ prepared
→ executing
→ externally_observed
→ settled
```

Failure/recovery lifecycle:

```text
prepared/executing
→ needs_reconciliation
→ reconciled
→ settled
```

Conflict lifecycle:

```text
prepared/executing
→ needs_reconciliation
→ conflict
```

Terminal unrecoverable lifecycle:

```text
prepared/executing
→ needs_reconciliation
→ unrecoverable
```

---

## 6. Data model

## 6.1 `side_effects`

```sql
CREATE TABLE side_effects (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  stage_execution_id TEXT NOT NULL,
  agent_execution_id TEXT,
  effect_kind TEXT NOT NULL,
  target_key TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  request_fingerprint TEXT NOT NULL,
  status TEXT NOT NULL,
  prepared_at TEXT,
  started_at TEXT,
  externally_observed_at TEXT,
  settled_at TEXT,
  failed_at TEXT,
  conflict_at TEXT,
  last_error TEXT,
  expected_evidence_json TEXT,
  observed_evidence_json TEXT,
  settlement_txn_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Required indexes:

```sql
CREATE INDEX idx_side_effects_run_id ON side_effects(run_id);
CREATE INDEX idx_side_effects_stage_execution_id ON side_effects(stage_execution_id);
CREATE INDEX idx_side_effects_status ON side_effects(status);
CREATE UNIQUE INDEX idx_side_effects_idempotency_key ON side_effects(idempotency_key);
```

## 6.2 `side_effect_attempts`

```sql
CREATE TABLE side_effect_attempts (
  id TEXT PRIMARY KEY,
  side_effect_id TEXT NOT NULL REFERENCES side_effects(id),
  attempt_number INTEGER NOT NULL,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  exit_status TEXT,
  stdout_path TEXT,
  stderr_path TEXT,
  observed_evidence_json TEXT,
  error TEXT
);
```

## 6.3 `side_effect_settlements`

```sql
CREATE TABLE side_effect_settlements (
  id TEXT PRIMARY KEY,
  side_effect_id TEXT NOT NULL REFERENCES side_effects(id),
  status TEXT NOT NULL,
  receipt_artifact_id TEXT,
  reconciliation_report_artifact_id TEXT,
  applied_at TEXT,
  applied_by TEXT
);
```

---

## 7. Persistence rules

P078 must obey P075.

### 7.1 Barrier writes

These are barrier writes:

- insert side-effect intent,
- mark executing,
- settle side effect,
- mark needs reconciliation,
- mark conflict/unrecoverable.

### 7.2 Evidence spooling

These go to files, not row-by-row database writes:

- command stdout,
- command stderr,
- remote readback payloads,
- upload probe responses,
- verbose git command traces.

SQLite stores paths/checksums/summaries only.

---

## 8. Effect coordinator

Introduce:

```text
DurableEffectCoordinator
```

Responsibilities:

- create effect intent,
- enforce idempotency key uniqueness,
- execute or resume effect,
- record attempts,
- collect readback evidence,
- perform reconciliation,
- settle effect atomically,
- block unsafe retries,
- publish effect status to projections.

The release coordinator must become a client of this coordinator.

---

## 9. Release flow

Current release flow should be replaced conceptually with:

```text
prepare git_commit effect
execute/reconcile git_commit
settle git_commit

prepare git_push effect
execute/reconcile git_push
settle git_push

prepare build_archive effect
execute/reconcile build_archive
settle build_archive

prepare connect_upload effect
execute/reconcile connect_upload
settle connect_upload

settle release stage
```

Each step has its own idempotency key and evidence model.

---

## 10. Git-specific policy

## 10.1 `git_commit`

Idempotency key should include:

- run id,
- stage execution id,
- target branch,
- tree/diff fingerprint,
- commit intent version.

Commit message should include trailers:

```text
Chainworks-Effect-ID: <effect_id>
Chainworks-Run-ID: <run_id>
```

Before creating a new commit, reconciliation should check:

- whether a commit with matching trailer already exists,
- whether the tree/diff fingerprint matches,
- whether the worktree state is compatible.

If the commit already exists, do not create a second commit.

## 10.2 `git_push`

Expected evidence:

- remote name,
- target branch,
- expected commit SHA.

Readback:

```text
git ls-remote <remote> <target_branch>
```

Decision table:

| Readback | Result |
|---|---|
| remote branch points to expected SHA | settle as completed |
| remote branch missing, local commit exists | push can be attempted/re-attempted with same expected SHA |
| remote branch points to different SHA | conflict |
| remote inaccessible | needs reconciliation / operator review |

---

## 11. Upload / distribution policy

For upload/publish effects, expected evidence must include:

- destination,
- artifact checksum,
- artifact size,
- upload key or idempotency key where supported,
- expected remote artifact id if known.

Readback should prefer:

- exact artifact id,
- checksum,
- destination,
- idempotency key,
- bounded time window only as fallback.

If the external system cannot prove whether upload happened, retry must remain blocked until operator decision.

---

## 12. Retry policy

All retry commands must check unresolved side effects before executing.

Affected MCP tools:

- `stages.retry`
- `agents.retry`
- `runs.recover`
- any future release retry command.

If unresolved effect exists:

```json
{
  "error": "requires_effect_reconciliation",
  "effect_id": "...",
  "recommended_mcp_tool": "effects.reconcile"
}
```

Retry may not proceed.

---

## 13. MCP tools

Add MCP tools:

- `effects.list`
- `effects.inspect`
- `effects.reconcile`
- `effects.mark_unrecoverable`
- `effects.clear_after_manual_verification`

Optional aliases:

- `release.inspect_settlement`
- `release.reconcile_settlement`

Canonical namespace should be `effects.*`.

## 13.1 `effects.inspect`

Returns:

- effect id,
- effect kind,
- run/stage/agent,
- idempotency key,
- current status,
- expected evidence,
- observed evidence,
- retry forbidden flag,
- recommended next action.

## 13.2 `effects.reconcile`

Does readback only.

It must not:

- push,
- upload,
- publish,
- mutate external systems,
- retry a release operation.

It may:

- read remote state,
- inspect local artifacts,
- rebuild receipts,
- complete settlement transaction,
- emit reconciliation report.

---

## 14. GraphQL projections

GraphQL is read-only for this subsystem.

Expose:

- run blocked reason,
- unresolved side effects,
- effect status,
- retry forbidden,
- recommended MCP command,
- reconciliation report availability.

Example shape:

```graphql
run(id: ID!) {
  id
  status
  blockedReason
  unresolvedSideEffects {
    id
    effectKind
    status
    retryForbidden
    recommendedMcpTool
    evidenceSummary
  }
}
```

No GraphQL mutation for reconciliation.

---

## 15. Startup recovery

On daemon startup:

1. scan side effects in `executing`, `prepared`, or stale `externally_observed`;
2. classify stale records as `needs_reconciliation`;
3. block retry on related run/stage/agent;
4. publish recovery projection;
5. optionally enqueue reconciliation suggestion, not automatic retry.

---

## 16. Settlement transaction

After successful effect readback, a single DB transaction should update:

- `side_effects.status = settled`,
- `side_effect_settlements`,
- receipt artifact metadata,
- runtime facts linkage,
- agent execution status where applicable,
- stage status where applicable,
- workflow cursor / queue advance,
- projection invalidation event.

Projection materialization may be async, but invalidation/advance must be part of the settlement transaction.

---

## 17. Decision table

| Condition | Action |
|---|---|
| Intent exists, external evidence proves effect completed, settlement missing | `effects.reconcile` completes settlement |
| Intent exists, no external evidence found | mark `needs_operator_review`; retry only after explicit decision |
| Intent exists, conflicting evidence | mark `conflict`; retry forbidden |
| No intent, durable receipt exists | create recovery effect record, reconcile from evidence |
| Settlement row exists, canonical execution missing | reconcile canonical state from settlement/evidence |
| Retry requested while effect unresolved | reject with `requires_effect_reconciliation` |

---

## 18. Tests

Required tests:

1. Crash after `git push` before DB settlement.
2. Crash after upload before DB settlement.
3. Retry blocked with unresolved effect.
4. Remote branch conflict.
5. Missing external evidence.
6. Startup recovery marks stale executing effect as reconciliation-needed.
7. Reconciliation completes settlement without external write.
8. GraphQL exposes warning but no mutation.
9. MCP `effects.reconcile` writes reconciliation report.

---

## 19. Acceptance criteria

P078 is complete when:

1. release side effects create durable intent before execution;
2. retry is blocked when unresolved effect exists;
3. git push can be reconciled from remote readback without second push;
4. upload/publish can be reconciled from durable evidence without duplicate upload where possible;
5. unresolved effects are visible through GraphQL read projections;
6. reconciliation is MCP-only;
7. settlement updates canonical state in one barrier transaction;
8. high-volume evidence is spooled according to P075.

---

## 20. Final recommendation

This proposal is the local control-plane replacement for the most important part of Temporal-style side-effect safety.

It does not make external side effects magically exactly-once.

It makes them:

- intent-recorded,
- idempotency-keyed,
- evidence-backed,
- reconciliation-aware,
- retry-blocking,
- and visible to the operator.
