# Agent Work Continuation

This document is the canonical contract for server-owned agent work continuation: MCP admission commands, read-only GraphQL/macOS readback, durable SQLite state, continuation evidence artifacts, lead-directed continuation, and provider-session continuity behavior.

`P086`, `proposal-086`, `p086-continuation-*`, and `docs/reference/p086/schemas/` remain as retained historical aliases for the implemented live-handle continuation baseline, gates, evidence, migration names, and schema paths. The active P086 proposal now owns only the remaining provider-session resurrection completion work.

## Overview

The continuation contract manages explicit same-session continuation points for stage-owned `code_writer` executions. The control plane owns admission, idempotency, queueing, side-effect safety, runtime execution, recovery, metrics, and readback; the macOS app reads server projections and does not issue continuation commands.

## API Contracts

### GraphQL

The GraphQL interface provides read-only access to continuation-related data. It exposes raw enum companions, display projections, freshness fields, projection lag, and `UNKNOWN` display states for unknown daemon values. There are no GraphQL mutations for continuation.

Implemented operator-read queries (see `control-plane/crates/graphql-server/src/schema.rs`):

- `continuationStatus(agentExecutionId: ID!)` — returns the active continuation record (if any), full history, and freshness state for an `AgentExecution`.
- `continuationCandidates(runId: ID!)` — returns eligible continuation candidates for a `Run`, with eligibility, raw/display status, and disabled reason per stage-owned `code_writer` `AgentExecution`.
- `continuations(runId: ID!)` — returns the run-level continuation history used by the SwiftUI operator readback card. It is read-only and exposes the same raw/display status, artifact id, freshness, and projection-lag fields as `continuationStatus`.
- `continuationMetricsSummary(runId: ID!)` — returns the durable continuation metric rollup for the run: admission/accept/reject/replay totals, terminal outcome totals, fresh-session avoidance, lead-auto/operator totals and success rates, changed-file/test-gate/test-passed counts, useful-progress/no-progress rates, follow-up validation success rate, average time saved versus the fresh-retry estimate, provider/session budget impact totals, orphan reap attempts/verifications, provider-session resurrection attach success/failure totals, and unsupported resurrection attempts.
- `providerSessionAttachReceipt(continuationId: ID!, runId: ID)` — returns the provider-session attach receipt projection for a `provider_session_resurrection` continuation, gated by principal class. `Operator` (with matching `runId`) receives the full raw v2 receipt JSON. `Observer` receives the reviewer-redacted projection (session ids hashed to `prefix...sha256`, process identifiers and runtime paths absent rather than null-set, `identity_proof_artifact_id` replaced with `[redacted]`). `Agent` receives only `continuation_id` and `resurrection_phase`. Wrong-run operators are rejected with `auth_failure` with no existence oracle. Every successful read, reviewer projection, and denial is recorded in `p086_receipt_access_audit`.

Both queries are exposed to the `Operator` and `Observer` principal classes; the `Agent` class has no visibility. Unauthorized rows are omitted with no existence leak rather than returning partial data.

### Model Context Protocol (MCP)

The MCP defines the command surface for interacting with agent continuations.
`agents.continue_work` is intentionally an admission/enqueue command. It returns
a bounded `accepted` / `replay` / `rejected` response and never waits for the
provider turn to complete. Terminal response, session, provider, and artifact
fields are read through `agents.continuation_status`, `continuations(runId:)`,
or `continuationStatus(agentExecutionId:)` after admission.

Output is an admission response, not a terminal execution response. Terminal fields are readback, not command output. `agents.continue_work` returns a bounded admission response.

Key commands include:

- `agents.continue_work`: The primary command for advancing agent work.
  - Required request fields: `agent_execution_id`, `continuation_mode`, `trigger_kind`, and `idempotency_key`. `mode` is accepted only as a deprecated compatibility alias; if both fields are supplied, they must match.
  - `operator_mcp` is Operator-only.
  - `lead_auto` (trigger_kind) may be requested by Operator or Agent principals, but additionally requires `lead_decision_artifact_id` and `lead_decision_artifact_sha256`, and verifies `continuation_instruction_sha256`, current run/stage/agent/session identity, `agent_id=code_writer`, `decision=continue`, required safety checks, and request budget bounds before any admission row is created. Modes are `live_handle_continuation` and `provider_session_resurrection`; trigger kinds are `operator_mcp` and `lead_auto`.
  - Lead-auto policy limits are enforced inside the same atomic admission transaction as idempotency and queue checks: max one lead-directed continuation per agent execution and max two lead-directed continuations per stage execution. Terminal lead-auto rows count toward these limits.
  - Engine-owned lead orchestration also consumes `lead_continuation_decision_v1` after lead-agent completion. When the artifact contains a non-empty `continuation_instruction` whose UTF-8 SHA-256 equals `continuation_instruction_sha256`, and all target/capability/safety checks pass, the engine writes the same durable admission row with `trigger_kind=lead_auto`, records `caller_surface=engine`, and enqueues `ProcessContinuation`. This is not Swift-local inference and does not require a manual MCP call.
- `agents.continuation_status`: Provides direct read access to continuation status. Unauthorized rows are omitted with no existence leak.
- `agents.continuation_candidates`: Provides direct read access to available continuation candidates. Unauthorized rows are omitted with no existence leak.
- `agents.attach_receipt.get`: Fetches the `provider_session_attach_receipt_v2` body for a `provider_session_resurrection` continuation. Dispatched through the `agents.*` prefix route (alongside the other continuation tools). Request requires `continuation_id`; Operator callers also supply `run_id` for run-scope verification. Response shape varies by principal class: Operator (run-scoped, `run_id` matches) returns the full raw JSON receipt with `outcome=ok`; Observer returns a constant-shape redacted projection with `outcome=reviewer_projection` (session ids hashed, process identifiers and runtime paths absent rather than null-set); Agent/Guest returns `outcome=redacted` with only existence indicator and `resurrection_phase`. Wrong-run Operator returns `auth_failure` with no existence oracle. Every successful read, reviewer projection, and denial writes a row to `p086_receipt_access_audit`.

MCP enum strings are canonical raw daemon values. `agents.continue_work.response`
requires `outcome`; `continuation_id`, `request_fingerprint_sha256`, and
current `status` are present on accepted/replay paths, while terminal fields
such as `response_fingerprint_sha256`, `response_artifact_id`,
`attach_receipt_artifact_id`, `evidence_bundle_artifact_id`,
`worktree_readback_artifact_id`, `continuation_report_artifact_id`, and
`result_or_no_progress_artifact_id` are owned by continuation readback.
`continuation_status.response_schema` defines
`response_schema.$defs.continuation_history_item_v1`, and `history.items`
references `#/$defs/continuation_history_item_v1`.

## Schema Materialization

The following JSON Schema artifacts define the precise structure and validation rules for the continuation API contracts:

### Artifact Schemas

- [`docs/reference/p086/schemas/artifacts/continuation_canonical_request_v1.schema.json`](./p086/schemas/artifacts/continuation_canonical_request_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/lead_continuation_decision_v1.schema.json`](./p086/schemas/artifacts/lead_continuation_decision_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/continuation_response_snapshot_v1.schema.json`](./p086/schemas/artifacts/continuation_response_snapshot_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/continuation_result_v1.schema.json`](./p086/schemas/artifacts/continuation_result_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/continuation_no_progress_report_v1.schema.json`](./p086/schemas/artifacts/continuation_no_progress_report_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v1.schema.json`](./p086/schemas/artifacts/provider_session_attach_receipt_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json`](./p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json) — v2 receipt body for `provider_session_resurrection`, including resurrection phase, deadline/heartbeat/timeout class, identity-proof fields, supervised-child pid/pgid/start_time, runtime-home realpath/dev_ino, orphan reap evidence, and output-only repair fields.
- [`docs/reference/p086/schemas/artifacts/worktree_continuation_readback_v1.schema.json`](./p086/schemas/artifacts/worktree_continuation_readback_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/agent_continuation_evidence_bundle_v1.schema.json`](./p086/schemas/artifacts/agent_continuation_evidence_bundle_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/agent_continuation_report_v1.schema.json`](./p086/schemas/artifacts/agent_continuation_report_v1.schema.json)

### MCP Schemas

- [`docs/reference/p086/schemas/mcp/agents.continue_work.request.schema.json`](./p086/schemas/mcp/agents.continue_work.request.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json`](./p086/schemas/mcp/agents.continue_work.response.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continuation_status.request.schema.json`](./p086/schemas/mcp/agents.continuation_status.request.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continuation_status.response.schema.json`](./p086/schemas/mcp/agents.continuation_status.response.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continuation_candidates.request.schema.json`](./p086/schemas/mcp/agents.continuation_candidates.request.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continuation_candidates.response.schema.json`](./p086/schemas/mcp/agents.continuation_candidates.response.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.attach_receipt.get.request.schema.json`](./p086/schemas/mcp/agents.attach_receipt.get.request.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.attach_receipt.get.response.schema.json`](./p086/schemas/mcp/agents.attach_receipt.get.response.schema.json) — closed-enum `oneOf` over `operator_raw_response`, `reviewer_projection_response`, `guest_redacted_response`, `not_found_response`, `not_available_response`, and `error_response`.

## Rules

Schemas adhere to Draft 2020-12 JSON Schema specifications, with `additionalProperties=false` unless a bounded versioned extension map is explicitly declared.

## Implemented Behavior

The MCP command surface, atomic admission, idempotency conflict handling (`-32044`), saturation backpressure (`-32051`), and the request/response/artifact schemas listed above are implemented in the Rust control plane. Persistence lives in SQLite migrations `control-plane/crates/db/migrations/065_p086_agent_work_continuations.sql` (tables `agent_work_continuations`, `agent_external_side_effect_ledger`, `supervised_workers_continuation`), `control-plane/crates/db/migrations/066_p086_supervised_worker_provider_process.sql` (durable provider pid/process-group binding for restart recovery), and `control-plane/crates/db/migrations/067_p086_continuation_metric_events.sql` (bounded durable metric events and run-level rollups). The proposal referenced filename slot `046` for the main migration; on the implementation branch slots 046-064 were already occupied, so the file landed at slot `065` with identical schema content. Test gates and tooling reference the on-disk filename.

Phase gating at admission:

- `live_handle_continuation` with `trigger_kind=operator_mcp` is the enabled admission path.
- `lead_auto` is enabled only behind server-side decision-artifact validation. Missing, malformed, stale, unreadable, mismatched, hash-invalid, wrong-target, wrong-agent, non-continue, unsafe, or over-budget decision artifacts fail closed before admission.
- `provider_session_resurrection` is admission-blocked behind a Phase 4 per-adapter gate and is rejected unconditionally for all adapters until enablement.
- Continuation is disabled unless the frozen run catalog contains `code_writer.continuation_capability.enabled=true` with the requested trigger and live-handle mode allowed. The implementation catalog declares the opt-in for `operator_mcp` and `lead_auto`; old or malformed snapshots without the field fail closed instead of falling back to a fresh retry path.
- Admission rejects release/publish/git-push/upload/distribution stage lanes and any target stage with unresolved P078 `side_effects` rows. Continuation is therefore limited to implementation/code-editing work and does not take ownership of external release or distribution side effects.

The background worker spawns a continuation admission-timeout sweeper that drives accepted/queued/starting rows past `MAX_ADMISSION_TO_START_SECONDS` to `failed` with `failure_reason="admission_timeout"`, and processes `WorkItemKind::ProcessContinuation` items through `run_continuation_worker`. The worker walks the `accepted → queued → starting → running → prompt_sent → observing → worktree_observed → finalizing → succeeded | no_progress | failed` state machine, inserts ordered runtime/worktree/provider-send rows into `agent_external_side_effect_ledger` (idempotent under the same `idempotency_key` / `request_fingerprint_sha256`) before the durable `prompt_sent` transition so replay never re-sends, registers `supervised_workers_continuation` ownership during claim, builds the canonical P086 mode-reset prompt from the admitted operator/lead context, and dispatches that prompt through the ACP live-session reuse path with the recorded `session_generation_id` / `provider_session_id`. Missing live session or `session_generation_id` settles `no_progress` without provider I/O. Post-`prompt_sent` reconciliation reads transcript evidence or records an explicit transcript absence before terminal settlement, then only settles success when post-continuation worktree evidence is paired with a committed `provider_send` ledger row; worktree mutation without provider-send evidence fails closed as `no_progress`.

Terminal evidence is materialized before terminal settlement is considered complete. The worker writes `continuation_canonical_request_v1`, `provider_session_attach_receipt_v1`, `worktree_continuation_readback_v1`, `agent_continuation_evidence_bundle_v1`, `continuation_response_snapshot_v1`, `continuation_result_v1` or `continuation_no_progress_report_v1`, and `agent_continuation_report_v1` JSON artifacts under the run artifact root, inserts those files into the `artifacts` table, and stores the artifact UUIDs on `agent_work_continuations`. The admission-timeout sweeper also repairs terminal rows that are missing response/result artifact ids before operator readback can pass.

Recovery and cancellation behavior is guarded and fail-closed. Continuation workers refresh `supervised_workers_continuation.last_heartbeat_at` and, once the live ACP session is verified, persist the provider child pid, process group id, and process uid. Startup recovery first tries to close an in-memory registered ACP session; after daemon restart or missing-memory handles, it targets only the recorded provider process group, verifies uid/process-group identity, sends bounded termination signals, records `orphan_reap_attempted`, `orphan_reap_verified`, signal counts, and TERM/KILL deadlines in durable stale-generation evidence, and moves affected continuations to reconciliation with `stale_worker_reaped` or `stale_worker_reap_unverified`. Run cancellation marks active continuations `cancelling`; the worker either settles pre-send cancellation as `cancelled` or, if provider send already happened, records a `provider_cancel` observation and settles cancellation as `cancelled_after_provider_send` instead of allowing a late provider response to overwrite cancellation. Duplicate `ProcessContinuation` work after `prompt_sent` reconciles only from post-continuation worktree evidence and never sends a second provider prompt.

Operator UI readback is passive. The Swift P031 run-detail query reads `continuations(runId:)` and `continuationMetricsSummary(runId:)`; `RunsHomeView` renders an Overview card with latest status, mode, trigger, continuation/stage/agent identifiers, evidence artifact count, and metric summary. The app does not expose `agents.continue_work` or any GraphQL continuation mutation.

Provider-specific resurrection enablement remains gated per adapter until that adapter can attach/resume by provider session id. Unsupported provider-session resurrection remains an explicit fail-closed mode rather than a fallback to fresh retry, and unsupported attempts are counted in the durable continuation metric table.

### Resurrection durable state and readback infrastructure

Persistence and readback surfaces required by `provider_session_resurrection` are now in place even though no adapter declares attach/resume support yet. Migration `079_p086_resurrection_state_and_idempotency.sql` adds `resurrection_phase` (closed enum: `admitted`, `launching`, `launched`, `attaching`, `attached_unprompted`, `prompting`, `settling`, `completed`, `failed_closed`), `resurrection_deadline_at`, `resurrection_last_heartbeat_at`, and `resurrection_timeout_class` (closed enum) columns on `agent_work_continuations` (each constrained to `mode='provider_session_resurrection'`), creates `continuation_terminal_idempotency_ledger(run_id, idempotency_key, request_fingerprint_sha256, prompt_related, continuation_id, terminal_status, terminal_at, retention_until)` plus two named partial unique indexes (`idx_terminal_idempotency_ledger_uniq_prompt_related_run_key` WHERE `prompt_related=1` and `idx_terminal_idempotency_ledger_uniq_pre_prompt_run_key` WHERE `prompt_related=0`), replaces the previous active idempotency uniqueness with `uniq_continuations_active_idempotency_run_scoped` on `(run_id, idempotency_key)`, adds the single-active-per-target guard `uniq_continuations_single_active_resurrection` and the `idx_agent_work_continuations_resurrection_deadline_active` watchdog index. Migration `080_p086_timeout_settled_at.sql` adds `timeout_settled_at` for the rollout-contract readback. Migration `081_p086_resurrection_phase_cancelling.sql` widens the `resurrection_phase` CHECK to include `cancelling`. Migration `082_p086_receipt_access_audit.sql` creates the durable `p086_receipt_access_audit(principal_id, principal_class, continuation_id, run_id, requested_at, source_channel, outcome, denial_reason, …)` table for every raw read / reviewer projection / denial. Migration `083_p086_deadline_invariant.sql` tightens the deadline invariant so that non-terminal resurrection rows must carry a non-null `resurrection_deadline_at` while terminal phases (`completed`, `failed_closed`) must have a null deadline. Migration `084_p086_terminal_ledger_reconciliation.sql` extends `terminal_status` to include `needs_continuation_reconciliation`. Migration `085_p086_raw_receipt_db_storage.sql` introduces `p086_resurrection_raw_receipts` (DB-backed raw receipt JSON) so that the raw v2 body never resides on the filesystem reachable by same-UID ACP child processes through `CHAINWORKS_META_ROOT` traversal; `DATABASE_URL` is excluded from the child env (`env_clear` + allowlist) so child processes cannot reach the table.

Idempotency and ledger probes always carry `WHERE run_id=:authorized_run_id`. Two principals scoped to different runs may carry identical `idempotency_key` values without collision and without producing an observable timing or shape difference. Negative fixtures under `docs/evidence/rollout-contract/p086/negative/active-idempotency-cross-run-no-collision.fixture.json` and `…/active-idempotency-cross-run-no-existence-oracle.fixture.json` assert both behavior and constant-shape constant-time response shape.

Raw v2 receipt access follows the principal access matrix. Operator (run-scoped, `run_id` must match the receipt's `run_id`) sees the full raw body; Observer sees a constant-shape reviewer-redacted projection (session ids hashed to `prefix...sha256`; process identifiers and runtime paths absent rather than null-set; `identity_proof_artifact_id` replaced with `[redacted]`); Agent sees only `continuation_id` and `resurrection_phase`. Wrong-run Operators receive `auth_failure` with no existence oracle. Every access path writes a `p086_receipt_access_audit` row with `outcome` ∈ {`raw_read`, `reviewer_projection`, `denied`} and a `denial_reason` for denials. The MCP `agents.attach_receipt.get` tool and the GraphQL `providerSessionAttachReceipt` query share this enforcement path. Negative fixtures `docs/evidence/rollout-contract/p086/negative/attach-receipt-fetch-{operator-wrong-run-rejected,reviewer-redacted,guest-redacted,unauthenticated-rejected}.fixture.json` assert presence-and-content of each response shape.

The `agents.attach_receipt.get` tool is dispatched through the `agents.*` prefix route in `control-plane/crates/mcp-server/src/server.rs` (alongside the other continuation tools) rather than through `CapabilityToolId`-keyed dispatch, and is therefore advertised in `tool_specs()` but absent from the typed capability table.

Expansion/soak validation (14-day no-hold window, SLO-budget validation, 100 continuations across 30 runs) is intentionally tracked by [Proposal 093](../proposals/093-agent-work-continuation-expansion-soak.md). Resurrection-specific soak evidence depends on completing the active P086 provider-session resurrection proposal first.
