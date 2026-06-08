# Proposal 083: Execution-Truth Ownership and Invariant Model

| Field | Value |
|---|---|
| Date | 2026-05-01 |
| Status | Draft |
| Author | Codex |
| Depends on | [execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [durable side-effect ledger](../reference/rust-control-plane.md#durable-side-effect-ledger), [081-boundary-first-api-auth-contract-matrix.md](081-boundary-first-api-auth-contract-matrix.md) |
| Related | P017 closure follow-ups, thin UI artifact hierarchy, P064, P072 approval provenance findings |
| Scope | Define single-source ownership for lifecycle identifiers and artifact linkage, then add invariant tests proving downstream code does not trust caller-owned copies. |
| Goal | Stop provenance and lineage drift from appearing after implementation is nominally complete. |

---

## Current Implementation Approval Contract

The active Chainworks run proposal revision is `P083-r68-refined-r66-score-lift`.
The latest proposal review summary approved that revision with `blocker_count=0` and aggregate score `9.3`.
Implementation may start after the human implementation approval gate is granted.

The following hardening requirements are mandatory implementation scope. They may not be marked non-blocking, deferred, or handoff unless a separate approved successor proposal explicitly owns the exact item and the operator approves that scope reduction.
P083 is not implementation-complete, closeout-ready, or release-ready until all of them are implemented and proven:

1. GraphQL SDL inventory for lifecycle mutations: enumerate lifecycle mutation signatures with non-null `CallerRequestId`, shared typed denial union or error-code enum mirrored in MCP, and explicit nullability for every field.
2. MCP tool inventory and shared denial vocabulary: list every P083 MCP tool with JSON Schema 2020-12 input/output, `additionalProperties=false`, canonical schema path, and denial vocabulary shared with GraphQL.
3. `artifact_lineage.report_kind` backfill posture: either implement an additive backfill for pre-existing active report rows or provide executable evidence that no such rows exist before enforcing bounded `report_kind` values.
4. Schema-version evolution policy: define append-only `schema_version` semantics, same-version additive-safe field policy, version bump rules, prior-version readability, and unknown-schema diagnostic behavior.
5. SwiftData isolation boundary: pin MainActor access for projection-only `ModelContext`, a `@ModelActor` or equivalent adapter for non-main access, and Sendable snapshots before crossing back to SwiftUI roots.
6. Command idempotency intent hashing: define per-command logical fields and canonical JSON serialization with sorted keys, UTF-8, and no whitespace before `intent_hash` calculation.
7. Failed-terminal retry policy per lifecycle command: add a centralized per-command `failed_terminal_retry_policy` table and fixtures proving when a new same-intent request may or may not acquire a new lease.
8. Atomic late-output counter increments: specify and test atomic counter increment and cap enforcement for concurrent late-output writers, including overflow latch behavior.
9. External side-effect composition for idempotent commands: for every idempotent lifecycle command with external effects, name planned rows, receipt rows, and crash-between-commit-and-external-action fixtures.
10. Durable monotonic clock contract: define `boot_id`, baseline sample semantics, monotonic-to-wall-clock conversion, restart/reboot comparison, clock rollback handling, stale baseline fallback, and fixtures.
11. Minimum command lease TTL policy: raise the global minimum lease TTL or define per-command `recommended_min_ttl_seconds` with rollout lint warnings below recommendation, especially for `provider_session.shutdown`.

The P083 proof gate must fail when any item above lacks the code, schema, migration, metric, UI, readback, or fixture evidence required by its outcome.

## 1. Problem

Execution truth keeps leaking across layers:

- approval provenance can be confused between caller-supplied identity and durable approval ownership;
- artifact linkage has required follow-up corrections after implementation closeout;
- per-attempt attribution and lineage truth were added after main implementation work;
- lifecycle commands can accept `run_id`, `stage_id`, `approval_id`, or artifact ids without always naming which record owns truth.

This creates a repeated review class: the code works for the happy path, but downstream systems can trust caller-owned copies where the durable record should be authoritative.

## 2. Decision

For every lifecycle command and projection path, the repository must name the single source of truth for:

- `run_id`;
- workflow `stage_id`;
- `stage_execution_id`;
- `agent_execution_id`;
- `approval_id`;
- artifact id and artifact linkage;
- side-effect idempotency key.

Caller-supplied identifiers are selectors or hints. They are not authority unless the ownership matrix explicitly says they are.

## 3. Ownership Matrix

Add canonical artifact:

```text
docs/reference/execution-truth-ownership-matrix.md
```

Initial ownership rows:

| Field | Caller may supply | Authoritative record | Downstream rule | Invariant test |
|---|---:|---|---|---|
| `run_id` | yes | `runs.id` | command handler reloads run before mutation | caller run id cannot override loaded run |
| workflow `stage_id` | yes | workflow snapshot + latest stage lookup | convert to active/latest `stage_execution_id` before mutation | retry uses latest eligible execution |
| `stage_execution_id` | diagnostic only unless tool declares it | `stage_executions.id` | cannot stand in for workflow stage id unless contract says so | UUID-vs-stage-id rejection |
| `agent_execution_id` | diagnostic or retry target | `agent_executions.id` + supersession lineage | active attempt is latest non-superseded attempt | late output cannot attach to active attempt |
| `approval_id` | yes | `approvals.id` + approval status | actionability comes from durable approval + caller policy | caller provenance cannot approve inactive approval |
| artifact linkage | no for writes; diagnostic for reads | artifact record + stage/agent execution link | projections derive links from execution truth | artifact cannot link to superseded attempt unless historical |
| side-effect idempotency | generated by service | side-effect ledger | retry blocked until effect reconciled | duplicate external effect blocked |

## 4. Required Behavior

### 4.1 Command handlers

Every mutating command must:

- load the authoritative record before mutation;
- validate selector identifiers against that record;
- write command journal provenance separately from execution truth;
- reject mismatched caller-supplied lineage with a typed denial.

### 4.2 Projections

Read models must derive execution lineage from durable records, not from command payload copies.

### 4.3 Artifacts

Artifact rows must preserve both:

- active lineage for current workflow reasoning;
- historical lineage for superseded attempts.

The UI may display historical artifacts, but stage/action decisions must ignore superseded lineage unless explicitly inspecting history.

### 4.4 Approvals

Approval actionability must be based on approval status, approval scope, caller class, and policy. UI/client-provided provenance is never enough to authorize settlement.

## 5. Tests

Add proof gate:

```text
proposal-083|p083
```

Required tests:

- one invariant test per ownership matrix row;
- caller-supplied mismatched `run_id`/`stage_id`/`approval_id` rejection;
- superseded agent output cannot update active stage/artifact linkage;
- approval settlement ignores caller-owned provenance when durable approval is inactive;
- artifact projections preserve historical attempt labels without using them as active truth.

## 6. Non-Goals

- Do not change workflow semantics by itself.
- Do not remove historical evidence.
- Do not make UI the authority for execution truth.
- Do not rely on GraphQL or MCP payload shape as durable provenance.

## 7. Acceptance Criteria

P083 is complete when:

1. the ownership matrix exists under `docs/reference/`;
2. every lifecycle command names its authoritative records;
3. invariant tests prove caller-owned copies cannot override durable truth;
4. artifact and approval projections expose historical context without changing active execution truth.
