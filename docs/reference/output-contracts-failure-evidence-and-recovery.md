# Output Contracts, Failure Evidence, and Narrow Recovery

Stable reference for the output-contract, failure-evidence, retry-lineage, typed validation-failure reader, and bounded proposal-resilience slice.

## Purpose

The runtime must be able to say, with persisted evidence rather than inference:

- which output contract was authoritative,
- whether a reviewer or aggregate step produced contract-valid artifacts,
- what canonical evidence survives when validation fails after generation,
- which same-run retry path remains valid before clone-run,
- and which declarative contract controls are enforced versus rejected.

## Scope

This reference covers:

- the structured-output envelope and contract-validation substrate used by the Rust control plane,
- catalog-backed output-contract authority,
- strict proposal-review and aggregate summary contract enforcement,
- canonical validation-failure and failed-stage evidence,
- same-run retry lineage and artifact namespace rules,
- narrow recovery and report/export evidence references,
- declarative Tier 1 contract enforcement for `contracts.*` and `backend_profiles.*.structured_output`,
- and bounded proposal-drafting compaction truth.

It does not replace:

- transport-level envelope extraction and canonical binding details in [structured-output-envelope-and-contract-validation.md](structured-output-envelope-and-contract-validation.md),
- lower-layer settlement and transport truth in [execution-truth-and-recovery.md](execution-truth-and-recovery.md),
- broader orchestrator topology in [workflow-execution-engine.md](workflow-execution-engine.md),
- frozen run state and artifact boundaries in [runtime-contract.md](runtime-contract.md),
- or operator-shell interaction rules in [operator-experience.md](operator-experience.md).

## Core Rules

### Structured-output validation is implemented, not aspirational

The Rust control plane now implements the structured-output substrate for this slice:

- ACP extracts named `CHAINWORKS_OUTPUT` envelopes,
- the workflow compiler carries the expanded contract schema,
- the executor binds declared artifacts to canonical names and paths before persistence,
- undeclared envelope outputs survive as generic artifacts,
- and validation-failure evidence is durable and typed.

The detailed owner chain for that substrate lives in [structured-output-envelope-and-contract-validation.md](structured-output-envelope-and-contract-validation.md). This document treats that substrate as implemented baseline, then defines how failure evidence, retry truth, and narrow recovery consume it.

### Required outputs flow through one materialization path

Agent-authored required outputs should enter the system through the final `CHAINWORKS_OUTPUT` object returned by the provider session. Prompt generation must provide contract-complete skeletons for each declared output; repair prompts must use the same contract-complete skeletons for failed outputs.

The executor then binds those payloads to the compiled output declarations, validates them against `AgentCatalog.contracts`, and materializes canonical artifact files. Exact-path filesystem outputs and legacy block envelopes remain accepted as compatibility evidence, but they do not create a second contract authority and do not bypass validation.

The retained `proposal-089|p089` gate is the focused Junie proof for this
materialization path. It validates that the production `code_writer` Junie ACP
binding can return the agent-authored output set through `CHAINWORKS_OUTPUT`
without completion repair, while the executor generates
`changed_files_manifest` as control-plane evidence and settles all declared
outputs through the normal discovery-decision materialization functions. That
gate is provider-specific proof, not a broader replacement for this contract or
for the P088 completion-repair boundary.

### Missing outputs get one same-session repair turn

When an agent turn finishes without required outputs, the runtime must try one narrow repair turn in the same live ACP session before it invalidates that session or blocks the run for `missing_required_outputs`.

The repair turn is not a task retry. It must:

- reuse the same `session_generation_id`,
- send a narrow output-repair prompt,
- tell the agent not to revisit or redo the task,
- request only the missing or invalid `CHAINWORKS_OUTPUT` payloads,
- include contract-complete skeletons for the failed declared outputs,
- and validate the repaired payloads through the same declared-output import path.

If the repair turn produces contract-valid required outputs, the executor merges the repair result into the original execution result and the stage may continue without a new provider session. If the repair turn fails, cannot be settled, or still does not produce valid outputs, the runtime records the failed output settlement and then invalidates/closes the active session generation so the next operator retry starts from a fresh session.

Repair settlement must bind only exact declared output identities. When prompts use canonical target paths as
`CHAINWORKS_OUTPUT` keys, those keys must match byte-for-byte after JSON decoding: no leading/trailing whitespace,
no alternate absolute path spelling, no display label, and no companion path. A repaired payload keyed as
`" /abs/path/to/output.json"` is an undeclared envelope, not the declared `/abs/path/to/output.json` artifact, even
if the JSON value itself is contract-valid. Recovery evidence should surface this as a binding/key mismatch rather
than implying that the agent did no useful work.

For implementation writer repair specifically, a failed same-session repair should not be allowed to erase the
broader implementation continuity policy. The repair generation may be invalidated after the failed settlement, but
the next normal retry should still follow the `code_writer` implementation family reuse policy documented in
[session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md#family-reuse-is-opt-in-only).

`AcpRuntimeManager` therefore keeps a requested live session alive after both `completed` and `failed` prompt statuses. A failed prompt is preserved only so the executor can make this bounded repair attempt; normal cross-invocation reuse still follows the session-lineage policy in [session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md).

### One contract authority

`AgentCatalog.contracts` remains the single contract authority for this slice.

`OutputContractResolverV2` is the only runtime reader that normalizes that authority for:

- `WorkflowOrchestrator`,
- `ArtifactManager`,
- `RuntimeSessionBridge`,
- `RunReportBuilder`,
- and recovery/report surfaces.

No runtime component may keep a second contract registry or silently override catalog truth with output-name heuristics.

### Mandatory adopters

The mandatory contract adopters in this slice are:

- `proposal_review_ui`
- `proposal_review_ux`
- `proposal_review_architect`
- `proposal_review_po`
- `proposal_review_summary`
- `implementation_self_assessment_v2`
- `docs_report`
- `docs_delta`

The aggregate `proposal_review_summary` output is a first-class contract, not an implicit transition side effect.
`implementation_self_assessment_v2` is the canonical truth for implementation completion and handoff.

### Structured-output modes are explicit

The runtime-normalized schema for this slice includes:

- `machine_format`
- `human_format`
- `validation_mode`
- `required_fields`
- `raw_artifact_name`
- `normalized_artifact_name`

Supported validation modes are:

- `strict_structured`
- `structured_with_human_companion`
- `human_only`

Rules:

- `strict_structured` may not silently accept prose in place of the machine payload.
- `structured_with_human_companion` must persist both the machine-valid artifact and the human companion artifact.
- if the product wants markdown, the contract must say markdown; if the contract says JSON, the runtime must require JSON.

### Aggregate inputs must already be contract-valid

Aggregate steps consume only normalized, contract-valid reviewer outputs.

Raw invalid reviewer artifacts remain evidence only and must not be treated as aggregate inputs.

This keeps aggregate transition truth tied to validated stage artifacts instead of markdown or partially parsed payloads that happened to exist on disk.

### Canonical artifact contracts drive transition truth

Machine-consumed workflow artifacts are represented as canonical artifact contracts.
Agents may still write raw JSON or Markdown reports, but raw files are evidence, not
decision truth. The control-plane validates and normalizes declared outputs into
SQLite-owned artifact generations, chooses active contract pointers, and then exports
`artifacts/active-index.json` and `state/run-state.json` as readback projections after
the SQLite transaction commits.

The current controlled contracts are:

| Contract | Canonical path | Canonical status values |
|---|---|---|
| `audit_report_v1` | `audit/proposal-vs-implementation.json` | `implemented`, `needs_code_fixes`, `invalid`, `unknown` |
| `security_report_v1` | `security/report.json` | `pass`, `block`, `invalid`, `unknown` |
| `prepush_review_v1` | `review/prepush.json` | `pass`, `block`, `invalid`, `unknown` |
| `docs_report_v1` | `docs/report.json` | `pass`, `not_needed`, `block`, `invalid`, `unknown` |
| `docs_delta_v1` | `docs/changed-files.json` | generated evidence |
| `implementation_closeout_readiness_v1` | `review/implementation-closeout-readiness.json` | `ready`, `ready_with_risks`, `handoff_required`, `not_ready`, `blocked`, `invalid`, `unknown` |
| `implementation_self_assessment_v2` | `implementation/self-assessment.json` | `complete`, `handoff_required`, `needs_code_fixes`, `blocked`, `unknown`, `invalid` |
| `tests_result_v1` | `implementation/tests.json` | `green`, `red`, `blocked`, `unknown` |
| `implementation_review_summary_v1` | `review/implementation-summary.json` | `code_complete`, `needs_code_fixes`, `release_evidence_blocked`, `invalid` |
| `run_state_projection_v1` | `state/run-state.json` | generated only |

Contract normalization is explicit and contract-specific:

- `prepush_review_v1`: `PASS`, `PASS_WITH_NOTES`, `pass`, and reviewer `conditional_pass` normalize to `pass`; `BLOCK`, `needs_fixes`, and reviewer `changes_required` normalize to `block`.
- `security_report_v1`: `PASS`, `pass`, and reviewer `pass_with_notes` normalize to `pass`; `BLOCK`, `block`, and reviewer `fail` normalize to `block`.
- `docs_report_v1`: `success`, `synced`, `aligned`, and `pass` normalize to `pass`; `not_needed` remains `not_needed`; `blocked` normalizes to `block`.
- `audit_report_v1`: implementation truth comes from `implementation_status`; release evidence blockers stay in a separate `release_evidence_status` dimension and must not make code completeness look incomplete.
- `implementation_self_assessment_v2`: `implementation_complete`, `verification_green`, and blocking `remaining_code_tasks` are canonical dimensions. The workflow implementation loop reads `implementation_complete`, not legacy `seemingly_complete`.
- `tests_result_v1`: status is the canonical test outcome; workflow guards read `tests_result_v1.status`, not legacy `tests_result.green`.
- `implementation_review_summary_v1`: reviewer `changes_required`, `blocked`, and `block` normalize to `needs_code_fixes`; `release_evidence_blocked` remains separate so non-code release evidence can be routed without disguising source-code readiness.

### Implementation self-assessment and handoff

`implementation_self_assessment_v2` is the stable implementation-completeness contract for code-writer output.
It separates source/test completion from downstream non-code handoff so the implementation loop does not keep
asking `code_writer` to perform manual release, documentation, operator, or evidence work.

The canonical artifact path is `implementation/self-assessment.json`. The artifact contains:

- `implementation_complete`: whether code-writer-owned implementation work is complete.
- `verification_green`: whether the writer-owned verification evidence is green.
- `remaining_code_tasks`: code-writer-owned follow-up tasks with `summary`, `owner`, `blocking`, and `evidence`.
- `handoff_tasks`: non-code tasks with `summary`, `owner_class`, `target_stage`, `blocking_review`, and `evidence`.
- `known_risks`, `tests_run`, and `docs_impacted`: normalized display evidence for review, release, reports, and operator surfaces.

The domain parser owns validation, status normalization, warning generation, summary dimensions, and display rows.
Consumers must not independently infer status, blocker counts, owner-class counts, or handoff detail from raw JSON.
Engine import, active artifact generation, workflow guards, run-state projection, reports, GraphQL, MCP, and Swift
operator surfaces consume the same normalized summary.

The stable status vocabulary is:

- `invalid`: the artifact is malformed, missing required fields, or has invalid nested item shape.
- `needs_code_fixes`: code-writer-owned source or test work remains.
- `blocked`: verification is not green and there is no blocking code-writer task that can resolve it in the loop.
- `handoff_required`: code work can leave the implementation loop, but downstream non-code handoff remains.
- `complete`: implementation and verification are complete with no blocking handoff.
- `unknown`: no valid v2 or compatible legacy truth is available yet.

`needs_code_fixes` is reserved for blocking code-writer-owned source or test work.
If `verification_green` is true and every `remaining_code_tasks` entry is
`blocking=false`, the parser normalizes the summary out of the code loop as
`handoff_required` even when the raw artifact says `implementation_complete=false`.
Non-blocking code-tail notes remain visible in the summary for downstream review,
but they do not keep scheduling `code_writer`.

Workflow transitions read the active contract row, not raw files. `needs_code_fixes` keeps the run in the implementation
loop. `complete`, `handoff_required`, and `blocked` leave the code-writer loop and route the run to the downstream
review, release, or operator decision surfaces that own the remaining work. Legacy `implementation_self_assessment`
artifacts remain readable as compatibility evidence, but `seemingly_complete` is not transition authority for new
implementation-loop decisions.

### Implementation closeout readiness

`implementation_closeout_readiness_v1` is the decision contract for moving from implementation review
to manual release or handoff. It evaluates proposal-specific readiness by combining self-assessment,
implementation audit, proposal gates, and release evidence handoff.

The canonical operational reference for this behavior is
[`implementation-closeout-readiness.md`](implementation-closeout-readiness.md).

#### Readiness Mode

Closeout readiness is governed by a per-run **closeout readiness mode** frozen at run admission:
- **`advisory`** (and the `legacy_fallback` diagnostic variant): Diagnostic-only mode for manual release. Closeout readiness is synthesized and visible to operators, and the synthesizer caps `enter_manual_release` to `await_operator_decision` until enforcement cutover. `return_to_code_refine` remains an allowed repair route: a missing release/closeout gate must not block implementation refinement when review truth says code fixes are required. Observability fields (status, blocker counts, gate status) are preserved unchanged so operators see what the matrix decided.
- **`enforcement`**: Strict gating mode. Transition to manual release requires a resolved `enter_manual_release` decision.

#### Decision and Gating

The canonical artifact path is `review/implementation-closeout-readiness.json`. The artifact contains:

- `status`: the readiness status (`ready`, `ready_with_risks`, `handoff_required`, `not_ready`, `blocked`, `invalid`, `unknown`).
- `decision`: the workflow routing decision (`enter_manual_release`, `await_non_code_handoff`, `return_to_code_refine`, `await_gate_definition`, `await_operator_decision`, `block_with_evidence`).
- `proposal_gate`: status of the required canonical proposal gate.
- `audit`: status and reference to the implementation audit report.
- `code_blockers`: list of proposal-critical code blockers.
- `handoff_blockers`: list of non-code handoff blockers.
- `loop_policy`: current refine cycles used, remaining budget, and whether a soft convergence checkpoint was reached.
- `fingerprint_json`: optional snapshot of the run state used to detect stale or replayed results.

The closeout decision distinguishes code-owned blockers from non-code blockers to prevent infinite
code/review loops. If code blockers exist and budget remains, the run returns to code refine.
If only handoff blockers remain, the run moves to a handoff or operator-decision state.

#### Soft Convergence Checkpoint

If the same set of code blockers recurs across repeated audit/refine cycles without meaningful progress or a change in the blocker set, the synthesizer marks a **soft convergence checkpoint**. This routes the run to `await_operator_decision` instead of looping silently back to code refinement, even if the hard loop budget (P052) is not yet exhausted.

#### Closeout Fingerprint and Latency Budget

To ensure decision consistency, the synthesizer consumes a **Closeout Fingerprint** that captures the state of the run at the time of evaluation. If the fingerprint computation exceeds the **5,000ms latency budget**, the synthesizer fails closed with `status: unknown` and `decision: block_with_evidence`, preventing a transition based on potentially stale or inconsistent state.

After the state-9 closeout transaction commits the active gate/readiness pair,
the orchestrator rebuilds run-state projections so transition evaluation,
GraphQL `runs.get`, and MCP `runs.get` see current closeout readiness truth in
the same `AdvanceRun` cycle. A rebuild failure is logged and retried on the
next cycle: active SQLite truth remains authoritative and projections are
eventually consistent. The exported run-state projection includes a derived
`fingerprint_hash` short hash for each closeout readiness row, sourced from
`fingerprint_json` via `CloseoutFingerprint::short_hash`; it is the
operator-facing identifier used in tooltips, copy-to-clipboard, and VoiceOver
announcements, while the full fingerprint payload remains available only
through artifact readback. Rows without a fingerprint expose
`fingerprint_hash: null`.

#### Risk Lineage

For states such as `ready_with_risks`, enforcement mode requires **typed risk lineage** (accepted lineage or governed settlement) for each risk. Free-form `known_risks` text alone never satisfies the requirement to `enter_manual_release`.

GraphQL exposes nullable `implementationCloseoutReadinessSummary` and compatibility
`closeoutReadinessSummaryJson` fields on run read models. MCP run detail/list and
report readbacks expose the same projection as `implementation_closeout_readiness_summary`
and compatibility `closeout_readiness_summary`.

After implementation review aggregation, `implementation_review_summary_v1.status`
is an input to closeout readiness rather than direct transition authority. A canonical
`needs_code_fixes` or `invalid` review summary feeds `return_to_code_refine`;
`code_complete` can feed `enter_manual_release`, and `release_evidence_blocked`
is preserved as a separate release-hold/manual decision input.

### Generated run-state projection
GraphQL exposes a nullable `implementationSelfAssessmentSummary` field on run read models. MCP run detail/list
responses expose the same projection as `implementation_self_assessment_summary`. `null` means no v2/v1 projection
exists yet; raw artifact readback remains available for evidence inspection.

Transition evaluation must read SQLite active contract values. If a controlled artifact
such as `prepush_review_report.status`, `implementation_self_assessment_v2.implementation_complete`,
or `tests_result_v1.status` has no active validated contract row, evaluation fails
closed instead of falling back to the raw file, even when a matching file exists under
the run meta root. Controlled `exists(...)` checks follow the same rule: raw file
presence alone is not enough.

Invalid or missing controlled artifacts block with structured evidence, including the
contract id, path, and validation errors. A status such as `PASSISH` is evidence of an
invalid report, not a string to coerce or patch by hand.

The active artifact index is canonical in SQLite:

- `artifact_contract_generations` stores each imported generation and its validation result.
- `active_artifact_contracts` stores the current generation pointer per run and contract.
- `run_state_projections` stores the DB-owned generated run-state projection.
- `artifact_contract_overrides` stores typed operator overrides.
- JSON exports are diagnostic projections only and are rebuilt from SQLite when missing, stale, malformed, or partially written.

Artifact import and projection rebuild are one transaction up to the DB commit:

1. read the raw artifact from the run-owned meta root,
2. validate and normalize it against the registry,
3. insert the artifact generation,
4. update the active pointer only when the generation is valid and current,
5. apply active typed overrides,
6. rebuild the DB-owned run-state projection,
7. commit,
8. export `artifacts/active-index.json` and `state/run-state.json` from committed DB truth.

Crash behavior is fail-closed. A crash before commit leaves the previous active truth
authoritative. A crash after commit but before export leaves SQLite authoritative; the
next read, daemon startup, or projection rebuild re-exports JSON. The runtime must not
use exported JSON to drive transition truth.

### Generated run-state projection

`state/run-state.json` is generated by the control-plane. It is built from DB run state,
stage/approval state, active artifact contracts, override truth, partial-output warnings,
and available loop/recovery context.

If a legacy agent writes `state/run-state.json`, the daemon imports or records it as
advisory/superseded evidence. It must not overwrite the DB-owned run-state projection
and must not poison GraphQL/MCP readback.

### Typed operator overrides

Operator overrides are typed records in SQLite. They can affect canonical transition
truth while active, remain visible in GraphQL/MCP readback after expiry, and never mutate
raw agent report files.

The override command path is:

- shared command payload, for example `Command::OverrideArtifactContract`;
- MCP tool surface `artifacts.override_contract`;
- dedicated operator capability, such as `CapabilityToolId::ArtifactsOverrideContract`;
- command journal entry with caller surface, principal, reason, source artifacts, old/new normalized values, and `journal_id`;
- expiry by `expires_at_stage`, after which transition evaluation ignores the override while readback keeps expired override evidence.

Observers and agents cannot create overrides. Non-operator attempts fail at the command
or MCP capability boundary.

### Degraded output policy

Failed executions with valid declared outputs are not automatically successful. A failed
execution may produce `valid_outputs_from_failed_execution`, but those outputs satisfy
transition truth only when the compiled workflow stage explicitly permits it.

Missing `degraded_output_policy` means:

```yaml
degraded_output_policy:
  mode: deny
```

The explicit allow form is:

```yaml
degraded_output_policy:
  mode: allow_valid_contract_outputs
  contracts:
    - prepush_review_v1
  failure_kinds:
    - provider_quota
  max_settlement: valid_outputs_from_failed_execution
```

Compiler validation rejects unknown modes, unknown contract ids, unknown failure kinds,
unknown settlement values, and `allow_valid_contract_outputs` without at least one known
contract id.

### Agent output settlement is separate from provider failure classification

Artifact truth must describe both dimensions of an ACP execution:

- why the provider or transport failed or succeeded,
- and what happened to the declared outputs.

The Rust control plane records the second dimension as `AgentOutputSettlement`.
The stable values are:

- `none`
- `valid_outputs_from_completed_execution`
- `valid_outputs_from_failed_execution`
- `missing_required_outputs`
- `invalid_required_outputs`
- `ignored_late_outputs`

This settlement is derived from `OutputDiscoveryDecision` records built by the engine discovery pipeline, stored on runtime facts, and copied into artifact-contract generation
evidence. The raw bounded meta-root and exact-path discovery logs live in `agent_execution_discovery_diagnostics`. It must not be collapsed into `AgentFailureKind`; for example,
`ignored_late_outputs` is output ownership truth, not a provider failure reason.

### Source-generation claims own active artifact writes

Post-P058 artifact-producing `InvokeAgent` executions write through a source-generation
claim. The active claim records:

- run id,
- stage execution id,
- agent execution id,
- source work item id,
- current session generation id,
- claim state,
- supersession metadata,
- close timestamps.

Claim states are:

- `active`
- `superseded_pending_retry`
- `superseded`
- `closed`
- `legacy_unowned`

An output import may update active artifact truth only when the source claim is still
`active` and the output comes from the same session generation recorded on the claim.
Outputs from closed, superseded, or superseded-pending-retry claims are preserved as
evidence but must never replace the active artifact contract pointer.

The import transaction owns these updates together:

- declared-output validation,
- artifact-contract generation insert,
- active artifact contract pointer update when valid and current,
- runtime facts settlement/counter update,
- run-state projection rebuild.

If the transaction rolls back, both active artifact truth and runtime facts roll back.
This prevents late outputs, retried stages, or stale provider subprocesses from changing
current run truth after ownership has moved on.

### Failure evidence survives post-generation validation failure

When validation fails after output generation, the runtime preserves canonical evidence rather than collapsing to summary-only status.

The durable evidence path for this slice includes:

- raw output artifacts,
- receipt artifacts,
- transcript artifacts,
- `ValidationFailureRecord`,
- and the stage-owned failed-stage evidence packet.

`ArtifactPersistenceOrderingPolicy` keeps the persistence order explicit: raw artifacts first, validation and evidence second, settlement last.

Reports, exports, and recovery surfaces should reference the canonical failed-stage evidence object rather than reconstructing the failure from loose file scans.

Because canonical evidence may contain sensitive data, operator-visible summaries should default to summarized or redacted presentation until explicit inspection is requested.

### Current northbound readers consume the typed failure record

For this slice, the current northbound readers are:

- GraphQL artifact reads,
- GraphQL `Run.implementationSelfAssessmentSummary`,
- MCP `reports.get`,
- MCP `report://{run_id}`,
- MCP `runs.get` / `runs.list` implementation summary fields,
- and stage projections for the lightweight `has_validation_failure` bit.

The typed source of truth for failure detail is the durable `ValidationFailureRecord`, not loose artifact metadata.

That means:

- stage projections carry only the lightweight stage-status signal,
- artifact and report readers decode the persisted typed failure record,
- and no alternate metadata-only or heuristic reader lane should compete with the durable record.

### Same-run retry keeps lineage and inspectable history

Same-run retry is distinct from clone-run.

For this slice:

- the failed attempt remains inspectable,
- the retry stays on the same logical frozen snapshot,
- retry artifacts use a disjoint namespace rather than overwriting prior attempt artifacts,
- artifact lineage metadata and reused-sibling references remain persisted,
- and recovery surfaces explain why same-run retry is valid before clone-run.

When a same-run retry supersedes a source generation, active claims move through
`superseded_pending_retry` before being finalized as `superseded` by the replacement
execution. Any output that arrives from the old source after supersession is treated as
late evidence and does not update active artifact truth.

This retry truth is stage-owned and depends on the lower execution-truth substrate documented in [execution-truth-and-recovery.md](execution-truth-and-recovery.md).

### Operator retry instructions

Operators can attach a durable, one-shot instruction to a stage or agent retry
attempt using the `operator_instruction` field on `stages.retry`.

Rules for retry instructions:

- **Scope**: The instruction is bound only to the next retry attempt and the
  invocation(s) it creates. It does not become sticky context for later stages,
  cloned runs, or unrelated retry attempts.
- **Validation**: Instructions must be 1-2000 characters, trimmed of surrounding
  whitespace, and free of non-whitespace ASCII control characters.
- **Auditability**: Instructions are persisted in the `command_journal` (redacted
  in read-only summaries) and in a dedicated `retry_operator_instruction_bindings`
  table.
- **Delivery**: For targeted agent retries, the instruction is injected directly
  into the work-item payload. For full-stage retries, the orchestrator manages
  fan-out delivery to each retry-created invocation.
- **Immutability**: Once accepted, the instruction cannot be mutated. Issuing a
  new retry with a different instruction creates a fresh scope.

Agent prompt guidance instructs recipients to obey the instruction within the
approved proposal boundaries and report conflicts instead of silently overriding
frozen truth.

### Recovery is narrow before clone-run

The canonical recovery surfaces remain:

- `RecoverySheet`
- `BlockedRunRecoveryView`

They must prefer the narrowest valid next action from canonical stage evidence:

- `Retry Failed Agent`
- `Retry Failed Stage`
- `Retry Aggregate Step`
- `Clone Frozen Snapshot`
- `Clone Current Config`

Clone-run is not an acceptable default when narrower recovery is still valid.

### Declarative Tier 1 contract fields are enforce-or-reject

The mandatory declarative runtime surfaces in this slice are:

- `contracts.*`
- `backend_profiles.*.structured_output`

Rules:

- no Tier 1 field may silently no-op,
- unsupported provider/schema combinations must fail in preflight,
- successful transport-level structured-output support does not remove post-generation contract validation,
- metadata-only or later-slice declarations must stay explicitly tiered rather than overclaimed.

`DeclarativeCoverageReport` is the persisted/testable inventory of that tiering.

### Proposal drafting compaction is explicit

`ProposalDraftCompactionPolicy` bounds oversized proposal outputs without silently dropping useful drafts.

When compaction is invoked, the runtime preserves:

- raw draft artifacts,
- compacted or normalized artifacts,
- compaction metadata,
- and outcome truth about whether the stage succeeded with compaction or failed despite compaction.

## Operator-Visible Outcomes

After this slice, a blocked proposal-review or aggregate stage should make all of the following explicit:

- whether an individual reviewer failed contract validation,
- whether the aggregate `proposal_review_summary` step failed or never produced its required output,
- where raw outputs, receipts, and transcripts live,
- which canonical failed-stage evidence object explains the block,
- which narrow recovery action is valid and why,
- and which declarative contract controls were actually enforced.

## Adjacent References

Use:

- [structured-output-envelope-and-contract-validation.md](structured-output-envelope-and-contract-validation.md) for the implemented ACP envelope, canonical binding, validation-mode, and persistence-order substrate,
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) for lower-layer outcome and settlement truth,
- [workflow-execution-engine.md](workflow-execution-engine.md) for orchestrator and executor topology,
- [runtime-contract.md](runtime-contract.md) for frozen snapshot and artifact boundaries,
- [operator-experience.md](operator-experience.md) for shell and recovery presentation rules,
- [test-gates.md](test-gates.md) for the current verification lanes.
