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
- `code_writer` completion-boundary subtypes, engine-owned failure envelopes, and provider-neutral direct-file repair settlement,
- workflow-owned quality-gate blocker boundary contracts and readback,
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

The executor then binds those payloads to the compiled output declarations, validates them against `AgentCatalog.contracts`, and materializes canonical artifact files. For large declared outputs, the payload may be a `direct_file` / `direct_file_ref` manifest that points to the declared canonical path instead of embedding the full file content. That manifest is only a transport hint: the executor must read the canonical file, verify path ownership, freshness, digest/size when supplied as real values, and then validate the file contents against the same declared contract before publication. Synthetic digest/size placeholders such as non-hex `sha256:*` markers or approximate sizes are recorded as diagnostics and do not override a fresh, owned, schema-valid canonical file. Exact-path filesystem outputs and legacy block envelopes remain accepted as compatibility evidence, but they do not create a second contract authority and do not bypass validation.

The retained `proposal-089|p089` gate is the focused Junie proof for this
materialization path. It validates that the production `code_writer` Junie ACP
binding can return the agent-authored output set through `CHAINWORKS_OUTPUT`
without completion repair, while the executor generates
`changed_files_manifest` as control-plane evidence and settles all declared
outputs through the normal discovery-decision materialization functions. That
gate is provider-specific proof, not a broader replacement for this contract or
for the P088 completion-repair boundary.

Control-plane generated outputs are not provider-authored outputs. If a provider
returns a `CHAINWORKS_OUTPUT` envelope for a control-plane-owned output such as
`changed_files_manifest`, the executor must use the generated canonical file as
the source of truth and ignore conflicting provider manifests for that logical
output. Provider manifests can describe agent-authored files, but they must not
shadow control-plane evidence or cause a generated output to be reported as
missing.

### Missing or malformed outputs get one same-session repair turn

When an agent turn finishes without required outputs, or with a malformed final
`CHAINWORKS_OUTPUT` object for a declared output, the runtime must try one narrow
repair turn in the same live ACP session before it invalidates that session or
blocks the run for `missing_required_outputs` / `invalid_required_outputs`.

The repair turn is not a task retry. It must:

- reuse the same `session_generation_id`,
- send a narrow output-repair prompt,
- tell the agent not to revisit or redo the task,
- request only the missing or invalid `CHAINWORKS_OUTPUT` payloads,
- include contract-complete skeletons for the failed declared outputs,
- and validate the repaired payloads through the same declared-output import path.

If the repair turn produces contract-valid required outputs, the executor merges the repair result into the original execution result and the stage may continue without a new provider session. If the repair turn fails, cannot be settled, or still does not produce valid outputs, the runtime records the failed output settlement and then invalidates/closes the active session generation so the next operator retry starts from a fresh session.

Malformed final-envelope repair is strict. The engine records
`malformed_json_contract_output` with the parser error, output name, and canonical
target path, then asks for a fresh valid JSON object. It must not auto-edit the
provider's malformed JSON, even when a small syntax fix would make it parse.
Repair output is accepted only after the normal declared-output binding,
contract validation, and materialization path succeeds. A repair payload that
returns a `direct_file` manifest is not itself the declared artifact content; it
is accepted only if the referenced canonical file is current-attempt evidence and
the file contents validate against the declared output contract.

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

### Junie code-writer completion boundary

Junie `code_writer` uses the same structured-output substrate as other
providers, with additional runtime facts for long-running ACP attempts. A
`code_writer` attempt with required outputs has one authoritative terminal
completion shape:

- a valid final `CHAINWORKS_OUTPUT` payload;
- an engine-synthesized `code_writer_engine_failure.v1` receipt section;
- an engine-synthesized `code_writer_repair_failure.v1` receipt section; or
- an explicit narrative, missing, truncated, repair, or runtime subtype in
  `completion_boundary_subtype`.

Free-form final prose is not ordinary success for this path. Provider-authored
JSON that looks like a failure envelope is untrusted input. It can be diagnostic
extraction text, but persisted failure-envelope truth is synthesized from
engine-owned runtime facts, execution/session identity, preflight facts,
settlement decisions, and repair validation. Provider failure claims that spoof
engine-owned envelopes are read back as `provider_claim_rejected` and must not
materialize outputs.

The public subtype field is a provider-neutral subtype wrapper:

- receipt: `code_writer_completion_receipt_v1.completion_boundary_subtype`;
- GraphQL: `implementationCompletion.completionBoundarySubtype`;
- MCP: `runs.get.implementationCompletion.completion_boundary_subtype`;
- run report: `implementation_completion.completion_boundary_subtype`.

The first known values are Junie-specific because the covered runtime family is
Junie ACP: `junie_final_response_missing`,
`junie_final_response_truncated`, `junie_progress_without_terminal_handoff`,
`junie_repair_returned_narrative`, `junie_repair_returned_malformed_json`,
`junie_repair_outputs_partially_materialized`, and
`junie_runtime_tool_path_failure_before_publication`. Unknown subtype values
round-trip as raw values in the existing enum-wrapper style.

The receipt stores runtime and forensic detail in additive fields:

- `provider_runtime_family`
- `final_payload_status`
- `progress_before_handoff`
- `runtime_preflight_phase`
- `runtime_tool_path_preflight_json`
- `final_completion_payload_capture_json`
- `engine_failure_envelope_json`
- `repair_failure_envelope_json`
- `repair_materialization_summary_json`
- `repair_materialization_mode`

P088 completion-repair artifacts, including worktree fingerprints, redacted
prompts, expected-output snapshots, completion captures, receipts, and
failed-stage evidence, are written only after the executor verifies that the
target path remains under the canonical run `workspace_root` and creates parent
directories through a symlink-safe component walk.

`runtime_preflight_phase` values such as `preflight_running`,
`preflight_remediating`, `passed`, and `failed_no_launch` are runtime facts, not
new `AgentStatus` values. Public preflight JSON records attempt count,
remediation, provider launch state, and redacted operation/path/failure classes.

### Provider-neutral direct-file and per-output repair settlement

When completion or repair settlement returns multiple declared outputs, the
engine validates and settles each output independently. This behavior is
provider-neutral for ACP providers such as Claude, Codex, Gemini, Auggie, and
Junie. Valid siblings may commit even when another sibling is malformed, stale,
or absent. Invalid siblings remain rejected with typed reasons and must not
overwrite canonical files or active artifact pointers.

`direct_file` / `direct_file_ref` candidates follow the same staged row model.
The candidate digest is the digest of the referenced canonical file contents, not
the tiny manifest. A manifest keyed by output name or canonical path may be used
only to locate the expected file; settlement success still requires the file to
match the compiled output declaration, remain inside the authorized run
meta-root or artifact root, be fresh relative to the current attempt, and pass
contract validation.

The durable settlement table is `code_writer_output_settlement_rows`. Each row
belongs to exactly one `code_writer_completion_receipts.id` through
`receipt_id TEXT NOT NULL REFERENCES code_writer_completion_receipts`, and
`(receipt_id, output_name)` remains the receipt readback key. The replay
idempotency key is `(agent_execution_id, repair_attempt, output_name,
candidate_digest)` for non-null candidate digests.

Accepted rows are the only source for canonical mutation and active artifact
pointer publication. Staged rows without accepted settlement may be retried or
discarded; committed rows are recovered from their canonical digest; failed rows
preserve the prior digest and failure reason.

P090 Junie hardening is always enabled in runtime code. Strict final-payload
handling and Junie tool-path preflight enforcement remain Junie-specific.
Provider-neutral staged/direct-file settlement does not depend on process
environment variables during normal daemon startup.

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
| `proposal_decomposition_plan_v1` | `proposal/decomposition-plan.json` | `ready_with_declared_boundaries`, `split_required`, `invalid`, `unknown` |
| `quality_gate_blocker_assessment_v1` | `quality-gate/blocker-assessment.json` | `candidate`, `accepted`, `rejected`, `invalid`, `unknown` |
| `blocker_boundary_status_v1` | `quality-gate/blocker-boundary-status.json` | `output_settlement_required`, `side_effect_reconciliation_required`, `runtime_recovery_required`, `review_refresh_required`, `local_code_tail_present`, `invalid_claim`, `blocked_no_progress`, `awaiting_human_boundary_approval`, `pass`, `invalid`, `unknown` |
| `blocker_boundary_approval_request_v1` | `quality-gate/blocker-boundary-approval-request.json` | `requested`, `invalid`, `unknown` |
| `blocker_boundary_human_decision_v1` | `quality-gate/blocker-boundary-human-decision.json` | `granted`, `rejected`, `invalid`, `unknown` |
| `followup_proposal_seed_v1` | `quality-gate/followup-proposal-seed.json` | `created`, `invalid`, `unknown` |
| `run_state_projection_v1` | `state/run-state.json` | generated only |

Contract normalization is explicit and contract-specific:

- `prepush_review_v1`: `PASS`, `PASS_WITH_NOTES`, `pass`, and reviewer `conditional_pass` normalize to `pass`; `BLOCK`, `needs_fixes`, and reviewer `changes_required` normalize to `block`.
- `security_report_v1`: `PASS`, `pass`, and reviewer `pass_with_notes` normalize to `pass`; `BLOCK`, `block`, and reviewer `fail` normalize to `block`.
- `docs_report_v1`: `success`, `synced`, `aligned`, and `pass` normalize to `pass`; `not_needed` remains `not_needed`; `blocked` normalizes to `block`.
- `audit_report_v1`: implementation truth comes from `implementation_status`; release evidence blockers stay in a separate `release_evidence_status` dimension and must not make code completeness look incomplete.
- `implementation_self_assessment_v2`: `implementation_complete`, `verification_green`, and blocking `remaining_code_tasks` are canonical dimensions. The workflow implementation loop reads `implementation_complete`, not legacy `seemingly_complete`.
- `tests_result_v1`: status is the canonical test outcome; workflow guards read `tests_result_v1.status`, not legacy `tests_result.green`.
- `implementation_review_summary_v1`: reviewer `changes_required`, `blocked`, and `block` normalize to `needs_code_fixes`; `release_evidence_blocked` remains separate so non-code release evidence can be routed without disguising source-code readiness.
- `blocker_boundary_human_decision_v1`: human `accept` / `reject` labels normalize to durable approval states `granted` / `rejected`.

### Workflow-owned quality-gate boundary contracts

Quality-gate blocker routing is owned by canonical artifact contracts and the
compiled workflow graph, not by lead-agent prose or ad hoc human route choices.
The boundary path begins with a `quality_gate_blocker_assessment_v1` generation
and an in-process `quality_gate_boundary_evaluator` system task. The evaluator
normalizes that assessment into `blocker_boundary_status_v1`, records canonical
dimensions, and exposes the same truth through GraphQL, MCP run detail, operator
reports, and runtime health readback.

Workflow-consumed fields for the boundary are SQLite-owned extracted fields. The
runtime must fail closed rather than parse raw JSON when an unregistered field is
used in `artifact.field` transitions. The controlled field set includes:

- `proposal_decomposition_plan_v1.requires_split`,
  `implementation_start_decision`, `split_candidates`, and
  `blocking_split_candidate_count`;
- `blocker_boundary_status_v1.status`, `projection_integrity`,
  `primary_owner_class`, `workflow_route_hint`, `blocker_freshness`,
  `allowed_workflow_routes`, blocker fingerprints, hard blockers,
  `followup_proposal_required`,
  `has_release_blocking_external_blockers`, and
  `has_no_release_blocking_external_blockers`;
- `blocker_boundary_approval_request_v1.status` and the approval id linked to
  the concrete manual approval row created for the boundary gate.

The evaluator gives lower-layer recovery precedence before any boundary
approval. Output settlement, side-effect reconciliation, and runtime recovery
route to their dedicated recovery states. Stale, superseded, unknown, or
proposal-owned review evidence routes to review refresh. Fresh local
code-writer-owned tails route back to implementation refinement. External,
release-blocking, follow-up, and server-verified no-progress cases may create a
manual boundary approval request. A pass status routes directly to manual
release.

Manual boundary approval remains accept/reject only. The human decision records
whether the operator accepts the server-evaluated boundary; it does not select a
route. Accepted boundaries use `blocker_boundary_status_v1` fields to choose
follow-up seed generation, external evidence hold, or manual release. Rejected
boundaries route back to review refresh for fresh evidence.

`runtime.health` and operator reports expose the quality-gate boundary mode,
readiness, current metric values, go/hold criteria, owner decision, and
promotion readiness. Enforcement-mode changes require command-journal or
rollout-contract evidence. The retained historical `proposal-094|p094` gate
proves this contract against the stable workflow/catalog examples and focused
Rust suites.

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

Artifact-producing `InvokeAgent` executions write through a source-generation
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

`work_items.complete` has the same fail-closed boundary for running
`InvokeAgent` items. A running invoke work item may be completed only when its
payload carries the claimed `agent_execution_id` and
`agent_execution_runtime_facts` already proves valid required outputs through
`valid_outputs_from_completed_execution` or
`valid_outputs_from_failed_execution` with `valid_required_outputs > 0`. If
that proof is absent, completion is rejected before mutating the work item,
agent execution, post-invoke advance scheduling, or active artifact
source-generation claim.

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

For malformed final `CHAINWORKS_OUTPUT` failures, `ValidationFailureRecord`
includes `diagnostic_artifact_paths` when redacted completion text was captured.
This makes the final provider handoff inspectable even if a native provider
session archive is incomplete or later unavailable. The linked completion text is
diagnostic evidence only; aggregate stages still consume only normalized,
contract-valid outputs.

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
- [p079-repair-prompt-template.md](p079-repair-prompt-template.md) for the pinned `p079_repair_v1` prompt contract,
- [p079-recovery-attribution.md](p079-recovery-attribution.md) for bounded transcript/provider-envelope recovery attribution,
- [p079-adapter-idempotency.md](p079-adapter-idempotency.md) for lease, restart, and adapter idempotency rules,
- [test-gates.md](test-gates.md) for the current verification lanes.

## P079: Output Contract Repair and Fallback Details

Operator guidance for inspecting and rolling back P079 lives in [../runbooks/orchestration/p079-output-repair.md](../runbooks/orchestration/p079-output-repair.md).

### Implementation status

P079 is implemented for the current safe repair/recovery scope. Wired today: the SQLite migration, domain schema/enums, repair-event and lease repositories, GraphQL/MCP/run-report readback, Swift DTO decode/presentation helpers, read-only macOS inspector surfacing, deterministic fixture same-session repair with lease/evidence lifecycle, MCP runtime receipt sanitization (SEC-P079-MCP-001), crash-consistent materialization (SEC-P079-SETTLEMENT-001), Junie plan-evidence capture/redaction, and bounded transcript/provider-envelope recovery for transport-attributed current-execution output. Raw transcript JSON and unattributed provider envelopes fail closed with typed evidence.

Current hardening includes component-aware plan-evidence path containment (`p079_path_inside_root` rejects sibling-prefix paths and `..` escapes), GraphQL/MCP readback rejection of URL-encoded traversal in evidence and plan-evidence paths (including mixed literal/encoded forms validated after percent-decode), broader redaction for embedded absolute paths and common token prefixes in JSON-RPC method names and persisted transport/settlement errors, explicit permission-denial responses in P079 repair mode when no safe `allow_once` option exists, multi-path ambiguity rejection in the posture check (`p079_posture_denied` denies tool calls that present more than one distinct structured target path), fail-closed advisory posture for unknown provider strings, and dirfd/openat materialization that rejects symlinked parent or final output components at depth >= 2 while allowing OS-managed first-level symlinks. Plan-evidence collection canonicalizes the source `.junie` directory, rejects sources that escape the workspace, rejects symlinks and hard-linked entries, writes through the dirfd `O_NOFOLLOW` materializer, and creates the P079-owned `plan_evidence` directory through `sec001_mkdirall_dirfd_unix`.

Per SEC-P079-HIGH-003, production same-session repair remains fail-closed for advisory-only providers until enforceable sandbox/permission restrictions exist; that fail-closed posture is part of the P079 acceptance boundary, not a remaining P079 blocker. `p079_advisory_posture_opt_in` is currently hard-wired to `false`. Junie provider-mode mismatch skips persist `blocked_provider_mode_mismatch` as `final_output_settlement` and `manual_investigation` as `recommended_next_action`. Advisory fail-closed skips record `final_output_settlement = NULL` rather than an unchecked synthetic enum value so SQLite CHECK constraints remain authoritative.

Out-of-scope future work: controlled provider fallback dispatch from frozen YAML policy, production same-session repair for providers whose permission posture is only advisory, projection artifact rebuild/recovery sweep, and independent plan-evidence purge on source-generation retirement or permanent archive. Current P079 keeps fallback schema, lease, parent-link, metric vocabulary, projection-health fields, and bounded plan-evidence readback as compatibility substrate but does not dispatch fallback child executions or own projection/retention lifecycle jobs. The implemented repair prompt, recovery attribution, and adapter idempotency/readback contracts are split into [p079-repair-prompt-template.md](p079-repair-prompt-template.md), [p079-recovery-attribution.md](p079-recovery-attribution.md), and [p079-adapter-idempotency.md](p079-adapter-idempotency.md).

### Problem

Chainworks can complete useful provider work and still block a run because the final declared output set is missing, empty, invalid, emitted through the wrong provider mode, or stranded in the current provider envelope. The recovery path after an output contract failure was not fully governed, leading to costly repetitions of work and loss of same-session context.

### Goals

- Attempt at most one same-session corrective output repair turn for eligible missing, empty, invalid, or mode-mismatched required outputs.
- Recover contract-valid output already present in the current invocation transcript or provider result envelope when it is transport-attributed to the current execution.
- Allow at most one controlled provider fallback attempt after repair or recovery is unavailable or unsuccessful (deferred — see implementation status above).

This section provides an in-depth reference for P079, detailing the schema, contracts, and operational aspects of the contract-aware output repair and provider fallback mechanism. This mechanism allows the system to attempt to correct agent output failures (e.g., missing, invalid, or malformed outputs) or invoke fallback strategies before blocking a run.

### Rollout Contract Summary

P079's rollout contract specifies its applicability and enforcement mechanisms. It includes:

-   **Applicability**: Required for specific contexts.
-   **Gate Aliases**: `proposal-079` and `p079`.
-   **Commands**: Allowlisted commands for gate checks, such as `./scripts/test-gate.sh proposal-079`.
-   **Migrations**: Requires a SQLite migration (`p079_output_contract_repair_v1`) to create tables for events, leases, and fallback parent links.
-   **Metrics**: Defines the contract inventory for adoption and operational monitoring. Core DB repair lifecycle events emit bounded P079 counters/gauges for repair attempts, terminal outcomes, transcript-recovery evidence, repair/fallback budget exhaustion, invalid repairs, provider-mode mismatch, and recovery-bound exceedance. Full rollout readback for future controlled provider-fallback dispatch remains deferred with that dispatch lane.

### Key Schema Purposes

Beyond the `output_contract_repair.v1` schema, P079 relies on:

-   **`fallback_context_packet_v1_schema`**: Defines the intended provider-fallback context packet, including sanitized, size-capped, content-addressed transfer. Controlled fallback dispatch remains deferred.
-   **`fallback_policy_schema`**: Specifies intended matching rules for a future fallback dispatcher. YAML `output_repair_policies` parsing and frozen fallback dispatch are not P079 closeout behavior.

### OutputContractRepair.v1 Schema Enums Detail

The `output_contract_repair.v1` schema defines the structured evidence captured during output repair and fallback attempts. Key enumerations within this schema are critical for understanding the state, outcomes, and failure classifications.

-   **`status`**: The top-level status of an output contract repair attempt.
    -   `not_attempted`: No repair or fallback was attempted.
    -   `in_progress`: A repair or fallback attempt is currently underway.
    -   `recovered`: The output contract was successfully repaired or recovered.
    -   `blocked`: The repair or fallback attempt was blocked.
    -   `skipped`: The repair or fallback attempt was skipped.
    -   `cancelled`: The repair or fallback attempt was cancelled.
    -   `failed`: The repair or fallback attempt failed.

-   **`initial_failure_class`**: Broad classification of the initial failure that triggered a repair or fallback attempt.
    -   `no_output_produced`: The agent produced no declared output.
    -   `empty_output`: The agent produced an empty declared output.
    -   `missing_required_outputs`: Some required outputs were missing.
    -   `invalid_required_outputs`: Some required outputs were invalid.
    -   `output_contract_mismatch`: The output produced did not match the expected contract.
    -   `provider_mode_mismatch`: The provider mode used was incorrect for the output.

-   **`initial_failure_subtype`**: More granular details about the initial failure. Can be null.
    -   `plan_event_instead_of_output`: A plan event was produced instead of a final output.
    -   `empty_submit_after_plan`: An empty submit was received after a plan.
    -   `file_plan_written_instead_of_payload`: A plan file was written instead of the final payload.
    -   `repair_repeated_plan_behavior`: Repair attempt repeated the plan behavior.
    -   `malformed_envelope`: The output envelope was malformed.
    -   `wrong_output_key`: The output was keyed incorrectly.
    -   `wrong_channel`: The output was sent on the wrong channel.
    -   `wrong_canonical_path`: The output was written to the wrong canonical path.
    -   `unknown_enum_value`: An unknown enumeration value was encountered.
    -   `missing_required_field`: A required field was missing.
    -   `unsafe_continuation`: An unsafe continuation was detected.
    -   `oversized_payload`: The payload exceeded the size limit.
    -   `unattributable_envelope`: The output envelope could not be attributed.
    -   `oversized_fallback_packet`: The fallback packet exceeded the size limit.
    -   `principal_revoked`: The principal associated with the attempt was revoked.
    -   `transcript_recovery_flag_missing`: The transcript recovery flag was missing.

-   **`same_session_repair_result`**: The outcome of a same-session repair attempt.
    -   `not_needed`: Same-session repair was not needed.
    -   `accepted`: Same-session repair was successful and accepted.
    -   `rejected_invalid`: Same-session repair produced invalid output and was rejected.
    -   `unavailable`: Same-session repair was unavailable.
    -   `skipped_ineligible`: Same-session repair was skipped as ineligible.
    -   `failed_transport`: Same-session repair failed due to transport error.
    -   `deadline_exceeded`: Same-session repair exceeded its deadline.
    -   `cancelled`: Same-session repair was cancelled.
    -   `budget_exhausted`: Same-session repair budget was exhausted.
    -   `superseded_ignored`: Same-session repair was superseded and ignored.

-   **`transcript_recovery_result`**: The outcome of a transcript recovery attempt.
    -   `not_needed`: Transcript recovery was not needed.
    -   `accepted`: Transcript recovery was successful and accepted.
    -   `rejected_invalid`: Transcript recovery produced invalid output and was rejected.
    -   `unavailable`: Transcript recovery was unavailable.
    -   `skipped_ineligible`: Transcript recovery was skipped as ineligible.
    -   `failed_transport`: Transcript recovery failed due to transport error.
    -   `cancelled`: Transcript recovery was cancelled.

-   **`provider_fallback_result`**: The outcome of a provider fallback attempt.
    -   `not_needed`: Provider fallback was not needed.
    -   `scheduled`: Provider fallback was scheduled.
    -   `accepted`: Provider fallback was successful and accepted.
    -   `rejected_invalid`: Provider fallback produced invalid output and was rejected.
    -   `unavailable`: Provider fallback was unavailable.
    -   `skipped_ineligible`: Provider fallback was skipped as ineligible.
    -   `failed_transport`: Provider fallback failed due to transport error.
    -   `deadline_exceeded`: Provider fallback exceeded its deadline.
    -   `cancelled`: Provider fallback was cancelled.
    -   `budget_exhausted`: Provider fallback budget was exhausted.
    -   `lease_contended`: Provider fallback lease was contended.
    -   `superseded_ignored`: Provider fallback was superseded and ignored.

-   **`recovery_source`**: Indicates where the recovered output originated. Can be null.
    -   `transcript`: Recovered from the session transcript.
    -   `provider_envelope`: Recovered from the provider's result envelope.

-   **`final_output_settlement`**: Describes how the final output was settled.
    -   `valid_outputs_from_completed_execution`: Valid outputs from a completed execution.
    -   `valid_outputs_from_repair`: Valid outputs obtained through repair.
    -   `valid_outputs_from_transcript_recovery`: Valid outputs obtained through transcript recovery.
    -   `valid_outputs_from_provider_envelope`: Valid outputs obtained from the provider envelope.
    -   `valid_outputs_from_fallback`: Valid outputs obtained through fallback.
    -   `blocked_missing_required_outputs`: Blocked due to missing required outputs.
    -   `blocked_invalid_required_outputs`: Blocked due to invalid required outputs.
    -   `blocked_provider_mode_mismatch`: Blocked due to provider mode mismatch.
    -   `ignored_late_outputs`: Late outputs were ignored.
    -   `cancelled`: Output settlement was cancelled.
    -   `failed_transport`: Output settlement failed due to transport error.
    -   `deadline_exceeded`: Output settlement exceeded its deadline.

-   **`recommended_next_action`**: The recommended action for the operator.
    -   `continue`: The run can continue automatically.
    -   `inspect_repair_evidence`: Operator should inspect repair evidence.
    -   `configure_fallback_policy`: Operator should configure fallback policy.
    -   `operator_resolve_approval`: Operator needs to resolve an approval.
    -   `operator_resolve_workflow_conflict`: Operator needs to resolve a workflow conflict.
    -   `retry_after_transport_restored`: Retry after transport is restored.
    -   `cancel_acknowledged`: Cancellation was acknowledged.
    -   `manual_investigation`: Manual investigation is required.

-   **`presentation_category`**: Categorization for UI presentation.
    -   `informational`: For informational display.
    -   `recovered`: The output was recovered.
    -   `blocked`: The run was blocked.
    -   `skipped`: The attempt was skipped.
    -   `failed`: The attempt failed.
    -   `cancelled`: The attempt was cancelled.

### P079 Hold Conditions Detail

The following conditions are strictly enforced to prevent unintended behavior and maintain system integrity under P079. Any of these conditions being met will result in the system holding or blocking the operation.

-   **Contract Validator Bypass**: All repaired, recovered, and fallback payloads must traverse declared-output validation and source-generation settlement.
-   **Canonical Path Bypass**: Returned output paths must byte-match the frozen snapshot resolved path string; equivalent-looking macOS paths are rejected. Materialization uses `openat` with `O_NOFOLLOW` per component.
-   **Single-Flight Bypass**: Fallback uses a transactional unique lease keyed by run, stage execution, parent agent execution, schema version, and frozen fallback policy hash before child execution creation.
-   **Durable Ordering Bypass**: Lease commit precedes ACP dispatch; recovery treats `prompt_sent` as do-not-redispatch.
-   **Lease Liveness Bypass**: Leases carry TTL and a reconciliation sweep; stale leases are reclaimed deterministically.
-   **Restart Reprompt**: After `prompt_sent`, recovery must not issue a second same-session repair prompt for the same parent execution.
-   **Side Effect Lane Exclusion**: Release and external side-effect lanes remain owned by the durable side-effect ledger.
-   **Fallback Packet Sanitization**: Fallback context packet is a closed v1 schema with redaction tier, size cap, content-addressed hash, and principal binding.
-   **Recovery Bounds**: Recovery uses a streaming fail-closed decoder with byte/depth/chunk caps and transport-allocated attribution.
-   **Plan Evidence Protection**: Plan evidence is copied into a P079-owned `0700/0600` directory, redacted, size-capped, retained under the existing run-meta lifecycle, and exposed meta-root-relative only. P079 does not implement independent purge on source-generation retirement or permanent archive.
-   **Repair Turn Posture**: Repair and fallback turns run under a server-side permission posture allowlisting only `fs.write` to frozen canonical output paths.
-   **Principal Binding**: Fallback inherits the failed execution's principal; revocation aborts fallback.
-   **Auto-Retry Observe Only**: The auto-retry ledger may classify P079 terminal states but remains observe-only, debounced per (`parent_agent_execution_id`, `terminal_class`).
-   **Swift Client Decode and Inspector Surface**: The macOS app's DTO module decodes the v1 readback surface, handles GraphQL enum casing plus unknown future values, and exposes the selected run's highest-severity P079 evidence in the read-only inspector.

### P079 Rollback Disposition

In the event of a rollback or disablement of P079-adjacent optional capabilities, the following disposition and steps are applied to ensure data integrity and system stability. Core output repair remains enabled as settlement behavior.

- **Mode**: `optional_feature_disable_keep_evidence_readback`
- **Data Loss Risk**: `none`
- **Rollback Steps**:
    1.  Disable optional transcript-recovery and provider-fallback rollout flags, if they are enabled.
    2.  Keep `output_contract_repair.v1` SQLite rows and rebuilt evidence artifacts readable through run report, MCP, and GraphQL.
    3.  Stop scheduling new optional fallback leases while leaving existing source-generation, repair, and artifact-settlement history untouched.
    4.  Allow the recovery sweep to continue reclaiming stale leases so optional-capability-disabled runs do not leave hanging rows.
    5.  Keep eligible output failures on the always-on repair path; unrecoverable failures still block with retained evidence.
    6.  Re-enable optional capabilities only after the retained `proposal-079`/`p079` gate aliases pass.

### P079 GraphQL and MCP Readback

The `OutputContractRepairEvidence` is exposed via GraphQL and MCP for readback and observability.

-   **GraphQL**:
    -   The `outputContractRepair` field is available on `AgentExecution` types.
    -   It is nullable for pre-P079 runs and executions without an output-contract failure.
    -   Nested objects (e.g., `sameSessionRepair`, `transcriptRecovery`, `providerFallback`, `providerPlanEvidence`, `budget`, `lease`) are non-null when the parent `OutputContractRepairEvidence` is non-null, as they are always materialized with default values.
    -   Optional scalar fields use `null` rather than empty sentinel strings.
    -   SwiftUI row identity is `(repair_attempt_id, agent_execution_id)`; `evidence_version` is a content-version/refresh-invalidation field.
    -   The canonical GraphQL contract is the `OutputContractRepairEvidence` field and closed enum mapping in `control-plane/crates/graphql-server/src/types/stage.rs`, with compatibility pinned by the retained `proposal-079`/`p079` gate aliases and Swift readback tests.

-   **MCP and Run Report JSON**:
    -   The `output_contract_repair` object matches the `output_contract_repair.v1` schema in snake_case.
    -   For older runs, `output_contract_repair` is null, and `output_contract_repair_status` is `not_attempted` in flattened operator report views.
    -   MCP and run report JSON use the `output_contract_repair.v1` readback object described in this section, the report serializer, and the operator-readback fixture under `docs/evidence/rollout-contract/operator-readback/`.

### P079 SQLite Migration Appendix

P079 introduces a new SQLite migration (`p079_output_contract_repair_v1`) to persist the state and evidence related to output contract repair and fallback.

-   **Tables Created**:
    -   `output_contract_repair_events`: Stores authoritative per-(repair_attempt, parent agent execution) evidence rows.
    -   `output_contract_repair_leases`: Manages single-flight scheduling authority for repair/fallback dispatch.
    -   `output_contract_repair_fallback_parent_links`: Provides explicit forward and reverse linkage for fallback agent executions.
-   **Schema Versioning**: Each JSON column embeds its own schema_version for forward compatibility.
-   **Rollback Compatibility**: Existing rows remain readable after rollback, and no data loss occurs.

### P079 Retained Gate Aliases

`proposal-079` and `p079` remain stable gate aliases for the implemented safe repair/recovery scope. They are historical names only; operational truth for the feature lives in this reference section, the P079 repair prompt/recovery/idempotency reference docs, and the operator runbook.
