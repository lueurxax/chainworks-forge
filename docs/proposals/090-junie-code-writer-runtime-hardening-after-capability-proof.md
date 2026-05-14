# Proposal 090: Junie Code Writer Runtime Hardening After Capability Proof

| Field | Value |
|---|---|
| Date | 2026-05-13 |
| Status | Draft |
| Author | Codex |
| Depends on | P037 ACP supervision and idle-hang watchdog, P079 contract-aware output repair and provider fallback, P088 code-writer completion contract and output freshness, P089 closed-out reference/evidence truth |
| Related | P036 UX consolidation, `docs/reference/output-contracts-failure-evidence-and-recovery.md`, `docs/reference/acp-runtime-transport.md`, `docs/reference/rust-control-plane.md`, `docs/reference/structured-output-envelope-and-contract-validation.md`, `docs/evidence/089/junie-structured-output-canary/` |
| Scope | Close the Junie-specific runtime and completion-boundary defects still visible after P089 proved structured-output capability. |
| Non-goal | No re-litigation of whether Junie can emit strict structured output at all, no provider swap decision, no feature-scope fixes for proposal content, and no silent auto-heal of historical blocked runs. |

## 1. Problem

P089 answered the capability question:

1. Junie can return strict JSON.
2. Junie can return strict `CHAINWORKS_OUTPUT`.
3. Junie can do so over the real ACP `code_writer` path.

That closed the wrong question for current blocked runs.
The remaining failures on `P036` are no longer "Junie only writes prose". They are runtime and completion-boundary failures that happen after the session has already started useful work.

The expensive failure mode now looks like this:

1. Junie starts normally over ACP.
2. Junie performs useful repository work or at least meaningful session progress.
3. The session fails to produce a durable final completion payload the engine can settle.
4. Repair either receives a truncated source, returns prose, or fails to materialize fresh outputs.
5. The run blocks even though the original capability question is already solved.

This proposal exists to close that post-P089 gap.

## 2. Proven Baseline From P089

P090 must treat the following as proven and out of scope for re-debate:

- Junie is capable of emitting strict JSON on demand.
- Junie is capable of emitting `CHAINWORKS_OUTPUT` on demand.
- The ACP route itself is capable of carrying a valid Junie structured result.

P089 is no longer a live proposal dependency. P090 must reference P089 through:

- `docs/reference/structured-output-envelope-and-contract-validation.md`
- `docs/reference/acp-runtime-transport.md`
- `docs/evidence/089/junie-structured-output-canary/evidence-index.json`
- `docs/evidence/089/junie-structured-output-canary/live-gate-run.json`

Therefore, P090 must not frame the problem as:

- "Junie cannot do structured output"
- "repair prompts are fundamentally unsupported by Junie"
- "the only path is replacing Junie"

Instead, P090 addresses reliability of long-lived `code_writer` executions and the final completion boundary after substantive work or long narrative output.

## 3. Observed P036 Failure Shapes

The investigation around `P036 / 8456e930-8a67-42f1-a5fe-83e34597857b` exposed multiple distinct failure shapes. They should be treated as one family with several subtypes, not as one generic missing-output bucket.

These shapes are design inputs, not automatically accepted proof. Implementation of P090 must either extract durable evidence from historical P036/P088 receipts or recreate deterministic fixtures under `docs/evidence/090/junie-runtime-hardening/`. Each subtype below must have one evidence entry with:

- `source_run_id` and `source_agent_execution_id` when historical;
- redacted raw/captured payload artifact path when available;
- receipt path and receipt SHA-256;
- subtype expected by P090;
- reason historical evidence is trustworthy, or a note that the shape is reproduced by a synthetic fixture.

### 3.1 Runtime/tool-path failure before publication

Earlier attempts showed:

- MCP/Xcode-readable files were available;
- shell reads like `sed` or `cat` on `Chainworks_ForgeApp.swift` returned `Operation not permitted`;
- Junie then exited early without publishing required outputs.

That proved the session could die inside the working loop before final handoff.

### 3.2 Completion text captured but unusable for settlement

Another attempt completed with a large prose response:

- the main completion capture exceeded the extraction boundary;
- `completion_text_truncated = true`;
- `ingestion_boundary_failure = extraction_input_truncated`;
- required outputs were not settled from the main completion;
- a repair turn ran but did not materialize fresh outputs.

That proved the problem can happen even when the provider "completes".

### 3.3 Completion succeeded but repair still failed to materialize outputs

In the same family of attempts:

- `changed_files_manifest` could settle;
- `implementation_progress`, `implementation_self_assessment`, and `tests_result` remained stale or missing;
- repair could produce output-like material but still fail final settlement.

That proved the defect is not only "provider said nothing", but also "engine did not convert repair material into fresh settled outputs".

### 3.4 Handoff silence after useful work

One later `P036` attempt produced:

- `current_attempt_changed_path_count > 0`;
- explicit evidence that Junie made worktree changes;
- then `ACP_HANDOFF_IDLE_AFTER_DIFF` / `acp_final_text_not_collected`;
- no final ACP terminal payload.

That proved useful work can occur even when the final handoff never completes.

### 3.5 Session completed without durable final text

Another attempt ended with:

- `status = completed`;
- but `completion_text_status = unavailable`;
- `text_absence_reason = provider_did_not_emit_text`;
- `transcript_absence_reason = provider_did_not_supply`.

That proved we also have a "provider session completed but no durable final text was captured" subtype.

## 4. Root Cause Model

P036 does not expose one Junie bug. It exposes a boundary with five coupled gaps:

### 4.1 Capability and reliability are different contracts

P089 proved one-shot structured-output capability.
P036 shows that capability does not guarantee reliable final output publication after a long-running `code_writer` pass.

### 4.2 Final response is too permissive

The runtime still allows a long narrative completion to occupy the only authoritative completion channel.
When that narrative is too large or never turns into a final structured payload, settlement fails late and expensively.

### 4.3 Completion extraction and forensic transcript are over-coupled

The same captured text is trying to serve as:

- final output extraction input
- forensic completion record

This coupling makes `extraction_input_truncated` both a diagnostics problem and a settlement problem.

### 4.4 Repair is not narrow and fail-closed enough

Repair can still end in:

- prose instead of strict JSON
- partial output materialization
- rejection of one output causing effective loss of the whole turn

### 4.5 Runtime classification is still too coarse

Different real-world failures collapse into broad labels like:

- `missing_required_outputs`
- `no_output_produced`

That makes retries and operator decisions less precise than they should be.

## 5. Goals

- Preserve P089's conclusion that Junie is viable as a `code_writer`.
- Make long-running Junie `code_writer` completions deterministic at the final handoff boundary.
- Prevent large prose completions from consuming the only authoritative output channel.
- Preserve forensic transcript/debug value without weakening output settlement truth.
- Make repair output settlement per-output and current-attempt-scoped.
- Distinguish provider progress, final handoff silence, truncation, malformed repair, and stale outputs as separate failure classes.
- Provide durable receipts that let an operator understand the exact reason a Junie attempt blocked.

## 6. Non-Goals

- Do not re-prove the P089 capability result.
- Do not weaken `implementation_progress`, `implementation_self_assessment_v2`, or `tests_result` contracts.
- Do not accept stale previous-attempt files as fresh truth.
- Do not solve proposal-scope blockers in `P036`; this proposal is only about orchestration/runtime hardening.
- Do not special-case Junie as a hidden bypass around the normal contract pipeline.

## 7. Alternatives Considered

### Option A: Treat P036 as evidence that Junie is not a viable code writer

Rejected because P089 already disproved the capability objection.
The observed defects are reliability and handoff defects, not incapability.

### Option B: Increase timeouts and keep the rest of the boundary unchanged

Rejected because:

- some failures happen after meaningful progress but before final handoff;
- some failures are truncation, not slowness;
- some failures are malformed or non-materialized repair outputs.

Longer timeouts would hide, not close, the boundary defect.

### Option C: Force all completion text through transcript storage and parse from there

Rejected because this still couples forensic capture and settlement input, and it does not make final structured output mandatory.

### Option D: Narrow the final completion contract and harden repair/readback around it

Recommended.
This keeps Junie as a supported provider while fixing the production reliability gap exposed by P036.

## 8. Decision

Adopt a Junie runtime-hardening layer with four pillars:

1. a strict final completion envelope for `code_writer`;
2. decoupled final-output extraction and forensic transcript capture;
3. per-output repair settlement with tighter failure classes;
4. Junie-specific runtime and handoff diagnostics that feed durable operator truth.

P090 is not a provider-capability proposal.
It is a runtime-boundary hardening proposal built on the assumption that P089 capability proof stands.

## 9. Proposed Design

### 9.1 Strict final completion envelope for `code_writer`

For `code_writer` executions with required outputs, the authoritative completion channel must be one of:

- a valid `CHAINWORKS_OUTPUT` final payload;
- a typed engine failure envelope;
- a typed repair failure envelope.

Free-form final prose must no longer be considered an acceptable terminal completion shape.

If the provider returns narrative instead of a structured completion payload:

- settlement must classify it explicitly;
- the run must not treat it as an ordinary successful completion;
- repair must be invoked under a narrower contract.

#### 9.1.1 Envelope authority and trust boundary

P090 adds two versioned JSON envelopes, but persisted failure truth is engine-owned.

Provider-authored final text is untrusted input. A provider may emit `CHAINWORKS_OUTPUT`, prose, or a JSON object that resembles a failure envelope, but the engine must not treat provider-authored failure claims as authoritative. The engine may use provider text only as extraction input. The persisted `code_writer_engine_failure_v1` receipt section must be synthesized from engine-owned facts:

- ACP transport capture metadata;
- AgentExecution row and session lineage;
- runtime preflight results;
- worktree/progress facts collected by the engine;
- settlement and repair validation decisions.

The repair failure envelope is also engine-synthesized from repair-turn parsing and validation. A repair response that contains a `code_writer_repair_failure_v1`-shaped object is a provider claim until the engine validates identifiers, source generation, repair attempt, and output decisions.

The proposal therefore uses "envelope" to mean a persisted receipt/readback shape, not a provider authority boundary. Provider-authored envelope-shaped JSON can contribute to diagnostics only after engine validation.

Required spoof/mismatch handling:

- mismatched `run_id`, `stage_execution_id`, `agent_execution_id`, or `session_generation_id` fails closed as `provider_envelope_identity_mismatch`;
- provider-authored `code_writer_engine_failure_v1` cannot override engine runtime facts;
- unknown envelope schemas fail closed as `provider_envelope_unrecognized`;
- no failure envelope may materialize outputs;
- GraphQL/MCP/report readback must say whether an envelope was `engine_synthesized` or `provider_claim_rejected`.

P090 adds two versioned persisted JSON receipt sections. They are generated only by engine-owned extraction/settlement code and are not accepted from arbitrary files in the worktree.

`code_writer_engine_failure_v1`:

```json
{
  "schema_version": "code_writer_engine_failure.v1",
  "provider": "junie",
  "stage_id": "state_10_implementation_refined",
  "stage_execution_id": "<uuid>",
  "agent_execution_id": "<uuid>",
  "session_generation_id": "<uuid>",
  "completion_boundary_subtype": "junie_progress_without_terminal_handoff",
  "final_payload_status": "missing",
  "transcript_capture_status": "present",
  "progress_before_handoff": "worktree_diff_detected",
  "retry_recommendation": "retry_stage",
  "public_message": "Junie made progress but did not emit a terminal structured handoff."
}
```

`code_writer_repair_failure_v1`:

```json
{
  "schema_version": "code_writer_repair_failure.v1",
  "provider": "junie",
  "stage_id": "state_10_implementation_refined",
  "stage_execution_id": "<uuid>",
  "agent_execution_id": "<uuid>",
  "session_generation_id": "<uuid>",
  "repair_attempt": 1,
  "completion_boundary_subtype": "junie_repair_returned_malformed_json",
  "repair_payload_status": "malformed_json",
  "output_decisions": [
    {
      "output_name": "implementation_self_assessment",
      "contract_id": "implementation_self_assessment_v2",
      "decision": "rejected",
      "reason": "malformed_json",
      "canonical_path": "implementation/self-assessment.json"
    }
  ],
  "public_message": "Repair returned malformed JSON for one or more required outputs."
}
```

Parser precedence:

1. valid `CHAINWORKS_OUTPUT`;
2. engine-synthesized `code_writer_engine_failure_v1`;
3. engine-synthesized `code_writer_repair_failure_v1`;
4. narrative/prose classification.

Unknown envelope schemas fail closed as `provider_envelope_unrecognized` and must not materialize outputs.

### 9.2 Separate output extraction input from full forensic transcript

The runtime must keep two distinct artifacts:

1. `final_completion_payload_capture`
   - bounded
   - optimized for output extraction
   - exact source for settlement

2. `completion_transcript_capture`
   - full or best-effort
   - optimized for forensics/postmortem
   - not used as the main extraction input

This prevents a large narrative stream from making output settlement fail only because the forensic capture is large.

This extends the P088/P089 completion capture model rather than creating a second unrelated subsystem. Ownership remains:

- ACP transport owns final text source detection and capture metadata.
- Engine owns extraction, receipt construction, output decisions, and materialization.
- DB/readback owns durable public enum exposure.

`final_completion_payload_capture` must reuse the existing capture fields where possible: source, byte count, raw byte limit, truncation flag, redacted artifact path, extraction SHA-256, and absence reason. New fields are allowed only when they cannot be represented by P088 capture metadata.

Source precedence:

1. ACP terminal final text;
2. ACP session update stream selected as final extraction input;
3. repair-turn final text;
4. no source, with typed absence reason.

Retention:

- failed or blocked attempts retain redacted final payload and transcript artifacts;
- successful attempts retain the final payload digest and redacted payload when under the existing P088 capture policy;
- transcript retention remains best-effort and must not affect settlement.

### 9.3 Public completion-boundary subtype contract

The public field is provider-neutral:

- receipt: `code_writer_completion_receipt_v1.completion_boundary_subtype`;
- GraphQL: `implementationCompletion.completionBoundarySubtype`;
- MCP: `runs.get.implementationCompletion.completion_boundary_subtype`;
- run report: `implementation_completion.completion_boundary_subtype`.

The enum wrapper is provider-neutral and follows the existing known/raw pattern:

```json
{
  "known": true,
  "raw": "junie_progress_without_terminal_handoff",
  "value": "junie_progress_without_terminal_handoff",
  "provider_runtime_family": "junie_acp"
}
```

Known values may be provider-specific when the subtype is genuinely provider-specific. For P090, the first known values are Junie-prefixed because they describe Junie ACP runtime behavior. The field name and wrapper must not be Junie-specific, so later providers can add their own known subtype values without another public field migration.

Introduce at least these known subtype values for Junie `code_writer`:

- `junie_final_response_missing`
- `junie_final_response_truncated`
- `junie_progress_without_terminal_handoff`
- `junie_repair_returned_narrative`
- `junie_repair_returned_malformed_json`
- `junie_repair_outputs_partially_materialized`
- `junie_runtime_tool_path_failure_before_publication`

The generic top-level status may still roll up into `missing_required_outputs`, but the subtype must survive into receipts and readback.

Public enum contract:

| Subtype | `completion_status` rollup | `final_payload_status` | `next_operator_action` |
|---|---|---|---|
| `junie_final_response_missing` | `missing_required_outputs` | `missing` | `retry_stage` |
| `junie_final_response_truncated` | `missing_required_outputs` | `truncated` | `run_narrow_repair_or_retry` |
| `junie_progress_without_terminal_handoff` | `missing_required_outputs` | `missing` | `retry_stage` |
| `junie_repair_returned_narrative` | `missing_required_outputs` | `narrative_only` | `retry_stage_or_provider_fallback` |
| `junie_repair_returned_malformed_json` | `missing_required_outputs` | `present` | `retry_stage_or_provider_fallback` |
| `junie_repair_outputs_partially_materialized` | `missing_required_outputs` | `present` | `retry_missing_outputs_only` |
| `junie_runtime_tool_path_failure_before_publication` | `no_output_produced` | `missing` | `fix_runtime_environment_then_retry` |

Readback mapping:

- `code_writer_completion_receipt_v1.completion_boundary_subtype`
- GraphQL run `implementationCompletion.completionBoundarySubtype`
- MCP `runs.get.implementationCompletion.completion_boundary_subtype`
- run report `implementation_completion.completion_boundary_subtype`

Unknown subtypes must be represented as `{ "known": false, "raw": "<value>" }` in existing enum-wrapper style, not dropped.

This resolves the schema-scope decision: P090 does not add a Junie-only public API. It adds a provider-neutral subtype wrapper whose initial known values are Junie-specific.

### 9.4 Repair must be per-output, not effectively all-or-nothing

When a repair turn returns several outputs:

- each output is validated independently;
- valid outputs may settle even if sibling outputs are malformed or stale;
- invalid outputs remain explicitly rejected with typed reasons;
- final status reflects the exact mix:
  - fresh settled
  - stale
  - malformed
  - absent

This closes the class where one broken repair output prevents otherwise valid fresh outputs from materializing.

#### 9.4.1 Validate-before-materialize rules

Repair materialization must be staged. The engine must not write a repair-provided artifact to its canonical path until that specific output has passed validation.

P090 adds a durable per-output settlement model. The exact storage can be a DB table or the existing receipt table plus a normalized child table, but it must expose the following row shape and transaction semantics.

Minimum durable row shape:

```sql
CREATE TABLE code_writer_output_settlement_rows (
  id TEXT PRIMARY KEY,
  receipt_id TEXT NOT NULL REFERENCES code_writer_completion_receipts(id),
  run_id TEXT NOT NULL,
  stage_id TEXT NOT NULL,
  stage_execution_id TEXT NOT NULL,
  agent_execution_id TEXT NOT NULL,
  session_generation_id TEXT NOT NULL,
  repair_attempt INTEGER NOT NULL DEFAULT 0,
  output_name TEXT NOT NULL,
  contract_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  source_generation_owner TEXT NOT NULL,
  candidate_digest TEXT,
  staging_path TEXT,
  canonical_path TEXT NOT NULL,
  canonical_before_sha256 TEXT,
  canonical_after_sha256 TEXT,
  decision TEXT NOT NULL,
  rejection_reason TEXT,
  materialization_state TEXT NOT NULL,
  active_pointer_generation_id TEXT,
  created_at TEXT NOT NULL,
  committed_at TEXT,
  UNIQUE(receipt_id, output_name),
  UNIQUE(agent_execution_id, repair_attempt, output_name, candidate_digest)
);
```

Required values:

- `decision`: `accepted`, `rejected`, `absent`, `stale`;
- `materialization_state`: `staged`, `committed`, `failed`, `not_materialized`;
- `source_kind`: `repair_chainworks_output`, `completion_chainworks_output`, `control_plane_generated`;
- `source_generation_owner`: `agent`, `control_plane`.

The active artifact index and receipt `output_decisions` must be derived from accepted settlement rows, not from raw staged files or provider text.

Receipt linkage is mandatory. Each settlement row belongs to exactly one `code_writer_completion_receipts.id`, and receipt readback must join through `receipt_id`. The existing P088 `(receipt_id, output_name)` output-decision authority remains the public readback key; P090 refines it by making per-output materialization rows the durable source for accepted/rejected/stale/absent repair outcomes.

Idempotency constraints:

- `(receipt_id, output_name)` must be unique, matching the P088 receipt output-decision contract;
- `(agent_execution_id, repair_attempt, output_name, candidate_digest)` must be unique for non-null candidate digests so replaying the same repair payload is idempotent;
- a replay with the same candidate digest must return the existing row and must not rewrite canonical files;
- a replay with a different digest for the same `agent_execution_id`, `repair_attempt`, and `output_name` must fail closed unless a new repair attempt number is allocated.

Required flow:

1. parse repair envelope into per-output candidates;
2. write candidates only to an attempt-scoped staging directory under `.chainworks/runs/<run_id>/repair-staging/<agent_execution_id>/<repair_attempt>/`;
3. validate each candidate against its contract and current-attempt lineage;
4. start one settlement transaction for the repair attempt;
5. insert one durable settlement row per declared output before canonical mutation;
6. for each valid candidate, atomically replace or create the canonical output path and update that row to `decision = accepted`, `materialization_state = committed`;
7. for each invalid candidate, leave canonical truth unchanged and record `decision = rejected|stale|absent`, `materialization_state = not_materialized`;
8. publish active artifact pointers only for accepted rows in the same transaction boundary as the row commit;
9. compute stage success from the full required-output set after per-output settlement.

Rollback/idempotency:

- retrying the same repair attempt with the same payload digest must be idempotent;
- a malformed sibling must never overwrite a previously valid canonical output;
- if materialization fails after one output is accepted, the receipt must show the accepted output and the failed output separately rather than rolling the whole repair turn into an ambiguous all-or-nothing failure;
- active artifact index updates must be derived from accepted settlement rows, not from raw staged files.

Crash recovery:

- `staged` rows without canonical mutation are safe to discard or retry from the same candidate digest;
- `committed` rows are idempotently re-read from `canonical_after_sha256`;
- `failed` rows must preserve `canonical_before_sha256` and the failure reason;
- recovery must never promote a staged file that lacks an accepted settlement row.

Acceptance fixture:

- canonical `implementation_self_assessment` starts with valid old content;
- repair returns valid `implementation_progress`, valid `tests_result`, malformed `implementation_self_assessment`;
- after settlement, progress/tests are current-attempt accepted, self-assessment canonical file remains unchanged, and readback reports `junie_repair_outputs_partially_materialized`.

### 9.5 Repair payload should prefer output-name keys

Following the direction already proven useful in recent fixes, repair prompts for `code_writer` must prefer:

- `implementation_progress`
- `implementation_self_assessment`
- `changed_files_manifest`
- `tests_result`

as primary keys, with canonical paths accepted only as fallback.

This reduces brittle failures caused by absolute-path typos or formatting drift in long payloads.

### 9.6 Final response size budget and overflow handling

The runtime must stop silently depending on multi-hundred-kilobyte or megabyte final prose.

Rules:

- a bounded maximum extraction payload size remains allowed;
- if the final completion overflows that budget before a valid output envelope is found, classify it explicitly as `junie_final_response_truncated`;
- do not attempt to infer success from truncated prose;
- transition immediately into the narrow repair path or typed failure path.

### 9.7 Provider-progress-without-handoff classification

If Junie:

- produces meaningful session progress,
- possibly changes the worktree,
- but never emits a final ACP response,

the receipt must classify this as:

- progress happened
- final handoff did not

This is distinct from empty execution or startup failure.

### 9.8 Transcript absence should not erase completion truth

If transcript persistence fails but final completion capture exists:

- settlement must continue;
- the receipt must record transcript persistence failure separately;
- the operator must still see the final completion subtype and any recoverable payload facts.

Likewise, if final text is absent but runtime progress exists, the receipt must say so explicitly.

### 9.9 Junie runtime tool-path preflight and remediation

`junie_runtime_tool_path_failure_before_publication` is not only a classifier. P090 must add a preflight/remediation path before long-running Junie `code_writer` work starts.

Lifecycle owner:

- the engine owns the preflight decision and persisted runtime facts;
- the Junie ACP adapter owns launch config construction and remediation mechanics;
- the work queue owns claim/ack semantics;
- provider capacity accounting starts only after preflight passes.

Persisted state machine:

1. work item is claimed;
2. AgentExecution row is created or updated with existing `status = running`;
3. preflight attempt is written with `attempt_number`, redacted path classes, operation classes, and result;
4. a separate persisted `runtime_preflight_phase` / runtime-facts field records `preflight_running`;
5. if preflight passes, `runtime_preflight_phase = passed` and provider capacity is acquired;
6. if preflight fails with remediable `wrong_cwd` or `runtime_home_unwritable`, the adapter applies one remediation and writes `runtime_preflight_phase = preflight_remediating`;
7. if the second preflight passes, `runtime_preflight_phase = passed` and provider capacity is acquired;
8. if preflight fails with permission denied, TCC/sandbox denial, or failed remediation, AgentExecution transitions to existing terminal `status = failed` with subtype `junie_runtime_tool_path_failure_before_publication`;
9. work item is acked only after the terminal preflight failure is persisted or after provider launch is handed off to the normal ACP execution path.

P090 does not add new `AgentStatus` enum values. `preflight_running`, `preflight_remediating`, `passed`, and `failed_no_launch` are preflight phases stored in runtime facts/readback, not top-level AgentExecution statuses. This avoids breaking existing `running/completed/failed/cancelled` parsing.

Retry counters:

- preflight remediation attempts increment `preflight_attempt_count`;
- they do not increment provider completion repair counts;
- they do not count as a provider launch attempt until provider capacity is acquired and Junie is started;
- terminal preflight failure counts as the stage attempt's runtime failure and must not loop through generic provider retry.

Receipt fields must include `preflight_attempt_count`, `preflight_remediation_applied`, `provider_launched`, and redacted failure facts. Raw absolute paths must not be exposed in public readback.

Preflight scope:

- run inside the same worktree root and sandbox/home configuration that Junie will receive;
- verify read access to the resolved project root;
- verify read access to at least one proof fixture file and one changed-file manifest target parent;
- verify write access to `.chainworks/runs/<run_id>/implementation/` and the ACP runtime temp/home directories;
- verify that shell reads used by Junie-compatible tooling do not fail with `Operation not permitted`.

Failure behavior:

- fail before provider launch when preflight fails;
- record subtype `junie_runtime_tool_path_failure_before_publication`;
- include redacted path class, operation class (`read_project_file`, `write_output_dir`, `read_runtime_home`, etc.), errno/category, and remediation hint;
- do not start a provider session that is known not to be able to read/write required paths.

Remediation rules:

- if failure is caused by runtime cwd mismatch, rebuild the Junie launch config with the canonical worktree root and retry once;
- if failure is caused by unwritable ACP runtime home/cache, rebuild the runtime home and retry once;
- if macOS/TCC/sandbox denies project file reads, fail closed and ask for operator environment repair rather than looping;
- all automatic retries must be reflected in the receipt with `preflight_attempt_count` and `preflight_remediation_applied`.

Acceptance fixture:

- a synthetic Junie launch config points at an unreadable or wrong project path;
- preflight fails before provider launch;
- receipt/readback expose `junie_runtime_tool_path_failure_before_publication`;
- no required output is marked fresh;
- remediation succeeds only for the cwd/runtime-home fixture, not for permission-denied fixture.

## 10. Data Model and Contract Changes

### 10.1 Extend `code_writer_completion_receipt_v1`

Add Junie-focused fields or subfields:

- `provider_runtime_family`
- `completion_boundary_subtype`
- `final_payload_status`
  - `present`
  - `missing`
  - `truncated`
  - `narrative_only`
- `transcript_capture_status`
  - `present`
  - `missing`
  - `persist_failed`
- `progress_before_handoff`
  - `none`
  - `session_updates_only`
  - `meaningful_progress`
  - `worktree_diff_detected`
- `repair_materialization_summary`
  - counts for fresh / stale / malformed / absent outputs
- `final_completion_payload_capture`
- `runtime_tool_path_preflight`

Required schema fragment:

```json
{
  "provider_runtime_family": "junie_acp",
  "completion_boundary_subtype": "junie_repair_outputs_partially_materialized",
  "final_payload_status": "present",
  "transcript_capture_status": "persist_failed",
  "progress_before_handoff": "worktree_diff_detected",
  "final_completion_payload_capture": {
    "capture_source": "acp_terminal_final_text",
    "status": "captured",
    "redacted_text_artifact_path": "evidence/p090/<agent_execution_id>/final-payload-redacted.txt",
    "raw_byte_limit": 1048576,
    "captured_byte_count": 1204,
    "truncated": false,
    "extraction_input_sha256": "sha256:<hex>",
    "absence_reason": null
  },
  "runtime_tool_path_preflight": {
    "status": "passed",
    "attempt_count": 1,
    "remediation_applied": null,
    "failed_operation_class": null,
    "redacted_path_class": null,
    "failure_category": null
  },
  "repair_materialization_summary": {
    "fresh_count": 2,
    "stale_count": 0,
    "malformed_count": 1,
    "absent_count": 1,
    "staging_root": ".chainworks/runs/<run_id>/repair-staging/<agent_execution_id>/1",
    "output_decisions": [
      {
        "output_name": "implementation_progress",
        "contract_id": "implementation_progress",
        "decision": "accepted",
        "source_kind": "repair_chainworks_output",
        "canonical_path": "implementation/progress.md",
        "content_sha256": "sha256:<hex>",
        "materialized_at": "2026-05-13T00:00:00Z"
      },
      {
        "output_name": "implementation_self_assessment",
        "contract_id": "implementation_self_assessment_v2",
        "decision": "rejected",
        "source_kind": "repair_chainworks_output",
        "canonical_path": "implementation/self-assessment.json",
        "rejection_reason": "malformed_json",
        "canonical_path_unchanged": true
      }
    ]
  }
}
```

The schema is additive. Existing P088 fields remain authoritative for current success/failure rollups. P090 fields refine diagnosis and repair materialization truth; they must not contradict `output_decisions`.

### 10.2 Add durable final-payload capture artifact

The engine should store a redacted artifact specifically for:

- the authoritative final completion payload capture used for settlement

This artifact is distinct from any full transcript artifact.

Artifact rules:

- canonical evidence root: `docs/evidence/090/junie-runtime-hardening/<fixture-or-run-id>/`;
- live run evidence root: existing application support evidence root, linked from the receipt;
- redaction policy: same redaction class as P088 completion captures;
- hash policy: every final payload artifact path in a receipt must include SHA-256 in the receipt or evidence index;
- absence policy: if no artifact exists, `final_payload_status` must be `missing` and `absence_reason` must be non-null.

### 10.3 Preserve compatibility

Old receipts remain readable.
New fields are additive and default to `unknown` or `null` for historical rows.

Compatibility requirements:

- GraphQL and MCP must preserve old enum values and add nullable/readback-wrapped P090 fields;
- run reports must include P090 fields only when present;
- legacy `completion_status = missing_required_outputs` consumers continue to work;
- new subtype-aware consumers can branch without string-parsing the public message;
- unknown subtype and unknown payload statuses round-trip as raw values.

## 11. Runtime Semantics

### 11.1 Normal successful path

For Junie `code_writer`:

1. ACP session runs.
2. Final completion payload arrives.
3. Payload contains valid `CHAINWORKS_OUTPUT`.
4. Required outputs settle.
5. Receipt records success without invoking repair.

### 11.2 Truncated narrative path

If final payload capture is truncated before valid structured output:

1. classify `junie_final_response_truncated`;
2. invoke narrow repair;
3. settle valid repair outputs per-output;
4. if none settle, block with the new subtype preserved.

### 11.3 Progress-without-handoff path

If runtime shows meaningful progress but no final ACP response:

1. classify `junie_progress_without_terminal_handoff`;
2. do not misclassify as empty execution;
3. preserve worktree delta evidence;
4. do not claim completion success.

### 11.4 Repair narrative path

If repair returns prose instead of strict JSON:

1. classify `junie_repair_returned_narrative`;
2. do not treat that as a valid second completion;
3. settle nothing from that turn;
4. preserve the captured narrative as evidence only.

### 11.5 Runtime tool-path failure path

If Junie runtime preflight cannot read/write the required paths:

1. do not launch Junie for the long-running task;
2. classify `junie_runtime_tool_path_failure_before_publication`;
3. attempt only allowed remediation (`cwd` or runtime-home rebuild) once;
4. if remediation fails or the failure is permission denied, block with the subtype preserved;
5. keep canonical outputs unchanged.

### 11.6 Partial repair path

If repair returns a mixed-validity payload:

1. parse into staged output candidates;
2. validate each output independently;
3. materialize only accepted outputs;
4. leave rejected outputs' canonical files and active pointers unchanged;
5. publish a receipt where stage success remains false until every required output is valid and current-attempt.

## 12. Operator and Readback Semantics

Readback for blocked Junie `code_writer` attempts must answer, explicitly:

- Did the provider start?
- Was there meaningful progress?
- Was there a final ACP response?
- Was the final payload truncated?
- Did repair run?
- Did repair return strict JSON?
- Which outputs settled fresh, stale, malformed, or absent?

Operators should not have to infer all this from a generic `missing_required_outputs`.

Minimum readback fields:

```json
{
  "implementationCompletion": {
    "status": "failed",
    "completion_status": "missing_required_outputs",
    "completion_boundary_subtype": {
      "known": true,
      "raw": "junie_repair_outputs_partially_materialized",
      "value": "junie_repair_outputs_partially_materialized"
    },
    "final_payload_status": "present",
    "transcript_capture_status": "persist_failed",
    "progress_before_handoff": "worktree_diff_detected",
    "runtime_tool_path_preflight": {
      "status": "passed",
      "attempt_count": 1
    },
    "repair_materialization_summary": {
      "fresh_count": 2,
      "malformed_count": 1,
      "absent_count": 1
    }
  }
}
```

GraphQL, MCP, and run-report readback must agree on these fields for the same receipt.

## 13. Acceptance Criteria

P090 is done when all of the following are true:

1. P089 capability proof still passes unchanged.
2. A Junie long-running refine-like canary can complete with fresh settled outputs under the hardened boundary.
3. A deliberately large narrative final response is classified as truncation or narrative-only, not as ambiguous generic failure.
4. Repair that returns one malformed output and multiple valid outputs settles the valid outputs per-output.
5. Progress-without-terminal-handoff is read back distinctly from empty execution and startup failure.
6. Transcript persistence failure does not erase completion-boundary truth.
7. P036-class attempts no longer block with only a broad `missing_required_outputs` diagnosis when a more precise subtype is available.
8. `Operation not permitted`/wrong-cwd/unwritable-runtime-home fixtures fail before provider launch or self-remediate once, with `junie_runtime_tool_path_failure_before_publication` preserved when remediation is not allowed or fails.
9. `code_writer_engine_failure_v1` and `code_writer_repair_failure_v1` fixtures prove parser precedence, unknown-schema fail-closed behavior, and GraphQL/MCP/run-report readback compatibility.
10. Partial repair materialization proves malformed siblings cannot overwrite canonical truth or active artifact pointers.
11. `docs/evidence/090/junie-runtime-hardening/evidence-index.json` maps every observed subtype to either a trustworthy historical artifact or a deterministic synthetic fixture.
12. `./scripts/test-gate.sh proposal-090` exists and verifies the P090 evidence index, subtype coverage, spoof/mismatch fixtures, preflight lifecycle fixtures, and per-output settlement transaction fixtures.

## 14. Test Plan

### 14.1 Capability regression guard

Reuse the P089-style canaries:

- strict JSON only
- strict `CHAINWORKS_OUTPUT` only
- ACP code-writer canary

### 14.2 Truncation fixture

Add a deterministic fixture where:

- Junie-like provider emits a very large final narrative
- no valid output envelope appears before the extraction limit
- the runtime must classify `junie_final_response_truncated`

### 14.3 Progress-without-handoff fixture

Add a fixture where:

- session updates show meaningful progress
- no terminal response arrives
- worktree diff exists
- the runtime classifies `junie_progress_without_terminal_handoff`

### 14.4 Repair narrative fixture

Add a fixture where:

- original completion fails
- repair returns prose
- the runtime classifies `junie_repair_returned_narrative`

### 14.5 Partial repair materialization fixture

Add a fixture where repair returns:

- one valid `implementation_progress`
- one valid `tests_result`
- one malformed `implementation_self_assessment`

The valid outputs must settle; the malformed one must remain rejected.

The fixture must also assert that the previous canonical `implementation_self_assessment` file is byte-for-byte unchanged and that the active artifact index still points to the previous valid generation for that contract.

### 14.6 Transcript persistence degradation fixture

Add a fixture where:

- final completion payload exists
- transcript persistence fails
- receipt still records final completion subtype and settled output truth

### 14.7 Runtime tool-path preflight fixture

Add two fixtures:

1. wrong cwd/runtime-home:
   - preflight fails first;
   - remediation rebuilds launch config/runtime home;
   - second preflight passes;
   - provider launch is allowed.

2. permission denied:
   - preflight read of a project file returns permission denied;
   - remediation is not attempted or fails closed;
   - provider launch is not attempted;
   - receipt subtype is `junie_runtime_tool_path_failure_before_publication`.

### 14.8 Failure envelope fixture

Add fixtures for:

- valid `code_writer_engine_failure_v1`;
- valid `code_writer_repair_failure_v1`;
- unknown envelope schema;
- envelope with mismatched `stage_execution_id` or `agent_execution_id`.

Unknown or mismatched envelopes must fail closed and must not materialize outputs.

The fixture must also prove that provider-authored failure envelopes are not authoritative. A provider final text containing a plausible `code_writer_engine_failure_v1` with mismatched or unverifiable runtime facts must produce `provider_claim_rejected` readback and must not overwrite engine-owned receipt truth.

### 14.9 P036 evidence inventory check

Add a gate check that reads `docs/evidence/090/junie-runtime-hardening/evidence-index.json` and verifies that every subtype in section 9.3 has one evidence entry with a present receipt/artifact hash or an explicit synthetic fixture marker.

### 14.10 Canonical proposal gate

Add `./scripts/test-gate.sh proposal-090|p090`.

Initial readiness-mode gate scope:

- validate `docs/evidence/090/junie-runtime-hardening/evidence-index.json`;
- verify every section 9.3 subtype has one evidence row;
- verify every synthetic fixture path exists and matches its SHA-256;
- verify each fixture records whether it proves subtype coverage, spoof/mismatch rejection, staged repair settlement, or runtime preflight lifecycle;
- verify `docs/reference/test-gates.md` documents `proposal-090`.

Implementation-mode gate scope, added with code changes:

- focused Rust tests for receipt/readback subtype mapping;
- focused Rust tests for engine-synthesized failure envelope authority and provider-spoof rejection;
- focused Rust tests for durable per-output settlement rows and crash recovery;
- focused Rust tests for Junie runtime preflight lifecycle;
- GraphQL/MCP readback compatibility tests.

Readiness-mode gate must also validate concrete negative fixture files for provider-authored spoofing, identity mismatch, unknown envelope schema, malformed repair sibling overwrite, and permission-denied preflight no-launch behavior. These fixtures are not enough for implementation enforcement, but they prevent the spoof/mismatch contract from being only declarative.

## 15. Rollout

Roll out in four steps:

1. Add receipt/readback subtyping and final-payload capture without changing provider selection.
2. Add Junie runtime tool-path preflight in observe/diagnostic mode, then enforce fail-before-launch for permission-denied and invalid-cwd cases.
3. Enable staged per-output repair settlement and strict repair narrative rejection.
4. Turn on the Junie long-running canary as a required regression gate for further Junie `code_writer` runtime changes.

Rollout controls:

| Control | Default | Effect |
|---|---|---|
| `CHAINWORKS_P090_STRICT_FINAL_PAYLOAD` | `0` | When `0`, capture and classify P090 subtypes but preserve legacy settlement enforcement. When `1`, free-form final prose is not a successful terminal completion shape. |
| `CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE` | `0` | When `0`, preflight records diagnostics and allows launch unless the existing runtime would fail. When `1`, permission-denied, invalid-cwd after remediation, and unwritable-runtime-home after remediation fail before provider launch. |
| `CHAINWORKS_P090_STAGED_REPAIR_SETTLEMENT` | `0` | When `1`, repair outputs use staged per-output settlement rows and active pointers are derived from accepted rows. Requires `CHAINWORKS_P090_STRICT_FINAL_PAYLOAD=1`. |
| `CHAINWORKS_P090_DISABLE_STAGED_REPAIR_SETTLEMENT` | `0` | Emergency kill switch. When `1`, strict final-payload capture remains active but staged repair settlement is disabled and repair falls back to existing all-or-nothing behavior with readback `staged_repair_disabled`. |

Downgrade/readback behavior:

- If strict final-payload capture is enabled while staged repair settlement is disabled, readback must show `strict_final_payload_enabled = true`, `staged_repair_settlement_enabled = false`, and `repair_materialization_mode = legacy_all_or_nothing`.
- If the emergency disable is active, readback must show `repair_materialization_mode = staged_repair_disabled` and must not claim `junie_repair_outputs_partially_materialized`.
- If staged repair is requested without strict final-payload capture, startup/config validation must fail closed or coerce staged repair to disabled with an explicit warning; it must not silently enable partial materialization under the legacy completion boundary.

Do not auto-repair historical blocked runs.
Historical `P036` attempts remain evidence fixtures; operators may explicitly retry them after rollout.

## 16. Risks and Mitigations

### Risk: Overfitting to P036

Mitigation:
- keep the new classes Junie-focused but contract-driven;
- validate with deterministic fixtures that cover multiple subtypes.

### Risk: Treating transcript and settlement as two unrelated worlds

Mitigation:
- preserve explicit linkage in the receipt between:
  - final payload capture
  - transcript capture
  - settlement decision

### Risk: Partial output settlement complicates transition truth

Mitigation:
- transition truth stays per required contract;
- valid outputs settle independently, but stage success still depends on all required outputs being in a valid state.

### Risk: Provider-authored failure envelopes become a spoofing path

Mitigation:
- engine-owned persisted facts are the only authority for `code_writer_engine_failure_v1`;
- provider-authored envelope-shaped JSON is treated as a claim until identity/runtime facts validate;
- mismatch fixtures must fail closed and preserve `provider_claim_rejected` readback.

## 17. Open Questions

1. Should the final completion payload capture artifact be retained indefinitely, or only for failed/blocked executions?
2. Should runtime tool-path preflight run for all ACP providers after Junie proves the value, or remain Junie-only for this proposal?

Closed decisions:

- `proposal-090|p090` is the canonical gate.
- `completion_boundary_subtype` is a provider-neutral public wrapper with Junie-prefixed initial known values.
- partial repair materialization is enabled behind the same P090 rollout flag as strict final-payload capture, with a separate emergency disable allowed only for staged repair materialization.

## 18. Evidence Appendix Required Before Implementation

P090 cannot enter implementation until this appendix exists in the proposal or as `docs/evidence/090/junie-runtime-hardening/evidence-index.json`.

The repository must keep the evidence index as the canonical inventory. The proposal table below defines the required rows; the gate validates the JSON index.

Minimum inventory:

| Subtype | Evidence source | Required proof |
|---|---|---|
| `junie_final_response_missing` | historical or synthetic | receipt with no final text and no settled required outputs |
| `junie_final_response_truncated` | historical or synthetic | final payload/truncation metadata with SHA-256 |
| `junie_progress_without_terminal_handoff` | historical or synthetic | session progress or worktree diff plus missing terminal handoff |
| `junie_repair_returned_narrative` | synthetic acceptable | repair capture that is prose and rejected as evidence-only |
| `junie_repair_returned_malformed_json` | synthetic acceptable | repair capture parse failure and no materialization |
| `junie_repair_outputs_partially_materialized` | synthetic required | staged settlement with accepted and rejected outputs |
| `junie_runtime_tool_path_failure_before_publication` | synthetic required, historical preferred | preflight failure before provider launch, with permission/category readback |

For historical P036 evidence, the index must include:

- `source_run_id`
- `source_agent_execution_id`
- `receipt_artifact_path`
- `receipt_sha256`
- `raw_or_redacted_capture_path`
- `raw_or_redacted_capture_sha256`
- `trusted_for_subtype`
- `limitations`

If historical P036 evidence is dirty, incomplete, or mixed with unrelated failed attempts, the index must mark it as `diagnostic_only` and use a synthetic fixture for acceptance.

## 19. Recommendation

Use P090 as the production-hardening follow-up to P089 after the evidence index and readiness gate pass. The proposal now defines the required contracts, but enforcement implementation should not start until the P036/P090 subtype evidence inventory exists and the fixtures in section 14 are ready to enforce the acceptance criteria.

P089 answered:
"Can Junie produce strict structured output at all?"

P090 answers:
"Can Junie reliably deliver that output at the end of a long `code_writer` execution without losing truth at the completion boundary?"

That is the actual open problem exposed by P036.
