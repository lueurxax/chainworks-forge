# Proposal 013: Output Contract Alignment, Retry Truth, and Failure Evidence Hardening

| Field | Value |
|---|---|
| Date | 2026-03-28 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/workflow-execution-engine.md](../reference/workflow-execution-engine.md), [reference/operator-experience.md](../reference/operator-experience.md), [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md), [reference/mvp-sign-off.md](../reference/mvp-sign-off.md) |
| Scope | Stage-output contracts, failed-stage persistence, retry semantics, blocked-run recovery extensions, and bounded proposal-output resilience |
| Goal | Eliminate the class of runtime failures where agents produce useful work but the run still blocks because output contracts, persistence ordering, retry bookkeeping, and recovery UX disagree about what happened. |

---

## 1. Context

The current runtime can successfully execute long, artifact-heavy runs, but one recent blocked run made a deeper integrity problem obvious:

- the app reported a failed stage,
- recovery suggested one path while the persisted report suggested a narrower retry path,
- raw reviewer outputs existed on disk,
- but the run still blocked as if nothing trustworthy had been produced.

The incident is not a one-off operator mistake.
It exposes a proposal-level gap in how the app models stage success, failure, retries, and evidence.

The motivating example is run `B18A8E99-287E-4383-BCA6-9494DAE059A4`.
Its persisted reports and artifacts show all of the following at once:

1. `Proposal drafted` originally failed, then later succeeded inside the same `runID`.
2. `Proposal reviewed` later failed, even though all four reviewer outputs were written to disk.
3. Recovery UI preferred full run cloning, while the persisted report already computed a narrower retry path for a specific failed agent.
4. The operator could not tell whether the problem was agent quality, transport failure, contract mismatch, or persistence corruption.

That combination is not acceptable for MVP hardening.
Proposal 013 is the bounded slice that fixes it.

### 1.1 What the incident proves

The app currently has at least four distinct failure classes that can overlap:

1. **Output-contract mismatch**
   - agent outputs do not match the declared contract format, even when the human-readable content is usable.
2. **Retry truth mismatch**
   - recovery mutates an existing stage as if the run is retrying in place, while orchestration later creates a fresh execution record that resets attempt semantics.
3. **Failure-evidence mismatch**
   - raw output files can exist on disk without receipts, transcripts, validation reports, or stage-level provenance being persisted coherently.
4. **Recovery-action mismatch**
   - the runtime can already infer a precise retry path, but the operator only sees a coarse clone-run action.

### 1.2 Why this proposal is needed now

Proposal 007 proved the full repo-backed delivery slice.
Proposal 008 is about sign-off and launch gate.

Neither of those should absorb this problem as incidental cleanup because this is not polish.
It is a runtime-truth defect:

- stage success is ambiguous,
- retry history is ambiguous,
- failure evidence is incomplete,
- recovery ownership is misleading.

If this remains unresolved, future blocked runs will keep looking random even when the underlying cause is deterministic.

### 1.3 What this proposal is not

Proposal 013 is **not**:

- a redesign of the full workflow model,
- a new provider family proposal,
- a general UI-polish bucket,
- parallel-agent orchestration expansion,
- or broad artifact format unification across the whole app.

It is specifically about making failed and retried stages truthful, diagnosable, and recoverable.

---

## 2. Product questions this proposal must answer

After Proposal 013, the engineer must be able to answer all of these with persisted evidence rather than inference:

1. Did the agent fail before producing an output, or did it produce a usable output that violated a declared contract?
2. Is this run retrying the same failed stage, retrying one agent inside that stage, or starting a new cloned run?
3. Can the operator see and execute the narrowest valid recovery action from the blocked-run surface?
4. When validation fails after output generation, are the raw output, receipt, transcript, and validation error all preserved as first-class evidence?
5. Are large proposal outputs bounded so a single oversized document does not silently collapse an otherwise valid drafting stage?

Proposal 013 is done only when all five answers are explicit in the persisted model, operator surfaces, and test evidence.

---

## 3. What we build

Proposal 013 delivers four tightly coupled layers.

### Layer M: Output Contract Alignment

| Component | Responsibility |
|---|---|
| **OutputContractSchemaV2** | Typed schema derived from the existing catalog-backed contract truth, including machine format, human-readable companion format, and validation mode |
| **OutputContractResolverV2** | Canonical runtime reader that resolves the typed schema from `AgentCatalog.contracts` and exposes it to validation, persistence, reporting, and recovery |
| **StructuredOutputEnvelope** | Persisted wrapper for structured outputs, raw payload, parsed payload, validation result, and origin metadata |
| **ProposalReviewContractAdapter** | Aligns proposal-review agents and runtime so the declared contract matches the produced artifact format |
| **ValidationFailureRecord** | First-class persisted record describing why output validation failed after agent execution completed |

### Layer N: Retry and Attempt Truth

| Component | Responsibility |
|---|---|
| **StageRetryCoordinator** | Owns retry-in-place versus clone-run semantics and prevents stage/attempt identity drift |
| **StageAttemptHistoryRecord** | Persisted per-stage attempt record with stable attempt numbering and retry cause |
| **AgentAttemptHistoryRecord** | Persisted per-agent attempt lineage for agent-only retries inside one stage attempt |
| **RunRecoveryPolicy** | Computes the narrowest valid recovery action from the actual failed stage and agent state |
| **RecoveryActionSnapshot** | Persisted summary of the recommended and available recovery actions shown to the operator |

### Layer O: Failure Evidence Persistence

| Component | Responsibility |
|---|---|
| **FailedStageEvidenceBuilder** | Persists raw output, transcript, receipt, validation errors, and timing evidence even when the stage later fails |
| **ArtifactPersistenceOrderingPolicy** | Freezes the order of raw-output persistence, receipt persistence, structured validation, and stage settlement |
| **BlockedStageReportBuilder** | Produces one consistent failed-stage report packet for operator and audit consumption |

### Layer P: Recovery UX and Proposal Output Resilience

| Component | Responsibility |
|---|---|
| **RecoverySheet Extension** | Extends the existing shell-owned `RecoverySheet` with precise retry provenance and failure-evidence explanation |
| **BlockedRunRecoveryView Extension** | Extends the existing `BlockedRunRecoveryView` with `Retry Failed Agent`, `Retry Failed Stage`, `Clone Frozen Snapshot`, and `Clone Current Config` when each is valid |
| **FailedStageEvidencePanel** | Shell-owned evidence panel showing raw output presence, validation failure cause, receipt/transcript availability, and recommended next action |
| **ProposalDraftCompactionPolicy** | Applies bounded output-size discipline to proposal drafting and stores truncation/compaction metadata when invoked |

---

## 4. Output contract hardening

### 4.1 Current defect

The current app allows a stage to declare one output contract while agents effectively emit another shape.
The motivating run showed this concretely in `Proposal reviewed`:

- the agent catalog declares proposal reviews as structured JSON,
- reviewer outputs on disk were markdown reviews,
- the runtime treated this as stage failure,
- and the operator did not see a truthful explanation.

### 4.2 Canonical contract source

Proposal 013 does **not** create a second contract authority.

The canonical source of truth remains:

- `AgentCatalog.contracts`

resolved through:

- `OutputContractResolver`

Proposal 013 introduces a stricter typed schema and runtime semantics layer on top of that existing source, not alongside it.

Rules:

1. `OutputContractSchemaV2` is derived from `AgentCatalog.contracts`.
2. `OutputContractResolverV2` is the only runtime reader used by:
   - `WorkflowOrchestrator`
   - `ArtifactManager`
   - `RunReportBuilder`
   - blocked-run recovery surfaces
3. No runtime component may read one contract shape from the catalog and another from an unrelated registry.
4. If the contract schema needs new fields, the catalog contract definition is migrated directly; the typed resolver layer only normalizes it for code.

### 4.3 Required contract model

Every output contract must declare:

- `machine_format`
- `human_format`
- `validation_mode`
  - `strict_structured`
  - `structured_with_human_companion`
  - `human_only`
- `required_fields`
- `raw_artifact_name`
- `normalized_artifact_name`

Rules:

1. A contract that declares `strict_structured` may not silently accept human-readable prose in place of the machine payload.
2. A contract that declares `structured_with_human_companion` must persist both:
   - machine-valid structured output,
   - human-readable rendered companion artifact.
3. If the app wants proposal reviews as markdown, the contract must say markdown.
4. If the app wants proposal reviews as JSON, the agents must actually emit JSON and any rendered markdown must be explicitly secondary.

### 4.4 First mandatory adopter

Proposal review outputs are the first mandatory adopter:

- `proposal_review_ui`
- `proposal_review_ux`
- `proposal_review_architect`
- `proposal_review_po`

Proposal 013 is not done until these outputs have one coherent contract across:

- `examples/agents/agents.yaml`
- runtime validation
- artifact persistence
- run reports
- blocked-run recovery UI

---

## 5. Retry and attempt truth

### 5.1 Current defect

The app currently mixes two incompatible ideas:

1. recovery mutates an existing failed stage as if it will retry in place;
2. orchestration later creates a new stage execution record as if this were a fresh execution.

This makes:

- `attemptNumber`
- stage history
- report versions
- and operator expectations

internally inconsistent.

### 5.2 Required retry semantics

Proposal 013 freezes three distinct actions:

1. **Retry Failed Agent**
   - same run
   - same stage
   - same stage attempt
   - same frozen logical snapshot as the failed attempt
   - new agent attempt for the failed agent only
   - sibling successful agent outputs remain frozen and explicitly reused
   - no cloned run
2. **Retry Failed Stage**
   - same run
   - same stage lineage
   - new stage attempt
   - no cloned run
3. **Clone Run**
   - new run
   - new frozen snapshot or current-config snapshot
   - old run remains terminal history

These actions must not share the same persistence path.

### 5.3 Attempt model

Each stage must have stable persisted attempt history:

- `stageID`
- `stageExecutionID`
- `attemptNumber`
- `retryMode`
  - `agent_retry`
  - `stage_retry`
  - `fresh_execution`
- `triggerReason`
- `supersedesAttemptNumber`

Rules:

1. A retry in place must not create a new attempt with number `1`.
2. A clone run must not pretend to be an in-place retry.
3. Reports must describe the current stage attempt and the prior attempts from the same stage lineage.

In addition, agent-only retry must be representable explicitly.

Each agent-only retry must persist:

- `stageExecutionID`
- `agentID`
- `agentExecutionID`
- `agentAttemptNumber`
- `supersedesAgentExecutionID`
- `reusedSiblingExecutionIDs`
- `retryReason`

Rules:

1. `Retry Failed Agent` does not increment the stage attempt number.
2. `Retry Failed Stage` increments the stage attempt number and supersedes the whole stage attempt.
3. Reports and recovery views must show both:
   - current stage attempt
   - current failed or retried agent attempt, when agent-only retry is in play.
4. Reused sibling outputs must remain immutable and explicitly linked into the new agent-attempt evidence so the operator can see what was rerun and what was reused.

### 5.4 Storage truth for same-stage agent retry

Proposal 013 preserves the existing runtime guarantee from `runtime-contract.md` that stage-attempt artifacts are immutable.
`Retry Failed Agent` must therefore add agent-attempt lineage without mutating or overwriting the prior stage-attempt files.

Required persisted artifact truth:

- `Artifact.attemptNumber` remains the stage-attempt number.
- `Artifact` gains optional agent-retry lineage fields:
  - `agentAttemptNumber`
  - `supersedesAgentArtifactID`
  - `artifactLineageKind`
    - `stage_attempt_primary`
    - `agent_retry_delta`
    - `reused_sibling_reference`

Required path truth:

1. Stage-attempt-primary artifacts remain at the current path shape:
   - `{artifactRoot}/{stageID}.{iteration}/{agentID}/{stageAttemptNumber}/{name}`
2. Agent-only retry artifacts live in a disjoint namespace under the same stage attempt:
   - `{artifactRoot}/{stageID}.{iteration}/{agentID}/{stageAttemptNumber}/agent-retry-{agentAttemptNumber}/{name}`
3. Receipts and transcripts produced by an agent-only retry live in that same `agent-retry-{agentAttemptNumber}` namespace.
4. Prior failed-agent artifacts from the same stage attempt are never overwritten; they are superseded explicitly through artifact lineage metadata.
5. Successful sibling-agent outputs from the original stage attempt are not recopied into the retry namespace. They remain immutable in their original stage-attempt path and are referenced through `reusedSiblingExecutionIDs` plus `reused_sibling_reference` metadata.
6. Same-run `Retry Failed Agent` must prove reuse of the same frozen logical snapshot through persisted snapshot linkage, not only by inference from the surrounding run.

Required runtime and reporting rule:

- when resolving the effective output for one agent inside one stage attempt, runtime and reporting choose:
  - the latest successful `agent_retry_delta` artifact if one exists,
  - otherwise the `stage_attempt_primary` artifact from the original stage attempt.

This keeps same-stage `Retry Failed Agent` truthful without violating the existing stage-attempt immutability contract or introducing path collisions.

Default recovery policy for this failure class:

- output-contract mismatch and post-generation validation failure are non-auto-retryable by default;
- narrow retry remains valid only as an explicit recovery action or policy override after the operator can inspect the preserved failure evidence.

---

## 6. Failure evidence persistence

### 6.1 Current defect

The current runtime can leave the system in an ambiguous state:

- raw output files exist,
- but structured validation fails,
- and receipts/transcripts/metadata are missing or incomplete.

That makes blocked runs look nondeterministic.

### 6.2 Required persistence ordering

Proposal 013 freezes this ordering for every agent execution and anchors each step to the current runtime boundary:

1. `AgentExecutor` returns raw agent payloads, low-level execution receipt, transcript path or transcript payload, and timing metadata.
2. `ArtifactManager.persistOutputs(...)` persists:
   - raw payload artifacts
   - receipt artifacts
   - transcript artifacts
   - provisional artifact metadata sufficient to survive later validation failure
3. `WorkflowOrchestrator.validateStructuredOutputs(...)` runs structured validation against the persisted raw output set using `OutputContractResolverV2`.
4. `ArtifactManager` persists:
   - normalized artifacts, when validation succeeds
   - `ValidationFailureRecord`, when validation fails
   - `StructuredOutputEnvelope` or equivalent persisted artifact-plus-metadata pair capturing raw payload, parsed payload if any, and validation result
5. `WorkflowOrchestrator` settles `AgentExecution`, `StageExecution`, and `Run` status.
6. `RunReportBuilder` materializes immutable report truth from the settled status plus the preserved evidence packet.

Validation must never be the point where all downstream evidence disappears.

For same-stage `Retry Failed Agent`, the ordering above applies inside the `agent-retry-{agentAttemptNumber}` namespace from Section 5.4.
That means:

- raw retry outputs, receipts, and transcripts are persisted under the agent-retry namespace before validation;
- validation failure records point to that exact namespace;
- the original stage-attempt-primary artifacts remain untouched;
- reports can explain both the original failed agent output and the retried replacement without ambiguous supersession.

### 6.3 Failed-stage evidence packet

Every failed stage must preserve one evidence packet containing:

- raw outputs that were actually produced
- receipt, even if the result is later classified as failed
- transcript, if any streamed content existed
- validation errors
- contract metadata used for validation
- timing
- chosen recovery recommendation

The blocked-run operator UI and exported reports must both read from this same packet.

Proposal 013 leaves one bounded implementation choice open:

- the packet may be modeled as SwiftData metadata plus persisted artifacts,
- or as a primary artifact with supporting metadata,

but it may not exist only as transient executor state.

`ValidationFailureRecord` or the failed-stage evidence packet is the canonical persisted reference target for:

- recovery UI
- immutable run reports
- exported evidence or failure packets

Summary fields may derive from that canonical object, but may not replace it as the durable source of truth.

---

## 7. Recovery UX

### 7.1 Current defect

The current shell already has recovery surfaces and can already compute narrow retry actions in some cases.
The real defect is trust explanation:

- the user cannot see why one action is recommended,
- the user cannot easily distinguish missing evidence from validation failure after output generation,
- and retry lineage is not explained precisely enough to trust same-run recovery.

### 7.2 Required recovery actions

Proposal 013 extends the existing shell-owned recovery owners:

- `RecoverySheet`
- `BlockedRunRecoveryView`

Those surfaces must show, when valid:

- `Retry Failed Agent`
- `Retry Failed Stage`
- `Clone Run (Frozen Snapshot)`
- `Clone Run (Current Config)`

Each action must carry one-line explanation text:

- why it is available,
- what will be reused,
- what will be re-executed,
- whether this stays in the same run or creates a new run.

For output-contract mismatch and post-generation validation failure, the default action posture is operator-mediated:

- no silent blind auto-retry by default;
- retry remains allowed only as an explicit action or policy override after the evidence panel explains the failure class.

### 7.3 Required evidence in recovery view

The recovery surface must also show:

- failed stage label
- failed agent label, if one exists
- whether raw output exists
- whether a receipt exists
- whether validation failed after output generation
- validation failure summary
- current retry recommendation source
  - `runtime policy`
  - `operator override`

The recovery surface must additionally say whether each available action:

- stays in the same run,
- creates a new run,
- reuses sibling outputs,
- or re-executes the whole stage.

The recovery surface must link back to the canonical `ValidationFailureRecord` or failed-stage evidence packet, not only to summary prose.

---

## 8. Proposal drafting resilience

### 8.1 Current defect

The motivating run suggests an earlier drafting failure likely happened after producing an oversized proposal artifact.

Proposal 013 treats this as a bounded resilience problem, not a new content-generation feature.

### 8.2 Required behaviour

Proposal drafting must enforce one bounded compaction policy:

- keep canonical proposal content complete enough for downstream review,
- avoid pathological oversized artifacts,
- persist whether compaction occurred and why,
- never silently discard large sections without auditability.

Required persisted metadata:

- original output size
- compacted output size
- compaction strategy
- whether the stage succeeded with compaction or failed despite compaction

---

## 9. Operator-visible outcomes

After Proposal 013, the operator experience for a run like `B18A8E99-287E-4383-BCA6-9494DAE059A4` must be:

1. the blocked run clearly states that `Proposal reviewed` failed because the output violated its declared contract;
2. the evidence panel shows that raw reviewer outputs exist;
3. the recovery surface offers `Retry Failed Agent` or `Retry Failed Stage` before clone-run, when policy allows;
4. reports and stage history show truthful attempt numbering;
5. if the operator clones instead, the UI explicitly states that this creates a new run.

---

## 10. Verification

Proposal 013 requires all of the following.

### 10.1 Unit and integration proof

- output-contract validation tests for review outputs
- retry-in-place attempt-number persistence tests
- clone-run versus retry lineage tests
- failed-stage evidence persistence tests
- proposal-drafting compaction tests

### 10.2 App-level proof

At least one app-launched run must prove:

1. a stage produces raw outputs,
2. validation fails,
3. failed-stage evidence is preserved,
4. recovery UI shows the narrow retry action,
5. retry succeeds without cloning the run,
6. prior failed-attempt artifacts, receipts, and transcripts remain inspectable after the later retry succeeds.

### 10.3 Regression proof on the motivating class

One canonical regression test must cover:

- proposal drafted succeeds,
- proposal reviewed fails on contract mismatch,
- failed-stage evidence survives,
- retry path is `Retry Failed Agent` or `Retry Failed Stage`,
- retry completes,
- prior failed-attempt evidence remains inspectable,
- reports remain truthful throughout.

Proposal 013 is not done if the only surviving recovery path remains full run clone.

---

## 11. Acceptance criteria

Proposal 013 is complete only when all of the following are true:

1. proposal-review output contracts are aligned across agent catalog, runtime validation, and persisted artifacts;
2. a failed stage that produced outputs preserves receipts, transcripts or equivalent execution evidence, raw outputs, and validation error records;
3. retry-in-place no longer resets attempt numbering or obscures stage lineage;
4. same-stage `Retry Failed Agent` has explicit artifact, receipt, and transcript storage truth that does not collide with immutable stage-attempt artifacts;
5. blocked-run recovery surfaces expose the narrowest valid retry action before clone-run;
6. at least one canonical regression proves a failed review stage can be retried and completed without creating a new run;
7. recovery, reporting, and export surfaces reference the canonical `ValidationFailureRecord` or failed-stage evidence packet rather than only derived summary fields;
8. proposal-drafting oversized-output failures are bounded by explicit compaction policy and evidence.

---

## 12. Out of scope

- changing the fundamental workflow topology
- redesigning the approval model
- new provider families or provider routing policy
- repo-backed delivery changes already covered by Proposal 007
- general UI visual polish already covered by Proposal 012
- broad artifact migration of all existing stage outputs in historical runs

---

## Appendix A: Motivating defect package

The proposal is grounded in the persisted evidence from one real blocked run:

- run ID: `B18A8E99-287E-4383-BCA6-9494DAE059A4`
- initial blocked report: `artifacts/reports/run_report_v1.*`
- later blocked report after recovery progression: `artifacts/reports/run_report_v2.*`
- failed review outputs present on disk without coherent reviewer receipts/transcripts:
  - `artifacts/state_4_proposal_reviewed.2/...`

This proposal exists to make that class of failure impossible to misread.
