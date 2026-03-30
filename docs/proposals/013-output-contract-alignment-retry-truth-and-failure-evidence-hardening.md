# Proposal 013: Output Contract Alignment, Aggregate Contract Hardening, Failure Evidence, and Narrow Recovery

| Field | Value |
|---|---|
| Date | 2026-03-29 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/workflow-execution-engine.md](../reference/workflow-execution-engine.md), [reference/operator-experience.md](../reference/operator-experience.md), [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md), [reference/mvp-sign-off.md](../reference/mvp-sign-off.md) |
| Scope | Proposal-review output contract alignment, aggregate `proposal_review_summary` hardening, declarative contract-tier hardening for `contracts.*` and `backend_profiles.*.structured_output`, canonical failure-evidence persistence for contract failures, narrow recovery actions, and bounded proposal-output resilience |
| Goal | Eliminate the blocked-run class where proposal-review or aggregate outputs exist but the runtime still blocks because contracts, declarative controls, failure evidence, and recovery/reporting surfaces disagree about what those outputs mean. |

---

## 1. Context

The motivating run `B18A8E99-287E-4383-BCA6-9494DAE059A4` proved two different problem layers.

The deeper layer is runtime truth:

- receipts can report success and transport error at the same time,
- stale `running` and `waitingApproval` executions can survive beside newer attempts,
- aggregate and report truth can drift away from actual stage lineage,
- reports can show configured provider/model truth instead of actual runtime truth.

That lower layer is now further addressed by the companion migration slice [016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md](016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md).

What remains after that lower layer is still important and still proposal-worthy:

- review agents can emit markdown when the catalog declares JSON,
- the aggregate transition depends on `proposal_review_summary`, which also needs first-class contract truth,
- contract failures after useful output generation still need canonical failure evidence,
- recovery and report surfaces still need to explain the narrowest valid next action from that evidence,
- declarative contract controls still need honest runtime enforcement or fail-closed behavior.

Proposal 013 is the bounded follow-up slice for that higher layer.

### 1.1 Relationship to Proposal 016

Earlier versions of Proposal 013 tried to absorb transport truth, stage settlement, retry lineage, and recovery/report alignment in one slice.

Implementation evidence showed that was the wrong order.

Proposal 013 remains the right bounded proposal for contract alignment, failure evidence, and narrow recovery.
What changed is only the causal map around it:

- Proposal 013 owns contract and evidence semantics,
- Proposal 016 owns the deeper execution-truth migration where stale active records, contradictory transport outcomes, or provider-truth drift corrupt what Proposal 013 later reads.

The app cannot fully trust contract-alignment conclusions if the runtime still does not know:

1. whether an agent completed, timed out, or failed after output,
2. which stage execution is canonical,
3. whether a stale `running` or `waitingApproval` record should be repaired before a new attempt begins,
4. which provider/model actually executed,
5. whether the aggregate step itself settled or never produced its required output.

Proposal 016 repairs that substrate.
Proposal 013 remains valid whether 016 lands before it, alongside it, or after an initial partial 013 rollout, but report/recovery trust should be capped until the 016 migration is also applied.

### 1.2 What this proposal is

Proposal 013 is the bounded slice that makes proposal-review and aggregate contract truth operational:

- reviewer outputs and aggregate outputs match declared contracts,
- contract failures after output generation preserve canonical failure evidence,
- blocked-run recovery surfaces can point at the narrowest valid next action,
- and Phase B extends the slice to declarative contract hardening plus bounded proposal-output resilience.

### 1.3 What this proposal is not

Proposal 013 is **not**:

- transport outcome normalization,
- stage settlement repair,
- resume / relaunch idempotency,
- provider binding truth repair,
- a full workflow-state-machine redesign,
- a new provider family proposal,
- a skill-resolution proposal,
- or a general UI-polish bucket.

Those lower-level runtime truths belong to Proposal 016.

---

## 2. Product questions this proposal must answer

After Proposal 013, the engineer must be able to answer all of these with persisted evidence rather than inference:

1. Did each proposal-review agent output match its declared contract, or did it produce a useful but invalid artifact that was preserved as canonical failure evidence?
2. Did the aggregate `proposal_review_summary` step produce its required output contract, or did the stage fail with explicit aggregate-level evidence?
3. Can the operator see the narrowest valid recovery action from the blocked-run surface without inferring from raw files on disk?
4. When validation fails after output generation, are the raw output, receipt, transcript, and validation error all preserved as first-class evidence?
5. Do `contracts.*` and `backend_profiles.*.structured_output` either affect runtime behavior or fail validation / preflight when unsupported?
6. Are large proposal outputs bounded so a single oversized document does not silently collapse an otherwise valid drafting stage?

Proposal 013 is done only when all six answers are explicit in the persisted model, operator surfaces, and test evidence.

---

## 3. What we build

Proposal 013 delivers one incident-closing core and one explicitly later phase.

### 3.1 Phase split

**Phase A — incident-closing core**

- proposal-review output contract alignment
- `proposal_review_summary` contract truth
- `ValidationFailureRecord` / failed-stage evidence
- narrow recovery actions and operator explanation

**Phase B — bounded follow-up inside 013**

- declarative coverage reporting for Appendix B Tier 1
- proposal drafting compaction / oversized-output resilience

Implementation may ship Phase A before Phase B.
Proposal 013 should be considered materially successful for the motivating incident only when Phase A is closed.
Phase B must not start until Phase A is green on the motivating-run replay proof from Section `9.3`.

### Layer M: Output Contract Alignment

| Component | Responsibility |
|---|---|
| **OutputContractSchemaV2** | Typed schema derived from the existing catalog-backed contract truth, including machine format, human-readable companion format, and validation mode |
| **OutputContractResolverV2** | Canonical runtime reader that resolves the typed schema from `AgentCatalog.contracts` and exposes it to validation, persistence, reporting, and recovery |
| **StructuredOutputEnvelope** | Persisted wrapper for structured outputs, raw payload, parsed payload when present, validation result, and origin metadata |
| **ProposalReviewContractAdapter** | Aligns each proposal-review agent so the declared contract matches the produced artifact format |
| **ProposalReviewSummaryContractAdapter** | Gives `aggregate_proposal_reviews` and `proposal_review_summary` the same first-class contract truth as the individual reviewers |
| **ValidationFailureRecord** | First-class persisted record describing why output validation failed after agent execution completed |

### Layer N: Phase B — Declarative Runtime Coverage

Mandatory in this proposal (Appendix B Tier 1):

| Component | Responsibility |
|---|---|
| **OutputContractDeclarativeBridge** | Eliminates hardcoded `outputName -> contractID` fallback branches so output-to-contract binding is fully catalog-driven and testable |
| **StructuredOutputSchemaGate** | Applies provider-aware preflight so `backend_profiles.*.structured_output` either reaches transport in a supported shape or fails before execution |
| **DeclarativeCoverageReport** | Emits testable evidence of which YAML fields are executable truth versus intentionally non-runtime metadata, including tier classification for every Appendix B row |

Deferred to later proposals (Appendix B Tier 3):

| Component | Deferred Reason |
|---|---|
| **SkillResolutionBridge** | Resolving `skills.*`, `skill_ref`, and `skill_role` into live execution requires the dedicated skill-runtime slice in Proposal 015 |
| **ExecutionPolicyTranslatorV2** | Enforcing permission profile allowlists and `required_tools` at the transport level still depends on transport capabilities outside this proposal |
| **BackendRuntimeSettingsBridge** | Propagating `max_turns`, `temperature`, and `effort` to the live transport still depends on transport-level parameter support outside this proposal |
| **WorkflowConfigCoverageGate** | Enforcing broader workflow-level execution declarations remains outside this bounded slice |

### Layer O: Failure Evidence and Reporting

| Component | Responsibility |
|---|---|
| **FailedStageEvidenceBuilder** | Persists raw output, transcript, receipt, validation errors, and aggregate failure evidence when a contract failure happens after output generation |
| **CanonicalFailureReferenceBridge** | Makes recovery, reporting, and export surfaces point at the same durable failure object or failed-stage evidence packet |
| **FailedStageEvidencePanel** | Shell-owned evidence panel showing raw output presence, validation failure cause, receipt/transcript availability, and recommended next action |

### Layer P: Narrow Recovery and Proposal Output Resilience

| Component | Responsibility |
|---|---|
| **RecoverySheet Extension** | Extends the existing shell-owned `RecoverySheet` with contract-failure explanation and the precise next valid action from the canonical recovery snapshot |
| **BlockedRunRecoveryView Extension** | Extends the existing `BlockedRunRecoveryView` with `Retry Failed Agent`, `Retry Failed Stage`, `Retry Aggregate Step`, `Clone Frozen Snapshot`, and `Clone Current Config` when each is valid |

### Layer Q: Phase B — Proposal Output Resilience

| Component | Responsibility |
|---|---|
| **ProposalDraftCompactionPolicy** | Applies bounded output-size discipline to proposal drafting and stores truncation / compaction metadata when invoked |

---

## 4. Output contract alignment

### 4.1 Current defect

The current app allows a stage to declare one output contract while agents effectively emit another shape.
The motivating run showed this concretely in `Proposal reviewed`:

- the catalog declares structured JSON review outputs,
- review artifacts on disk were markdown review content,
- the fan-out work materially happened,
- but the stage still blocked because the runtime could not reconcile declared contract, produced artifact, and aggregate transition truth.

The same class applies to the aggregate step:

- `aggregate_proposal_reviews` drives the transition out of `Proposal reviewed`,
- but `proposal_review_summary` is not yet treated with the same hard contract truth as the four individual reviewer outputs.

### 4.2 Canonical contract source

Proposal 013 does **not** create a second contract authority.

The canonical source of truth remains:

- `AgentCatalog.contracts`

resolved through:

- `OutputContractResolverV2`

Rules:

1. `OutputContractSchemaV2` is derived from `AgentCatalog.contracts`.
2. `OutputContractResolverV2` is the only runtime reader used by:
   - `WorkflowOrchestrator`
   - `ArtifactManager`
   - `RunReportBuilder`
   - blocked-run recovery surfaces
3. No runtime component may read one contract shape from the catalog and another from an unrelated registry.
4. If the contract schema needs new fields, the catalog contract definition is migrated directly; the typed resolver layer only normalizes it for code.
5. The current hardcoded `outputName -> contractID` branches in `OutputContractResolver` are transitional drift and must be removed or isolated behind explicit migration logic with tests.

### 4.3 Mandatory adopters

The following outputs are mandatory adopters in this proposal:

- `proposal_review_ui`
- `proposal_review_ux`
- `proposal_review_architect`
- `proposal_review_po`
- `proposal_review_summary`

### 4.4 Required contract model

Every output contract in this slice must declare:

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

1. A contract that declares `strict_structured` may not silently accept prose in place of the machine payload.
2. A contract that declares `structured_with_human_companion` must persist both:
   - machine-valid structured output,
   - human-readable rendered companion artifact.
3. If the app wants reviewer outputs as markdown, the contract must say markdown.
4. If the app wants reviewer outputs as JSON, the agents and aggregate step must actually emit JSON and any rendered markdown must be explicitly secondary.
5. `proposal_review_summary` must be treated as a first-class contract citizen, not a fallback or implicit transition artifact.
6. Aggregate contract validation must attach to the aggregate state's canonical `StageExecution` and its failed-stage evidence path; it must not introduce a parallel aggregate terminality authority.
7. Aggregate execution may consume only normalized, contract-valid reviewer outputs; raw invalid reviewer artifacts are evidence only and must not be treated as aggregate inputs.

### 4.5 Legacy contract-schema migration rule

Proposal 013 does not require a flag day migration of the entire catalog before the mandatory adopters can land.

During migration:

- legacy `format` maps into `machine_format`
- `validation_mode` gets a safe default derived from the contract class:
  - proposal-review outputs default to `structured_with_human_companion`
  - strictly machine-only outputs default to `strict_structured`
- `human_format` may be derived until catalog migration finishes
- `raw_artifact_name` and `normalized_artifact_name` may be derived from the output name until catalog migration finishes

Rules:

1. The canonical authority remains `AgentCatalog.contracts`; migration defaults are a bridge, not a second schema source.
2. Mandatory adopters may land incrementally, but a migrated adopter must not rely on silent fallback once its contract is explicitly upgraded.
3. Catalog-wide cleanup may finish later, but mandatory adopters in this proposal must be explicit and test-covered.

---

## 5. Phase B: Declarative contract hardening

### 5.1 Current declarative coverage gap

The latest audit of `examples/agents/agents.yaml` and `examples/workflows/workflow.yaml` found one directly relevant truth gap and several adjacent metadata-only declarations:

- `contracts.*` are only partially declarative today because output-to-contract binding still contains hardcoded fallback branches.
- `backend_profiles.*.structured_output` is declared in YAML but is not yet enforced as provider-aware runtime truth.
- multiple YAML sections still behave as documentation or validation metadata rather than executable runtime truth.

Proposal 013 solves only the contract-related declarative subset and audits the rest into explicit tiers.

### 5.2 Execution-critical YAML coverage boundary

The following surfaces are mandatory for implementation hardening in this proposal (Appendix B Tier 1):

- `contracts.*` and output-to-contract binding
- `backend_profiles.*.structured_output`

Rules:

1. No mandatory-tier YAML field may silently no-op after Proposal 013.
2. If a mandatory-tier field remains in schema, it must either change runtime behavior or trigger explicit validation / preflight failure when unsupported by the active backend or schema subset.
3. Successful transport-level structured output support does not remove the need for post-generation contract validation.
4. Purely descriptive metadata may remain non-runtime, but must not be mixed with execution-critical declarations in acceptance claims.
5. Appendix B is tiered into mandatory, metadata-only, and later buckets. Only Tier 1 rows are implementation inventory for this proposal.

---

## 6. Failure evidence and narrow recovery

### 6.1 Current defect

The app can still leave the operator in an ambiguous state:

- raw outputs exist,
- receipts and transcripts may exist,
- the aggregate step may still be missing,
- but blocked-run surfaces and reports fail to point at one canonical failure object or one canonical next action.

### 6.2 Required evidence behavior

Proposal 013 requires canonical failure evidence even on the current runtime and benefits further from Proposal 016 once lower settlement truth is repaired.
Within that boundary, Proposal 013 requires:

1. contract failure after output generation must persist:
   - raw output artifacts,
   - receipt artifacts,
   - transcript artifacts,
   - `ValidationFailureRecord`,
   - and a failed-stage evidence packet or equivalent canonical failure object
2. the aggregate step must produce the same evidence bundle when `proposal_review_summary` fails validation or is missing after fan-out completes
3. reports, exports, and recovery surfaces must reference the canonical failure object directly, not only a derived summary
4. canonical failure evidence may contain sensitive data, so operator-visible summaries should default to summarized or redacted presentation unless explicit full-detail inspection is requested
5. aggregate input eligibility must be explicit: only reviewer outputs that already passed contract normalization/validation may feed `proposal_review_summary`; invalid raw reviewer artifacts remain evidence only.

### 6.3 Required recovery behavior

Recovery surfaces must extend the existing shell-owned:

- `RecoverySheet`
- `BlockedRunRecoveryView`

and must expose the narrowest valid recovery action from the canonical recovery snapshot:

- `Retry Failed Agent`
- `Retry Failed Stage`
- `Retry Aggregate Step`
- `Clone Frozen Snapshot`
- `Clone Current Config`

`Clone run` is not acceptable as the only surviving path when narrower recovery is valid.

Required precedence:

| Failure class | Required narrowest action |
|---|---|
| invalid reviewer contract, aggregate not started | `Retry Failed Agent` |
| reviewer fan-out valid, aggregate missing or aggregate contract-invalid | `Retry Aggregate Step` |
| stage-level settlement is canonical but one reviewer attempt is the only invalid input | `Retry Failed Agent` |
| stage-level settlement canonical, multiple reviewer outputs invalid or stage must be rebuilt | `Retry Failed Stage` |
| lower-layer runtime truth is `legacy_unverifiable`, startup repair incomplete, or canonical failed step cannot be trusted | block same-run retry and prefer clone or explicit operator stop |

Rules:

1. Recovery UI must not invent its own priority order from raw artifacts on disk.
2. `Retry Aggregate Step` is valid only when reviewer inputs are already contract-valid and the aggregate step is the narrowest broken unit.
3. Clone is fallback, not the default operator escape hatch, unless lower-layer execution truth is not trustworthy enough for same-run recovery.

---

## 7. Phase B: Proposal drafting resilience

### 7.1 Current defect

Proposal drafting can still fail after producing a large useful document if one oversized artifact or summary collapses the stage.

### 7.2 Required behaviour

`ProposalDraftCompactionPolicy` must:

1. bound proposal-drafting output size before settlement fails silently,
2. persist compaction / truncation metadata when applied,
3. preserve the raw artifact plus the compacted or normalized artifact,
4. make the compaction decision visible in report and recovery surfaces.

---

## 8. Operator-visible outcomes

After Proposal 013, a blocked proposal-review stage must make all of the following explicit:

1. whether an individual reviewer failed contract validation,
2. whether the aggregate `proposal_review_summary` step failed or never produced its required output,
3. where the raw outputs, receipts, and transcripts live,
4. which canonical failure object explains the block,
5. which narrow recovery action is valid and why,
6. which declarative contract controls were active and actually enforced.

---

## 9. Verification

Proposal 013 requires all of the following.

### 9.1 Phase A core proof

- output-contract validation tests for all four reviewer outputs
- aggregate contract tests for `proposal_review_summary`
- failed-stage evidence persistence tests for contract failures after output generation

### 9.2 Phase A app-level proof

At least one app-launched run must prove:

1. all four proposal-review fan-out artifacts are produced,
2. `proposal_review_summary` either validates successfully or fails with canonical aggregate evidence,
3. contract failure evidence is preserved and inspectable,
4. recovery UI shows the narrowest valid next action.

### 9.3 Phase A regression proof on the motivating class

One canonical regression test must cover:

- proposal drafted succeeds,
- proposal reviewed fan-out produces all four reviewer outputs,
- aggregate summary fails due to contract mismatch or missing required output,
- failed-stage evidence survives,
- narrow recovery is available without requiring a full clone,
- reports remain truthful throughout,
- no mandatory-tier YAML field involved in the run is left in silent metadata-only limbo.

Proposal 013 is not done if the only surviving recovery path remains full run clone.

### 9.4 Phase B additional proof

Phase B proof is out of bounds until Section `9.3` is green on the motivating incident class.

- contract-resolution tests proving contract lookup no longer depends on hardcoded output-name branches
- structured-output schema gate tests proving `backend_profiles.*.structured_output` reaches transport in a supported shape or triggers provider-aware preflight failure
- declarative-coverage report tests proving every Appendix B row has an explicit tier classification and mandatory-tier rows have corresponding enforcement evidence
- proposal-drafting compaction tests

---

## 10. Acceptance criteria

Proposal 013 is complete only when all of the following are true:

### 10.1 Phase A core acceptance

1. proposal-review output contracts are aligned across agent catalog, runtime validation, and persisted artifacts;
2. `proposal_review_summary` is a first-class contract with runtime validation and persisted artifact truth;
3. a failed review or aggregate stage that produced outputs preserves receipts, transcripts or equivalent execution evidence, raw outputs, and validation error records;
4. blocked-run recovery surfaces expose the narrowest valid recovery action before clone-run;
5. reports, exports, and recovery surfaces reference the canonical `ValidationFailureRecord` or failed-stage evidence packet rather than only derived summary fields.

### 10.2 Phase B additional acceptance

Phase B acceptance must not be used to delay or redefine incident closure. `DeclarativeCoverageReport` and `ProposalDraftCompactionPolicy` are explicitly gated behind green `Phase A` proof on the motivating-run replay class.

6. mandatory-tier YAML fields from Appendix B (`contracts.*` and `structured_output`) are either enforced by runtime code or rejected by validation / preflight; non-mandatory fields are explicitly tiered as metadata-only or deferred to a later proposal;
7. Appendix B tiering is persisted and testable: every audited row has an explicit tier classification, and the mandatory tier has corresponding verification evidence;
8. proposal-drafting oversized-output failures are bounded by explicit compaction policy and evidence.

---

## 11. Out of scope

Proposal 013 does **not** own:

- transport outcome normalization,
- stage settlement atomicity,
- resume / relaunch idempotency,
- repair of stale `running` or `waitingApproval` executions,
- actual runtime provider/model provenance repair,
- broader skill resolution and runtime injection,
- broader transport policy enforcement beyond `structured_output`,
- or workflow-topology redesign.

Those lower-level runtime-truth repairs are owned by Proposal 016, but they are a companion migration slice rather than a reason to invalidate this proposal's contract/evidence scope.

---

## Appendix A: Current YAML coverage audit

Each audited row is assigned to one of three tiers:

- **Tier 1 — 013 mandatory hardening**: must gain runtime enforcement or fail-closed behavior in this proposal.
- **Tier 2 — metadata-only by design after 013**: intentionally non-runtime; schema and docs must say so explicitly.
- **Tier 3 — later proposal / later platform work**: execution-relevant but out of scope for this bounded slice; tracked for a future proposal.

Only Tier 1 rows are implementation inventory for Proposal 013. Tier 2 and Tier 3 rows require explicit tier classification but not runtime enforcement in this slice.

### Agents catalog — Tier 1 (013 mandatory)

| Surface | Status | Current truth |
|---|---|---|
| `contracts.*` | Partial | `format` and `required_fields` participate in format detection and JSON validation, but output-to-contract binding still relies on hardcoded fallback mapping. |
| `backend_profiles.*.structured_output` | Unused | Parsed from YAML, but not consumed by runtime execution or validation policy. |

### Agents catalog — Tier 2 (metadata-only by design)

| Surface | Status | Current truth |
|---|---|---|
| `app.*` | Unused | Decoded from YAML, but no runtime component reads catalog-level app settings. Metadata-only by design. |
| `paths.*` | Unused | Only scanned by env-placeholder validation; they do not drive runtime path resolution. Metadata-only by design. |
| `artifacts.*` | Partial | Artifact names are validated and declared paths can hint format detection, but declared paths do not control on-disk persistence layout. Persistence layout is engine-owned. |
| `agents.*.worktree_policy.strategy` / `path` / `base_branch` | Unused | Parsed and validated as strings only; worktree provisioning uses delivery configuration instead. |
| `agents.*.notes` | Unused | Present in schema, but not consumed by runtime or UI. Purely descriptive. |

### Agents catalog — Tier 3 (later proposal / later platform work)

| Surface | Status | Current truth |
|---|---|---|
| `skills.*` | Partial | Skill definitions exist in YAML, but skill content is not resolved into live execution. |
| `agents.*.skill_ref` / `skill_role` | Partial | Parsed, validated, displayed, and hashed into provenance; not injected into Goose prompts or tool/session policy. |
| `backend_profiles.*.effort` | Partial | Persisted in provenance / receipts and provider binding, but not sent as a transport control to Goose. |
| `backend_profiles.*.max_turns` / `temperature` | Partial | Carried into `ResolvedAgent` and hashes, but not enforced by the live Goose transport. |
| `permission_profiles.*` | Partial | Profile existence is validated, profile ID is sent to Goose, and some profile names drive side-effect heuristics, but detailed allowlists are not enforced by the current Goose transport. |
| `agents.*.required_tools` | Unused | Declared in YAML, but not checked before or during execution. |
| `agents.*.requires_human_approval` | Partial | Used by resume-side-effect heuristics, but actual gate behavior is owned by workflow approval states. |

### Agents catalog — already used (no action needed)

| Surface | Status | Current truth |
|---|---|---|
| `backend_profiles.*.provider` / `model` | Used | These fields drive provider-family resolution and live model selection. |
| `agents.*.worktree_policy.write_enabled` | Used | This is the only worktree-policy field that changes runtime behavior. |

### Workflow — Tier 1 (013 mandatory)

No workflow-level fields are mandatory for Proposal 013. The motivating failure class is contract-driven, not workflow-declaration-driven.

### Workflow — Tier 2 (metadata-only by design)

| Surface | Status | Current truth |
|---|---|---|
| `workflow.notes` | Unused | Descriptive only. |
| `workflow.labels` | Unused | Descriptive only. |

### Workflow — Tier 3 (later proposal / later platform work)

| Surface | Status | Current truth |
|---|---|---|
| workflow-level execution declarations beyond currently used approval / routing fields | Partial | Parsed and validated unevenly, but not all are runtime-authoritative yet. |

### Workflow — already used (no action needed)

| Surface | Status | Current truth |
|---|---|---|
| approval-state routing and current used state-machine fields | Used | Already drive runtime behavior today. |
