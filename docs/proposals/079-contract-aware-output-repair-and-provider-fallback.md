{
  "schema_version": "proposal_current_v1",
  "proposal_id": "P079",
  "proposal_revision_id": "p079-contract-aware-output-repair-and-provider-fallback-r5",
  "source_review_pass_id": "f5d0824f-9086-4d1f-99b7-f6e42b5cb945",
  "title": "Contract-Aware Output Repair and Provider Fallback",
  "status": "draft_for_implementation_review",
  "document_markdown": "# Proposal 079: Contract-Aware Output Repair and Provider Fallback\n\n| Field | Value |\n|---|---|\n| Proposal ID | P079 |\n| Revision | p079-contract-aware-output-repair-and-provider-fallback-r5 |\n| Date | 2026-05-30 |\n| Status | Draft for implementation review |\n| Source review pass | f5d0824f-9086-4d1f-99b7-f6e42b5cb945 |\n| Primary gate | `./scripts/test-gate.sh proposal-079` and `./scripts/test-gate.sh p079` |\n| Related | P027, P029, P086, P088, P095, output settlement, artifact claims, auto-retry observation ledger, executable rollout gate |\n\n## Problem\n\nChainworks can complete useful provider work and still block a run because the final declared output set is missing, empty, invalid, emitted through the wrong provider mode, or stranded in the current provider envelope. Normal output settlement already validates declared outputs and source-generation ownership, but the recovery lane after a contract failure is not fully governed. That creates a costly path: the provider does substantial work, settlement rejects the required output envelope, the stage blocks, and a later retry repeats work while losing same-session context.\n\nP079 treats this as an invocation settlement problem. It adds a bounded recovery lane after normal output collection fails and before the run is durably blocked for an output-contract failure. It does not replace normal output collection, human approvals, workflow-conflict mediation, release safety, or quality retries.\n\n## Goals\n\n- Attempt at most one same-session corrective output repair turn for eligible missing, empty, invalid, or mode-mismatched required outputs.\n- Recover contract-valid output already present in the current invocation transcript or provider result envelope when it is attributable to the current agent execution by transport-allocated identifiers.\n- Allow at most one controlled provider fallback attempt after repair or recovery is unavailable or unsuccessful, and only from frozen fallback policy.\n- Preserve declared output contracts, canonical output paths, source-generation claims, and existing settlement as the only active artifact truth.\n- Exclude release, publish, upload, distribution, git push, and other durable side-effect lanes.\n- Expose typed repair, recovery, fallback, budget, lease, status, and final-settlement evidence through run reports, MCP, and GraphQL, with a compiled Swift DTO migration and decode-test gate so the canonical operator shell is part of acceptance.\n- Validate the behavior with deterministic fixture ACP transports and local control-plane tests only.\n\n## Non-Goals\n\n- Do not accept invalid, stale, partial, schema-mismatched, wrong-path, or previous-attempt artifacts.\n- Do not infer approval, rejection, workflow-conflict resolution, or operator intent from repaired output.\n- Do not rerun implementation work inside the repair prompt.\n- Do not add release-agent fallback or weaken durable side-effect settlement.\n- Do not require live Claude, Codex, Gemini, Junie, Auggie, network access, or external services for the proposal gate.\n- Do not broaden legacy filesystem discovery or workspace scanning.\n- Do not add a SwiftUI repair or fallback mutation surface in this proposal.\n- Do not make the auto-retry ledger dispatch work; it remains observe-only.\n- Do not trust provider-self-declared agent_execution_id, session_generation_id, or recovery_source values from envelope or transcript bodies.\n\n## UX/UI Notes\n\nThe macOS app remains a passive operator shell. P079 evidence is read-only diagnostic state owned by Rust control-plane projections and rendered from typed GraphQL or run-report DTOs. The app must not parse raw transcripts, scan `.junie/plans`, discover workspace files, or persist P079 evidence as canonical SwiftData artifact truth unless a later migration proposal adds that authority.\n\nOperator status should be compact and factual: repair accepted, repair rejected, repair skipped because approval is pending, fallback scheduled, fallback accepted, fallback unavailable, or plan evidence preserved but not accepted. Primary UI badges bind to the stable `presentation_category` enum: informational, recovered, blocked, skipped, failed, or cancelled. Detailed failure class, subtype, lease, budget, and provider-plan evidence appear in inspectors or reports.\n\n### Swift Client Migration (APPLE-001)\n\nThe Swift app must compile and decode the new readback surface as part of the proposal-079 gate. The migration includes:\n\n- A versioned DTO module under `Chainworks Forge/Engine/Readback/OutputContractRepair/` with codable structs for `OutputContractRepairEvidence`, `OutputContractRepairAttempt`, `OutputContractTranscriptRecovery`, `OutputContractProviderFallback`, `OutputContractPlanEvidence`, `OutputContractBudget`, `OutputContractLease`, `OutputContractRepairPresentation`, and `OutputContractRepairStatus`.\n- Optional parent (`OutputContractRepairEvidence?`) decoding for pre-P079 and feature-disabled runs that returns nil without throwing.\n- Closed `presentation_category` and `recommended_next_action` mapping into the Swift presentation layer with a conservative `unknownDiagnostic` fallback case that preserves the raw string in inspectors. The conservative fallback never authorizes output settlement, transition truth, or operator-initiated dispatch.\n- **SwiftUI identity contract (APPLE-R3-001):** the stable SwiftUI row identity is `(repair_attempt_id, agent_execution_id)` only. `evidence_version` is a monotonic content-version / refresh-invalidation field and MUST NOT participate in identity. `Identifiable.id` and `ForEach` keys are derived from `repair_attempt_id + agent_execution_id`; `Equatable` on the DTO compares the full snapshot so SwiftUI recomputes the body when any presentation field (including `status`, `presentation_category`, `recommended_next_action`, `lease.expires_at`, `evidence_version`) changes, while diffing keeps the row in place. MainActor coalescing tests assert that `reserved -> prompt_sent -> settled` updates replace the same row (single visible row across the three transitions), that subscription replay of an unchanged `evidence_version` snapshot causes zero row churn, and that projection rebuild replay does not produce duplicates. All observable presentation fields (`status`, `presentation_category`, `recommended_next_action`, `lease`, `budget`) are derived from the immutable snapshot and refresh only when the snapshot itself changes.\n- A SwiftData cache invalidation note: P079 evidence is rendered from ephemeral read-model snapshots only; any cached projection invalidates on `evidence_version` or `projection_integrity` changes. The app does not store canonical P079 artifact truth in SwiftData.\n- Fixture decode tests under `Chainworks Forge/Tests/Engine/Readback/` covering: old-run (parent null), feature-disabled (parent null), not-attempted, recovered, blocked, cancelled, stale projection, orphan diagnostic evidence, lease reclamation, unknown enum future-tolerance.\n- The proposal-079 gate runs `./scripts/test-gate.sh build` plus the Swift fixture-decode slice (`./scripts/test-gate.sh p079-swift-readback`) so the canonical operator shell is verified at acceptance.\n\nAll new GraphQL fields are additive and optional at the parent boundary. Pre-P079 runs and feature-disabled runs decode without crashing: `outputContractRepair = null`, flattened report status `not_attempted`, attempt flags false, result `not_needed`, and path arrays empty. Live updates flow through existing run and stage observation channels as immutable snapshots; SwiftUI coalesces them on the MainActor and keeps timeline identity stable with `(repair_attempt_id, agent_execution_id)` — `evidence_version` drives content refresh and Equatable comparison, not identity.\n\n### Status → Presentation Category Projection (macos-r3-001)\n\nThe top-level `status` and `presentation_category` enums are pinned by a normative projection function. The Swift DTO module exposes the projection as a pure function `presentationCategory(for: OutputContractRepairStatus, finalSettlement: FinalOutputSettlement) -> PresentationCategory`. The parity fixture `p079-output-repair-full-surface.fixture.json` asserts one row per `status` value with the projected `presentation_category`.\n\n| `status` | `presentation_category` | Notes |\n|---|---|---|\n| `not_attempted` | `informational` | Pre-P079 / feature-disabled / no failure rendered as informational pill. |\n| `in_progress` | `informational` | Active repair or fallback turn; loading badge applies (see below). |\n| `recovered` | `recovered` | Output recovered, repaired, or fallback-accepted. |\n| `blocked` | `blocked` | Terminal contract failure; operator action required. |\n| `skipped` | `skipped` | Eligibility skip (approval pending, Junie risk, side-effect lane). |\n| `cancelled` | `cancelled` | Operator cancellation terminal. |\n| `failed` | `failed` | Transport / deadline / unsafe_continuation terminal. |\n\n### Presentation Polish Contract (ui-001..004, macos-r3-002..006)\n\n- **`in_progress` / `prompt_sent` visual state:** a standard SwiftUI indeterminate `ProgressView()` chip is rendered alongside the `informational` badge while `lease.state in {reserved, prompt_sent}` and `status = in_progress`. Once `lease.state = settled` the chip is removed in the same SwiftUI transaction as the new badge.\n- **Inspector grouping:** the detailed inspector groups fields under three labeled `GroupBox` sections — Diagnostics (`initial_failure_class`, `initial_failure_subtype`, `recommended_next_action`), Execution Details (`lease`, `budget`, `same_session_repair`, `transcript_recovery`, `provider_fallback`, `permission_decisions`), and Evidence (`required_outputs`, `provider_plan_evidence.paths`, `final_output_settlement`, `evidence_artifact_path`).\n- **Plan evidence interactivity:** `provider_plan_evidence.paths` and `evidence_artifact_path` render with `.textSelection(.enabled)` and a context-menu `Copy Path` + `Reveal in Finder` action. `Reveal in Finder` resolves the path against `run_meta_root` and uses `NSWorkspace.activateFileViewerSelecting(_:)`; paths that do not resolve under `run_meta_root` are non-interactive label-only.\n- **`unknownDiagnostic` visual:** rendered as a neutral caution (yellow SF Symbol `questionmark.diamond`) with no primary action affordance. It never authorizes settlement, transition truth, or operator-initiated dispatch.\n- **Accessibility:** each `presentation_category` value renders with (a) a distinct SF Symbol (`info.circle`, `checkmark.shield`, `exclamationmark.octagon`, `slash.circle`, `xmark.octagon`, `xmark.circle`), (b) semantic colors (`Color.accentColor`, `.green`, `.red`, `.secondary`, `.red`, `.secondary`) overridable under Increase Contrast, (c) a localized `accessibilityLabel` combining category and underlying `status`, and (d) a stable `accessibilityIdentifier` per badge for UI test selection. `permission_decisions` rows are focusable list items with VoiceOver labels combining `method + decision + reason`.\n- **Date formatting:** `lease.expires_at`, `lease.acquired_at`, and `projection_stale_since` use `Date.RelativeFormatStyle(presentation: .numeric, unitsStyle: .abbreviated)` for inline display and `Date.ISO8601FormatStyle` for copy-to-clipboard / audit views, both `Locale.autoupdatingCurrent`. The DTO decoder accepts RFC3339 / ISO8601 with or without fractional seconds and offset.\n- **Pasteboard surfaces:** diagnostic identifiers (`repair_attempt_id`, `fallback_agent_execution_id`, `fallback_packet_hash`, `lease.key`, `evidence_artifact_path`, redacted plan-evidence paths) render with `.textSelection(.enabled)` and a right-click `Copy Identifier` action; a menu bar action jumps to the most recent blocked run's P079 inspector. No new mutation surface.\n- **Stale projection UX (macos-r3-005):** rows with `projection_integrity = stale` render a `Stale · retrying…` chip with relative `projection_stale_since`. An optional `Refresh` affordance re-issues the existing read-model query (read-only; never dispatches repair or fallback). Automatic recovery occurs when the next snapshot carries `projection_integrity = fresh`. If a projection rebuild exceeds 12 consecutive sweep attempts (≈ 1h elapsed under the 60s→5m capped backoff), the row also surfaces `projection_integrity = permanently_stale` and `recommended_next_action = manual_investigation` (REL-r2-9 abandonment ceiling, addresses rel-r3-n2).\n- **Background notification:** the app posts a `UNUserNotification` (subject to user preferences) when a previously blocked run transitions to `status = recovered` via P079 or terminates at `status = blocked` with `recommended_next_action = manual_investigation`. Read-only observer over the existing snapshot stream; no engine change required.\n- **Module layout (macos-r3-008):** the DTO module is introduced as a new sub-package `Chainworks Forge/Engine/Readback/OutputContractRepair/`, sibling to `RunPlanCompiler.swift`, `WorkflowOrchestrator.swift`, and the existing typed readback consumers. The proposal explicitly creates the `Readback/` directory under `Chainworks Forge/Engine/` and lists it in the test plan for `./scripts/test-gate.sh build`.\n\n## Architecture\n\n### Invocation Order\n\nP079 starts only after normal output collection fails. The order is: work turn, P095 output collection when applicable, declared output settlement, transcript/provider-envelope recovery, same-session repair when eligible, provider fallback when policy allows, then final settlement or typed blocked evidence. Transcript/provider-envelope recovery runs before spending the single repair turn because it uses already captured current-invocation material. Provider fallback runs only after recovery and repair are unavailable, skipped, rejected, cancelled, or exhausted.\n\n### Eligible Failure Classes\n\nRepair and recovery are eligible only when the invocation declares required outputs, the validation result is `no_output_produced`, `empty_output`, `missing_required_outputs`, `invalid_required_outputs`, `output_contract_mismatch`, or `provider_mode_mismatch`, the source-generation claim is still active, and no human approval, workflow conflict, cancellation, release lane, or superseded execution owns the blocker.\n\n`provider_mode_mismatch` subtypes are closed for v1: `plan_event_instead_of_output`, `empty_submit_after_plan`, `file_plan_written_instead_of_payload`, and `repair_repeated_plan_behavior`. Additional subtype values require a schema revision and fixtures.\n\n### Same-Session Repair\n\nThe executor issues at most one narrow corrective turn in the same live ACP session. The prompt includes runtime identifiers, failed output names, contract ids, canonical output paths, validator errors, and contract-complete `CHAINWORKS_OUTPUT` skeletons for failed outputs. It tells the agent not to redo task work and accepts only corrected required outputs. A torn-down ACP session is not same-session repairable; the engine records `unavailable` and proceeds to fallback policy if allowed.\n\nRepair settlement is atomic for the failed output set. If a multi-output repair returns two valid outputs and one missing or invalid output, none of the repair candidates update active artifact truth. The evidence records per-output validation details and the result is `rejected_invalid` or the remaining blocked class.\n\n#### Repair Prompt Template (sec-005)\n\nThe repair-prompt template is pinned by `repair_prompt_template_version` (initial value `p079_repair_v1`) and persisted on each `output_contract_repair.v1` row. Reflected validator-error fragments and any rejected-artifact bytes are wrapped in delimited untrusted-content fences (`<<<UNTRUSTED_VALIDATOR_ERROR>>> ... <<<END_UNTRUSTED_VALIDATOR_ERROR>>>`), per-string capped at 2 KiB and total reflected-fragment payload capped at 8 KiB. Known prompt-injection markers (`ignore previous instructions`, role-takeover phrasing, embedded fence tokens, well-known provider role tags) are redacted with `[redacted:injection_marker]` before assembly. The template is published verbatim in `docs/reference/p079-repair-prompt-template.md` and unit-fixtured under `docs/evidence/rollout-contract/p079/repair-prompt-template-pinned.json`.\n\n#### Same-Session Repair Permission Posture (sec-004)\n\nDuring the bounded corrective turn, the ACP transport replaces its standard auto-grant policy with the P079 repair-turn posture:\n\n- The only permission decisions auto-granted are `fs.write` requests whose resolved target byte-matches a frozen canonical output path declared on the failed agent execution.\n- All other `session/request_permission` requests (shell execution, broad filesystem write, network, custom tools, additional fs.read outside meta-root, etc.) are denied. On a denied request the repair turn terminates with `same_session_repair.result = rejected_invalid`, `initial_failure_subtype = unsafe_continuation`, and a `permission_decisions` array entry capturing the denial.\n- Every permission decision (allow or deny) is appended to `output_contract_repair.v1.permission_decisions` as `{method, resource_kind, decision, reason}` with no raw paths outside the frozen canonical output path set.\n- The same posture applies to fallback agent executions for output-contract recovery. Release lanes are already excluded by hold conditions.\n- The transport posture is enforced server-side in the Rust ACP transport, not by prompt directives.\n\n### Transcript and Provider-Envelope Recovery\n\nRecovery may parse ACP message chunks, provider envelope fields, or bounded transcript excerpts for the current `agent_execution_id` and `session_generation_id`. It must not use prior attempts, prior session memory, workspace scans, broad artifact discovery, or provider plan files as output truth. Accepted recovered payloads pass the same declared-output validator and canonical path binding as normal output.\n\n#### Recovery Bounds and Attribution Integrity (sec-002)\n\nRecovery enforces explicit numeric bounds, persisted on every `output_contract_repair.v1.transcript_recovery` row:\n\n- `max_recovery_payload_bytes` default 256 KiB, knob `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_MAX_BYTES` (1 KiB <= value <= 1 MiB).\n- `max_json_depth` default 32. Any payload deeper than the cap is rejected.\n- `max_chunks_examined` default 64. Recovery scans at most this many ACP message chunks per attempt.\n- The decoder is streaming and fail-closed: when any bound is exceeded the recovery result is `unavailable` with subtype `oversized_payload`, no bytes are stored beyond the bound, and no budget is consumed.\n- Attribution: `agent_execution_id`, `session_generation_id`, and `recovery_source` are taken from transport-allocated message identifiers and the daemon's own session ledger, never from provider-emitted body fields. Provider-claimed attribution fields inside an envelope body are ignored or, if they conflict with the transport view, the recovery result is `unavailable` with subtype `unattributable_envelope`.\n- Per-adapter attribution rules are pinned in `docs/reference/p079-recovery-attribution.md` (which envelope fields carry which transport-allocated id, what happens when omitted). Cross-provider attribution is from transport metadata only.\n- A `recovery_parser_version` (initial value `p079_recovery_v1`) is persisted alongside each recovery record so hardening regressions are detectable in audit.\n- Recovery on stored evidence does not replay parsing: stored evidence is the post-bound projection; the original oversized payload is not retained.\n\n### Provider Plan-Evidence Protection (sec-003)\n\nProvider plan files such as `.junie/plans/*.md` are diagnostic evidence only. They may be attached with `accepted_as_output=false`, but they must not satisfy a required output, update artifact truth, drive transition truth, or substitute for `CHAINWORKS_OUTPUT`. P079 imposes the following protections on retention and exposure:\n\n- Plan evidence is copied into a P079-owned directory under the run meta-root: `${run_meta_root}/output_contract_repair/<agent_execution_id>/plan_evidence/` with parent directory mode `0700` and file mode `0600`. The original file location is not surfaced.\n- A redaction pass runs at copy time and strips: `CHAINWORKS_MCP_TOKEN` values, `Authorization: Bearer <token>` headers, well-known provider token prefixes (`sk-`, `sk-ant-`, `AIza`, `anth-`), Codex `auth.json` JSON fragments, and any absolute filesystem path that does not resolve under `workspace_root` or `run_meta_root`. Redacted bytes are replaced with `[redacted:<class>]` markers and counted in `provider_plan_evidence.redactions_applied`.\n- Per-file size cap: 256 KiB. Per-execution total plan-evidence cap: 1 MiB. Excess is truncated at the cap boundary and recorded as `truncated_at_cap=true`.\n- Retention is bound to source-generation lifetime: plan evidence is purged when the parent source-generation claim is retired or the run is permanently archived, whichever comes first.\n- Exposed `provider_plan_evidence.paths` values in GraphQL, MCP, and run report are normalized to run-meta-root-relative form (e.g. `output_contract_repair/<agent_execution_id>/plan_evidence/<file>.md`). Any plan-evidence path that does not resolve under `workspace_root` or `run_meta_root` is dropped with a diagnostic record and is never exposed.\n\n### Junie Strict Structured Output\n\nIf adapter family is `junie`, required output mode is `strict_structured`, and captured material contains only plan evidence or `provider_mode_mismatch`, same-session repair is skipped as `provider_mode_mismatch_risk`. Fallback may run only if frozen policy and feature flags allow it.\n\n### Controlled Provider Fallback\n\nFallback is a fresh agent execution with the same declared output contract and a strictly-defined sanitized context packet (`output_contract_repair_fallback_packet.v1`). It must not mutate approvals, workflow-conflict decisions, loop budgets, or active artifact truth from the failed execution. Only successful fallback output can update active truth, and only through the existing validator and source-generation settlement path.\n\nFallback policy is declared in YAML under `output_repair_policies`, compiled into `RunPlanSnapshot`, and read from the frozen snapshot during execution. Live catalog changes after run start are drift, not automatic behavior changes. Missing policy, disabled policy, missing feature flag, or pre-P079 snapshot absence is read back distinctly as `policy_not_in_snapshot`, `feature_disabled`, or `fallback_unavailable`.\n\nFallback is bound to the same principal that owned the failed agent execution (P029 bearer principal). If the principal is revoked between dispatch decision and child execution start, the fallback terminates with `result = unavailable` and `subtype = principal_revoked`. The principal id and capability set in effect at fallback dispatch are persisted in `output_contract_repair.v1.fallback_principal_id` and `fallback_principal_capability_hash`.\n\n#### Fallback Context Packet Contract (sec-001)\n\nThe fallback context packet is the only cross-vendor data flow P079 introduces. Its closed schema (`output_contract_repair_fallback_packet.v1`) is included in `fallback_context_packet_v1_schema` in this proposal JSON. Required behavior:\n\n- Top-level field set (closed, `additional_properties=false`): `schema_version`, `validation_failure_class`, `validation_failure_subtype`, `required_output_names`, `required_output_contract_ids`, `required_output_canonical_paths_relative`, `prior_attempt_summary`, `repair_prompt_template_version`, `recovery_parser_version`.\n- Paths inside the packet are run-meta-root-relative; absolute paths outside the run meta-root are rewritten or dropped.\n- A redaction tier strips environment values, absolute filesystem paths outside `run_meta_root`, `CHAINWORKS_MCP_TOKEN` values, `Authorization` headers, provider token prefixes (`sk-`, `sk-ant-`, `AIza`, `anth-`), and Codex `auth.json` fragments. Operator rationale text, operator-instruction overrides, and approval rationales are forbidden by schema.\n- Total serialized packet size cap: 32 KiB. Per-string cap: 4 KiB. Excess causes packet assembly to fail closed (`provider_fallback.result = unavailable`, `subtype = oversized_fallback_packet`, no fallback dispatch).\n- The serialized packet is hashed (sha256) and bound to the fallback agent execution under `output_contract_repair.v1.fallback_packet_hash`. The fallback child execution stores the same hash so audit can prove byte-equality.\n- Negative fixtures: `secret_in_artifact_redacted_before_fallback`, `auth_principal_not_transmitted`, `operator_rationale_not_transmitted`, `absolute_path_rewritten_to_meta_root_relative`, `oversized_fallback_packet_blocks_dispatch`.\n- This contract must land before `CHAINWORKS_P079_PROVIDER_FALLBACK_ENABLED` is enabled for any role.\n\n#### Fallback Policy YAML Contract\n\nThe machine-readable YAML contract is included in `fallback_policy_schema` in this proposal JSON. Required concepts are policy id, schema version, role-family match, eligible failure classes, allowed output modes, repair budget, transcript recovery sources, failed backend profile, fallback backend profile, side-effect exclusion, required feature flags, disabled reason, snapshot serialization, and drift readback. The initial lead-orchestrator mapping is `gemini_reasoning_pro_high` to `claude_orchestrator_high` for output-contract settlement failures only. Proposal writer, proposal reviewer, lead orchestrator, security checker, and side-effect-free prepush reviewer roles may opt into the policy. Release and durable side-effect lanes cannot opt in.\n\n**Transcript-recovery flag binding (api-contract-r2-002):** when a policy sets `transcript_recovery.enabled: true`, `feature_flags_required` MUST include `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED`. Snapshot compilation rejects any policy where this binding is missing with `disabled_reason = transcript_recovery_flag_missing`. The flag is not separately global; transcript recovery only runs under a frozen policy that explicitly enables it. Operator readback exposes the flag set in `output_contract_repair.v1.policy_feature_flags`.\n\n### Canonical Artifact Path Binding\n\nCanonical output targets are resolved once from the frozen snapshot path templates at run start. Returned output paths must match the frozen resolved path string exactly. The runtime does not trim whitespace, expand environment variables, expand `~`, case-fold, Unicode-normalize, accept display labels, accept trailing slashes for file outputs, or accept alternate absolute spellings. Dot segments are removed during snapshot compilation; returned paths containing unresolved `.` or `..` are rejected unless the exact frozen string contains them for a documented compatibility reason.\n\nBecause output files may not exist before settlement, primary validation is exact string binding to the frozen target. Materialization of accepted output uses `openat(2)` with `O_NOFOLLOW` per path component (or canonicalize-then-fstat-after-open) so that a parent symlink swapped between check and write is rejected at write time (sec-006). Case-insensitive macOS volumes do not relax matching; `/A/File.md` and `/a/File.md` are different contract strings. Unicode-equivalent strings are different unless byte-identical to the frozen path. Negative fixtures cover whitespace, symlink escape, symlink-swapped-after-check, case-only mismatch, decomposed Unicode mismatch, env-var literal, tilde literal, trailing slash, companion path, and undeclared output name.\n\n### Evidence and Readback Contract\n\n`output_contract_repair.v1` is a closed schema for this proposal. The machine-readable enum vocabulary, nested object shapes, and evolution policy are included in `output_contract_repair_v1_schema` in this proposal JSON. Evidence records carry repair/fallback attempt ids, run/stage/agent/session ids, role/provider fields, failure class and subtype, required outputs, nested repair/recovery/fallback objects, budget accounting, lease state with TTL, plan evidence, final settlement, recommended next action, presentation category, permission decisions, status, timestamps, evidence_version, and template/parser version pins.\n\nThe top-level `status` enum (api-contract-r2-001) is a stable presentation-and-routing summary distinct from per-subobject `result` enums. Closed values: `not_attempted`, `in_progress`, `recovered`, `blocked`, `skipped`, `cancelled`, `failed`. It is required on every v1 evidence row. Pre-P079 runs decode the absent parent as `null`; flattened operator-report views render `not_attempted`. Status is a derived field (computed from subobject results and final_output_settlement) and is part of the parity fixture.\n\n`repair_attempt_id` and `fallback_agent_execution_id` are random v4 UUIDs. Lease keys are deterministic-but-non-enumerable composite hashes of `(run_id, stage_execution_id, parent_agent_execution_id, schema_version, frozen_fallback_policy_hash)` so they are guessable only with knowledge of all components (sec-008).\n\nGraphQL exposes nullable `outputContractRepair: OutputContractRepairEvidence` from agent execution read models and includes it in run/stage subscription snapshots. MCP `reports.get` and `report://{run_id}` expose the same snake_case `output_contract_repair` object. The run report may also flatten selected fields for tables, but the nested object is the parity authority. The parity fixture named in `rollout_contract_v1.readback_fixture` proves identical semantic values across run report, MCP, and GraphQL camelCase projection, including `status`, lease TTL, permission decisions, fallback packet hash, and recovery bounds.\n\nA minimal GraphQL SDL appendix for `OutputContractRepairEvidence` and nested types (enum names, nullable fields, list element nullability, timestamp scalar, v1 compatibility defaults) is included in `graphql_sdl_appendix` in this proposal JSON.\n\n### Persistence and Migration\n\nP079 requires a SQLite migration. The control-plane DB stores typed `output_contract_repair_events`, `output_contract_repair_leases`, and fallback parent linkage. These rows are the scheduling and readback authority. Evidence artifacts under the run directory are materialized projections rebuilt from DB rows. If an evidence artifact is missing or corrupt, recovery rebuilds it from SQLite and marks projection integrity. If an artifact exists without a DB row, it is reported as orphaned diagnostic evidence and cannot schedule repair, consume budget, or settle output.\n\nBudget consumption, lease state, terminal evidence, and final settlement are written in the same DB transaction where possible. When filesystem report materialization fails after the DB commit, readback remains available from SQLite and the report lane exposes `projection_integrity=stale` until rebuild succeeds. The projection rebuild trigger (REL-r2-9) is: on next operator read of the affected row, plus a bounded background sweep with first attempt at 60s after detection and exponential backoff capped at 5 minutes. Staleness duration is surfaced in operator readback as `projection_stale_since` so stuck projections are diagnosable. After 12 consecutive failed rebuild attempts (≈1h under the capped backoff) the row escalates to `projection_integrity = permanently_stale` and `recommended_next_action = manual_investigation` (rel-r3-n2 abandonment ceiling). The underlying SQLite row remains readable; only the materialized projection artifact is marked permanently stale.\n\nRollback disables scheduling but leaves migrated rows and artifacts readable. Migration version names and rollback readback behavior are enumerated in `rollout_contract_v1.migrations.versions`. The complete table-by-table column/PK/UK/FK/index/CHECK/transactional-invariant contract is in the top-level `sqlite_migration_appendix` of this proposal JSON (resolves api-contract-r3-002), with `projection_schema_version` and per-JSON-payload embedded versioning so v2 nested-object shapes can coexist with v1 rows without rewriting.\n\n### Reliability Semantics\n\n#### Deployment Posture (rel-r3-n4)\n\nThe control plane is a single-daemon deployment in this revision; multi-daemon coordination is not a supported posture. CAS reclamation language in the lease table is defensive against transient overlap during restart (old daemon still draining as new daemon starts), not a multi-tenant scheduling contract. Future multi-daemon work requires a separate proposal.\n\n#### Graceful Shutdown Drain (rel-r3-n6)\n\nOn graceful shutdown the daemon attempts to await ACP result for in-flight `prompt_sent` repair turns up to the role watchdog deadline; on timeout it terminates the ACP subprocess and hands off to startup recovery. Startup recovery observes the `prompt_sent` lease state and MUST NOT re-prompt; it either records `unavailable` or resumes settlement if the result is durably recoverable from the transcript. The repair budget is consumed because provider work occurred. Fixture: `graceful_shutdown_drains_or_hands_off_no_reprompt`.\n\n#### Sweep Failure Retry (rel-r3-n1)\n\nThe reconciliation sweep is wrapped in a SQLite transaction. On transaction failure (BUSY, transient I/O, etc.) the sweep logs the failure, emits `recovery_sweep_total{kind, result=failed}`, and retries on the next deterministic 60s tick. The cadence is short enough that no exponential escalation is needed; persistent failure is alerting-visible via the metric and through `projection_stale_since` on affected rows. Fixture: `sweep_transaction_conflict_retries_on_next_tick`.\n\n#### Concurrency Cap and Backpressure (rel-r3-n3)\n\nNo global P079 concurrency cap is intended. Per-execution budget (1 repair + 1 fallback) is the bound. An observability-only metric `p079_repair_inflight_total{role}` is exposed for ops visibility; it does not gate dispatch.\n\n#### Durable Ordering Invariants (REL-r2-1)\n\nP079 ordering is normative:\n\n- **The lease transition from `reserved` to `prompt_sent` MUST be durably committed to SQLite BEFORE the ACP prompt is dispatched.** Recovery treats `reserved` as not-yet-dispatched and may re-issue; recovery treats `prompt_sent` as already-dispatched and MUST NOT re-issue.\n- Terminal settlements happen in the same DB transaction as final output settlement and lease finalization. There is no observable state where dispatch occurred but the lease row remains `reserved`.\n- Cancellation precedes late output: once a cancellation terminal record is committed, any subsequent provider output is classified as `ignored_late_outputs` regardless of validity.\n- The source-generation claim check is gated by lease state: claim activity is re-checked at lease finalization in the same DB transaction that records terminal settlement.\n\nFixtures: `lease_commit_precedes_dispatch_under_crash`, `dispatch_without_lease_commit_invariant_violation` (negative; must fail the gate), and `cancellation_precedes_late_output`.\n\n#### Lease TTL and Stale-Owner Reconciliation (REL-r2-2)\n\nLease rows carry `lease_owner_principal_id`, `lease_acquired_at`, `lease_expires_at`, and a typed lease state machine. Default lease lifetimes:\n\n- Repair: `lease_lease_seconds = 180` (1.5x repair deadline).\n- Fallback: `lease_lease_seconds = 1200` (1.33x fallback deadline cap).\n\nReconciliation: a recovery sweep runs on daemon startup and on a deterministic cadence (every 60s). Any lease whose `lease_expires_at` is in the past is eligible for reclamation under a transactional compare-and-set. Reclamation transitions the lease to `settled` and emits a typed reliability evidence record. Reclamation rules:\n\n- A `reserved` lease past TTL reclaims to `result = unavailable`. Budget is NOT consumed because no prompt was dispatched.\n- A `prompt_sent` lease past TTL reclaims to `result = failed_transport`. Budget IS consumed because the provider may have acted.\n- Concurrent daemons attempting reclamation are resolved by the CAS; the loser observes the winning reclamation as a normal post-terminal read.\n- `output_contract_repair.v1.lease.expires_at` is exposed in operator readback so operators can spot stale leases.\n\nFixtures: `stale_lease_reclaimed_no_double_consume`, `concurrent_reclamation_single_winner`, `prompt_sent_lease_reclaimed_consumes_budget`.\n\n#### Auto-Retry Debounce (REL-r2-4)\n\nAuto-retry rollup of `budget_exhausted`, `unavailable`, and `p079_terminal_blocked` is debounced per `(parent_agent_execution_id, terminal_class)` with one emission per terminal classification per parent execution. The debounce resets on operator `manual_investigation` acknowledgement or on stage cancellation. The metric label lint forbids finer-grained debounce dimensions.\n\n#### Same-Session Repair Restart\n\nIf the daemon restarts while a lease is `reserved` and no provider prompt was sent (enforced by the durable-commit invariant above), recovery may reuse the lease and dispatch once. If it restarts after `prompt_sent` and before result collection, recovery must not re-prompt; it records `unavailable` or resumes only if the same session result is durably recoverable. This consumes the repair budget because provider work happened.\n\n#### Fallback Single-Flight Lease\n\nFallback dispatch is protected by a transactional single-flight lease keyed by `(run_id, stage_execution_id, parent_agent_execution_id, output_contract_repair.v1, frozen_fallback_policy_hash)`. The lease key uses the **frozen** policy hash captured at parent execution start in the `RunPlanSnapshot` (REL-r2-10), not a live `fallback_policy_id`, so policy drift between attempts cannot bypass single-flight. The lease is inserted before creating the fallback agent execution. Concurrent recovery paths that lose the insert report `lease_contended` and observe the existing child execution. The fallback child stores `parent_failed_agent_execution_id` and `parent_repair_evidence_path`; the parent evidence stores `fallback_agent_execution_id`.\n\n#### Fallback Lease-Insert Without Child Execution (REL-r2-3)\n\nIf a fallback lease exists with no corresponding child `agent_execution` row (crash window between insert and child creation), recovery completes the child creation idempotently using the lease key as the `parent_repair_evidence` anchor, then continues normal child execution. Budget remains unconsumed because no prompt has been dispatched. Fixture: `fallback_lease_without_child_recovers_idempotently`.\n\n#### Fallback Deadline Combination (REL-r2-5)\n\n`fallback_agent_execution_deadline_seconds = min(role_watchdog_seconds, policy_max_cap_seconds)`, with `policy_max_cap_seconds` initially 900. The resolved deadline is surfaced in `provider_fallback.deadline_seconds`. Fixture: `fallback_deadline_uses_min_of_role_and_policy`.\n\n#### Fallback Child Restart (REL-r2-6)\n\nFallback child execution recovers under normal agent execution recovery rules. Its lease state authorizes single-flight; its result on completion settles back to the parent evidence row via `parent_repair_evidence` linkage. Budget consumed by the parent at lease commit is not re-evaluated on child restart. Fallback children inherit parent cancellation when the parent is cancelled (no separate chase through linkage edges). Fixture: `fallback_child_restart_settles_back_to_parent`.\n\n#### Pre-ACK Transport Retry and Lost-ACK Duplicate (REL-r2-7)\n\nTransport failure before provider prompt acknowledgement does not consume the repair/fallback attempt, but is bounded by `infrastructure_retry_max = 2` within the same recovery evaluation with a fixed 250 ms backoff between attempts (rel-r3-n5); after that, the terminal result is `unavailable` for this execution. Transport failure after prompt acknowledgement consumes the attempt and records `failed_transport`.\n\nTo neutralize lost-ACK duplicate dispatch, each repair/fallback prompt carries an `idempotency_token` (random v4 UUID minted at lease commit). The Rust ACP transport tracks the most recent `idempotency_token` per session and refuses to redispatch a token that has already been written to the wire. Adapters that cannot honor adapter-side dedup commit `prompt_sent` on the earliest observable transport-level send signal (bytes transmitted), distinct from provider ack, documented per-adapter in `docs/reference/p079-adapter-idempotency.md`. Fixtures: `lost_ack_does_not_double_dispatch`, `adapter_without_idempotency_uses_transport_send_signal`.\n\n#### Cancellation\n\nOperator cancellation wins over repair and fallback. If cancellation is recorded before terminal P079 settlement, active settlement becomes `cancelled`, in-flight provider work is asked to stop through the existing cancellation path, and late output is ignored as `ignored_late_outputs`. If valid output was already committed before cancellation, the committed settlement remains audit history but the run cancellation state is still terminal. Cancellation evidence uses result `cancelled` and recommended action `cancel_acknowledged`.\n\nLease finalization on cancellation (REL-r2-8): on cancellation while lease is in `reserved` or `prompt_sent`, the lease moves to `settled` with result `cancelled` in the same DB transaction as the cancellation terminal record. Late provider output continues to be ignored as `ignored_late_outputs`. Fixture: `cancelled_lease_finalized_in_same_transaction`.\n\n#### Source-Generation Supersession Mid-Turn (REL-r2-11)\n\nSource-generation claim activity is checked before dispatch, mid-turn (on a polling cadence bounded to once every 5 seconds during the prompt window), and at settlement. If the claim is superseded mid-turn, late repair or fallback output is recorded as `superseded_ignored`, consumes the attempt if prompt dispatch occurred, and cannot update active artifact truth. On mid-turn supersession, the existing cancellation signal is dispatched to the provider on a best-effort basis to free provider time, and local wait is short-circuited. Classification remains `superseded_ignored`. Fixture: `mid_turn_supersession_cancels_provider_best_effort`.\n\n#### Budget Table\n\n| Outcome | Repair budget consumed | Fallback budget consumed | Lease final state | Notes |\n|---|---:|---:|---|---|\n| accepted | yes | yes | settled(accepted) | Output settled through validator. |\n| rejected_invalid | yes | yes | settled(rejected_invalid) | Atomic failed output set rejected. |\n| skipped_ineligible | no | no | settled(skipped_ineligible) | Approval, conflict, side-effect lane, disabled policy, or Junie risk skip. |\n| unavailable before prompt acknowledgement | no | no | settled(unavailable) | Bounded infra retry, then terminal unavailable for this execution. |\n| failed_transport after prompt acknowledgement | yes | yes | settled(failed_transport) | Provider may have acted. |\n| deadline_exceeded after prompt acknowledgement | yes | yes | settled(deadline_exceeded) | Watchdog maps to deadline_exceeded. |\n| cancelled after dispatch | yes | yes | settled(cancelled) | Cancellation terminal; late output ignored. |\n| superseded_ignored after dispatch | yes | yes | settled(superseded_ignored) | No active truth update. |\n| lease_contended | no | no | settled(lease_contended) on observer | Observer uses existing fallback child. |\n| budget_exhausted | no new consumption | no new consumption | settled(budget_exhausted) | Terminal classification after budget already spent. |\n| stale_lease_reclaimed_reserved | no | no | settled(unavailable, reclaimed) | TTL sweep reclamation, no prompt dispatched. |\n| stale_lease_reclaimed_prompt_sent | yes | yes | settled(failed_transport, reclaimed) | TTL sweep reclamation after dispatch. |\n| oversized_recovery_payload | no | no | settled(unavailable, oversized_payload) | Recovery bound exceeded. |\n| unattributable_envelope | no | no | settled(unavailable, unattributable_envelope) | Attribution failure. |\n| oversized_fallback_packet | no | no | settled(unavailable, oversized_fallback_packet) | Packet assembly failed closed. |\n| principal_revoked | no | no | settled(unavailable, principal_revoked) | Fallback principal revoked before child start. |\n| unsafe_continuation | yes | yes | settled(rejected_invalid, unsafe_continuation) | Repair/fallback turn requested non-allowlisted permission. |\n\n## Metrics\n\nAdoption metric: `p079_eligible_output_failures_recovered_percent`.\n\nOperational metrics are listed in `rollout_contract_v1.metrics`. Result label values must come from the evidence enums for the corresponding subobject. Labels must not include run ids, stage ids, agent execution ids, session ids, raw paths, prompts, transcript excerpts, provider session ids, operator text, artifact payload contents, or free-form failure strings.\n\nMetric label lint references the same closed enum table as the evidence schema for `provider_family` and `role` (sec-009). YAML typos or unknown future role values are rejected and emit `rollout_contract_lint_total{proposal_id=p079,failure_reason=metric_label_enum_drift}`.\n\n## Rollout\n\n1. Add the SQLite migration, typed DB repos, projection rebuild path (with bounded sweep), and feature flags with repair, fallback, and transcript recovery disabled by default.\n2. Add JSON schemas and parity fixtures for `output_contract_repair.v1`, `output_contract_repair_fallback_packet.v1`, GraphQL SDL, MCP, run report, old-run compatibility, and feature-disabled compatibility.\n3. Add deterministic fixture ACP tests for transcript/envelope recovery (including bounds and attribution), same-session repair, invalid repair rejection, Junie plan evidence (with redaction and meta-root-relative exposure), and role/provider fallback.\n4. Add reliability fixtures for durable-commit-precedes-dispatch invariant, lease TTL reclamation, lease-insert-without-child recovery, transport failure before and after prompt acknowledgement, lost-ACK idempotency, daemon restart in `reserved` and `prompt_sent`, fallback duplicate lease (with frozen-policy-hash), deadline combination, fallback child restart, cancellation lease finalization, supersession mid-turn cancel, and projection rebuild trigger.\n5. Add security fixtures for fallback packet redaction (secret_in_artifact, auth_principal, operator_rationale, absolute_path rewrite, oversized packet), recovery oversized payload, envelope forged attribution, plan-evidence redaction and meta-root-relative path, repair-turn permission posture (shell denial, fs.read denial, fs.write to canonical path allowed), repair prompt template pin and injection-marker redaction, symlink-swapped-after-check rejection, principal revocation, and metric label enum drift.\n6. Add the Swift readback DTO module, MainActor coalescing tests, decode fixtures, and the `p079-swift-readback` gate slice; require `./scripts/test-gate.sh build` to pass with the new module compiled.\n7. Enable same-session repair for proposal writer, proposal reviewer, and lead orchestrator behind `CHAINWORKS_P079_OUTPUT_REPAIR_ENABLED`.\n8. Enable transcript/provider-envelope recovery behind `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED` after parity, bounds, and attribution fixtures pass.\n9. Enable provider fallback only for roles with frozen YAML policy and `CHAINWORKS_P079_PROVIDER_FALLBACK_ENABLED`, and only after sec-001 fallback packet contract fixtures pass.\n10. Add security checker and side-effect-free prepush reviewer only after side-effect exclusion fixtures pass.\n11. Wire auto-retry rollups to consume P079 terminal classifications without dispatching actions, with the debounce window and reset conditions specified.\n12. Remove temporary provider-specific emergency reroutes once governed fallback policy covers the compatibility case.\n\nRollback disables the three feature flags, leaves migrated evidence readable, stops new lease creation, and returns eligible failures to pre-P079 blocked-stage behavior.\n\n## Inline rollout_contract_v1\n\nThe strict inline rollout contract is the top-level `rollout_contract_v1` object in this proposal JSON. It follows `docs/reference/executable-rollout-gate-template.md`, declares required applicability, includes a non-not-applicable migration description, names gate aliases and fixtures, and includes hold conditions for contract validation, canonical path binding, side-effect exclusion, single-flight fallback, restart behavior, durable-commit ordering, lease TTL, cancellation, fallback packet sanitization, recovery bounds, plan-evidence protection, repair-turn permission posture, principal binding, and metric cardinality.\n\n## Test Plan\n\nThe `proposal-079` and `p079` gates must run without live providers or network access. They cover: valid repair; invalid enum repair; wrong key and wrong path rejection; transcript recovery; provider-envelope recovery; oversized recovery payload rejected; unattributable envelope rejected; recovery attribution from transport ids only; repair failure followed by configured fallback success; no-policy fallback blocked; oversized fallback packet blocks dispatch; fallback packet redacts secrets, auth principal, operator rationale, rewrites absolute paths; principal revoked between dispatch and child start; Junie strict structured plan evidence with redaction and meta-root-relative exposure; lead orchestrator Gemini-to-Claude fallback; release lane exclusion; old-run readback; feature-disabled readback; GraphQL/MCP/run-report parity (including `status`, lease TTL, permission decisions, packet hash); migration rebuild and bounded sweep; metric label lint with enum reference; rollout-contract lint; durable-commit invariant; lost-ACK no double dispatch; transport failure; daemon crash in reserved and prompt_sent; duplicate fallback dispatch with policy drift; fallback lease without child recovery; repair deadline; fallback deadline min-of; cancellation lease finalization; source supersession mid-turn cancel; partial multi-output repair rejection; symlink swapped after check rejected; repair-turn shell permission request rejected; repair prompt template pin and injection-marker redaction; Swift DTO old-run, feature-disabled, recovered, blocked, cancelled, stale, orphan, lease-reclaimed, and unknown-enum decode fixtures.\n\n## Acceptance Criteria\n\n- P079 starts only after normal output collection fails or is unavailable.\n- Eligible roles receive at most one same-session repair turn and at most one provider fallback attempt.\n- Recovery, repair, and fallback outputs update active truth only through declared contract validation, exact canonical path binding, and active source-generation settlement.\n- `output_contract_repair.v1` has closed enums (including `status`), nullability, compatibility behavior, nested object shapes, and parity fixtures.\n- Fallback context packet is a closed v1 schema with redaction tier, size cap, content-addressed hash, and negative fixtures; lands before provider fallback flag enable.\n- Recovery has explicit numeric bounds, fail-closed truncation, transport-derived attribution, parser version pin, and negative fixtures.\n- Plan evidence is copied into a P079-owned 0700/0600 directory, redacted at copy time, size-capped, retention-bound, and exposed only as meta-root-relative paths.\n- Same-session repair turn uses the P079 permission posture; non-allowlisted permission requests are denied and terminate the turn with `unsafe_continuation`.\n- Lease transition `reserved -> prompt_sent` is durably committed before provider dispatch; recovery never re-prompts after `prompt_sent`.\n- Lease rows carry TTL and are reconciled by a deterministic sweep; reclamation budget rules are deterministic.\n- Fallback lease key uses the frozen policy hash; lost-ACK duplicate dispatch is neutralized by idempotency tokens or adapter-side transport send signal.\n- Fallback is bound to the failed execution's principal; revocation aborts fallback.\n- Fallback policy is YAML-declared, snapshot-frozen, drift-aware, feature-flag gated (with `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED` required when `transcript_recovery.enabled`), and never hardcoded as durable orchestrator behavior.\n- P079 readback is available through nested GraphQL (with SDL appendix), MCP, and run-report objects with Swift-friendly optional parent fields and a top-level `status` enum.\n- A compiled Swift DTO module decodes the new readback surface, old-run and feature-disabled cases, and unknown-enum fallback; the `p079-swift-readback` gate slice passes.\n- SQLite projections, leases, and fallback parent linkage make readback and restart recovery deterministic; projection rebuild trigger and bound are specified and fixtured.\n- Repair/fallback budget consumption, deadlines, transport outcomes, restart behavior, single-flight fallback, cancellation, supersession, and stale-lease reclamation are deterministic and fixture-proven.\n- Human approvals and workflow conflicts are never auto-resolved.\n- Release and durable side-effect lanes cannot use provider fallback.\n- Provider plan files are evidence only and never output truth.\n- The auto-retry ledger remains observe-only, debounced per (parent_agent_execution_id, terminal_class) with manual_investigation or stage cancellation reset.\n- The inline rollout contract passes lint and the P079 gate passes locally.\n\n## Risks and Mitigations\n\n| Risk | Mitigation |\n|---|---|\n| Repair prompt causes unrelated work | Narrow prompt, atomic acceptance, and server-side repair-turn permission posture limited to canonical output writes. |\n| Invalid output is accepted because it is close | Same validator, exact path binding, and source-generation settlement are mandatory. |\n| Provider fallback hides quality defects | Fallback is only for output-contract settlement failures and records typed evidence. |\n| Cross-provider data leak through fallback packet | Closed v1 schema, redaction tier, size cap, content-addressed hash, principal binding, and negative fixtures. |\n| Adversarial transcript/envelope feeds unbounded data into evidence | Streaming decoder with byte/depth/chunk caps; fail-closed `oversized_payload`; transport-derived attribution; parser version pin. |\n| Plan-evidence leakage of secrets or filesystem paths | Copy into 0700/0600 P079-owned directory with redactor; meta-root-relative readback. |\n| Repair turn obtains tool/network permissions outside contract | Server-side permission posture denies non-allowlisted requests; permission_decisions audit trail; unsafe_continuation terminal. |\n| Crash duplicates repair prompt | `reserved -> prompt_sent` durable commit invariant precedes dispatch. |\n| Lost ACK duplicates prompt | Adapter-side idempotency token or earliest-transport-send commit. |\n| Stale lease blocks stage indefinitely | TTL + scheduled reconciliation sweep with deterministic budget rules. |\n| Restart duplicates fallback | Transactional single-flight lease with frozen-policy-hash key; lease-without-child recovery. |\n| Hung repair blocks forever | Explicit repair deadline and watchdog mapping. |\n| Transient transport consumes all recovery | Pre-ack transport failures do not consume budget but are bounded by infra retry max. |\n| Cancellation races settlement | Cancellation terminal state wins; late output is ignored; lease finalized in same transaction. |\n| Plan files become transition truth | Plan evidence has `accepted_as_output=false`, negative fixtures, and meta-root-relative exposure. |\n| SwiftUI diverges from backend truth | App consumes typed projections only, decodes are fixture-tested at the gate, unknown enums map to conservative diagnostic state. |\n| Metrics leak sensitive or high-cardinality data | Metric label lint references closed enum table; rejects YAML drift. |\n\n## Open Questions\n\n1. Should manual MCP controls for operator-triggered repair or fallback be added after automatic P079 evidence is observed in dogfood runs?\n2. Should `docs_guardian` and broader `code_writer` fallback become P079 follow-up scope, or remain owned by P088/P095 completion repair work?\n3. Should transcript recovery require per-role opt-in beyond frozen policy, current-invocation attribution, and contract validation after initial rollout?\n\n## Reviewer Feedback Resolution\n\n| Backlog item | Pass | Resolution |\n|---|---|---|\n| api-contract-r2-001 | R3 | Added top-level `status` enum to `output_contract_repair.v1`, closed values, required-field placement, derivation rule, pre-P079 null behavior, and parity fixture coverage. |\n| api-contract-r2-002 | R3 | Required `CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED` in `feature_flags_required` whenever `transcript_recovery.enabled: true`; snapshot compilation rejects missing binding; readback surfaces `policy_feature_flags`. |\n| APPLE-001 | R3 | Added Swift readback DTO module, MainActor coalescing tests, decode fixtures, and `p079-swift-readback` gate slice required at acceptance. |\n| REL-r2-1 | R3 | Added normative durable-ordering invariant: `reserved -> prompt_sent` commits to SQLite before ACP dispatch; positive and negative fixtures. |\n| REL-r2-2 | R3 | Added `lease_lease_seconds`, `lease_owner_principal_id`, `lease_acquired_at`, `lease_expires_at`, reconciliation sweep, CAS-resolved concurrent reclamation, and budget rules per pre-/post-ack reclamation. |\n| sec-001 | R3 | Added `output_contract_repair_fallback_packet.v1` closed schema with redaction tier, size caps, content-addressed hash, principal binding, and negative fixtures; gated by feature flag enable. |\n| sec-002 | R3 | Added explicit byte/depth/chunk bounds, streaming fail-closed decoder, transport-allocated attribution, `recovery_parser_version` pin, and negative fixtures. |\n| sec-003 | R3 | Added P079-owned 0700/0600 plan-evidence directory, redactor, size cap, retention bound, and meta-root-relative readback. |\n| sec-004 | R3 | Added server-side repair-turn permission posture, `permission_decisions` audit trail, `unsafe_continuation` terminal, and fixtures; applies to fallback agent executions as well. |\n| **api-contract-r3-001** | **R4** | **Expanded `graphql_sdl_appendix` with complete SDL for every nested object type (`OutputContractRepairAttempt`, `OutputContractTranscriptRecovery`, `OutputContractProviderFallback`, `OutputContractPlanEvidence`, `OutputContractBudget`, `OutputContractLease`, `OutputContractPermissionDecision`, `RequiredOutputBinding`) and every referenced enum (`OutputContractRepairStatus`, `InitialFailureClass`, `InitialFailureSubtype`, `AdapterFamily`, `ProviderFamily`, `RequiredOutputMode`, `SameSessionRepairResult`, `TranscriptRecoveryResult`, `ProviderFallbackResult`, `RecoverySource`, `FinalOutputSettlement`, `RecommendedNextAction`, `PresentationCategory`, `LeaseState`, `LeaseReclamationReason`, `PermissionDecisionValue`, `PermissionResourceKind`), with nullability, list element nullability, scalar definitions, parent-field extension, v1 compatibility defaults, schema-version-suffixed fixture naming, and a per-lane enum evolution rule (closed GraphQL enums, snake_case MCP/run-report, Swift display-only unknownDiagnostic).** |\n| **api-contract-r3-002** | **R4** | **Added `sqlite_migration_appendix` with full table schemas for `output_contract_repair_events`, `output_contract_repair_leases`, and the new `output_contract_repair_fallback_parent_links` table: columns + types + nullability + defaults, primary keys, unique constraints (including the deterministic lease key single-flight uniqueness and at-most-one-fallback-child-per-parent), foreign keys with explicit ON DELETE behavior, indexes (status+updated_at, projection_integrity+stale_since, fallback-child reverse lookup, lease state+expiry sweep), CHECK constraints for closed enum columns and the dispatch-commit invariant, transactional invariants (durable-commit-before-dispatch, settled-with-projection-and-source-generation single transaction, CAS reclamation), projection_schema_version and per-JSON-payload embedded version for safe rollback/forward read compatibility, and a six-fixture migration suite. Rollback preserves rows readable through GraphQL/MCP/run-report and the Swift DTO module.** |\n| **APPLE-R3-001** | **R4** | **Pinned SwiftUI identity contract: stable identity is `(repair_attempt_id, agent_execution_id)` only. `evidence_version` is a monotonic content-version / refresh-invalidation field used by `Equatable` and view-body recomputation, NOT by `Identifiable.id` or `ForEach` keys. The Swift Client Migration section, `readback_contract.swift_client.presentation`, the new `readback_contract.swift_client.identity_contract` block, and the coalescing fixtures all agree: `reserved -> prompt_sent -> settled` replaces a single visible row; unchanged-`evidence_version` snapshot replay causes zero row churn; projection rebuild replay does not duplicate rows.** |\n| **api-contract-r4-001** | **R5** | **Removed `scalar ID` from `graphql_sdl_appendix.scalars`. `ID` is a GraphQL built-in scalar and is not redeclared in the SDL. The `scalar_types.ID` block is retained as documentation only (no SDL emit). A new `sdl_parity_assertions_api_contract_r4_002.no_redeclared_builtin_scalars` assertion in the parity fixture enforces that no built-in scalar (`ID`, `String`, `Int`, `Float`, `Boolean`) appears in the SDL `scalars` block.** |\n| **api-contract-r4-002** | **R5** | **Added the `provider_family` enum domain to `output_contract_repair_v1_schema.enums` with values `{codex, claude, gemini, junie, auggie, fixture}` (identical to `adapter_family` at v1). A new parity assertion in `sdl_parity_assertions_api_contract_r4_002` asserts GraphQL `ProviderFamily` maps byte-for-byte (after SCREAMING_SNAKE_CASE → snake_case projection) to the JSON `provider_family` value. The proposal also documents the semantic difference between `provider_family` (vendor) and `adapter_family` (local ACP adapter) and notes that future divergence requires a v2 schema bump.** |\n\n### Advisory follow-ups (non-blocking) addressed in R5\n\n- api-contract-r4-003 (fallback_policy_schema implementability), api-contract-r4-004 (lease key derivation parity with lease uniqueness), APPLE-R4-001/002 (closed vs raw-string fixture distinction, MainActor copy/reveal helper), rel-r4-n1..n5 (lease reclamation ceiling, fallback supersession, ungraceful crash, idempotency table durability, fallback fan-out queue), macos-r4-001..007 (menu-bar surface, keyboard shortcuts, UNUserNotificationCenter authorization, pasteboard types, unknownDiagnostic in projection table, Inspector container, Increase Contrast API) and ui_designer follow-ups (Inspector density, Refresh placement, background notification navigation, empty state) remain explicitly **non-blocking advisory** for this pass; they are not claimed as R5 blocker resolutions to keep `proposal_feedback_coverage` strict against the current `score_lift_backlog`. They are tracked for the next revision cycle.\n\n### Advisory follow-ups (non-blocking) addressed in R4\n\n- api-contract-r3-003 (enum evolution per lane): pinned in `graphql_sdl_appendix.enum_evolution_per_lane_api_contract_r3_003`.\n- api-contract-r3-004 (canonical-path exposure lane separation): pinned. Internal settlement uses absolute frozen canonical_path; readback uses run-meta-root-relative for plan evidence and evidence_artifact_path; metrics never expose canonical paths; `RequiredOutputBinding.canonicalPath` remains absolute because it is the contract-bound frozen path operators copy verbatim.\n- api-contract-r3-005 (fallback policy schema precision): `fallback_policy_schema` now declares required/optional fields, nested-object shapes, enum domains, duplicate handling, precedence, and `disabled_reason` vocabulary in `feature_flag_binding_rules`.\n- rel-r3-n1..rel-r3-n6 (sweep retry, projection abandonment ceiling, concurrency cap explicitness, multi-daemon scope, infrastructure retry backoff, shutdown drain): each addressed in 'Reliability Semantics' and `advisory_followups_addressed_r4`.\n- macos-r3-001..macos-r3-008 (status -> presentation_category projection, accessibility, date locale, pasteboard/keyboard, stale projection UX, UNUserNotification, identity-avoids-evidence_version, module layout): pinned in the new 'Status -> Presentation Category Projection' and 'Presentation Polish Contract' sections.\n- ui-001..ui-004 (loading visual, inspector grouping, plan evidence interactivity, unknownDiagnostic visual): pinned in 'Presentation Polish Contract'.\n",
  "rollout_contract_v1": {
    "schema_version": "rollout_contract_v1",
    "applicability": "required",
    "gate_aliases": [
      "proposal-079",
      "p079"
    ],
    "commands": {
      "allowlist": [
        "./scripts/test-gate.sh proposal-079",
        "./scripts/test-gate.sh p079",
        "./scripts/test-gate.sh p079-swift-readback",
        "./scripts/test-gate.sh build"
      ],
      "commentary": "Gate commands are declarative expectations; the linter does not execute them."
    },
    "migrations": {
      "not_applicable": false,
      "description": "P079 requires a SQLite migration named p079_output_contract_repair_v1 that creates output_contract_repair_events, output_contract_repair_leases (with TTL and owner principal columns), and output_contract_repair_fallback_parent_links. The migration is non-destructive and rollback preserves rows readable. Evidence artifacts and reports are rebuilt from the typed SQLite records, not used as the scheduling authority. Full table schemas, columns, primary keys, unique constraints, foreign keys, indexes, CHECK constraints, and transactional invariants are in the top-level sqlite_migration_appendix of this proposal JSON."
    },
    "metrics": {
      "adoption_metric": "p079_eligible_output_failures_recovered_percent",
      "operational_metrics": [
        "p079_output_repair_attempt_total{role,provider_family,failure_class,result}",
        "p079_transcript_recovery_total{role,recovery_source,result}",
        "p079_provider_fallback_attempt_total{role,failed_provider_family,fallback_provider_family,result}",
        "p079_provider_mode_mismatch_total{role,provider_family,subtype}",
        "p079_plan_evidence_only_total{role,provider_family}",
        "p079_plan_evidence_redaction_total{role,redaction_class}",
        "p079_invalid_repair_rejected_total{role,reason}",
        "p079_repair_budget_exhausted_total{role}",
        "p079_fallback_budget_exhausted_total{role}",
        "p079_repair_transport_outcome_total{role,result}",
        "p079_fallback_lease_total{role,result}",
        "p079_release_lane_exclusion_total{role,reason}",
        "p079_fallback_packet_assembly_total{role,result}",
        "p079_recovery_bound_exceeded_total{role,bound_kind}",
        "p079_unsafe_continuation_total{role,turn_kind,resource_kind}",
        "p079_lease_reclamation_total{lease_kind,prior_state,result}",
        "p079_principal_revoked_total{role}",
        "auto_retry_output_contract_classification_total{classification}",
        "recovery_sweep_total{kind,result}",
        "rollout_contract_lint_total{proposal_id,status,failure_reason}",
        "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}"
      ]
    },
    "readback_lanes": [
      "run_report",
      "mcp",
      "graphql"
    ],
    "readback_fields": [
      "rollout_contract_status",
      "rollout_contract_decision",
      "rollout_contract_failure_reasons",
      "rollout_contract_waiver_state",
      "rollout_contract_waiver_expires_at",
      "rollout_contract_enforcement_mode",
      "rollout_contract_enforcement_mode_reason",
      "rollout_contract_hold_conditions",
      "rollout_contract_rollback_disposition",
      "rollout_contract_source_lane",
      "rollout_contract_enabled_state",
      "rollout_contract_disabled_reason_code",
      "rollout_contract_action_id",
      "rollout_contract_operator_message",
      "rollout_contract_projection_integrity",
      "rollout_contract_cutover_policy_revision",
      "rollout_contract_diagnostic_redaction",
      "rollout_contract_next_steps",
      "output_contract_repair",
      "output_contract_repair.schema_version",
      "output_contract_repair.repair_attempt_id",
      "output_contract_repair.status",
      "output_contract_repair.evidence_version",
      "output_contract_repair.presentation_category",
      "output_contract_repair.initial_failure_class",
      "output_contract_repair.initial_failure_subtype",
      "output_contract_repair.required_outputs",
      "output_contract_repair.same_session_repair.result",
      "output_contract_repair.transcript_recovery.result",
      "output_contract_repair.transcript_recovery.recovery_source",
      "output_contract_repair.transcript_recovery.bytes_examined",
      "output_contract_repair.transcript_recovery.max_recovery_payload_bytes",
      "output_contract_repair.transcript_recovery.recovery_parser_version",
      "output_contract_repair.provider_fallback.result",
      "output_contract_repair.provider_fallback.fallback_profile",
      "output_contract_repair.provider_fallback.fallback_packet_hash",
      "output_contract_repair.provider_fallback.fallback_principal_id",
      "output_contract_repair.provider_fallback.fallback_principal_capability_hash",
      "output_contract_repair.provider_plan_evidence.paths",
      "output_contract_repair.provider_plan_evidence.redactions_applied",
      "output_contract_repair.permission_decisions",
      "output_contract_repair.lease.state",
      "output_contract_repair.lease.acquired_at",
      "output_contract_repair.lease.expires_at",
      "output_contract_repair.lease.lease_seconds",
      "output_contract_repair.policy_feature_flags",
      "output_contract_repair.repair_prompt_template_version",
      "output_contract_repair.final_output_settlement",
      "output_contract_repair.recommended_next_action"
    ],
    "readback_fixture": "docs/evidence/rollout-contract/operator-readback/p079-output-repair-full-surface.fixture.json",
    "operator_report_fields": [
      "rollout_contract_status",
      "rollout_contract_decision",
      "rollout_contract_failure_reasons",
      "rollout_contract_waiver_state",
      "rollout_contract_waiver_expires_at",
      "rollout_contract_enforcement_mode",
      "rollout_contract_enforcement_mode_reason",
      "rollout_contract_hold_conditions",
      "rollout_contract_rollback_disposition",
      "rollout_contract_source_lane",
      "rollout_contract_enabled_state",
      "rollout_contract_disabled_reason_code",
      "rollout_contract_action_id",
      "rollout_contract_operator_message",
      "rollout_contract_projection_integrity",
      "rollout_contract_projection_stale_since",
      "rollout_contract_cutover_policy_revision",
      "rollout_contract_diagnostic_redaction",
      "rollout_contract_next_steps",
      "output_contract_repair",
      "output_contract_repair.schema_version",
      "output_contract_repair.repair_attempt_id",
      "output_contract_repair.status",
      "output_contract_repair.evidence_version",
      "output_contract_repair.presentation_category",
      "output_contract_repair.initial_failure_class",
      "output_contract_repair.initial_failure_subtype",
      "output_contract_repair.required_outputs",
      "output_contract_repair.same_session_repair.result",
      "output_contract_repair.transcript_recovery.result",
      "output_contract_repair.transcript_recovery.recovery_source",
      "output_contract_repair.provider_fallback.result",
      "output_contract_repair.provider_fallback.fallback_profile",
      "output_contract_repair.provider_fallback.fallback_packet_hash",
      "output_contract_repair.provider_fallback.fallback_principal_id",
      "output_contract_repair.provider_plan_evidence.paths",
      "output_contract_repair.permission_decisions",
      "output_contract_repair.lease.state",
      "output_contract_repair.lease.expires_at",
      "output_contract_repair.policy_feature_flags",
      "output_contract_repair.repair_prompt_template_version",
      "output_contract_repair.transcript_recovery.recovery_parser_version",
      "output_contract_repair.final_output_settlement",
      "output_contract_repair.recommended_next_action",
      "run_id",
      "stage_execution_id",
      "agent_execution_id",
      "session_generation_id",
      "role",
      "provider_family",
      "adapter_family",
      "required_output_mode",
      "failed_output_names",
      "same_session_repair.turn_count",
      "same_session_repair.deadline_seconds",
      "transcript_recovery.recovery_source",
      "provider_fallback.fallback_agent_execution_id",
      "provider_fallback.parent_failed_agent_execution_id",
      "provider_fallback.deadline_seconds",
      "budget.repair_consumed",
      "budget.fallback_consumed",
      "lease.key",
      "lease.state",
      "lease.expires_at",
      "lease.owner_principal_id",
      "evidence_artifact_path"
    ],
    "hold_conditions": [
      "Any repaired, recovered, or fallback output accepted without passing the declared contract validator",
      "Any repaired, recovered, or fallback output accepted for a non-canonical or undeclared target path",
      "Any output accepted after source-generation claim supersession except as ignored_late_outputs",
      "Any human approval or workflow conflict auto-resolved by repair or fallback",
      "Any release, publish, upload, distribution, git push, or durable side-effect lane using provider fallback",
      "Any provider plan file accepted as required output or transition truth",
      "Any plan-evidence file retained without the P079 redaction pass or outside the P079-owned 0700/0600 directory",
      "Any output_contract_repair.provider_plan_evidence.paths value that exposes a path outside run meta-root or that has not been rewritten to meta-root-relative",
      "Any provider fallback executed without frozen snapshot fallback policy and enabled feature flag",
      "Any transcript_recovery policy with transcript_recovery.enabled=true that does not require CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED in feature_flags_required",
      "Any fallback dispatch not protected by the P079 single-flight lease keyed by frozen policy hash",
      "Any fallback dispatched with a context packet that exceeds the v1 size cap, omits required fields, fails redaction, or is not content-addressed in evidence",
      "Any fallback agent execution not bound to the failed execution's principal at dispatch",
      "Any transcript or provider-envelope recovery accepted without transport-allocated attribution",
      "Any transcript or provider-envelope recovery that parses beyond the v1 byte, depth, or chunk bounds",
      "Any daemon dispatch that occurs before the lease transition to prompt_sent is durably committed to SQLite",
      "Any daemon restart that re-prompts after a prompt_sent repair lease",
      "Any lease lacking lease_owner_principal_id, lease_acquired_at, and lease_expires_at",
      "Any stale lease that is not reclaimed by the recovery sweep within bounded time",
      "Any cancellation that settles active output after the cancellation terminal state is recorded",
      "Any cancellation that does not finalize the in-flight lease in the same DB transaction",
      "Any repair or fallback turn that received a non-allowlisted permission grant (any decision other than fs.write to the frozen canonical output path)",
      "Any output_contract_repair.v1 row that is missing permission_decisions when the repair or fallback turn was dispatched",
      "Any auto-retry ledger action dispatching repair, fallback, retry, approval, cancellation, continuation, or release work",
      "Any auto-retry debounce that emits more than one terminal classification per (parent_agent_execution_id, terminal_class) without an operator manual_investigation acknowledgement or stage cancellation reset",
      "Any P079 gate requiring live provider, network, or external service availability",
      "Any metric label containing run id, stage id, agent execution id, session id, raw path, prompt text, transcript text, provider session id, or operator text",
      "Any metric label whose value for provider_family or role is not in the closed enum table",
      "Any P079 evidence field or readback projection that exposes a raw provider session id rather than the daemon's session_generation_id",
      "Any output_repair_policies overlay enabling P079 for a role identifier not on the server-side allowlist for repair / fallback",
      "Any Swift readback DTO that fails to decode old-run, feature-disabled, recovered, blocked, cancelled, stale, orphan, lease-reclaimed, or unknown-enum fixture cases"
    ],
    "hold_conditions_detail": {
      "contract_validator_bypass": "All repaired, recovered, and fallback payloads must traverse declared-output validation and source-generation settlement.",
      "canonical_path_bypass": "Returned output paths must byte-match the frozen snapshot resolved path string; equivalent-looking macOS paths are rejected; materialization uses openat with O_NOFOLLOW per component.",
      "single_flight_bypass": "Fallback uses a transactional unique lease keyed by run, stage execution, parent agent execution, schema version, and frozen policy hash before child execution creation.",
      "durable_ordering_bypass": "Lease commit precedes ACP dispatch; recovery treats prompt_sent as do-not-redispatch.",
      "lease_liveness_bypass": "Leases carry TTL and a reconciliation sweep; stale leases are reclaimed deterministically.",
      "restart_reprompt": "After prompt_sent, recovery must not issue a second same-session repair prompt for the same parent execution.",
      "side_effect_lane_exclusion": "Release and external side-effect lanes remain owned by the durable side-effect ledger.",
      "fallback_packet_sanitization": "Fallback context packet is a closed v1 schema with redaction tier, size cap, content-addressed hash, and principal binding.",
      "recovery_bounds": "Recovery uses a streaming fail-closed decoder with byte/depth/chunk caps and transport-allocated attribution.",
      "plan_evidence_protection": "Plan evidence is copied into a P079-owned 0700/0600 directory, redacted, size-capped, retention-bound, and exposed meta-root-relative only.",
      "repair_turn_posture": "Repair and fallback turns run under a server-side permission posture allowlisting only fs.write to frozen canonical output paths.",
      "principal_binding": "Fallback inherits the failed execution's principal; revocation aborts fallback.",
      "auto_retry_observe_only": "The auto-retry ledger may classify P079 terminal states but remains observe-only, debounced per (parent_agent_execution_id, terminal_class).",
      "swift_client_decode": "The macOS app's DTO module decodes the v1 readback surface and unknown-enum future values, verified at the proposal-079 gate."
    },
    "rollback_disposition": {
      "mode": "feature_flag_disable_keep_evidence_readback",
      "data_loss_risk": "none",
      "steps": [
        "Disable CHAINWORKS_P079_OUTPUT_REPAIR_ENABLED, CHAINWORKS_P079_PROVIDER_FALLBACK_ENABLED, and CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED.",
        "Keep output_contract_repair.v1 SQLite rows and rebuilt evidence artifacts readable through run report, MCP, and GraphQL.",
        "Stop scheduling new repair and fallback leases while leaving existing source-generation and artifact-settlement history untouched.",
        "Allow the recovery sweep to continue reclaiming stale leases so feature-disabled runs do not leave hanging rows.",
        "Return eligible output failures to pre-P079 blocked-stage behavior.",
        "Re-enable only after proposal-079 and p079 gates pass."
      ]
    },
    "decision_vocabulary": [
      "pass",
      "fail",
      "waived",
      "not_applicable",
      "timeout",
      "cancelled",
      "missing_contract",
      "tamper_detected",
      "stale"
    ],
    "negative_fixtures": {
      "repaired_output_wrong_path_rejected": "docs/evidence/rollout-contract/p079/negative/repaired-output-wrong-path-rejected.json",
      "repair_after_source_supersession_ignored": "docs/evidence/rollout-contract/p079/negative/repair-after-source-supersession-ignored.json",
      "human_approval_blocks_repair": "docs/evidence/rollout-contract/p079/negative/human-approval-blocks-repair.json",
      "workflow_conflict_blocks_repair": "docs/evidence/rollout-contract/p079/negative/workflow-conflict-blocks-repair.json",
      "release_lane_fallback_rejected": "docs/evidence/rollout-contract/p079/negative/release-lane-fallback-rejected.json",
      "provider_plan_file_not_output": "docs/evidence/rollout-contract/p079/negative/provider-plan-file-not-output.json",
      "fallback_without_policy_rejected": "docs/evidence/rollout-contract/p079/negative/fallback-without-policy-rejected.json",
      "fallback_duplicate_lease_rejected": "docs/evidence/rollout-contract/p079/negative/fallback-duplicate-lease-rejected.json",
      "fallback_policy_drift_single_flight_preserved": "docs/evidence/rollout-contract/p079/negative/fallback-policy-drift-single-flight-preserved.json",
      "fallback_lease_without_child_recovers_idempotently": "docs/evidence/rollout-contract/p079/negative/fallback-lease-without-child-recovers-idempotently.json",
      "fallback_deadline_uses_min_of_role_and_policy": "docs/evidence/rollout-contract/p079/negative/fallback-deadline-uses-min-of-role-and-policy.json",
      "fallback_child_restart_settles_back_to_parent": "docs/evidence/rollout-contract/p079/negative/fallback-child-restart-settles-back-to-parent.json",
      "repair_prompt_sent_restart_no_reprompt": "docs/evidence/rollout-contract/p079/negative/repair-prompt-sent-restart-no-reprompt.json",
      "lease_commit_precedes_dispatch_under_crash": "docs/evidence/rollout-contract/p079/negative/lease-commit-precedes-dispatch-under-crash.json",
      "dispatch_without_lease_commit_invariant_violation": "docs/evidence/rollout-contract/p079/negative/dispatch-without-lease-commit-invariant-violation.json",
      "stale_lease_reclaimed_no_double_consume": "docs/evidence/rollout-contract/p079/negative/stale-lease-reclaimed-no-double-consume.json",
      "concurrent_reclamation_single_winner": "docs/evidence/rollout-contract/p079/negative/concurrent-reclamation-single-winner.json",
      "prompt_sent_lease_reclaimed_consumes_budget": "docs/evidence/rollout-contract/p079/negative/prompt-sent-lease-reclaimed-consumes-budget.json",
      "lost_ack_does_not_double_dispatch": "docs/evidence/rollout-contract/p079/negative/lost-ack-does-not-double-dispatch.json",
      "adapter_without_idempotency_uses_transport_send_signal": "docs/evidence/rollout-contract/p079/negative/adapter-without-idempotency-uses-transport-send-signal.json",
      "repair_deadline_maps_to_budget": "docs/evidence/rollout-contract/p079/negative/repair-deadline-maps-to-budget.json",
      "repair_cancelled_no_late_settlement": "docs/evidence/rollout-contract/p079/negative/repair-cancelled-no-late-settlement.json",
      "cancelled_lease_finalized_in_same_transaction": "docs/evidence/rollout-contract/p079/negative/cancelled-lease-finalized-in-same-transaction.json",
      "mid_turn_supersession_cancels_provider_best_effort": "docs/evidence/rollout-contract/p079/negative/mid-turn-supersession-cancels-provider-best-effort.json",
      "transcript_recovery_oversized_payload_rejected": "docs/evidence/rollout-contract/p079/negative/transcript-recovery-oversized-payload-rejected.json",
      "provider_envelope_forged_attribution_rejected": "docs/evidence/rollout-contract/p079/negative/provider-envelope-forged-attribution-rejected.json",
      "recovery_attribution_transport_only": "docs/evidence/rollout-contract/p079/negative/recovery-attribution-transport-only.json",
      "fallback_packet_oversized_blocks_dispatch": "docs/evidence/rollout-contract/p079/negative/fallback-packet-oversized-blocks-dispatch.json",
      "fallback_packet_redaction_in_artifact": "docs/evidence/rollout-contract/p079/negative/fallback-packet-redaction-in-artifact.json",
      "fallback_packet_absolute_path_rewritten": "docs/evidence/rollout-contract/p079/negative/fallback-packet-absolute-path-rewritten.json",
      "principal_revoked_aborts_fallback": "docs/evidence/rollout-contract/p079/negative/principal-revoked-aborts-fallback.json",
      "plan_evidence_redaction_pattern": "docs/evidence/rollout-contract/p079/negative/plan-evidence-redaction-pattern.json",
      "plan_evidence_outside_root_dropped": "docs/evidence/rollout-contract/p079/negative/plan-evidence-outside-root-dropped.json",
      "plan_evidence_path_rewritten_relative": "docs/evidence/rollout-contract/p079/negative/plan-evidence-path-rewritten-relative.json",
      "repair_turn_shell_permission_request_rejected": "docs/evidence/rollout-contract/p079/negative/repair-turn-shell-permission-request-rejected.json",
      "repair_turn_fs_write_canonical_allowed": "docs/evidence/rollout-contract/p079/negative/repair-turn-fs-write-canonical-allowed.json",
      "repair_prompt_template_pinned": "docs/evidence/rollout-contract/p079/negative/repair-prompt-template-pinned.json",
      "repair_prompt_injection_marker_redacted": "docs/evidence/rollout-contract/p079/negative/repair-prompt-injection-marker-redacted.json",
      "symlink_swapped_after_check_rejected": "docs/evidence/rollout-contract/p079/negative/symlink-swapped-after-check-rejected.json",
      "transcript_recovery_flag_missing_rejected": "docs/evidence/rollout-contract/p079/negative/transcript-recovery-flag-missing-rejected.json",
      "auto_retry_observe_only_no_dispatch": "docs/evidence/rollout-contract/p079/negative/auto-retry-observe-only-no-dispatch.json",
      "auto_retry_debounce_one_per_parent_terminal": "docs/evidence/rollout-contract/p079/negative/auto-retry-debounce-one-per-parent-terminal.json",
      "metric_label_cardinality_violation": "docs/evidence/rollout-contract/p079/negative/metric-label-cardinality-violation.json",
      "metric_label_enum_drift_rejected": "docs/evidence/rollout-contract/p079/negative/metric-label-enum-drift-rejected.json",
      "swift_dto_old_run_decodes_null": "docs/evidence/rollout-contract/p079/swift/old-run-decodes-null.json",
      "swift_dto_feature_disabled_decodes_null": "docs/evidence/rollout-contract/p079/swift/feature-disabled-decodes-null.json",
      "swift_dto_unknown_enum_maps_to_diagnostic": "docs/evidence/rollout-contract/p079/swift/unknown-enum-maps-to-diagnostic.json",
      "swift_dto_lease_state_transitions_replace_row": "docs/evidence/rollout-contract/p079/swift/lease-state-transitions-replace-row.json"
    }
  },
  "output_contract_repair_v1_schema": {
    "schema_version": {
      "type": "string",
      "const": "output_contract_repair.v1"
    },
    "additional_properties": false,
    "closed_for_v1": true,
    "evolution_policy": "Consumers fail closed on unknown required fields and unknown enum values in v1. Additive optional fields require output_contract_repair.v2 or an explicit compatibility note and fixtures. Missing optional nested objects decode to null; missing arrays decode to empty arrays only where noted. The Swift presentation layer is permitted to map an unknown presentation_category or recommended_next_action value to a conservative unknownDiagnostic badge for display only; this never authorizes output settlement.",
    "identifier_generation": {
      "repair_attempt_id": "random_v4_uuid",
      "fallback_agent_execution_id": "random_v4_uuid",
      "lease_key": "deterministic_hash(run_id, stage_execution_id, parent_agent_execution_id, schema_version, frozen_fallback_policy_hash)",
      "rationale": "Random UUIDs are unguessable across runs; lease keys are deterministic for transactional single-flight but non-enumerable without all components."
    },
    "enums": {
      "status": [
        "not_attempted",
        "in_progress",
        "recovered",
        "blocked",
        "skipped",
        "cancelled",
        "failed"
      ],
      "initial_failure_class": [
        "no_output_produced",
        "empty_output",
        "missing_required_outputs",
        "invalid_required_outputs",
        "output_contract_mismatch",
        "provider_mode_mismatch"
      ],
      "initial_failure_subtype": [
        null,
        "plan_event_instead_of_output",
        "empty_submit_after_plan",
        "file_plan_written_instead_of_payload",
        "repair_repeated_plan_behavior",
        "malformed_envelope",
        "wrong_output_key",
        "wrong_channel",
        "wrong_canonical_path",
        "unknown_enum_value",
        "missing_required_field",
        "unsafe_continuation",
        "oversized_payload",
        "unattributable_envelope",
        "oversized_fallback_packet",
        "principal_revoked",
        "transcript_recovery_flag_missing"
      ],
      "adapter_family": [
        "codex",
        "claude",
        "gemini",
        "junie",
        "auggie",
        "fixture"
      ],
      "provider_family": [
        "codex",
        "claude",
        "gemini",
        "junie",
        "auggie",
        "fixture"
      ],
      "required_output_mode": [
        "strict_structured",
        "chainworks_output",
        "file_artifact",
        "status_artifact"
      ],
      "same_session_repair_result": [
        "not_needed",
        "accepted",
        "rejected_invalid",
        "unavailable",
        "skipped_ineligible",
        "failed_transport",
        "deadline_exceeded",
        "cancelled",
        "budget_exhausted",
        "superseded_ignored"
      ],
      "transcript_recovery_result": [
        "not_needed",
        "accepted",
        "rejected_invalid",
        "unavailable",
        "skipped_ineligible",
        "failed_transport",
        "cancelled"
      ],
      "provider_fallback_result": [
        "not_needed",
        "scheduled",
        "accepted",
        "rejected_invalid",
        "unavailable",
        "skipped_ineligible",
        "failed_transport",
        "deadline_exceeded",
        "cancelled",
        "budget_exhausted",
        "lease_contended",
        "superseded_ignored"
      ],
      "recovery_source": [
        null,
        "transcript",
        "provider_envelope"
      ],
      "final_output_settlement": [
        "valid_outputs_from_completed_execution",
        "valid_outputs_from_repair",
        "valid_outputs_from_transcript_recovery",
        "valid_outputs_from_provider_envelope",
        "valid_outputs_from_fallback",
        "blocked_missing_required_outputs",
        "blocked_invalid_required_outputs",
        "blocked_provider_mode_mismatch",
        "ignored_late_outputs",
        "cancelled",
        "failed_transport",
        "deadline_exceeded"
      ],
      "recommended_next_action": [
        "continue",
        "inspect_repair_evidence",
        "configure_fallback_policy",
        "operator_resolve_approval",
        "operator_resolve_workflow_conflict",
        "retry_after_transport_restored",
        "cancel_acknowledged",
        "manual_investigation"
      ],
      "presentation_category": [
        "informational",
        "recovered",
        "blocked",
        "skipped",
        "failed",
        "cancelled"
      ],
      "lease_state": [
        "reserved",
        "prompt_sent",
        "settled"
      ],
      "lease_reclamation_reason": [
        null,
        "ttl_expired_reserved",
        "ttl_expired_prompt_sent",
        "cancellation",
        "supersession",
        "principal_revoked"
      ],
      "permission_decision": [
        "allowed",
        "denied"
      ],
      "permission_resource_kind": [
        "fs_write_canonical_output_path",
        "fs_write_other",
        "fs_read",
        "shell",
        "network",
        "tool_custom",
        "tool_mcp"
      ]
    },
    "required_fields": [
      "schema_version",
      "repair_attempt_id",
      "run_id",
      "stage_execution_id",
      "agent_execution_id",
      "session_generation_id",
      "role",
      "provider_family",
      "adapter_family",
      "required_output_mode",
      "initial_failure_class",
      "required_outputs",
      "same_session_repair",
      "transcript_recovery",
      "provider_fallback",
      "provider_plan_evidence",
      "budget",
      "lease",
      "permission_decisions",
      "policy_feature_flags",
      "repair_prompt_template_version",
      "status",
      "evidence_version",
      "final_output_settlement",
      "recommended_next_action",
      "presentation_category",
      "recorded_at"
    ],
    "nested_objects": {
      "same_session_repair": {
        "additional_properties": false,
        "required_fields": ["result", "turn_count", "deadline_seconds"],
        "optional_fields": ["dispatched_at", "settled_at", "subtype", "idempotency_token"]
      },
      "transcript_recovery": {
        "additional_properties": false,
        "required_fields": ["result", "max_recovery_payload_bytes", "max_json_depth", "max_chunks_examined", "recovery_parser_version"],
        "optional_fields": ["recovery_source", "bytes_examined", "chunks_examined", "subtype", "dispatched_at", "settled_at"]
      },
      "provider_fallback": {
        "additional_properties": false,
        "required_fields": ["result", "deadline_seconds"],
        "optional_fields": ["fallback_profile", "fallback_agent_execution_id", "parent_failed_agent_execution_id", "fallback_packet_hash", "fallback_principal_id", "fallback_principal_capability_hash", "subtype", "dispatched_at", "settled_at"]
      },
      "provider_plan_evidence": {
        "additional_properties": false,
        "required_fields": ["paths", "accepted_as_output", "redactions_applied", "truncated_at_cap"],
        "optional_fields": ["per_file_size_cap_bytes", "per_execution_size_cap_bytes"]
      },
      "budget": {
        "additional_properties": false,
        "required_fields": ["repair_consumed", "fallback_consumed", "repair_max_per_invocation", "fallback_max_per_invocation"]
      },
      "lease": {
        "additional_properties": false,
        "required_fields": ["key", "state", "acquired_at", "expires_at", "lease_seconds", "owner_principal_id"],
        "optional_fields": ["reclamation_reason", "reclaimed_at"]
      },
      "permission_decisions": {
        "kind": "array",
        "items": {
          "additional_properties": false,
          "required_fields": ["method", "resource_kind", "decision"],
          "optional_fields": ["reason"]
        },
        "missing_decode": "empty_array"
      },
      "policy_feature_flags": {
        "kind": "array_of_strings"
      },
      "required_outputs": {
        "kind": "array",
        "items": {
          "additional_properties": false,
          "required_fields": ["name", "contract_id", "canonical_path"]
        }
      }
    }
  },
  "fallback_context_packet_v1_schema": {
    "schema_version": {
      "type": "string",
      "const": "output_contract_repair_fallback_packet.v1"
    },
    "additional_properties": false,
    "closed_for_v1": true,
    "required_fields": [
      "schema_version",
      "validation_failure_class",
      "validation_failure_subtype",
      "required_output_names",
      "required_output_contract_ids",
      "required_output_canonical_paths_relative",
      "prior_attempt_summary",
      "repair_prompt_template_version",
      "recovery_parser_version"
    ],
    "forbidden_fields": [
      "operator_rationale",
      "operator_instruction",
      "approval_rationale",
      "workflow_conflict_decision",
      "raw_secrets",
      "auth_token",
      "principal_id",
      "absolute_paths_outside_meta_root"
    ],
    "redaction_classes": [
      "environment_value",
      "absolute_path_outside_meta_root",
      "chainworks_mcp_token",
      "authorization_header",
      "provider_token_prefix_sk",
      "provider_token_prefix_sk_ant",
      "provider_token_prefix_aiza",
      "provider_token_prefix_anth",
      "codex_auth_json_fragment"
    ],
    "size_caps": {
      "total_packet_bytes": 32768,
      "per_string_bytes": 4096
    },
    "size_cap_failure_mode": "fail_closed_no_dispatch",
    "content_addressing": {
      "algorithm": "sha256",
      "bound_to_evidence_field": "output_contract_repair.v1.provider_fallback.fallback_packet_hash"
    }
  },
  "fallback_policy_schema": {
    "yaml_key": "output_repair_policies",
    "schema_version": 1,
    "additional_properties": false,
    "duplicate_handling": "policy ids must be unique within a snapshot; duplicates cause snapshot compile to fail with reason 'duplicate_policy_id'.",
    "precedence_rules": "at most one policy per role_family_match; multiple matches cause snapshot compile to fail with reason 'multiple_policies_for_role_family'.",
    "snapshot_behavior": "Compiled into RunPlanSnapshot with a policy hash per role (frozen_fallback_policy_hash). Resume compares frozen policy hash to current YAML and reports drift; frozen runs execute only the frozen policy unless an audited compatibility overlay is explicitly attached. Lease keys bind to the frozen hash, not a live id.",
    "feature_flag_binding_rules": [
      "If repair.enabled is true, feature_flags_required MUST include CHAINWORKS_P079_OUTPUT_REPAIR_ENABLED.",
      "If transcript_recovery.enabled is true, feature_flags_required MUST include CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED.",
      "If provider_fallback.enabled is true, feature_flags_required MUST include CHAINWORKS_P079_PROVIDER_FALLBACK_ENABLED.",
      "Missing required binding causes snapshot compile to fail with disabled_reason='<flag_name>_missing'."
    ],
    "role_allowlist_for_fallback": [
      "proposal_writer",
      "proposal_reviewer",
      "lead_orchestrator",
      "security_checker",
      "side_effect_free_prepush_reviewer"
    ],
    "example": {
      "output_repair_policies": [
        {
          "id": "p079_lead_orchestrator_output_contract_fallback",
          "schema_version": 1,
          "role_family_match": "lead_orchestrator",
          "failure_classes": [
            "missing_required_outputs",
            "invalid_required_outputs",
            "output_contract_mismatch",
            "provider_mode_mismatch"
          ],
          "allowed_output_modes": [
            "strict_structured",
            "chainworks_output"
          ],
          "repair": {
            "enabled": true,
            "max_same_session_turns_per_invocation": 1
          },
          "transcript_recovery": {
            "enabled": true,
            "sources": [
              "transcript",
              "provider_envelope"
            ]
          },
          "provider_fallback": {
            "enabled": true,
            "max_attempts_per_invocation": 1,
            "failed_backend_profile": "gemini_reasoning_pro_high",
            "fallback_backend_profile": "claude_orchestrator_high",
            "side_effect_lanes_excluded": true
          },
          "feature_flags_required": [
            "CHAINWORKS_P079_OUTPUT_REPAIR_ENABLED",
            "CHAINWORKS_P079_TRANSCRIPT_RECOVERY_ENABLED",
            "CHAINWORKS_P079_PROVIDER_FALLBACK_ENABLED"
          ],
          "disabled_reason_if_missing": "policy_not_in_snapshot"
        }
      ]
    }
  },
  "graphql_sdl_appendix": {
    "owner_crate": "control-plane/crates/graphql-server",
    "completeness_note": "Every type, enum, and scalar referenced by OutputContractRepairEvidence is defined below. Field nullability matches output_contract_repair_v1_schema; list element nullability is non-null inside non-null lists. The SDL is the authoritative contract for generated GraphQL clients and the parity fixture validates it against the JSON evidence object and Swift DTO mapping.",
    "scalars": "scalar DateTime\n",
    "scalars_note_api_contract_r4_001": "Only custom scalars are emitted in the SDL. `ID` is a GraphQL built-in scalar and MUST NOT be redeclared; the SDL parity fixture asserts that no built-in scalar (ID, String, Int, Float, Boolean) appears in this `scalars` block. The fixture file `docs/evidence/rollout-contract/p079/api/graphql-sdl-parity.fixture.json` includes a `no_redeclared_builtin_scalars` assertion.",
    "object_types_sdl": "type OutputContractRepairEvidence {\n  schemaVersion: String!\n  repairAttemptId: ID!\n  runId: ID!\n  stageExecutionId: ID!\n  agentExecutionId: ID!\n  sessionGenerationId: ID!\n  role: String!\n  providerFamily: ProviderFamily!\n  adapterFamily: AdapterFamily!\n  requiredOutputMode: RequiredOutputMode!\n  initialFailureClass: InitialFailureClass!\n  initialFailureSubtype: InitialFailureSubtype\n  requiredOutputs: [RequiredOutputBinding!]!\n  sameSessionRepair: OutputContractRepairAttempt!\n  transcriptRecovery: OutputContractTranscriptRecovery!\n  providerFallback: OutputContractProviderFallback!\n  providerPlanEvidence: OutputContractPlanEvidence!\n  budget: OutputContractBudget!\n  lease: OutputContractLease!\n  permissionDecisions: [OutputContractPermissionDecision!]!\n  policyFeatureFlags: [String!]!\n  repairPromptTemplateVersion: String!\n  status: OutputContractRepairStatus!\n  evidenceVersion: Int!\n  finalOutputSettlement: FinalOutputSettlement!\n  recommendedNextAction: RecommendedNextAction!\n  presentationCategory: PresentationCategory!\n  recordedAt: DateTime!\n}\n\ntype OutputContractRepairAttempt {\n  result: SameSessionRepairResult!\n  turnCount: Int!\n  deadlineSeconds: Int!\n  dispatchedAt: DateTime\n  settledAt: DateTime\n  subtype: InitialFailureSubtype\n  idempotencyToken: String\n}\n\ntype OutputContractTranscriptRecovery {\n  result: TranscriptRecoveryResult!\n  maxRecoveryPayloadBytes: Int!\n  maxJsonDepth: Int!\n  maxChunksExamined: Int!\n  recoveryParserVersion: String!\n  recoverySource: RecoverySource\n  bytesExamined: Int\n  chunksExamined: Int\n  subtype: InitialFailureSubtype\n  dispatchedAt: DateTime\n  settledAt: DateTime\n}\n\ntype OutputContractProviderFallback {\n  result: ProviderFallbackResult!\n  deadlineSeconds: Int!\n  fallbackProfile: String\n  fallbackAgentExecutionId: ID\n  parentFailedAgentExecutionId: ID\n  fallbackPacketHash: String\n  fallbackPrincipalId: ID\n  fallbackPrincipalCapabilityHash: String\n  subtype: InitialFailureSubtype\n  dispatchedAt: DateTime\n  settledAt: DateTime\n}\n\ntype OutputContractPlanEvidence {\n  paths: [String!]!\n  acceptedAsOutput: Boolean!\n  redactionsApplied: [String!]!\n  truncatedAtCap: Boolean!\n  perFileSizeCapBytes: Int\n  perExecutionSizeCapBytes: Int\n}\n\ntype OutputContractBudget {\n  repairConsumed: Boolean!\n  fallbackConsumed: Boolean!\n  repairMaxPerInvocation: Int!\n  fallbackMaxPerInvocation: Int!\n}\n\ntype OutputContractLease {\n  key: String!\n  state: LeaseState!\n  acquiredAt: DateTime!\n  expiresAt: DateTime!\n  leaseSeconds: Int!\n  ownerPrincipalId: ID!\n  reclamationReason: LeaseReclamationReason\n  reclaimedAt: DateTime\n}\n\ntype OutputContractPermissionDecision {\n  method: String!\n  resourceKind: PermissionResourceKind!\n  decision: PermissionDecisionValue!\n  reason: String\n}\n\ntype RequiredOutputBinding {\n  name: String!\n  contractId: String!\n  canonicalPath: String!\n}\n",
    "enum_types_sdl": "enum OutputContractRepairStatus { NOT_ATTEMPTED IN_PROGRESS RECOVERED BLOCKED SKIPPED CANCELLED FAILED }\nenum InitialFailureClass { NO_OUTPUT_PRODUCED EMPTY_OUTPUT MISSING_REQUIRED_OUTPUTS INVALID_REQUIRED_OUTPUTS OUTPUT_CONTRACT_MISMATCH PROVIDER_MODE_MISMATCH }\nenum InitialFailureSubtype { PLAN_EVENT_INSTEAD_OF_OUTPUT EMPTY_SUBMIT_AFTER_PLAN FILE_PLAN_WRITTEN_INSTEAD_OF_PAYLOAD REPAIR_REPEATED_PLAN_BEHAVIOR MALFORMED_ENVELOPE WRONG_OUTPUT_KEY WRONG_CHANNEL WRONG_CANONICAL_PATH UNKNOWN_ENUM_VALUE MISSING_REQUIRED_FIELD UNSAFE_CONTINUATION OVERSIZED_PAYLOAD UNATTRIBUTABLE_ENVELOPE OVERSIZED_FALLBACK_PACKET PRINCIPAL_REVOKED TRANSCRIPT_RECOVERY_FLAG_MISSING }\nenum AdapterFamily { CODEX CLAUDE GEMINI JUNIE AUGGIE FIXTURE }\nenum ProviderFamily { CODEX CLAUDE GEMINI JUNIE AUGGIE FIXTURE }\nenum RequiredOutputMode { STRICT_STRUCTURED CHAINWORKS_OUTPUT FILE_ARTIFACT STATUS_ARTIFACT }\nenum SameSessionRepairResult { NOT_NEEDED ACCEPTED REJECTED_INVALID UNAVAILABLE SKIPPED_INELIGIBLE FAILED_TRANSPORT DEADLINE_EXCEEDED CANCELLED BUDGET_EXHAUSTED SUPERSEDED_IGNORED }\nenum TranscriptRecoveryResult { NOT_NEEDED ACCEPTED REJECTED_INVALID UNAVAILABLE SKIPPED_INELIGIBLE FAILED_TRANSPORT CANCELLED }\nenum ProviderFallbackResult { NOT_NEEDED SCHEDULED ACCEPTED REJECTED_INVALID UNAVAILABLE SKIPPED_INELIGIBLE FAILED_TRANSPORT DEADLINE_EXCEEDED CANCELLED BUDGET_EXHAUSTED LEASE_CONTENDED SUPERSEDED_IGNORED }\nenum RecoverySource { TRANSCRIPT PROVIDER_ENVELOPE }\nenum FinalOutputSettlement { VALID_OUTPUTS_FROM_COMPLETED_EXECUTION VALID_OUTPUTS_FROM_REPAIR VALID_OUTPUTS_FROM_TRANSCRIPT_RECOVERY VALID_OUTPUTS_FROM_PROVIDER_ENVELOPE VALID_OUTPUTS_FROM_FALLBACK BLOCKED_MISSING_REQUIRED_OUTPUTS BLOCKED_INVALID_REQUIRED_OUTPUTS BLOCKED_PROVIDER_MODE_MISMATCH IGNORED_LATE_OUTPUTS CANCELLED FAILED_TRANSPORT DEADLINE_EXCEEDED }\nenum RecommendedNextAction { CONTINUE INSPECT_REPAIR_EVIDENCE CONFIGURE_FALLBACK_POLICY OPERATOR_RESOLVE_APPROVAL OPERATOR_RESOLVE_WORKFLOW_CONFLICT RETRY_AFTER_TRANSPORT_RESTORED CANCEL_ACKNOWLEDGED MANUAL_INVESTIGATION }\nenum PresentationCategory { INFORMATIONAL RECOVERED BLOCKED SKIPPED FAILED CANCELLED }\nenum LeaseState { RESERVED PROMPT_SENT SETTLED }\nenum LeaseReclamationReason { TTL_EXPIRED_RESERVED TTL_EXPIRED_PROMPT_SENT CANCELLATION SUPERSESSION PRINCIPAL_REVOKED }\nenum PermissionDecisionValue { ALLOWED DENIED }\nenum PermissionResourceKind { FS_WRITE_CANONICAL_OUTPUT_PATH FS_WRITE_OTHER FS_READ SHELL NETWORK TOOL_CUSTOM TOOL_MCP }\n",
    "parent_field_sdl": "extend type AgentExecution {\n  outputContractRepair: OutputContractRepairEvidence\n}\n",
    "nullability_rules": "Run.stageExecutions.agentExecutions.outputContractRepair is nullable for pre-P079 runs, feature-disabled runs, and executions with no output-contract failure. Nested object fields (sameSessionRepair, transcriptRecovery, providerFallback, providerPlanEvidence, budget, lease) are non-null when the parent is non-null because the v1 evidence row always materializes them with their NOT_NEEDED / default-empty values. Optional scalar fields (recoverySource, fallbackProfile, idempotencyToken, subtype, reclamationReason, fallbackPacketHash, fallbackPrincipalId, fallbackPrincipalCapabilityHash, reason, perFileSizeCapBytes, perExecutionSizeCapBytes, bytesExamined, chunksExamined, dispatchedAt, settledAt, reclaimedAt) use null rather than empty sentinel strings. List elements are non-null inside non-null lists ([Foo!]!).",
    "v1_compatibility_defaults": {
      "transcriptRecovery_when_disabled": "{ result: NOT_NEEDED, maxRecoveryPayloadBytes: 262144, maxJsonDepth: 32, maxChunksExamined: 64, recoveryParserVersion: 'p079_recovery_v1' }",
      "providerFallback_when_disabled": "{ result: NOT_NEEDED, deadlineSeconds: 0 }",
      "providerPlanEvidence_when_none": "{ paths: [], acceptedAsOutput: false, redactionsApplied: [], truncatedAtCap: false }",
      "permissionDecisions_when_none": "[]",
      "policyFeatureFlags_when_none": "[]",
      "lease_when_no_dispatch": "{ key: '<deterministic_hash>', state: SETTLED, acquiredAt: <now>, expiresAt: <now>, leaseSeconds: 0, ownerPrincipalId: '<principal>' }"
    },
    "enum_evolution_per_lane_api_contract_r3_003": {
      "graphql": "Closed enums. Adding a new value requires a v2 schema and parity fixture; until then, generated clients fail closed on unknown values. The GraphQL transport never carries an out-of-band raw enum string.",
      "mcp_and_run_report_json": "snake_case enum values exactly matching output_contract_repair.v1; unknown enum values cause v1 schema validation to fail closed at the parity-fixture boundary.",
      "swift_dto": "Closed Swift enums with a raw-value-preservation wrapper. Unknown presentation_category and recommended_next_action map to the conservative unknownDiagnostic display case; the raw string is preserved in inspectors for support. Swift fallback is display-only and never authorizes output settlement, transition truth, or operator dispatch."
    },
    "scalar_types": {
      "DateTime": "Custom scalar declared in SDL. RFC 3339 / ISO 8601 string scalar; Swift decoder accepts both with-and-without fractional seconds and explicit offset.",
      "ID": "GraphQL built-in scalar. NOT redeclared in the SDL appendix (api-contract-r4-001). Documentation only: ID values are opaque strings and never embed raw filesystem paths, prompts, or operator text."
    },
    "enum_casing": "GraphQL enums use SCREAMING_SNAKE_CASE values that map deterministically to the snake_case evidence enums (e.g. RECOVERED <-> recovered). The Swift DTO module owns the bidirectional projection and the parity fixture asserts byte-equivalence of the camelCase GraphQL projection against the snake_case MCP/run-report shape.",
    "deprecation_rule": "No fields are deprecated in v1; v2 may add optional fields, must not remove or repurpose v1 fields, and must publish a compatibility note + parity fixtures with schema-version-suffixed fixture names (e.g. output_contract_repair.v1.fixture.json coexists with output_contract_repair.v2.fixture.json).",
    "sdl_parity_fixture": "docs/evidence/rollout-contract/p079/api/graphql-sdl-parity.fixture.json",
    "sdl_parity_assertions_api_contract_r4_002": [
      "GraphQL enum ProviderFamily values map byte-for-byte (after lowercase SCREAMING_SNAKE_CASE -> snake_case projection) to output_contract_repair_v1_schema.enums.provider_family values. Domain at v1: {codex, claude, gemini, junie, auggie, fixture}.",
      "GraphQL enum AdapterFamily values map byte-for-byte (after the same projection) to output_contract_repair_v1_schema.enums.adapter_family values. provider_family and adapter_family share the same closed v1 domain by design: provider_family describes the vendor family (Codex/Claude/Gemini/Junie/Auggie) and adapter_family describes the local ACP adapter that dispatched the work; deterministic fixtures use the synthetic value `fixture`.",
      "Future divergence between provider_family and adapter_family (e.g. a Claude provider reached through a Gemini-compatible adapter) requires a v2 schema bump for the schema whose domain extends; both enums remain closed at v1.",
      "no_redeclared_builtin_scalars: assert that graphql_sdl_appendix.scalars contains only custom scalars (currently DateTime); built-in scalars ID, String, Int, Float, Boolean MUST NOT appear (api-contract-r4-001)."
    ]
  },
  "readback_contract": {
    "graphql": {
      "owner_crate": "control-plane/crates/graphql-server",
      "parent_fields": "Run.stageExecutions.agentExecutions.outputContractRepair: OutputContractRepairEvidence nullable and additive",
      "types": [
        "OutputContractRepairEvidence",
        "OutputContractRepairAttempt",
        "OutputContractTranscriptRecovery",
        "OutputContractProviderFallback",
        "OutputContractPlanEvidence",
        "OutputContractBudget",
        "OutputContractLease",
        "OutputContractPermissionDecision",
        "OutputContractRepairPresentation",
        "OutputContractRepairStatus"
      ],
      "nullability": "The parent outputContractRepair field is nullable for pre-P079 runs, feature-disabled runs, and executions with no output-contract failure. Nested objects are non-null when evidence exists; optional scalar fields use null rather than empty sentinel strings.",
      "subscriptions": "Existing run and stage observation channels publish immutable snapshots; SwiftUI does not derive repair state from local artifacts."
    },
    "mcp": {
      "tools_and_resources": [
        "reports.get",
        "report://{run_id}"
      ],
      "shape": "snake_case output_contract_repair object matching output_contract_repair.v1; unknown fields rejected by schema fixtures for v1.",
      "missing_behavior": "For old runs, output_contract_repair is null and output_contract_repair_status is not_attempted in flattened operator report views."
    },
    "run_report": {
      "shape": "Same snake_case object as MCP plus flattened display fields for existing report tables.",
      "parity_fixture": "docs/evidence/rollout-contract/operator-readback/p079-output-repair-full-surface.fixture.json"
    },
    "swift_client": {
      "ownership": "Read-only diagnostic state owned by control-plane projections. The macOS app renders optional DTOs and does not scan transcripts, discover plan files, or persist canonical SwiftData artifact truth for P079.",
      "presentation": "Primary badges bind to presentation_category and recommended_next_action via the pinned status -> presentation_category projection; inspectors group fields under Diagnostics, Execution Details, and Evidence GroupBoxes and may show failure class, subtype, lease.expires_at, fallback_packet_hash, and permission_decisions. SwiftUI row identity is (repair_attempt_id, agent_execution_id) only; evidence_version is a content-version / refresh-invalidation field used by Equatable and view refresh but NOT by identity. reserved -> prompt_sent -> settled updates therefore replace the same row.",
      "identity_contract": {
        "stable_identity_fields": ["repair_attempt_id", "agent_execution_id"],
        "content_refresh_fields": ["evidence_version", "status", "presentation_category", "recommended_next_action", "lease", "budget", "final_output_settlement", "permission_decisions"],
        "rule": "Identifiable.id and ForEach keys derive from stable_identity_fields. Equatable compares the full snapshot; SwiftUI recomputes the body when any content_refresh_field changes while diffing keeps the row in place. Re-delivery of an unchanged evidence_version snapshot MUST cause zero row churn.",
        "fixture": "docs/evidence/rollout-contract/p079/swift/identity-stable-across-evidence-versions.json"
      },
      "decode_gate": "./scripts/test-gate.sh p079-swift-readback compiles the new DTO module and runs decode fixtures (old-run, feature-disabled, recovered, blocked, cancelled, stale, orphan, lease-reclaimed, unknown-enum).",
      "swiftdata_invalidation": "P079 evidence is not stored as canonical SwiftData artifact truth; ephemeral read-model caches invalidate on evidence_version or projection_integrity changes."
    }
  },
  "sqlite_migration_appendix": {
    "owner_crate": "control-plane/crates/db",
    "migration_id": "p079_output_contract_repair_v1",
    "schema_version_constant": "p079_v1",
    "completeness_note": "Every table, column, primary key, unique constraint, foreign key, index, projection-version column, and rollback/read-compatibility rule needed to implement P079's durable scheduling and readback authority is defined below. Old-run rowsets and feature-disabled rowsets decode through these schemas without rewrite. Schema-version-suffixed fixture filenames (e.g. p079_v1) allow future output_contract_repair.v2 rows to coexist.",
    "tables": [
      {
        "name": "output_contract_repair_events",
        "purpose": "Authoritative per-(repair_attempt, parent agent execution) evidence row materialized as the JSON projection consumed by GraphQL/MCP/run-report and Swift DTOs.",
        "columns": [
          {"name": "repair_attempt_id", "type": "TEXT", "nullable": false, "comment": "random v4 UUID; primary key"},
          {"name": "schema_version", "type": "TEXT", "nullable": false, "default": "'output_contract_repair.v1'", "comment": "JSON schema lane version"},
          {"name": "run_id", "type": "TEXT", "nullable": false},
          {"name": "stage_execution_id", "type": "TEXT", "nullable": false},
          {"name": "agent_execution_id", "type": "TEXT", "nullable": false, "comment": "parent (failed) agent execution"},
          {"name": "session_generation_id", "type": "TEXT", "nullable": false},
          {"name": "role", "type": "TEXT", "nullable": false},
          {"name": "provider_family", "type": "TEXT", "nullable": false, "comment": "enum-checked at write time"},
          {"name": "adapter_family", "type": "TEXT", "nullable": false},
          {"name": "required_output_mode", "type": "TEXT", "nullable": false},
          {"name": "initial_failure_class", "type": "TEXT", "nullable": false},
          {"name": "initial_failure_subtype", "type": "TEXT", "nullable": true},
          {"name": "status", "type": "TEXT", "nullable": false, "comment": "top-level status enum; derived but materialized for indexed scan"},
          {"name": "evidence_version", "type": "INTEGER", "nullable": false, "default": "1", "comment": "monotonic content version; bumped on every projection write"},
          {"name": "final_output_settlement", "type": "TEXT", "nullable": false},
          {"name": "recommended_next_action", "type": "TEXT", "nullable": false},
          {"name": "presentation_category", "type": "TEXT", "nullable": false},
          {"name": "repair_prompt_template_version", "type": "TEXT", "nullable": false},
          {"name": "policy_feature_flags_json", "type": "TEXT", "nullable": false, "default": "'[]'", "comment": "JSON array of strings"},
          {"name": "required_outputs_json", "type": "TEXT", "nullable": false, "comment": "JSON array of {name, contract_id, canonical_path}"},
          {"name": "same_session_repair_json", "type": "TEXT", "nullable": false, "comment": "JSON object matching nested schema"},
          {"name": "transcript_recovery_json", "type": "TEXT", "nullable": false},
          {"name": "provider_fallback_json", "type": "TEXT", "nullable": false},
          {"name": "provider_plan_evidence_json", "type": "TEXT", "nullable": false},
          {"name": "budget_json", "type": "TEXT", "nullable": false},
          {"name": "lease_id", "type": "TEXT", "nullable": false, "comment": "FK to output_contract_repair_leases.lease_key"},
          {"name": "fallback_agent_execution_id", "type": "TEXT", "nullable": true, "comment": "child execution; FK to agent_executions.id"},
          {"name": "fallback_packet_hash", "type": "TEXT", "nullable": true, "comment": "sha256 hex"},
          {"name": "fallback_principal_id", "type": "TEXT", "nullable": true},
          {"name": "fallback_principal_capability_hash", "type": "TEXT", "nullable": true},
          {"name": "permission_decisions_json", "type": "TEXT", "nullable": false, "default": "'[]'"},
          {"name": "projection_integrity", "type": "TEXT", "nullable": false, "default": "'fresh'", "comment": "fresh | stale | permanently_stale"},
          {"name": "projection_stale_since", "type": "TEXT", "nullable": true, "comment": "RFC3339"},
          {"name": "projection_rebuild_attempts", "type": "INTEGER", "nullable": false, "default": "0"},
          {"name": "projection_schema_version", "type": "INTEGER", "nullable": false, "default": "1", "comment": "projection rebuild can read older rows and replay into newer projection version without rewriting the row"},
          {"name": "recorded_at", "type": "TEXT", "nullable": false},
          {"name": "updated_at", "type": "TEXT", "nullable": false}
        ],
        "primary_key": ["repair_attempt_id"],
        "unique_constraints": [
          {"name": "uq_output_contract_repair_events_parent", "columns": ["agent_execution_id"], "rationale": "At most one P079 evidence row per parent agent execution. Reuse for re-dispatch updates the row in place; new attempts get a new repair_attempt_id only if the prior row is in a terminal state (settled) AND a new failed execution is created."}
        ],
        "foreign_keys": [
          {"columns": ["run_id"], "references": "runs(id)", "on_delete": "CASCADE"},
          {"columns": ["stage_execution_id"], "references": "stage_executions(id)", "on_delete": "CASCADE"},
          {"columns": ["agent_execution_id"], "references": "agent_executions(id)", "on_delete": "CASCADE"},
          {"columns": ["lease_id"], "references": "output_contract_repair_leases(lease_key)", "on_delete": "RESTRICT", "rationale": "Evidence rows MUST NOT be deleted while a lease still references them; transactional lease finalization is required first."},
          {"columns": ["fallback_agent_execution_id"], "references": "agent_executions(id)", "on_delete": "SET NULL"}
        ],
        "indexes": [
          {"name": "ix_output_contract_repair_events_run", "columns": ["run_id"]},
          {"name": "ix_output_contract_repair_events_stage", "columns": ["stage_execution_id"]},
          {"name": "ix_output_contract_repair_events_status", "columns": ["status", "updated_at"], "rationale": "Operator readback filtering by status and recency."},
          {"name": "ix_output_contract_repair_events_projection_stale", "columns": ["projection_integrity", "projection_stale_since"], "rationale": "Bounded sweep picks up stale projections efficiently."},
          {"name": "ix_output_contract_repair_events_fallback_child", "columns": ["fallback_agent_execution_id"], "rationale": "Reverse lookup from child to parent for restart settlement."}
        ],
        "check_constraints": [
          "CHECK (status IN ('not_attempted','in_progress','recovered','blocked','skipped','cancelled','failed'))",
          "CHECK (presentation_category IN ('informational','recovered','blocked','skipped','failed','cancelled'))",
          "CHECK (projection_integrity IN ('fresh','stale','permanently_stale'))"
        ]
      },
      {
        "name": "output_contract_repair_leases",
        "purpose": "Single-flight scheduling authority for repair/fallback dispatch. Lease commit precedes ACP dispatch (REL-r2-1).",
        "columns": [
          {"name": "lease_key", "type": "TEXT", "nullable": false, "comment": "deterministic hash(run_id, stage_execution_id, parent_agent_execution_id, schema_version, frozen_fallback_policy_hash)"},
          {"name": "lease_kind", "type": "TEXT", "nullable": false, "comment": "enum: 'repair' | 'fallback'"},
          {"name": "schema_version", "type": "TEXT", "nullable": false, "default": "'output_contract_repair.v1'"},
          {"name": "run_id", "type": "TEXT", "nullable": false},
          {"name": "stage_execution_id", "type": "TEXT", "nullable": false},
          {"name": "parent_agent_execution_id", "type": "TEXT", "nullable": false},
          {"name": "frozen_fallback_policy_hash", "type": "TEXT", "nullable": false, "comment": "sha256 of RunPlanSnapshot policy slice; '' for repair leases"},
          {"name": "state", "type": "TEXT", "nullable": false, "default": "'reserved'", "comment": "enum: reserved | prompt_sent | settled"},
          {"name": "result", "type": "TEXT", "nullable": true, "comment": "set on transition to settled; null until terminal"},
          {"name": "owner_principal_id", "type": "TEXT", "nullable": false},
          {"name": "acquired_at", "type": "TEXT", "nullable": false},
          {"name": "expires_at", "type": "TEXT", "nullable": false},
          {"name": "lease_seconds", "type": "INTEGER", "nullable": false, "comment": "180 repair / 1200 fallback default"},
          {"name": "settled_at", "type": "TEXT", "nullable": true},
          {"name": "reclamation_reason", "type": "TEXT", "nullable": true},
          {"name": "reclaimed_at", "type": "TEXT", "nullable": true},
          {"name": "idempotency_token", "type": "TEXT", "nullable": true, "comment": "random v4 UUID minted at lease commit; bound to ACP wire write"},
          {"name": "dispatch_committed_at", "type": "TEXT", "nullable": true, "comment": "set in same DB transaction that flips state to prompt_sent; recovery treats this as the durable-commit-before-dispatch checkpoint"},
          {"name": "version", "type": "INTEGER", "nullable": false, "default": "0", "comment": "monotonic CAS guard for concurrent reclamation"}
        ],
        "primary_key": ["lease_key"],
        "unique_constraints": [
          {"name": "uq_output_contract_repair_leases_singleflight", "columns": ["run_id", "stage_execution_id", "parent_agent_execution_id", "schema_version", "frozen_fallback_policy_hash", "lease_kind"], "rationale": "Single-flight: at most one lease per (parent, schema, frozen policy, lease kind). Policy drift between attempts cannot bypass single-flight because the key uses the frozen hash."}
        ],
        "foreign_keys": [
          {"columns": ["run_id"], "references": "runs(id)", "on_delete": "CASCADE"},
          {"columns": ["stage_execution_id"], "references": "stage_executions(id)", "on_delete": "CASCADE"},
          {"columns": ["parent_agent_execution_id"], "references": "agent_executions(id)", "on_delete": "CASCADE"}
        ],
        "indexes": [
          {"name": "ix_output_contract_repair_leases_expiry", "columns": ["state", "expires_at"], "rationale": "Reconciliation sweep scans non-settled leases ordered by expiry."},
          {"name": "ix_output_contract_repair_leases_parent", "columns": ["parent_agent_execution_id"]}
        ],
        "check_constraints": [
          "CHECK (state IN ('reserved','prompt_sent','settled'))",
          "CHECK (lease_kind IN ('repair','fallback'))",
          "CHECK ((state = 'reserved' AND dispatch_committed_at IS NULL) OR (state IN ('prompt_sent','settled') AND dispatch_committed_at IS NOT NULL))"
        ],
        "transactional_invariants": [
          "Lease transition reserved -> prompt_sent updates `state='prompt_sent'`, `dispatch_committed_at=NOW`, and `idempotency_token=<minted>` in a single transaction that MUST commit BEFORE the ACP prompt is written to the transport wire. The transport is not invoked until the commit completes.",
          "Lease transition to settled writes `state='settled'`, `result=<terminal enum>`, `settled_at=NOW`, optional `reclamation_reason`, optional `reclaimed_at` in the same transaction that writes the corresponding `output_contract_repair_events` projection update and any final source-generation settlement.",
          "CAS reclamation: `UPDATE ... SET state='settled', version=version+1 WHERE lease_key=? AND version=? AND expires_at < NOW`. Loser observes terminal as a normal post-terminal read."
        ]
      },
      {
        "name": "output_contract_repair_fallback_parent_links",
        "purpose": "Explicit forward (parent -> child) and reverse (child -> parent) linkage for fallback. Decouples agent_executions from P079 columns and makes restart settlement deterministic.",
        "columns": [
          {"name": "fallback_agent_execution_id", "type": "TEXT", "nullable": false, "comment": "child execution id"},
          {"name": "parent_failed_agent_execution_id", "type": "TEXT", "nullable": false},
          {"name": "repair_attempt_id", "type": "TEXT", "nullable": false, "comment": "FK to output_contract_repair_events.repair_attempt_id"},
          {"name": "fallback_packet_hash", "type": "TEXT", "nullable": false, "comment": "sha256 hex; same hash stored on parent evidence and child execution allows byte-equality proof"},
          {"name": "created_at", "type": "TEXT", "nullable": false}
        ],
        "primary_key": ["fallback_agent_execution_id"],
        "unique_constraints": [
          {"name": "uq_fallback_parent_links_parent", "columns": ["parent_failed_agent_execution_id"], "rationale": "At most one fallback child per parent execution (per repair_attempt_id)."}
        ],
        "foreign_keys": [
          {"columns": ["fallback_agent_execution_id"], "references": "agent_executions(id)", "on_delete": "CASCADE"},
          {"columns": ["parent_failed_agent_execution_id"], "references": "agent_executions(id)", "on_delete": "CASCADE"},
          {"columns": ["repair_attempt_id"], "references": "output_contract_repair_events(repair_attempt_id)", "on_delete": "CASCADE"}
        ],
        "indexes": [
          {"name": "ix_fallback_parent_links_parent", "columns": ["parent_failed_agent_execution_id"]},
          {"name": "ix_fallback_parent_links_repair", "columns": ["repair_attempt_id"]}
        ]
      }
    ],
    "rollback_and_read_compatibility": {
      "rollback_mode": "feature_flag_disable_keep_evidence_readback",
      "rollback_steps_summary": "Disable the three P079 feature flags; do not drop tables; sweep continues reclaiming stale leases. Existing rows remain readable through GraphQL/MCP/run-report and the Swift DTO module.",
      "old_run_rows": "Old-run agent_executions have no row in output_contract_repair_events; the JOIN returns NULL and the GraphQL field decodes as null. No backfill is required.",
      "feature_disabled_rows": "When the feature flags are disabled mid-run, no new lease is inserted and no new event row is written. Existing rows remain readable. The Swift DTO decodes the parent as null for any execution lacking an event row.",
      "projection_version_evolution": "projection_schema_version on output_contract_repair_events allows projection rebuild logic to convert older projections to the current shape without rewriting rows. Future v2 schemas may introduce a new projection_schema_version constant; the rebuild path emits the current materialized projection while the underlying row keeps the column versions it was written with.",
      "json_payload_versioning": "Each *_json column embeds its own schema_version inside the JSON object so a v2 nested-object shape can coexist with v1 rows; the projection rebuild reads the embedded version and emits the current shape."
    },
    "migration_fixtures": [
      "docs/evidence/rollout-contract/p079/migration/p079_v1-old-run-rowset.fixture.json",
      "docs/evidence/rollout-contract/p079/migration/p079_v1-feature-disabled-rowset.fixture.json",
      "docs/evidence/rollout-contract/p079/migration/p079_v1-projection-rebuild-from-rows.fixture.json",
      "docs/evidence/rollout-contract/p079/migration/p079_v1-rollback-readback-preserved.fixture.json",
      "docs/evidence/rollout-contract/p079/migration/p079_v1-single-flight-uniqueness.fixture.json",
      "docs/evidence/rollout-contract/p079/migration/p079_v1-cas-reclamation-monotonic.fixture.json"
    ]
  },
  "advisory_followups_addressed_r4": {
    "api-contract-r3-003_enum_evolution_per_lane": "Pinned in graphql_sdl_appendix.enum_evolution_per_lane_api_contract_r3_003.",
    "api-contract-r3-004_canonical_path_exposure": "Internal settlement evidence uses absolute frozen canonical_path. GraphQL/MCP/run-report readback of provider_plan_evidence and evidence_artifact_path use run-meta-root-relative form (canonical_path_relative semantics). RequiredOutputBinding.canonicalPath remains absolute because it is the contract-bound frozen path; operators viewing it copy it verbatim. Metrics never expose canonical_path. Documented in graphql_sdl_appendix and plan-evidence section.",
    "api-contract-r3-005_fallback_policy_schema_precision": "fallback_policy_schema now declares required_fields, optional_fields, nested object shapes, enum domains, duplicate handling, precedence, and disabled_reason vocabulary in feature_flag_binding_rules; snapshot compile failures emit explicit disabled_reason codes.",
    "rel-r3-n1_sweep_failure_retry": "Recovery sweep failures emit `recovery_sweep_total{kind, result}` with result `failed` and are retried on the next 60s tick; no exponential escalation; persistent failure is alerting-visible via metric and `projection_stale_since`.",
    "rel-r3-n2_projection_abandonment_ceiling": "After 12 consecutive failed rebuild attempts (≈1h elapsed under capped backoff), `projection_integrity = permanently_stale` and `recommended_next_action = manual_investigation`.",
    "rel-r3-n3_concurrency_cap_explicit": "No global P079 concurrency cap is intended; per-execution budget (1 repair + 1 fallback) is the bound. A soft observability metric `p079_repair_inflight_total{role}` is exposed for ops visibility without enforcement.",
    "rel-r3-n4_multi_daemon_scope": "Control-plane is single-daemon deployment today; CAS reclamation language is defensive against overlap during restart. Stated in 'Reliability Semantics > Deployment Posture'.",
    "rel-r3-n5_infrastructure_retry_backoff": "Pre-ACK transport retry uses a 250ms initial backoff between attempts, fixed (not exponential) within the bounded retry of 2.",
    "rel-r3-n6_shutdown_drain_semantics": "On graceful shutdown the daemon attempts to await ACP result for in-flight `prompt_sent` repair turns up to the role watchdog deadline; on timeout it hands off to startup recovery, which observes `prompt_sent` and never re-prompts. Fixture: `graceful_shutdown_drains_or_hands_off_no_reprompt`.",
    "macos-r3-001_status_presentation_projection": "Pinned in 'Status -> Presentation Category Projection' table; parity fixture extended with one row per status value.",
    "macos-r3-002_accessibility_contract": "Pinned in 'Presentation Polish Contract'.",
    "macos-r3-003_date_locale_format": "Pinned in 'Presentation Polish Contract' (Date.RelativeFormatStyle + Date.ISO8601FormatStyle).",
    "macos-r3-004_pasteboard_keyboard": "Pinned in 'Presentation Polish Contract'.",
    "macos-r3-005_stale_projection_ux": "Pinned in 'Presentation Polish Contract' (Stale chip, optional Refresh affordance, permanently_stale escalation).",
    "macos-r3-006_unusernotification": "Pinned in 'Presentation Polish Contract'.",
    "macos-r3-007_identity_avoids_evidence_version": "Resolved as part of APPLE-R3-001: identity is (repair_attempt_id, agent_execution_id); evidence_version is content-version only; coalescing fixture asserts zero row churn for unchanged evidence_version.",
    "macos-r3-008_module_layout": "Pinned in 'Presentation Polish Contract' (new sibling sub-package under Engine/Readback/)."
  },
  "blocker_resolution_ids": [
    "api-contract-r2-001",
    "api-contract-r2-002",
    "APPLE-001",
    "REL-r2-1",
    "REL-r2-2",
    "sec-001",
    "sec-002",
    "sec-003",
    "sec-004",
    "api-contract-r3-001",
    "api-contract-r3-002",
    "APPLE-R3-001",
    "api-contract-r4-001",
    "api-contract-r4-002"
  ]
}
