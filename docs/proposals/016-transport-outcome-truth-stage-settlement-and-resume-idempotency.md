# Proposal 016: Transport Outcome Truth, Stage Settlement, and Resume Idempotency

| Field | Value |
|---|---|
| Date | 2026-03-29 |
| Status | Implemented |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/workflow-execution-engine.md](../reference/workflow-execution-engine.md), [reference/operator-experience.md](../reference/operator-experience.md), [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md), [reference/mvp-sign-off.md](../reference/mvp-sign-off.md), [reference/current-system-baseline.md](../reference/current-system-baseline.md) |
| Scope | Transport outcome normalization, atomic stage settlement, aggregate-step settlement truth, resume / relaunch idempotency, stale execution repair, runtime binding provenance migration over existing frozen-binding truth, and report / recovery alignment to canonical execution truth |
| Goal | Repair the lower execution-truth layer underneath the current 011/013 persistence seams so every agent and stage attempt settles exactly once with durable canonical truth, and later contract, report, and recovery surfaces stop describing contradictory runtime history. |

> **Implementation note:** This proposal is now implemented. Use [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) as the stable contract and [../evidence/execution-truth-and-recovery-proof.md](../evidence/execution-truth-and-recovery-proof.md) as the consolidated proof story. The review, evidence-pack, research-pack, and implementation-audit files remain historical implementation artifacts.

---

## 1. Context

Run `B18A8E99-287E-4383-BCA6-9494DAE059A4` shows that the stable failure class sits below output-contract alignment.

The evidence already shows all of the following:

- proposal-review fan-out artifacts exist on disk,
- the aggregate step never produced `proposal_review_summary`,
- old `running` and `waitingApproval` records survive beside newer attempts,
- receipts and transcripts can say “success” while also carrying a timeout error,
- reports can point to the wrong failed agent and the wrong retry path,
- configured provider/model truth can diverge from what actually executed.

Recent runs also show one more runtime-truth defect in the same layer:

- provider or app usage limits can terminate a session with partial output on disk while receipts and transcripts still present ordinary success.

This means the primary defect is not “the report describes the wrong contract failure.”
It is “the runtime cannot yet say, once and only once, what actually happened in an attempt after output, timeout, relaunch, and repeated stage entry.”

Proposal 016 is therefore a migration slice over runtime-truth surfaces that already exist in the codebase:

- **Proposal 011-era persisted truth already present in `Run`**:
  - `runtimeTrustLevel`
  - `providerBindingSnapshotJSON`
  - `bindingProvenanceJSON`
  - cancellation settlement fields
- **Proposal 013-era persisted truth already present in `AgentExecution` / `StageExecution`**:
  - `validationFailureJSON`
  - `outputEnvelopesJSON`
  - `retryMode`
  - `triggerReason`
  - `supersedesAttemptNumber`
  - `evidencePacketJSON`
  - `recoverySnapshotJSON`

Proposal 016 does not replace Proposal 013 or declare it invalid.
It tightens the lower execution-truth substrate those surfaces depend on.

### 1.1 What the incident proves

The app currently has at least five overlapping truth defects:

1. **Transport outcome truth mismatch**
   - one `AgentExecution` can look simultaneously successful and errored.
2. **Stage settlement mismatch**
   - later attempts can start while earlier stage records still claim `running`.
3. **Resume idempotency failure**
   - relaunch / recovery can multiply active stage or approval records for one logical lineage.
4. **Aggregate-step invisibility**
   - fan-out work can complete, but the aggregate transition is not treated as first-class settlement truth.
5. **Provider binding truth mismatch**
   - receipts, reports, and runtime provenance can disagree about which provider/model actually executed.
6. **Limit-exhaustion truth mismatch**
   - provider/app usage limits can end the stream after useful output, but the runtime still records `succeeded: true` because it treats `Finish: stop` as success by default.

### 1.2 How Proposal 016 relates to Proposal 013

Proposal 013 remains a valid bounded contract and recovery-hardening slice.

Proposal 016 is a companion migration proposal that fixes the lower runtime truths Proposal 013 reads:

- what terminal outcome actually happened,
- which stage execution is canonical,
- whether a stale active record had to be repaired,
- whether the aggregate step settled,
- which provider/model truth is frozen configuration versus actual runtime evidence.

That makes Proposal 016 a causally earlier fix in the incident chain, but not a reason to invalidate Proposal 013 or restate it as “wrong.”

### 1.3 What this proposal is not

Proposal 016 is **not**:

- a general workflow redesign,
- a new provider-family proposal,
- a broad contract-schema redesign,
- a UI-polish proposal,
- a skill-resolution proposal,
- a release-readiness gate,
- or a runtime audit of whether a new feature works.

It is specifically about execution truth and settlement truth in the host system.

---

## 2. Product questions this proposal must answer

After Proposal 016, the engineer must be able to answer all of these with persisted evidence rather than inference:

1. Did the agent complete, complete with a transport error after output, fail before output, fail after output validation, time out before output, time out after output, cancel before output, cancel after output, hit limits before output, or hit limits after output?
2. Can one logical stage lineage ever have more than one active `StageExecution` at the same time?
3. When output exists but transport later times out or errors, does the runtime preserve that output and settle the attempt consistently?
4. Can relaunch or resume repair stale `running` or `waitingApproval` records before creating new attempts?
5. Does the aggregate step have first-class runtime truth instead of being inferred from fan-out artifacts?
6. Do reports and recovery surfaces show the actual runtime provider/model and the actual canonical failed step rather than configured or inferred truth?
7. When the provider or app stops an execution because limits are exhausted, does the runtime preserve partial output truth without falsely recording success?

Proposal 016 is done only when all seven answers are explicit in the persisted model, operator surfaces, and test evidence.

---

## 3. What we build

Proposal 016 delivers five tightly coupled layers.

### Layer R: Agent Outcome Truth

| Component | Responsibility |
|---|---|
| **AgentOutcomeClassifier** | Classifies one canonical terminal outcome for each `AgentExecution` from raw output, receipt, transcript, validation result, and transport error data |
| **ExecutionReceiptV2** | Persists normalized raw outcome evidence: output presence, timeout / stream error metadata, receipt references, validation status, and timestamps |
| **AgentOutcomeReadBridge** | Makes UI, reports, and recovery read the canonical outcome first and legacy `status` only as a fallback |
| **LimitExhaustionBridge** | Classifies provider/app limit exhaustion and maps provider stop reasons, quota errors, and `Finish: stop` cases into truthful terminal outcomes |

### Layer S: Atomic Stage Settlement

| Component | Responsibility |
|---|---|
| **StageSettlementCoordinator** | Settles each stage exactly once after persistence, transport-outcome classification, validation, and aggregate checks complete |
| **StageTerminalityGuard** | Prevents a later stage attempt from starting while an earlier execution in the same logical lineage still claims active ownership |
| **AggregateSettlementRecord** | Persists first-class truth for aggregate steps such as `aggregate_proposal_reviews` and their required outputs |

### Layer T: Resume / Relaunch Idempotency

| Component | Responsibility |
|---|---|
| **ActiveExecutionUniquenessGuard** | Enforces no more than one active `StageExecution` and no more than one active approval lineage for one logical stage lineage |
| **StartupSettlementRepair** | Repairs or blocks stale `running` / `waitingApproval` records before new work starts after relaunch |
| **ResumeLineageResolver** | Resolves whether resume continues the same logical lineage or must create a distinct retry / clone lineage |

### Layer U: Binding Provenance Migration

| Component | Responsibility |
|---|---|
| **RuntimeBindingTruthResolver** | Reads existing frozen binding snapshot/provenance plus runtime receipt evidence and classifies what is authoritative, downgraded, or unverifiable |
| **RuntimeTruthDowngradeRules** | Defines when missing or contradictory runtime receipt/session evidence must downgrade provider/model truth to `unverifiable` |
| **RunReportRuntimeBindingReader** | Makes reports and operator surfaces read actual runtime execution truth and present frozen configuration as comparison context rather than runtime fact |

### Layer V: Recovery / Report Alignment

| Component | Responsibility |
|---|---|
| **CanonicalRecoverySnapshot** | Persists the exact failed step, aggregate state, narrowest valid next actions, why those actions are valid, and when limit/policy-bound terminal stops are non-auto-retryable by default |
| **RunReportTruthBridge** | Builds report timeline, failure summaries, retry path, and resume path from canonical settlement and recovery records instead of raw historical scans |
| **BlockedRunRepairPanel** | Explains stale-record repair, aggregate failure, and runtime-truth classification when the run is blocked or repaired at startup |

---

## 4. Agent execution outcome truth

### 4.1 Current defect

The runtime can currently preserve useful output and still leave the same attempt in an incoherent state:

- raw output exists,
- receipt may say `succeeded: true`,
- transcript may say success,
- error metadata may still carry a timeout or stream failure,
- downstream layers then disagree on whether this was success, partial success, or failure.

### 4.2 Required canonical outcome taxonomy

Every `AgentExecution` in this slice must settle to exactly one canonical terminal outcome:

- `completed`
- `completed_with_transport_error`
- `failed_before_output`
- `failed_after_output_validation`
- `timed_out_before_output`
- `timed_out_after_output`
- `cancelled_before_output`
- `cancelled_after_output`
- `limit_exhausted_before_output`
- `limit_exhausted_after_output`

Rules:

1. One `AgentExecution` may not simultaneously present as generic success and generic failure.
2. If useful output exists and transport later errors, the runtime must classify that explicitly instead of collapsing it into ambiguous `failed`.
3. Validation failure after output generation is distinct from transport failure before output.
4. Truthful operator stop / cancellation must also settle into this taxonomy rather than bypassing it through only `AgentStatus.cancelled`.
5. `Finish: stop`, `session_closed`, or other neutral transport-finish markers describe transport termination only; they are **not** sufficient to classify success on their own.
6. Provider/app limit exhaustion must classify explicitly into the canonical taxonomy rather than being flattened into generic timeout or generic success.
7. Success requires an explicit success criterion plus durable output semantics; neutral finish markers alone are never that criterion.
8. Outcome classification must persist enough raw evidence that later report and recovery layers do not need to reinterpret receipts heuristically.

### 4.3 Explicit persisted storage contract

Proposal 016 introduces one explicit storage contract for this new taxonomy.

Rules:

1. `AgentStatus` stays the coarse lifecycle field used for broad run progression:
   - `pending`
   - `ready`
   - `running`
   - `completed`
   - `failed`
   - `cancelled`
   - `skipped`
2. The canonical terminal outcome moves into a **new persisted field on `AgentExecution`**:
   - `canonicalOutcome`
3. The diagnostic raw evidence used to explain that outcome moves into a **new persisted field on `AgentExecution`**:
   - `outcomeEnvelopeJSON`
4. The minimum concrete persisted fields added in this slice are:

| Model | New field | Purpose |
|---|---|---|
| `AgentExecution` | `canonicalOutcome` | One explicit terminal outcome from the approved taxonomy |
| `AgentExecution` | `transportErrorKind` | Normalized transport failure kind: timeout, stream, provider, unknown |
| `AgentExecution` | `providerStopReason` | Normalized finish/stop reason from provider or app runtime, including limit exhaustion |
| `AgentExecution` | `outputPresence` | Whether durable output existed before failure/timeout settlement |
| `AgentExecution` | `settledAt` | Exact timestamp when attempt truth became terminal |
| `AgentExecution` | `runtimeProvider` | Actual provider identity from runtime receipt/session truth |
| `AgentExecution` | `runtimeModel` | Actual model identity from runtime receipt/session truth |
| `StageExecution` | `lineageID` | Stable logical lineage identity across retries and repair |
| `StageExecution` | `settlementKind` | How the stage terminated: completed, blocked, failed, repaired, superseded |
| `StageExecution` | `settledAt` | Exact terminal settlement timestamp |
| `StageExecution` | `activeOwnerToken` | Uniqueness token proving which active execution currently owns the lineage |
| `Approval` | `lineageID` | Stable logical approval-gate lineage identity |
| `Approval` | `repairedAt` | Timestamp when a stale approval record was explicitly repaired |

5. This proposal chooses one unambiguous owner model:
   - flattened persisted columns carry canonical outcome truth;
   - `outcomeEnvelopeJSON` is diagnostic supporting evidence only;
   - `outcomeEnvelopeJSON` must not become a second authority for keys that already have first-class persisted columns.
6. `providerReceiptJSON`, `validationFailureJSON`, and `outputEnvelopesJSON` remain authoritative supporting evidence; they are not replaced.
7. Raw receipts, transcripts, headers, and diagnostic envelopes may explain an outcome, but they must never contradict, outrank, or silently overwrite the persisted canonical outcome columns in readers.
8. `providerStopReason` and provider/app-limit evidence must be persisted even when the attempt produced useful output, so recovery/reporting can distinguish partial-success exhaustion from ordinary success.
9. UI, report, and recovery consumers must read outcome truth in this order:

| Consumer | Read order |
|---|---|
| UI detail surfaces | flattened outcome columns (`canonicalOutcome`, `providerStopReason`, `outputPresence`) -> supporting evidence (`providerReceiptJSON`, `validationFailureJSON`, `outcomeEnvelopeJSON`) -> fallback map from `AgentStatus` |
| Report builder | flattened outcome columns -> canonical recovery / stage settlement records -> supporting evidence -> fallback map from `AgentStatus` only for legacy rows |
| Recovery logic | flattened outcome columns -> stage settlement / recovery snapshot -> supporting evidence -> no heuristic retry-path inference from raw historical scans |

10. Legacy rows that predate `canonicalOutcome` may still be read through fallback mapping, but must be labeled as migrated / legacy rather than silently treated as fully authoritative.

This proposal intentionally prefers a **separate persisted outcome field** over overloading `AgentStatus`, because the current status enum is already used broadly by run orchestration and UI grouping and should remain a coarse lifecycle channel.
Truthful cancellation therefore remains visible in `AgentStatus.cancelled`, but its canonical explanation must also be representable in the new outcome columns.

#### 4.3.1 Required mapping from `canonicalOutcome` back to coarse `AgentStatus`

`AgentStatus` remains coarse and lossy on purpose.
Proposal 016 therefore requires one explicit mapping table so orchestration, UI badges, and report grouping do not reinterpret the same terminal outcome differently.

| `canonicalOutcome` | Required coarse `AgentStatus` | Why |
|---|---|---|
| `completed` | `completed` | Clean success |
| `completed_with_transport_error` | `completed` | Durable useful output won; transport defect remains in canonical fields |
| `failed_before_output` | `failed` | No durable output survived |
| `failed_after_output_validation` | `failed` | Durable output exists, but the attempt is terminally invalid |
| `timed_out_before_output` | `failed` | Timeout with no durable output |
| `timed_out_after_output` | `failed` | Timeout after durable output; retry/report must read canonical fields for nuance |
| `cancelled_before_output` | `cancelled` | Truthful operator/runtime cancellation before output |
| `cancelled_after_output` | `cancelled` | Truthful cancellation after durable output |
| `limit_exhausted_before_output` | `failed` | No durable output survived limit exhaustion |
| `limit_exhausted_after_output` | `failed` | Durable output survived, but the attempt did not complete cleanly |

Rules:

1. `AgentStatus` must never be used to recover the lost nuance between `completed_with_transport_error`, `timed_out_after_output`, `failed_after_output_validation`, or `limit_exhausted_after_output`.
2. UI grouping may use coarse `AgentStatus`, but detail surfaces, recovery policy, and reports must always read the canonical fields first.
3. Any new canonical outcome added later must ship with an explicit row in this table before implementation is considered complete.

#### 4.3.2 Receipt authority stack

Proposal 016 explicitly separates three receipt/evidence layers so readers do not invent sideways reconciliation logic:

1. **raw provider/session receipt**
   - provider-native payloads, headers, session-close details, transcripts
2. **normalized `ExecutionReceiptV2` / diagnostic envelope**
   - normalized transport facts and supporting evidence references
3. **flattened persisted columns on `AgentExecution`**
   - `canonicalOutcome`
   - `transportErrorKind`
   - `providerStopReason`
   - `outputPresence`
   - `settledAt`
   - `runtimeProvider`
   - `runtimeModel`

Rules:

1. Flattened persisted columns are canonical for readers.
2. Readers must not reconcile sideways between raw receipt JSON and envelope JSON once flattened columns already exist.
3. Raw receipts and normalized envelopes exist to explain canonical columns, not to compete with them.
4. Reader order is fixed:
   - flattened columns first
   - normalized envelope / `ExecutionReceiptV2` second
   - raw receipt/session artifacts third
   - legacy fallback mapping last
5. If the raw receipt contradicts the flattened columns on a non-legacy row, the runtime must treat that as a writer-time bug or migration defect, not as permission for the reader to “choose the better truth.”

### 4.4 Legacy migration and backfill policy

Backfill must be deterministic and fail closed when durable evidence is insufficient.

| Legacy durable evidence | Required backfill action |
|---|---|
| receipt/session truth + durable output + timeout/transport error after output | backfill `canonicalOutcome = timed_out_after_output` or `completed_with_transport_error` according to the same reconciliation table used for new rows |
| durable output + validation failure evidence after generation | backfill `canonicalOutcome = failed_after_output_validation`; persist migrated failure-evidence references |
| provider/app limit evidence + no durable output | backfill `canonicalOutcome = limit_exhausted_before_output` |
| provider/app limit evidence + durable output | backfill `canonicalOutcome = limit_exhausted_after_output` |
| only `AgentStatus.failed` or similarly coarse status with no durable supporting evidence | mark row as `legacy_unverifiable`; do not guess a canonical outcome |
| missing lineage fields for old `StageExecution` / `Approval`, but stable derivation is possible from persisted retry/approval evidence | derive `lineageID` once during migration and mark the row as migrated |
| missing lineage fields for old `StageExecution` / `Approval`, and stable derivation is not possible without guesswork | mark lineage as legacy/unverifiable and block startup repair for that row until operator repair or explicit migration tooling resolves it |

Rules:

1. Backfill must use the same canonical reconciliation rules as live settlement; it must not invent a second legacy-only classifier.
2. If backfill cannot be performed without inference beyond durable evidence, the row must be marked legacy / `unverifiable` rather than guessed into a terminal outcome.
3. Startup repair must not mutate legacy rows whose lineage identity cannot be derived deterministically.

---

## 5. Atomic stage settlement and aggregate truth

### 5.1 Current defect

The motivating run shows physically impossible stage history:

- old `Proposal drafted` executions still claim `running`,
- newer `Proposal drafted` executions already completed or failed,
- multiple approval gates can remain `waitingApproval`,
- aggregate truth can disappear behind raw fan-out artifacts.

### 5.2 Required settlement ordering

For every stage attempt, the runtime must follow one canonical ordering:

1. persist raw outputs when present
2. persist receipt and transcript artifacts when present
3. classify agent transport outcome
4. run contract or aggregate validation as applicable
5. persist failure evidence or normalized artifacts
6. settle the stage once through `StageSettlementCoordinator`

Rules:

1. Stage settlement happens once.
2. A later attempt in the same logical lineage may not start until the earlier active record is either resumed as the same owner or repaired into terminal state.
3. The aggregate step must participate in settlement truth explicitly when the workflow depends on aggregate output for the transition.

### 5.3 Aggregate-step truth

Aggregate steps such as `aggregate_proposal_reviews` are first-class runtime citizens in this proposal.

They must not be inferred from fan-out artifact presence.
Their terminality still remains stage-owned.

The runtime must persist:

- whether the aggregate step started,
- whether it received the required fan-out inputs,
- whether it produced its required output,
- which canonical outcome it settled with,
- and which recovery action is now valid.

### 5.4 Aggregate settlement model choice

Proposal 016 chooses one explicit model:

- aggregate settlement is persisted as a **separate `AggregateSettlementRecord` type**

It is not treated as an ordinary reviewer `AgentExecution`, and it is not allowed to remain a half-inferred side effect of fan-out artifacts.
It is also **not** a second terminality authority beside the aggregate state's `StageExecution`.

Minimum fields:

- `id`
- `runID`
- `stageExecutionID`
- `aggregateStepID`
- `lineageID`
- `canonicalOutcome`
- `inputCoverageJSON`
- `outputArtifactName`
- `validationFailureJSON`
- `evidencePacketJSON`
- `settledAt`

Rules:

1. The aggregate state's `StageExecution` remains the canonical owner of stage terminality.
2. `AggregateSettlementRecord` is subordinate detail attached to that aggregate stage through `stageExecutionID` and `lineageID`.
3. Reports and recovery must resolve aggregate truth through the aggregate state's canonical `StageExecution`, then traverse to `AggregateSettlementRecord` for aggregate-specific evidence.
4. Missing aggregate output after complete fan-out is itself a terminal aggregate outcome, not an inference gap.

---

## 6. Runtime binding truth migration over existing provenance

### 6.1 Current defect

Current reports can describe:

- configured provider/model,
- inferred retry path,
- inferred failed agent,

instead of the actual runtime truth recorded by the execution itself.

### 6.2 Migration table from existing persisted truth

Proposal 016 does not introduce a second abstract provider-truth stack.
It reuses and clarifies the fields that already exist in the codebase.

| Existing field / evidence | Meaning after Proposal 016 | Primary consumer | Downgrade / warning rule |
|---|---|---|---|
| `Run.providerBindingSnapshotJSON` | Frozen start-time binding intent for each agent | comparison context in report / UI | never shown alone as runtime fact |
| `Run.bindingProvenanceJSON` | Frozen explanation of how the model was resolved at run start | report / UI provenance panels | if no runtime receipt exists, provenance may remain frozen-only and runtime truth becomes `unverifiable` |
| `AgentExecution.provider` + `resolvedModel` + `resolvedBackendProfileID` + `configuredProviderID` | Per-attempt resolved fields captured during execution | recovery / report detail | if they conflict with receipt/session evidence, receipt/session wins and these fields become secondary |
| `AgentExecution.providerReceiptJSON` | Best runtime evidence for actual provider/model/transport result | canonical runtime truth reader | missing, malformed, or contradictory receipt downgrades runtime truth to `unverifiable` unless stronger session evidence exists |
| `Run.runtimeTrustLevel` | Run-level summary badge derived from receipt coverage and verification strength | shell badges / export summaries | may downgrade to `unverifiable` when receipt/session truth is absent, partial, or contradicted |

### 6.3 Runtime truth downgrade rule

Runtime receipt or session evidence may downgrade provider/model truth to `unverifiable` when any of the following is true:

1. no receipt/session evidence exists for the attempt;
2. receipt exists but omits provider/model identity;
3. frozen binding snapshot and runtime receipt materially disagree and no stronger session evidence resolves the conflict;
4. receipt/transcript/session artifacts disagree on whether the attempt actually reached transport completion.
5. provider/app stop reason is present but cannot be distinguished between operator stop, quota exhaustion, or neutral session close.

When downgraded:

- reports must show the frozen binding snapshot as configuration context only,
- operator surfaces must label runtime provider/model truth as `unverifiable`,
- recovery logic must avoid implying stronger provider certainty than the evidence supports.

---

## 7. Ownership, guard placement, and startup repair order

### 7.1 Owner matrix

| Owner | Owns | Must not own |
|---|---|---|
| **WorkflowOrchestrator** | Active execution, output persistence, receipt persistence, validation invocation, aggregate invocation, stage settlement request, approval request creation | startup repair classification, historical retry-path inference, duplicate-approval cleanup across stale records |
| **ResumeManager** | Startup scan, stale active-record detection, lineage classification, repair-or-block decision before new work begins | creating new retry attempts before lineage repair completes, reinterpreting contract failures, inventing provider truth |
| **RecoveryCoordinator** | Reading canonical recovery snapshot, exposing valid next actions, creating retry/clone actions only after lineage is canonical | deciding whether stale records should be repaired at startup, inferring canonical failed step from raw history |
| **Approval persistence** | Durable approval identity, approval request/decision timestamps, approval-lineage uniqueness, decision settlement | deciding runtime recovery strategy or stage ownership on its own |

`RunCancellationCoordinator` remains the owner of cancellation initiation and settlement logging from Proposal 011.
Proposal 016 does not move that responsibility; it requires the new agent-outcome taxonomy and readers to consume that settled cancellation truth consistently.

### 7.2 Approval-lineage identity

Proposal 016 adds an explicit persisted approval-lineage identity requirement.

One logical approval gate must have one stable lineage identity across relaunch and repair.

Preferred contract:

- add `Approval.lineageID`

with semantics:

- stable for the same logical gate across relaunch,
- superseded only when the stage lineage itself is superseded,
- used by startup repair to determine whether a new approval request is a duplicate sibling or a legitimate new lineage.

If the implementation chooses a different field name, the semantics above are mandatory.

### 7.2.1 Required lineage propagation rules

Proposal 016 requires one explicit propagation table so retries, repair, approvals, and aggregate settlement all speak the same lineage language.

| Event | Required lineage rule |
|---|---|
| same-run agent retry inside an existing stage lineage | preserve the existing `StageExecution.lineageID` |
| same-run stage retry | preserve the existing `StageExecution.lineageID` |
| startup repair of stale active stage | preserve the existing `StageExecution.lineageID` |
| startup repair of stale approval sibling | preserve the existing `Approval.lineageID` |
| approval re-arm for the same logical gate | preserve the existing `Approval.lineageID` |
| clone run / clone current config | create a new lineage namespace in the new run; do not reuse stage or approval lineage from the source run |
| aggregate settlement record creation | inherit the aggregate stage's `lineageID` |

Rules:

1. `lineageID` is stable for the same logical lineage and only changes when a new run or explicitly new logical lineage is created.
2. A repair pass must never invent a fresh lineage for a stale record that clearly belongs to an existing lineage.
3. Aggregate settlement must not introduce a parallel lineage vocabulary.

### 7.3 Guard placement on create-paths

`ActiveExecutionUniquenessGuard` is not only a startup repair idea.

It must sit on the boundary where new active records are created:

- before a new `StageExecution` becomes `running`
- before a stage is moved into `waitingApproval`
- before a new `Approval` record is persisted as the active gate for the same lineage

Rules:

1. create-path prevention is primary;
2. `StartupSettlementRepair` is secondary cleanup for stale data that already escaped prevention;
3. no runtime component may rely on startup repair as the normal way to prevent duplicate active siblings.

### 7.4 Startup repair order

Startup / relaunch must follow this order:

1. load runs with active stage and approval records;
2. derive logical stage lineage and approval lineage identity;
3. inspect persisted receipts, transcripts, outputs, and validation evidence for stale active executions;
4. repair or block stale `running` / `waitingApproval` records so only one active owner remains per lineage;
5. only then expose resume, retry, or clone actions and only then allow `WorkflowOrchestrator` to create new active attempts.

Recovery policy rule:

- limit exhaustion and provider policy-bound terminal stops are non-auto-retryable by default;
- a narrower retry/resume action may appear only when the canonical recovery snapshot records an explicit provider-aware or operator-approved override.

This order is mandatory so the app stops multiplying active stage or approval records on startup.

### 7.5 Deterministic reconciliation table

Startup repair and relaunch must use one shared reconciliation table.

| Observed durable evidence | Required canonical outcome / repair |
|---|---|
| output exists + receipt/transcript show timeout after output | settle `AgentExecution` as `timed_out_after_output` or `completed_with_transport_error`; then apply stage-level validation/aggregate rule |
| no output + timeout only | settle `AgentExecution` as `timed_out_before_output` |
| operator cancellation requested before durable output | settle `AgentExecution` as `cancelled_before_output`; preserve cancellation-settlement evidence |
| operator cancellation requested after durable output already exists | settle `AgentExecution` as `cancelled_after_output`; preserve output and cancellation-settlement evidence |
| provider/app reports quota, budget, rate-limit, or limit exhaustion before durable output | settle `AgentExecution` as `limit_exhausted_before_output`; preserve provider/app limit evidence; default recovery to non-auto-retryable unless canonical recovery snapshot records a narrower override |
| provider/app reports quota, budget, rate-limit, or limit exhaustion after durable output exists | settle `AgentExecution` as `limit_exhausted_after_output`; preserve partial output plus provider/app limit evidence; default recovery to non-auto-retryable unless canonical recovery snapshot records a narrower override |
| provider returns policy/safety/blocklist/prohibited-content terminal stop | settle into the existing canonical failure outcome that matches output presence and validation state; preserve provider stop reason; default recovery to non-auto-retryable unless canonical recovery snapshot records a narrower override |
| transcript/receipt ends with `Finish: stop` or neutral stop marker, but no explicit success criterion is satisfied | do not classify success from finish marker alone; inspect output presence plus provider/app stop reason and settle accordingly |
| output exists + validation failure after generation | settle `AgentExecution` as `failed_after_output_validation`; persist `ValidationFailureRecord`; do not collapse to generic transport failure |
| stale `waitingApproval` with unresolved canonical approval lineage and no newer active sibling | restore the same gate using the same approval lineage |
| stale `waitingApproval` with duplicate active sibling in same lineage | repair the older duplicate to non-active state and keep one canonical owner |
| stale `running` with no live owner and no durable evidence | repair to `blocked`; do not silently resume |
| stale `running` with durable output/receipt evidence sufficient to classify terminally | classify and settle terminally before any new attempt begins |
| fan-out outputs complete but no aggregate record exists | create terminal `AggregateSettlementRecord` with explicit missing-output / validation-failure outcome; do not infer success |
| receipt and transcript disagree on completion truth | downgrade runtime truth to `unverifiable` unless a stronger receipt/session record resolves the conflict |

---

## 8. Verification

Proposal 016 requires all of the following.

### 8.1 Unit and integration proof

- outcome-classification tests for all canonical agent terminal outcomes
- cancellation-bridge tests proving Proposal 011 cancellation settlement maps cleanly into `cancelled_before_output` / `cancelled_after_output`
- limit-exhaustion tests proving provider/app quota exhaustion maps cleanly into `limit_exhausted_before_output` / `limit_exhausted_after_output`
- finish-marker tests proving `Finish: stop` does not become success without an explicit success criterion
- recovery-policy tests proving limit-exhaustion and provider policy-bound terminal stops default to non-auto-retryable unless a narrower override is persisted in the canonical recovery snapshot
- storage-contract tests proving `canonicalOutcome` and `outcomeEnvelopeJSON` stay consistent with `AgentStatus`
- schema migration tests proving the new persisted fields on `AgentExecution`, `StageExecution`, `Approval`, and `AggregateSettlementRecord` are populated and readable without artifact scans
- receipt-normalization tests proving one attempt cannot persist contradictory success/error truth
- stage-settlement tests proving a stage settles once and later attempts do not start beside a still-active sibling
- aggregate-settlement tests proving aggregate steps are first-class runtime truth
- startup-repair tests proving stale `running` and `waitingApproval` records are repaired or blocked before new execution begins
- binding provenance migration tests proving report surfaces read frozen truth and runtime truth separately and downgrade to `unverifiable` when evidence is insufficient
- report-builder tests proving retry path and failure summaries are derived from canonical recovery and settlement records

### 8.2 Motivating-run replay proof

One canonical regression fixture based on the failure class of run `B18A8E99-287E-4383-BCA6-9494DAE059A4` must prove:

1. fan-out outputs can exist,
2. aggregate output can still fail or be absent,
3. stale active records are repaired before new work starts,
4. no contradictory receipt truth survives,
5. reports identify the actual blocked step and the actual narrowest valid next action,
6. provider/app limit exhaustion with partial output does not collapse into ordinary success.

### 8.3 App-level proof

At least one app-launched run must prove:

1. useful output can survive a transport timeout or stream error with explicit classified outcome,
2. no logical stage lineage has two active executions at once,
3. relaunch or resume repairs stale active records before new work begins,
4. reports and recovery surfaces show frozen binding truth separately from actual runtime truth and label unverifiable cases honestly,
5. provider/app limit exhaustion with partial output is surfaced as exhaustion, not success;
6. provider policy-bound terminal stops are surfaced honestly and do not advertise automatic retry unless a narrower override is explicitly recorded.

---

## 9. Acceptance criteria

Proposal 016 is complete only when all of the following are true:

1. every `AgentExecution` settles to exactly one canonical terminal outcome from the approved outcome set, including truthful cancellation and limit exhaustion;
2. the outcome taxonomy is stored explicitly in persisted data through dedicated `AgentExecution` outcome fields rather than inferred only from `AgentStatus`;
3. receipts, transcripts, and persisted execution truth no longer present ambiguous simultaneous success/error state for one attempt;
4. output-preserving timeout or transport-error cases are explicitly classified and preserved rather than collapsing into generic failure;
5. provider/app limit exhaustion and neutral stop markers do not collapse into ordinary success when explicit success criteria are absent;
6. limit exhaustion and provider policy-bound terminal stops are non-auto-retryable by default unless the canonical recovery snapshot records a narrower explicit override;
7. one logical stage lineage cannot have more than one active `StageExecution` at a time;
8. one logical approval lineage cannot have more than one active approval record at a time;
9. `ActiveExecutionUniquenessGuard` prevents duplicate active stage/approval records on create-paths and `StartupSettlementRepair` only handles residual stale records;
10. relaunch / resume repairs or blocks stale active records before new attempts begin, following one documented startup repair order and one deterministic reconciliation table;
11. aggregate steps use a first-class persisted settlement record that is subordinate to the aggregate state's canonical `StageExecution`, not a parallel terminality authority;
12. reports and recovery surfaces derive timeline, failed-step identity, retry path, resume path, failure summaries, and limit-exhaustion narrative from canonical settlement and recovery records;
13. reports and operator surfaces show frozen binding truth separately from actual runtime binding evidence and explicitly downgrade to `unverifiable` when runtime evidence is insufficient or contradictory;
14. ownership boundaries are explicit across `WorkflowOrchestrator`, `ResumeManager`, `RecoveryCoordinator`, and approval persistence.

---

## 10. Relationship to Proposal 013

Proposal 013 and Proposal 016 are complementary, not mutually exclusive.

- Proposal 013 owns output contracts, aggregate contract hardening, failure evidence, and narrow recovery presentation.
- Proposal 016 owns transport outcome truth, stage settlement truth, resume idempotency, approval-lineage uniqueness, and binding-provenance migration.

If implementation work is split, the lower runtime-truth repair from Proposal 016 should be applied before trusting report/recovery conclusions from Proposal 013, but Proposal 013 does not need to be re-authored as invalid or “waiting for 016 to exist.”

---

## 11. Out of scope

Proposal 016 does **not** own:

- reviewer contract schema redesign,
- aggregate contract schema redesign,
- declarative skill resolution and runtime injection,
- broad transport policy enforcement beyond canonical outcome truth,
- design-system work,
- proposal-drafting compaction,
- or feature-readiness validation of newly implemented behavior.

Those higher-layer concerns remain in Proposal 013 and Proposal 015.
