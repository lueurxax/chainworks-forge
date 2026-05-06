# Chainworks Forge Roadmap Update After Latest Main Commit

## Navigation

- Current short roadmap: [../ROADMAP.md](../ROADMAP.md)
- Implemented-system reference index: [../reference/README.md](../reference/README.md)
- Current system baseline: [../reference/current-system-baseline.md](../reference/current-system-baseline.md)
- UI action boundary reference: [../reference/ui-action-boundary.md](../reference/ui-action-boundary.md)
- Rust control plane reference: [../reference/rust-control-plane.md](../reference/rust-control-plane.md)
- P031 write-path guide: [../reference/p031-operator-write-path-guide.md](../reference/p031-operator-write-path-guide.md)
- P031 closeout evidence: [dogfood](../evidence/p031-dogfood-signoff.md), [degraded state](../evidence/p031-degraded-state-evidence.md), [freshness](../evidence/p031-freshness-baseline.md), [accessibility](../evidence/p031-ux-accessibility-signoff.md)

## Status

This roadmap update is based on the current public `main` branch.

The repository has moved forward enough that the stabilization plan should be adjusted.

The main change is:

> UI/action boundary work has become mostly a closeout/gate problem, while persistence pressure and durable side-effect safety have become the next core architecture risks.

---

## 1. Current Progress Assessment

### 1.1 Strong Progress

#### UI action boundary is now reference-level truth

The repository now has a dedicated [UI action boundary reference](../reference/ui-action-boundary.md) that defines the correct target boundary:

- SwiftUI is GraphQL-only.
- SwiftUI can use GraphQL reads and subscriptions.
- SwiftUI can use only two mutations:
  - `approveApproval`
  - `rejectApproval`
- SwiftUI must not use MCP, direct SQLite, local workflow mutation fallback, or broad GraphQL command mutations.
- MCP owns all non-approval operator actions.
- Agents and automations must not use GraphQL mutations or SQLite as a control path.

This is a major cleanup compared with earlier ambiguity.

#### P031 has been corrected conceptually

P031 now explicitly says it is partially superseded by the UI action boundary and remains the GraphQL thin UI read/subscription migration plus approval-only mutation stop-state.

That means P031 is no longer the owner for create/start/cancel/retry/reset/compact/clone/recover/runtime/context actions.

The durable P031 navigation points are:

- [P031 operator write-path guide](../reference/p031-operator-write-path-guide.md)
- [Query projections and client consumption contract](../reference/query-projections-and-client-consumption-contract.md)
- [P031 closeout evidence](../evidence/p031-dogfood-signoff.md)

This is the correct shape.

#### P066 is now scoped correctly

P066 is now an implementation-ready R4 proposal.

It clearly owns provider toolchain cache mapping only:

- Swift/Xcode cache roots,
- Go cache roots,
- `TOOLCHAIN_HOME`,
- `toolchain_cache_policy`,
- bounded diagnostics.

Related reference:

- [Provider toolchain cache mapping](../reference/acp-runtime-transport.md#toolchain-cache-mapping)

It explicitly does not own:

- release side-effect settlement,
- duplicate push prevention,
- SQLite write pressure,
- workflow transitions,
- operator control surfaces.

This is a good correction.

#### P075 and P078 now address the root durability concern

[P075](../proposals/075-local-persistence-write-budget-and-evidence-spooling.md) correctly states that SQLite must remain a compact local control-plane DB, not a runtime event stream.

[P078](../proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation.md) correctly states that external side effects require durable intent, idempotency, readback, reconciliation, and retry blocking.

This pair is now the most important safety foundation after the UI/action boundary.

#### P079 and P080 appeared as recovery-quality proposals

[P079](../proposals/079-contract-aware-output-repair-and-provider-fallback.md) covers contract-aware output repair and provider fallback.

[P080](../proposals/080-continuous-stale-execution-reconciliation.md) covers continuous stale execution reconciliation.

Both are useful, but both must remain downstream of P075/P078 safety rails.

---

## 2. Current Remaining Risks

### 2.1 Boundary is documented, but code still needs closeout

The current code already has approval-only behavior in the right direction, but the GraphQL schema still exposes legacy/control mutation resolvers behind policy gates.

That means the boundary should not be treated as fully done until:

- `ui_operator` / default operator cannot execute non-approval mutations,
- tests enforce this,
- old broad mutation permissions are clearly legacy/test/admin-only,
- P031/P068/P046 align with the reference boundary.

Related docs:

- [UI action boundary](../reference/ui-action-boundary.md)
- [P068 GraphQL UI boundary proposal](../proposals/068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md)
- [P081 API/auth contract matrix](../proposals/081-boundary-first-api-auth-contract-matrix.md)
- [P046 session management GraphQL API](../proposals/046-session-management-graphql-api.md)

### 2.2 SQLite write pressure is now a first-class risk

The system already uses SQLite for many control-plane concerns.

P075 is correct: adding more durability state without write discipline would make the database behave like an event stream.

This is now the most important technical risk before implementing P078 and P038.

Related docs:

- [P075 local persistence write budget](../proposals/075-local-persistence-write-budget-and-evidence-spooling.md)
- [P038 run compaction and artifact governance](../proposals/038-run-compaction-artifact-governance-and-canonical-snapshot-maintenance.md)
- [Rust control plane reference](../reference/rust-control-plane.md)

### 2.3 Retry/recovery is unsafe until side-effect ledger lands

P078 identifies that retry of release-side-effect stages can duplicate external actions.

Until P078 lands, retry automation must fail closed around release/publish/git stages.

P076/P080 should not perform active repair/retry for side-effect lanes until P078 is implemented.

Related docs:

- [P078 durable side-effect ledger](../proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation.md)
- [P076 auto-retry observation ledger](../proposals/076-auto-retry-observation-ledger-and-recovery-policy.md)
- [P080 continuous stale execution reconciliation](../proposals/080-continuous-stale-execution-reconciliation.md)
- [Execution truth and recovery](../reference/execution-truth-and-recovery.md)

### 2.4 P073 and ROADMAP need stronger current alignment

[P073](../proposals/073-stability-freeze-regression-budget-and-refactor-plan.md) should remain a stabilization operating mode, not a normal feature.

[../ROADMAP.md](../ROADMAP.md) should now explicitly include P075/P078/P079/P080/P081/P082/P083/P084 and should not only mention older P031/P038/P046/P068 style work.

---

## 3. Updated Implementation Order

### 3.1 Immediate operating mode

#### Step 0 - P073 freeze mode

Run [P073](../proposals/073-stability-freeze-regression-budget-and-refactor-plan.md) as a continuous operating mode.

Allowed work:

- boundary cleanup,
- P066 scoped implementation,
- P075 write budget,
- P078 side-effect safety,
- P031 closeout,
- P038 compaction after P075,
- P070/P083 consolidation after core safety rails.

Frozen work:

- new ACP provider families,
- new MCP tools unless required by boundary/safety,
- new UI surfaces,
- new context-strategy experiments,
- new agent roles,
- speculative runtime features.

### 3.2 First priority: make implementation proposals executable

#### Step 1 - P084 minimal rollout-gate template

Do a minimal [P084](../proposals/084-executable-rollout-gates-and-observability-contract.md) slice before major P075/P078 implementation.

Minimum deliverables:

- rollout template,
- gate registration convention,
- required fields for implementation proposals:
  - migrations,
  - gates,
  - metrics,
  - projection/readback,
  - hold conditions,
  - rollback disposition.

Do not expand P084 into a large product feature.

Purpose:

> Make P075/P078 executable and measurable before they touch core persistence/recovery behavior.

### 3.3 Boundary closeout

#### Step 2 - UI action boundary / P072 closeout

Close this as a gate, not a large new workstream.

Deliverables:

- [UI action boundary](../reference/ui-action-boundary.md) is canonical.
- GraphQL tests prove approval-only UI mutations.
- Non-approval mutations are denied for UI/default operator principals.
- Broad mutation fixtures are marked legacy/test/admin-only.
- Docs no longer describe the UI as fully read-only when approvals are allowed.

#### Step 3 - P031 corrected closeout

P031 should close as:

- GraphQL reads,
- GraphQL subscriptions,
- `approveApproval`,
- `rejectApproval`,
- no MCP in UI,
- no local workflow mutation fallback,
- no non-approval GraphQL mutations.

P031 should not own:

- visual polish,
- create/start/cancel/retry/reset/compact/clone,
- broader dogfood/productization tails.

Those remain [P032](../proposals/032-polish-stabilization-and-productization-backlog.md), [P036](../proposals/036-ux-consolidation-and-navigation-simplification.md), and future work.

### 3.4 Parallel infrastructure slice

#### Step 4 - P066 scoped toolchain cache mapping

P066 can proceed in parallel with boundary closeout as long as scope remains narrow.

Allowed:

- Swift/Xcode cache mapping,
- Go cache mapping,
- `toolchain_cache_policy`,
- diagnostics,
- frozen snapshot compatibility,
- Swift read adapter.

Forbidden:

- side-effect ledger,
- SQLite write-budget redesign,
- scheduler language awareness,
- GraphQL/MCP write surfaces,
- release settlement logic.

Reference:

- [ACP runtime transport: toolchain cache mapping](../reference/acp-runtime-transport.md#toolchain-cache-mapping)

### 3.5 Persistence and side-effect safety foundation

#### Step 5 - P075 local persistence write budget

This should happen before P078 and before P038.

First implementation slice:

- minimal `DbWriter` or controlled write lane for new subsystems,
- write classes,
- evidence spooling convention,
- high-volume evidence stays out of SQLite,
- write-pressure metrics,
- projection invalidation discipline.

Do not try to rewrite all existing persistence at once.

P075 should initially protect new P078/P038 paths.

#### Step 6 - P078 durable side-effect ledger

After P075.

First implementation slice:

- release-first;
- `git_commit`;
- `git_push`;
- `build_archive`;
- `connect_upload`;
- durable intent before execution;
- idempotency key;
- readback/reconciliation;
- `effects.inspect`;
- `effects.reconcile`;
- retry blocking for unresolved effects.

Required integration:

- `RetryStage` must check unresolved side effects before superseding old attempts or creating a new attempt.

### 3.6 Recovery automation after side-effect safety

#### Step 7 - P082 recovery/retry test matrix

Use [P082](../proposals/082-recovery-retry-state-machine-test-matrix.md) as the proof layer before enabling broader retry automation.

Minimum rows:

- stale ACP startup,
- scheduler ownership drift,
- retry identifier mismatch,
- release side-effect drift,
- late output after supersede,
- duplicate startup/session.

#### Step 8 - P076 / P080 recovery automation

Only after P078 guard exists.

Allowed early:

- detection-only,
- readback,
- typed stale classifications.

Allowed after P078:

- active repair for non-release lanes,
- release lanes routed to `requires_effect_reconciliation`.

Never retry release/publish/git work while unresolved side effects exist.

#### Step 9 - P079 output repair/fallback

After P075/P078 and preferably after P082 coverage.

P079 is useful, but it introduces repair/fallback behavior.

Keep initial scope narrow:

- proposal_writer,
- proposal_reviewer_*,
- lead_orchestrator,
- no release agents,
- fixture ACP transports only for the required gate.

### 3.7 Cleanup and operator surface maintenance

#### Step 10 - P046 session observability

Read/subscription only.

Do not add `resetSession` to GraphQL.

Reset remains MCP-only.

#### Step 11 - P038 MCP-only run compaction

After P075.

Preferably after P078 so compaction can preserve side-effect reconciliation evidence.

Deliverables:

- `runs.compact` MCP tool,
- GraphQL readback for compaction status/report/snapshot,
- no UI compact mutation,
- archive/dedupe/projection rebuild through P075 write discipline.

### 3.8 Consolidation

#### Step 12 - P083 ownership invariant model

Do [P083](../proposals/083-execution-truth-ownership-invariant-model.md) after P078 or alongside late P078.

Purpose:

- make ownership of run/stage/agent/approval/artifact/side-effect truth explicit,
- stop duplicate truth lanes,
- clarify identity and idempotency invariants.

#### Step 13 - P070 typed-boundary consolidation

Do [P070](../proposals/070-control-plane-architecture-consolidation-and-typed-boundary-refactor.md) after the system has stabilized.

Scope:

- typed boundary refactor,
- shared DTOs,
- shared capability registry,
- workflow policy extraction,
- removal of duplicate implicit JSON/policy paths.

Do not use P070 to add product features.

---

## 4. Recommended Roadmap Text

The short roadmap has been updated in [../ROADMAP.md](../ROADMAP.md).

---

## 5. Future Proposals to Avoid Number Collision

P079 and P080 are already used.

If the team still wants:

- a unified limit dashboard,
- runtime budget observability,
- limit-aware session pool,
- runtime fallback policy,

use new proposal numbers.

Suggested:

- **P086 - Agent Limit Observatory and Runtime Budget Dashboard**
- **P087 - Limit-Aware Session Pool and Runtime Fallback Policy**

Do not reuse P079/P080 for limit work.

---

## 6. Final Verdict

The repository is moving in the right direction.

The most important progress is:

- UI action boundary is now reference-level truth.
- P031 is corrected to approval-only mutation stop-state.
- P066 is correctly scoped and implementation-ready.
- P075/P078 now name the real local-control-plane durability problem.
- P079/P080 are useful but must remain downstream of P075/P078.

The plan should be updated, not replaced.

The new critical path is:

```text
P073
-> P084
-> P072 closeout
-> P066
-> P075
-> P078
-> P082
-> P076/P080
-> P079
-> P031 closeout
-> P038/P046
-> P083/P070
```

The system should not expand capabilities again until the write-budget and side-effect safety rails are in place.

