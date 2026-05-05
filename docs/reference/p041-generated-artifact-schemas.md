# P041 Generated Artifact Schemas

This document defines the versioned schemas for runtime parity artifacts produced by the P041 server parity harness.

## Runtime Handoff Schemas

### `p031-phase-0-runtime-manifest-row.v1`

The canonical same-tree P031 acceptance switch. Published to `control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json`.

| Field | Type | Description |
|---|---|---|
| `schema_version` | `String` | Fixed: `p031-phase-0-runtime-manifest-row.v1`. |
| `id` | `String` | Fixed: `p041_parity_evidence`. |
| `runtime_detail_path` | `String` | Repo-relative path to the runtime detail artifact. Must equal the canonical constant `control-plane/target/parity/publication/current/p031-p041-parity-evidence.json`; absolute paths and `..` traversal are rejected. |
| `reference_detail_path` | `String` | Repo-relative path to the promoted reference snapshot. Must equal the canonical constant `docs/reference/p031-p041-parity-evidence.json`; absolute paths and `..` traversal are rejected. |
| `validation_status` | `Status` | Canonical status (e.g., `ready_same_tree_verified`). |
| `publication_state` | `String` | e.g., `revoked_for_rerun`, `published`. |
| `publication_generation_id` | `String` | Unique ID for the current generation. |
| `detail_schema_version` | `String` | Must match the detail artifact's schema version. |
| `provenance` | `Provenance` | Git commit, tree ID, clean-tree proof, and timestamp. |

### `p031-p041-parity-evidence.v1`

The authoritative runtime detail artifact. Published to `control-plane/target/parity/publication/current/p031-p041-parity-evidence.json`.

| Field | Type | Description |
|---|---|---|
| `schema_version` | `String` | Fixed: `p031-p041-parity-evidence.v1`. |
| `overall_status` | `Status` | Canonical status; must match row's `validation_status`. |
| `publication_generation_id` | `String` | Must match row's generation ID. |
| `publication_state` | `String` | Must match row's publication state. |
| `required_fixtures` | `[String]` | List of all seven required fixture IDs. |
| `required_surfaces` | `[String]` | List of all six required surface IDs. |
| `fixtures` | `[FixtureVerdict]` | Per-fixture verdicts and provenance. Each entry has `fixture_id` (String), `report_path` (repo-relative path to `behavioral-diff-report.json`), `replay_path` (repo-relative path to `server-replay.json`), `shadow_report_path` (repo-relative path to `live-shadow-report.json`), `verdict` (per-fixture status), and `provenance` (fixture provenance summary). |
| `blocking_reasons` | `[String]` | Diagnostic reasons for non-ready status. |
| `missing_evidence` | `[MissingEvidenceEntry]` | Structured missing-evidence diagnostics. Each entry has `missing_path` (String), `expected_producer` (String), `affected_fixture_or_surface` (String), and `next_action` (String). |
| `provenance` | `Provenance` | Shared provenance block. |

## Fixture Work Product Schemas

These schemas describe the artifacts produced by the P041 parity harness in `control-plane/crates/engine/tests/proposal_041_parity.rs`. Field shapes below are the actual emitter output, not a target schema.

### `server-replay.v1`

Produced by the canonical replay of a golden fixture (offline or live-shadow). Written to `control-plane/target/parity/work/<generation_id>/<fixture_id>/server-replay.json` for offline replays and `control-plane/target/parity/shadow/<generation_id>/<fixture_id>/server-replay.json` for live-shadow replays. The active generation id is read from `P041_PUBLICATION_GENERATION_ID`; the schema-validation harness uses the literal `unscoped-fixture-replay` when the env var is unset.

| Field | Type | Description |
|---|---|---|
| `schema_version` | `String` | Fixed: `server-replay.v1`. |
| `overall_status` | `String` | `fixture_ready` when no blocking divergences; `blocked_divergence` otherwise. |
| `publication_generation_id` | `String` | Publication generation correlation ID. Currently `unscoped-fixture-replay` for the schema-validation harness; live gate runs override with the active generation ID. |
| `provenance` | `Provenance` | `{ generated_at: RFC3339 timestamp, gate: gate command string }`. |
| `fixture_id` | `String` | The fixture ID being replayed. |
| `run_id` | `String` | The replay's `RunId` produced by the canonical `StartRun` command path. |
| `mode` | `String` | `offline_fixture_replay` or `live_shadow`. |
| `owner_chain` | `[String]` | Ordered list of owner boundaries traversed during replay (e.g., `CommandHandler::StartRun`, `BackgroundExecutor::process_next_item`, `db::repos::projections::rebuild_all_for_run`). |
| `fixture_stage_stream_owner` | `String` | Identifies the frozen inputs that drive stage execution. |
| `executable_inputs` | `ExecutableInputs` | Repo-relative paths to: `workflow_snapshot`, `agent_catalog_snapshot`, `provider_profile`, `runtime_events`, `operator_decisions`, `database`. |
| `run_projection` | `RunProjection` | Snapshot of the run projection row: `id`, `idea_id`, `status`, `workflow_id`, `workflow_title`, `total_stages`, `completed_stages`, `failed_stages`, `pending_approvals`. |
| `stage_projection` | `[StageProjection]` | Per-stage projection rows: `id`, `run_id`, `stage_id`, `label`, `status`, `iteration`, `attempt_number`, `settlement_kind`, `has_artifacts`, `has_pending_approval`, `has_validation_failure`. |
| `artifact_index` | `[ArtifactIndex]` | Materialized artifact rows: `id`, `run_id`, `stage_id`, `agent_id`, `name`, `contract_id`, `format`, `file_path`, `provider`, `report_kind`, `report_version`. `file_path` is normalized to be portable: paths under the workspace root are emitted workspace-relative; remaining absolute paths under `/Users/`, `/var/folders/`, `/private/var/folders/`, or `/tmp/` are replaced with a `<machine-local>…` token so server-replay artifacts do not leak host or home-directory layout. |
| `operator_decisions` | `[OperatorDecision]` | Replay of the fixture's frozen operator decision stream: `stage_id`, `decision`, `at`. |

### `behavioral-diff-report.v1`

Produced by comparing the replayed canonical state against the fixture's golden client truth. Written to `control-plane/target/parity/reports/<generation_id>/<fixture_id>/behavioral-diff-report.json` for offline replays. The same schema is reused by the `fixture_regeneration` mode for regeneration diff reports stored alongside fixtures. Live-shadow replays emit the dedicated `live-shadow-report.v1` schema (see below) to `control-plane/target/parity/shadow/<generation_id>/<fixture_id>/live-shadow-report.json` instead.

| Field | Type | Description |
|---|---|---|
| `schema_version` | `String` | Fixed: `behavioral-diff-report.v1`. |
| `overall_status` | `String` | `fixture_ready` when no blocking divergences; `blocked_divergence` otherwise. Mirrors the publication-time status; consumers must reconcile against `verdict` for replay-local pass/fail. |
| `publication_generation_id` | `String` | Publication generation correlation ID. Currently `unscoped-fixture-replay` in the schema-validation harness; live gate runs override with the active generation ID. |
| `provenance` | `Provenance` | `{ generated_at: RFC3339 timestamp, gate: gate command string }`. |
| `report_id` | `String` | Stable per-fixture report identifier (`<fixture_id>-<timestamp>`). |
| `mode` | `String` | `offline_fixture_replay`, `live_shadow`, or `fixture_regeneration`. |
| `proof_mode` | `String` | Fixed: `canonical_replay` — asserts the replay traversed the canonical command/executor/projection boundary. |
| `run_fixture_id` | `String` | The fixture ID under comparison. |
| `fixture_revision` | `Integer` | Monotonic fixture revision from `fixture.json`. |
| `client_snapshot_ref` | `String` | Repo-relative path to the fixture's `fixture.json`. |
| `server_replay_ref` | `String` | Repo-relative path to the matching `server-replay.json`. |
| `database_ref` | `String` | Repo-relative path to the replay SQLite DB. Cross-binary consumers (e.g., GraphQL readback) reopen this DB to verify their own surface. |
| `executable_inputs` | `ExecutableInputs` | Repo-relative `*_ref` paths to: `frozen_workflow_snapshot_ref`, `frozen_agent_catalog_snapshot_ref`, `provider_profile_ref`, `runtime_events_ref`, `operator_decisions_ref`. |
| `comparison_surface` | `[String]` | Surfaces compared (the six required surfaces: `canonical_domain_state`, `projections`, `graphql_readback`, `mcp_report_readback`, `artifact_identity`, `operator_summary`). |
| `normalization_rules` | `[String]` | Normalization rules applied before comparison (sourced from the fixture). |
| `ignored_fields` | `[IgnoredField]` | Fields excluded from comparison: `{ path, reason }`. |
| `surface_comparisons` | `[SurfaceComparison]` | Per-surface results: `{ surface, path, status: "matched"\|"diverged", expected, actual }`. |
| `shadow_contract` | `ShadowContract \| null` | Present only when `mode == "live_shadow"`: `{ source_run_id, shadow_run_id, fixture_or_capture_id, idempotency_key, storage_namespace, artifact_root, settles_production_stages, live_adapter_invocation }`. `null` otherwise. |
| `divergences` | `[Divergence]` | Blocking, warning, or info divergences: `{ path, expected, actual, severity, owner_surface, investigation_hint }`. |
| `summary` | `Summary` | `{ blocking_count, warning_count, info_count, operator_message }`. |
| `verdict` | `String` | `ready` when `blocking_count == 0`; `red` otherwise. |
| `created_at` | `String` | RFC3339 timestamp at which the report was emitted. |

### `live-shadow-report.v1`

Produced by live-shadow replays. Written to `control-plane/target/parity/shadow/<generation_id>/<fixture_id>/live-shadow-report.json`. Field shape mirrors `behavioral-diff-report.v1` with the following pinned values:

- `schema_version` is `live-shadow-report.v1`.
- `mode` is `live_shadow`.
- `shadow_contract` is populated with `{ source_run_id, shadow_run_id, fixture_or_capture_id, idempotency_key, storage_namespace = "shadow", artifact_root, settles_production_stages = false, live_adapter_invocation = "forbidden" }`. The shadow contract enforces that the replay wrote to the `shadow` storage namespace, did not settle production stages, and forbade live adapter invocation.

## Ownership and Integrity

- Fixture schemas are owned by the Rust engine owner.
- Runtime publication schemas are owned jointly by the P031 release owner and Rust control-plane owner.
- The `p031-p041-parity-evidence.md` companion is explicitly **non-authoritative** and structurally non-normative. It is provided for human readability only; automated consumers must use the JSON evidence.
- Any field removal, rename, or type change requires a version bump and consumer audit.
- Downstream consumers must verify `row.detail_schema_version == detail.schema_version` and that provenance matches the live checkout.

## Status Vocabulary

| Status enum | Meaning | CLI Prefix |
|---|---|---|
| `ready_same_tree_verified` | Ready for P031 acceptance/promotion | `PASS` |
| `blocked_manual_recovery` | Stale/ambiguous owner requires manual resolution | `FAIL` |
| `blocked_missing_evidence` | Missing producer artifact | `FAIL` |
| `blocked_divergence` | Behavioral regression in fixture/surface | `FAIL` |
| `blocked_dirty_tree` | Rerun required on clean checkout | `WARN` |
| `blocked_timeout` | Timed out; descendant absence not proven | `WARN` |
| `blocked_interrupted` | Interrupted by operator | `WARN` |
| `blocked_in_progress` | Rerun active; do not trust stale evidence | `INFO` |

Blocked-state precedence (P041 §5) is fixed for CLI and summary rendering: `blocked_manual_recovery` overrides `blocked_missing_evidence`, which overrides `blocked_divergence`, which overrides `blocked_dirty_tree`, which overrides `blocked_timeout`, which overrides `blocked_interrupted`, which overrides `blocked_in_progress`. `ready_same_tree_verified` is legal only when none of those blocked states apply.

## Work Product Retention

The proposal target retention policy (P041 §6.4) is:

1. **Successful generations**: prunable oldest-first; the harness must retain at minimum the newest ready generation, the newest non-manual blocked diagnostic generation, and every `blocked_manual_recovery` generation.
2. **`blocked_manual_recovery` generations**: never auto-pruned. Operator action is required because their diagnostic value is the reason reclamation is blocked.
3. **Storage budget**: when retained parity generations exceed 500 MB, the CLI must warn and list preserved roots plus sizes; hitting the budget is diagnostic, not authorization to delete manual-recovery evidence.

Implementation status: the generation-scoped path layout is now wired into the harness — per-fixture work products are written under `control-plane/target/parity/work/<generation_id>/<fixture_id>/`, reports under `control-plane/target/parity/reports/<generation_id>/<fixture_id>/`, and shadow output under `control-plane/target/parity/shadow/<generation_id>/<fixture_id>/`. The active generation id comes from `P041_PUBLICATION_GENERATION_ID`, which the gate exports as `p041-<utc_iso>-<rand8>`; bare `cargo test` runs outside the gate use the sentinel `unscoped-fixture-replay`. Automatic pruning (oldest-first retention of one ready and one non-manual blocked diagnostic generation, never pruning `blocked_manual_recovery`) and the 500 MB storage-budget warning are wired into the gate. `control-plane/target/parity-control/` is implemented as a cleanup-safe root, is acquired by the gate (lease, current-step, release-marker, timeout-marker, and reclaim-marker written via atomic same-directory rename), and is never removed by gate cleanup. The gate validates `target/parity*` boundaries before mkdir/write, runs every external command through a process-group supervisor, records the active `pgid`, enforces deadline/drain, and applies the full A/A2/B/C/D reclaim matrix from the lease heartbeat, control sequence, PID birth time, and descendant-observation evidence.
