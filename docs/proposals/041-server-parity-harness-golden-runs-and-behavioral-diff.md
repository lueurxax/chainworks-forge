<!--
proposal_id: P041
proposal_revision_id: P041-draft-2026-04-30-r11
supersedes_revision_id: P041-draft-2026-04-29-r10
source_review_pass_id: proposal-p041-r11-aggregate-review
status: approved_for_implementation
frozen_at: 2026-04-30
run_id: 4459e17c-c5f0-48ee-9821-97ee937ec3dd
stage_id: state_7_implementation_started
review_decision: accept
aggregate_score: 46.4
average_score: 9.28
min_individual_score: 8.5
reviewer_count: 5
blocker_count: 0
-->

# Proposal P041: Server Parity Harness Golden Runs and Behavioral Diff

| Field | Value |
|---|---|
| Date | 2026-04-29 |
| Status | Approved for implementation (frozen from r11) |
| Author | Codex |
| Scope | Ratify the implemented parity harness, make same-tree publication retry-safe, and define the exact runtime-versus-reference handoff that P031 may trust fail-closed. |
| Canonical runtime P031 acceptance switch | `control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json` |
| Canonical runtime detail artifact | `control-plane/target/parity/publication/current/p031-p041-parity-evidence.json` |
| Canonical promoted reference snapshot | `docs/reference/p031-phase-0-artifact-manifest.json` row `p041_parity_evidence` plus `docs/reference/p031-p041-parity-evidence.json` |

## 1. Problem

P041 is already a real Rust control-plane harness, but the same-tree closeout contract is still weaker than the failure modes it is supposed to prevent. P031 same-tree acceptance depends on parity evidence that must be deterministic, current-checkout correct, and safe to rerun on the same tree without stale rows, torn control files, or surviving descendants producing a false green.

The r9 aggregate review left three score-critical gaps:

- The proposal still conflated the runtime row contract version with the runtime detail contract version, even though those are distinct artifacts.
- The live owner now proves descendant quiescence before release, but the successor-side stale-lease reclaim path after abnormal exit was still under-specified.
- The primary CLI transcript is now solid, but the secondary rendering contract still left too much discretion around grid layout, narrow terminals, status-vocabulary mapping, and missing-evidence or timeout examples.

This revision resolves those gaps directly in proposal text. It also makes the status-equivalence rule explicit across row, detail, and CLI surfaces; defines a fail-closed manual-recovery park state for reclaim ambiguity; adds a deterministic retention rule for preserved blocked generations; and makes the markdown companion explicitly non-authoritative and structurally non-normative.

## 2. Goals

- Preserve the existing seven-fixture, six-surface parity harness rather than narrowing scope to a smaller acceptance subset.
- Make `ready_same_tree_verified` trustworthy by requiring both publication-time proof and consumer-time live-checkout provenance checks.
- Keep same-tree reruns idempotent by separating generation-scoped work from authoritative runtime publication and from cleanup-safe lease metadata.
- Make the operator surface reproducible by defining one CLI transcript family, one status mapping, blocked-state precedence, grid rules, and narrow-terminal fallback behavior.
- Make Phase C a truly atomic P031 cutover by updating every manifest-required contract artifact that names the same client boundary.
- Keep the thin-client boundary intact: if parity readiness is ever shown in SwiftUI or AppKit, it must come from a daemon-owned GraphQL read surface derived from runtime artifacts, never from direct file reads.

## 3. Non-Goals

- No new SwiftUI, AppKit, or operator-shell screen ships in P041.
- No live daemon, live ACP or provider adapter, simulator, or macOS UI host is required for readiness. P041 remains offline replay plus northbound readback validation.
- No ordinary gate path may mutate tracked `docs/reference` files, create git commits, or restore tracked files automatically.
- No reduction to three fixtures, no optional closeout-only fast path, and no manual lifecycle shortcuts as a substitute for parity correctness are allowed.
- No app-owned persistence or direct filesystem readiness inference is introduced for future Apple-platform surfaces.

## 4. Current Repo Truth

- `control-plane/crates/engine/tests/proposal_041_parity.rs` already requires seven fixtures: `proposal-loop-basic`, `implementation-refine-review`, `approval-pause-resume`, `retry-recovery-flow`, `cancelled-or-blocked-run`, `terminal-report-evidence`, and `projection-readback-surface`.
- The same test already requires six comparison surfaces per fixture: `canonical_domain_state`, `projections`, `graphql_readback`, `mcp_report_readback`, `artifact_identity`, and `operator_summary`.
- `scripts/test-gate.sh proposal-041` is already the canonical parity wrapper, but it still contains stale subscription strings and does not yet consume the runtime row plus runtime detail artifact as the authoritative closeout seam.
- `docs/reference/query-projections-and-client-consumption-contract.md` is the implemented GraphQL read contract for thin macOS clients, but adjacent P031 artifacts are not yet fully aligned with its lower-camel GraphQL names.
- `docs/reference/p031-phase-0-artifact-manifest.json` still lists `schema_decision_record` as a manifest-required artifact, and `docs/reference/p031-schema-decision-record.json` still publishes stale argument names such as `stages(runID:)`, `agentExecutions(stageExecutionID:)`, and `artifacts(runID:)`.
- `docs/reference/p031-phase-0-artifact-manifest.json` does not yet contain a `p041_parity_evidence` row, and `docs/reference/p031-p041-parity-evidence.json` does not yet exist as promoted reference truth.
- The repo does not yet define the runtime row schema, an explicit `blocked_manual_recovery` state, a cleanup-safe `parity-control` reclaim matrix, or an explicit P041 reference-promotion command.

## 5. UX and UI Notes

P041 is infrastructure-first. Its user experience is evidence clarity, fast diagnosis, and no ambiguity about whether the current tree is actually safe to trust.

- No product UI ships in this proposal.
- The immediate operator-facing surfaces are the gate log, `control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json`, `control-plane/target/parity/publication/current/p031-p041-parity-evidence.json`, `control-plane/target/parity-control/current-step.json`, per-fixture `server-replay.json`, per-fixture `behavioral-diff-report.json`, per-fixture `live-shadow-report.json`, fixture `capture-record.md`, fixture `regeneration-diff-report.json`, and the optional markdown companion.
- Generated parity JSON artifacts must expose a top-level `overall_status` field so operators can scan them without deep parsing.
- Missing evidence is a first-class presentation state, not generic failure styling. Any missing-evidence callout must include `missing_path`, `expected_producer`, `affected_fixture_or_surface`, and `next_action`.
- The markdown companion, if generated, is a non-authoritative convenience view. Its existence, heading hierarchy, and layout are not part of the acceptance contract. Only the runtime row, runtime detail artifact, CLI contract, and JSON schemas are authoritative.
- If the markdown companion is intentionally omitted, the CLI final summary must say `JSON-only evidence` and point directly to the runtime detail artifact path.

| Status enum | Display label | CLI prefix | Grid token | Operator action |
|---|---|---|---|---|
| `ready_same_tree_verified` | Ready | `PASS` | `PASS` | Eligible for P031 runtime acceptance and optional reference promotion |
| `blocked_manual_recovery` | Manual recovery required | `FAIL` | `n/a` | Preserve the blocked generation, inspect the reclaim marker, and resolve descendant ambiguity before rerun |
| `blocked_missing_evidence` | Missing evidence | `FAIL` | `MISS` | Inspect the named missing producer and rerun after restoring it |
| `blocked_divergence` | Behavioral divergence | `FAIL` | `FAIL` | Inspect the named fixture and surface diff, then fix the regression |
| `blocked_dirty_tree` | Dirty checkout | `WARN` | `n/a` | Clean the checkout, then rerun on the same tree |
| `blocked_timeout` | Timed out | `WARN` | `TIMEOUT` | Inspect the active fixture or surface, host pressure, and descendant shutdown outcome |
| `blocked_interrupted` | Interrupted | `WARN` | `n/a` | Inspect the interruption marker and rerun |
| `blocked_in_progress` | Rerun in progress | `INFO` | `n/a` | Wait for the active generation; do not trust older ready evidence |

The status enum and the grid token vocabulary are intentionally different. Status enums describe whole-run readiness states. Grid tokens describe per-fixture or per-surface outcomes inside a rendered matrix. `n/a` means the run-level state does not project to a single fixture-surface cell.

Blocked-state precedence is fixed for CLI and summary rendering: `blocked_manual_recovery` overrides `blocked_missing_evidence`, which overrides `blocked_divergence`, which overrides `blocked_dirty_tree`, which overrides `blocked_timeout`, which overrides `blocked_interrupted`, which overrides `blocked_in_progress`. `ready_same_tree_verified` is legal only when none of those blocked states apply.

### 5.1 CLI rendering contract

The CLI log is append-only. The gate must not redraw tables in place or rely on a spinner-only experience. Non-TTY output uses the same line order and tokens without ANSI color. TTY color assignments are fixed: green for `PASS`, red for `FAIL`, yellow for `WARN`, and cyan for `INFO`.

Wide transcript reference:

```text
[INFO] p041 generation p041-2026-04-29T23:00:00Z-a1b2c3d4
[INFO] tree commit=0123456 tree=89abcde clean=true status_lines=0
[INFO] runtime publication revoked_for_rerun current generation updated
[INFO] [1/7] validate-capture proposal-loop-basic
[PASS] [1/7] validate-capture proposal-loop-basic
[INFO] [1/7] replay proposal-loop-basic
[INFO] active generation=p041-2026-04-29T23:00:00Z-a1b2c3d4 fixture=proposal-loop-basic step=graphql_readback surface=graphql_readback elapsed=18 heartbeat=2026-04-29T23:00:18Z
[PASS] [1/7] parity proposal-loop-basic
[INFO] [2/7] replay implementation-refine-review
[FAIL] [2/7] divergence implementation-refine-review surface=operator_summary report=control-plane/target/parity/reports/p041-2026-04-29T23:00:00Z-a1b2c3d4/implementation-refine-review/behavioral-diff-report.json
[FAIL] status=blocked_divergence passed_fixtures=1 failed_fixtures=1 missing_evidence=0
[FAIL] detail=control-plane/target/parity/publication/current/p031-p041-parity-evidence.json
```

Missing-evidence example:

```text
[FAIL] [4/7] missing-evidence projection-readback-surface missing_path=control-plane/target/parity/work/p041-2026-04-29T23:00:00Z-a1b2c3d4/projection-readback-surface/server-replay.json expected_producer=replay affected_fixture_or_surface=projection-readback-surface next_action=rerun-after-replay-restored
[FAIL] status=blocked_missing_evidence missing_evidence=1 failed_fixtures=0 timed_out_fixtures=0
```

Timeout example:

```text
[WARN] [5/7] timeout retry-recovery-flow step=live_shadow elapsed=60 deadline=60 descendant_state=draining
[WARN] status=blocked_timeout timed_out_fixtures=1 drain_deadline=30 detail=control-plane/target/parity/publication/current/p031-p041-parity-evidence.json
```

Passing-run footer example:

```text
[PASS] status=ready_same_tree_verified passed_fixtures=7 failed_fixtures=0 missing_evidence=0
[INFO] row=control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json
[INFO] detail=control-plane/target/parity/publication/current/p031-p041-parity-evidence.json
```

Summary grid contract:

- The wide summary grid is a 7-row by 6-column fixture-by-surface matrix.
- Row headers are full fixture ids and are never truncated.
- Column headers are fixed in this order: `canonical_domain_state`, `projections`, `graphql_readback`, `mcp_report_readback`, `artifact_identity`, `operator_summary`.
- Each rendered cell is width 9 including padding so `TIMEOUT` fits without squeezing adjacent columns.
- If the available width cannot render full row headers plus six 9-character cells, the CLI must switch to the vertical fallback instead of truncating headers.
- `SKIP` is reserved for future surface-exclusion cases and has no current producer in the seven-fixture acceptance set.

Reference grid rendering when width is sufficient:

```text
fixture                         canonical projections graphql  mcp       artifact  summary
proposal-loop-basic             PASS      PASS       PASS      PASS      PASS      PASS
implementation-refine-review    PASS      PASS       PASS      PASS      PASS      FAIL
approval-pause-resume           PASS      PASS       PASS      PASS      PASS      PASS
retry-recovery-flow             PASS      PASS       PASS      TIMEOUT   PASS      TIMEOUT
cancelled-or-blocked-run        PASS      PASS       PASS      PASS      PASS      PASS
terminal-report-evidence        PASS      PASS       PASS      PASS      PASS      PASS
projection-readback-surface     PASS      PASS       MISS      PASS      PASS      MISS
```

Narrow-terminal fallback reference at roughly 40 columns:

```text
proposal-loop-basic
  canon=P proj=P gql=P
  mcp=P art=P sum=P
implementation-refine-review
  canon=P proj=P gql=P
  mcp=P art=P sum=F
Legend: P=PASS F=FAIL
Legend: M=MISS S=SKIP T=TIMEOUT
```

CLI token rules:

- Prefix every line with `[INFO]`, `[PASS]`, `[WARN]`, or `[FAIL]`.
- Fixture progress lines use `[n/7] <step> <fixture_id>` exactly.
- Active-state lines must publish `generation`, `fixture`, `step`, `surface` or `mode`, `elapsed`, and `heartbeat` in a single append-only line, and the same fields must appear in `current-step.json`.
- The final summary line must project the same canonical status carried by `row.validation_status` and `detail.overall_status`.
- Summary-grid cells, when rendered in CLI or markdown, are limited to `PASS`, `FAIL`, `MISS`, `SKIP`, and `TIMEOUT`.
- If the runtime detail artifact omits the markdown companion, the final CLI footer must say `JSON-only evidence` before printing the `detail=` path.

## 6. Architecture

### 6.1 Fixture and checked-in schema contract

Each directory under `control-plane/crates/engine/tests/fixtures/parity/golden-runs/<fixture_id>/` is a checked-in executable contract, not a loose snapshot.

Required checked-in elements per fixture:

- `fixture.json` with `GoldenRunFixture` metadata, frozen inputs, normalization rules, and regeneration policy.
- Frozen workflow, agent-catalog, provider-profile, runtime-event, and operator-decision inputs required for offline replay.
- Expected truth for canonical state, projections, artifact identity, and operator summary.
- `capture-record.md` describing capture provenance.
- `regeneration-diff-report.json` describing intentional fixture changes.

Determinism rule: replaying the same frozen inputs on the same clean tree must reproduce the same normalized truth. Volatile timestamps, generated ids, and artifact-generation time belong in generated outputs under `control-plane/target/parity/`, not in checked-in fixture truth.

### 6.2 Runtime publication artifacts, provenance, versioning, and status equivalence

The runtime parity filesystem is split by purpose so retries stay idempotent and cleanup stays safe:

- `control-plane/target/parity/work/<publication_generation_id>/...` stores ephemeral replay databases and fixture-scoped generated work products.
- `control-plane/target/parity/shadow/<publication_generation_id>/...` stores live-shadow execution outputs.
- `control-plane/target/parity/reports/<publication_generation_id>/...` stores per-fixture generated reports.
- `control-plane/target/parity/publication/generations/<publication_generation_id>/...` stores generation-scoped candidate row and detail artifacts.
- `control-plane/target/parity/publication/current/` stores the authoritative runtime row and detail artifacts for the latest generation.
- `control-plane/target/parity-control/` stores lease, heartbeat, `current-step.json`, reclaim markers, and interruption or timeout markers. Cleanup never removes this root.

All generated parity artifacts use the same provenance shape:

- `provenance.commit_sha` is full `git rev-parse HEAD` captured by the generating invocation.
- `provenance.tree_id` is `git rev-parse HEAD^{tree}` captured by the same invocation.
- `provenance.tree_clean`, `provenance.status_snapshot_sha256`, and `provenance.status_snapshot_line_count` are derived from one exact `git status --porcelain=v1 --untracked-files=all` capture.
- `provenance.generated_at` is UTC ISO-8601.
- `provenance.gate` is `./scripts/test-gate.sh proposal-041`.

The canonical runtime detail artifact at `control-plane/target/parity/publication/current/p031-p041-parity-evidence.json` must contain:

- `schema_version: p031-p041-parity-evidence.v1`.
- `overall_status`, `publication_generation_id`, and `publication_state`.
- `required_fixtures` listing all seven fixture ids.
- `required_surfaces` listing all six surface ids in fixed order: `canonical_domain_state`, `projections`, `graphql_readback`, `mcp_report_readback`, `artifact_identity`, `operator_summary`.
- `fixtures[]` entries containing `fixture_id`, `report_path`, `replay_path`, `shadow_report_path`, `verdict`, and fixture provenance summary.
- `blocking_reasons[]` and `missing_evidence[]` when ready publication is not legal.

The canonical runtime row at `control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json` is a versioned contract, not an untyped helper. Required fields are:

- `schema_version: p031-phase-0-runtime-manifest-row.v1`.
- `id: p041_parity_evidence`.
- `runtime_detail_path` and `reference_detail_path`.
- `validation_status`, `publication_state`, `publication_generation_id`, and `detail_schema_version`.
- `provenance.commit_sha`, `provenance.tree_id`, `provenance.tree_clean`, `provenance.status_snapshot_sha256`, `provenance.status_snapshot_line_count`, and `provenance.generated_at`.

Cross-artifact compatibility rules are explicit:

- `row.schema_version` versions only the runtime row contract and is validated against `p031-phase-0-runtime-manifest-row.v1` independently of the detail artifact.
- `detail.schema_version` versions only the runtime detail contract and is validated against `p031-p041-parity-evidence.v1` independently of the row.
- `row.detail_schema_version` must equal `detail.schema_version` for every publication, including blocked diagnostic publication.
- `row.validation_status` must equal `detail.overall_status`, and the CLI final summary status must be a direct projection of that shared canonical status.
- `row.publication_state` must equal `detail.publication_state`, and `row.publication_generation_id` must equal `detail.publication_generation_id`.

Ready-state integrity rules:

- `ready_same_tree_verified` is legal only when `tree_clean == true`, `status_snapshot_line_count == 0`, every required fixture passes, every required surface passes, and the runtime row plus runtime detail artifact agree on status, publication state, generation id, detail-schema version, commit SHA, tree id, and clean-tree proof.
- Dirty trees may still publish diagnostic blocked evidence, but never authoritative ready evidence.
- `commit_sha` alone is never sufficient. Tree identity, clean-tree proof, and live-checkout comparison are all required.

### 6.3 Replay, ownership, interruption, timeout, and Darwin safety

The final P041 pipeline is fail-closed, ordered, and retry-safe.

Load-bearing `parity-control` metadata has one mandatory write contract: every lease, heartbeat, `current-step.json`, reclaim marker, interruption marker, timeout marker, and release marker must be written as valid JSON to a temporary file in the same directory as the target, durably flushed, atomically renamed into place, and followed by a parent-directory durability barrier when the platform exposes one. On Darwin, durable flush should prefer `F_FULLFSYNC` for crash-safe lease metadata rather than relying on plain `fsync` alone. In-place truncate-and-rewrite is forbidden for canonical control files.

Same-volume rule: temp files must be created in the same directory as the target file so rename remains atomic on APFS and cannot degrade into copy-plus-delete across volumes.

Darwin process identity rule:

- `process_birth_unix_ms` must come from `proc_pidinfo(..., PROC_PIDTBSDINFO, ...)` or `sysctl(KERN_PROC_PID)`, not from parsing `ps` output.
- Replay and live-shadow subprocesses must run in a dedicated process group created with `setpgid` so the gate can terminate and observe the whole descendant set using `killpg` and `waitpid(-pgid, ...)`.

Lifecycle contract:

1. Acquire same-tree ownership of `control-plane/target/parity-control/` before destructive work. The lease record contains PID, `process_birth_unix_ms`, process-group identity when descendants exist, hostname, commit SHA, tree id, heartbeat time, `publication_generation_id`, and a monotonic `control_sequence`.
2. Place `.metadata_never_index` at `control-plane/target/parity/` and `control-plane/target/parity-control/` before generation-specific directories are created so Spotlight does not race the first replay writes.
3. Capture `commit_sha`, `tree_id`, and the exact porcelain status snapshot before cleanup or replay work begins. Derive `tree_clean`, `status_snapshot_sha256`, and `status_snapshot_line_count` from that one capture and carry them through every generated artifact.
4. Create generation-scoped work, shadow, reports, and publication staging roots. Ordinary gate execution never writes to tracked `docs/reference` paths.
5. Before ephemeral cleanup begins, publish blocked replacement runtime row and detail artifacts with `overall_status = blocked_in_progress` and `publication_state = revoked_for_rerun` for the new `publication_generation_id`.
6. Install SIGINT, SIGTERM, and gate-deadline handling before cleanup begins. The handler must know the active process-group identity or an explicit proof that all work is in-process and synchronously joined.
7. Cleanup removes only abandoned generation-scoped work, reports, and shadow directories. It never removes `control-plane/target/parity/publication/current/` or anything under `control-plane/target/parity-control/`.
8. When removing an abandoned `parity.sqlite`, remove the `.sqlite`, `.sqlite-wal`, and `.sqlite-shm` files together. If checkpoint or open is impossible after a crash, remove all three unconditionally rather than attempting recovery on abandoned-generation data.
9. Replay each fixture offline into `control-plane/target/parity/work/<publication_generation_id>/<fixture_id>/parity.sqlite` with a 60 second replay deadline.
10. Run `PRAGMA wal_checkpoint(TRUNCATE)` before GraphQL and MCP readback, verify zero busy pages, and fail closed if busy pages remain after one retry with closed read handles.
11. Re-read the replayed run through the exact GraphQL and MCP subset from Section 6.5. Readback has its own per-fixture 30 second deadline and must update `current-step.json` with the active surface.
12. Run live-shadow validation with a 60 second deadline. Shadow execution may write only inside `control-plane/target/parity/shadow/<publication_generation_id>/`.
13. On timeout or SIGINT or SIGTERM, send `SIGTERM` to the tracked process group, wait at most 30 seconds of bounded drain, then send `SIGKILL` to the same group if any descendant is still alive. Lease release and abandoned-root cleanup are forbidden until `waitpid(-pgid, ...)` and birth-time revalidation prove descendant absence.
14. If descendant absence cannot be proven, publish blocked runtime evidence with `overall_status = blocked_timeout` or `blocked_interrupted`, preserve the generation root, keep the lease unreclaimed for that owner identity, and name the stuck descendant identity in the diagnostic output.
15. Successful ready publication is legal only after descendant quiescence is already true, runtime row and detail artifacts are published atomically for the same generation, and control metadata still matches the active owner identity.

Successor reclaim matrix after abnormal exit:

- Case A: the recorded owner PID is still alive and its current birth time matches `process_birth_unix_ms`. A successor must not reclaim. It publishes or preserves `blocked_in_progress` and exits fail-closed.
- Case A1: the recorded owner PID is still alive, birth time matches, and both heartbeat time and `control_sequence` are fresh. A successor must not reclaim. It publishes or preserves `blocked_in_progress` and exits fail-closed.
- Case A2: the recorded owner PID is still alive and birth time matches, but heartbeat time is older than the freshness window or `control_sequence` has not advanced across two observation intervals. A successor must not reclaim or clean up. It must write a reclaim marker with `overall_status = blocked_manual_recovery`, preserve the blocked generation, and surface the stale owner identity plus freshness evidence.
- The freshness window is bounded by the smaller of 2 minutes and one quarter of the remaining global gate deadline. Observation intervals are 30 seconds. A single missed heartbeat is diagnostic only; two consecutive stale observations are required before escalation to `blocked_manual_recovery`.
- `blocked_in_progress` is therefore bounded: it is legal only while owner identity is fresh or while the successor is still inside the two-observation freshness check. It is not legal as an indefinite terminal state.
- Case B: the recorded owner PID is gone, but process-group metadata is missing, unreadable, incomplete, or schema-invalid. A successor must not reclaim. It writes a reclaim marker with `overall_status = blocked_manual_recovery`, preserves the blocked generation, and surfaces the unresolved owner identity to the operator.
- Case C: the recorded owner PID is gone, process-group metadata exists, but descendant liveness for the recorded process group cannot be disproven or the group is still observable. A successor must not reclaim. It writes a reclaim marker with `overall_status = blocked_manual_recovery`, preserves the blocked generation, and names the lingering process-group identity in diagnostics.
- Case D: the recorded owner PID is gone, process-group metadata exists, and descendant absence is proven by recorded process-group inspection plus birth-time revalidation. A successor may reclaim, but only after writing a reclaim marker, rotating to a new `publication_generation_id`, and leaving the old generation preserved until the new invocation reaches either blocked publication or ready publication.

Manual-recovery rules:

- `blocked_manual_recovery` is a terminal blocked state for the abandoned generation, not a transient spinner state.
- `blocked_manual_recovery` is also required for a live-but-stalled owner when heartbeat freshness or `control_sequence` progress fails the bounded observation rule.
- Live-but-stalled diagnostics must include owner PID, `process_birth_unix_ms`, last heartbeat timestamp, current wall-clock timestamp, last observed `control_sequence`, observation count, freshness window, preserved generation root, and reclaim marker path.
- Automatic cleanup, lease reuse, and SQLite-path reuse are forbidden while the latest reclaim marker is `blocked_manual_recovery`.
- The CLI and runtime detail artifact must surface the unresolved lease owner fields, any recorded process-group identity, the preserved generation root, and the next human action.

### 6.4 Retention and pruning policy

- `blocked_manual_recovery` generations are never auto-pruned. They require explicit operator action because their diagnostic value is the reason reclamation is blocked.
- Non-manual-recovery generations may be pruned oldest-first only when no active lease points at them and at least one newer generation has reached blocked publication or ready publication.
- Automatic pruning retains, at minimum, the newest ready generation, the newest non-manual blocked diagnostic generation, and every `blocked_manual_recovery` generation.
- If retained parity generations exceed 500 MB, the CLI must warn and list preserved roots plus sizes. Hitting the storage budget is diagnostic, not authorization to delete manual-recovery evidence.

### 6.5 Northbound readback contract and acceptance subset

Normative GraphQL binding:

- `graphql_readback` is bound to the executable query readback subset produced by `fixture_graphql_readback_expected(...)` in `control-plane/crates/engine/tests/proposal_041_parity.rs`.
- The authoritative query names and argument names for P041 are `run(id:)`, `runs(ideaId:)`, `stages(runId:)`, `artifacts(runId:)`, `runQueueSummary(runId:)`, and `stageQueueSummary(stageExecutionId:)`.
- Subscription naming is part of Phase C acceptance even though P041 parity fixtures are query-readback driven. `runStatusChanged(runId:)` and `stageStatusChanged(runId:)` must be aligned in `scripts/test-gate.sh`, `scripts/p031-thin-ui-gate.py`, `docs/reference/query-projections-and-client-consumption-contract.md`, and `docs/reference/p031-schema-decision-record.json` in the same cutover. The proposal does not claim that live subscription traffic becomes a new fixture-level parity surface in this revision.

Normative MCP binding:

- `mcp_report_readback` is bound to the executable MCP parity contract in `fixture_mcp_readback_expected(...)`.
- The authoritative MCP surfaces for P041 are `reports.get` and `report://{run_id}` only.

Diagnostic rule: any missing producer artifact, missing live-shadow report, collector-owner mismatch, provenance mismatch, generation-id mismatch, missing required surface, shadow-path escape, row or detail disagreement, or live-checkout mismatch fails closed and names the exact fixture, surface, owner, step, and path.

### 6.6 P031 handoff contract and future-client boundary

Decision 1: the canonical same-tree P031 acceptance switch is `control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json`, not a per-rerun rewrite of tracked `docs/reference/p031-phase-0-artifact-manifest.json`.

Decision 2: the canonical runtime detail artifact is `control-plane/target/parity/publication/current/p031-p041-parity-evidence.json`. `docs/reference/p031-p041-parity-evidence.json` is a promoted snapshot only.

Decision 3: Phase C is atomic across the whole P031 contract family. The same change must update:

- `scripts/p031-thin-ui-gate.py`.
- `scripts/test-gate.sh` closeout logic and the `proposal-041` lane.
- `control-plane/crates/engine/tests/proposal_041_parity.rs`.
- `docs/reference/test-gates.md`.
- `docs/reference/query-projections-and-client-consumption-contract.md`.
- `docs/reference/p031-phase-0-artifact-manifest.json` by adding row `p041_parity_evidence`.
- `docs/reference/p031-p041-parity-evidence.json`.
- `docs/reference/p031-schema-decision-record.json`, which remains manifest-required and must be updated to the same lower-camel GraphQL naming, current governing-contract pointer, and runtime-handoff statement. If the repo later wants to retire that artifact, retirement must happen atomically with a manifest update in the same change.

Decision 4: downstream consumers must compare the published runtime evidence against the live checkout at evaluation time. `scripts/p031-thin-ui-gate.py` and any `scripts/test-gate.sh` closeout logic that accepts `ready_same_tree_verified` must capture current `HEAD` and `HEAD^{tree}` when reading the runtime row and detail artifact, and must fail closed unless published `commit_sha`, `tree_id`, `tree_clean`, and `status_snapshot_line_count` still match the live checkout.

Decision 5: if P041 readiness is ever shown in SwiftUI or AppKit, the UI must consume a daemon-owned GraphQL read surface derived from the runtime artifacts, not the runtime files directly.

### 6.7 Ownership and versioning policy

- Fixture schema and required fixture inventory are owned by the Rust engine owner. Semantic change requires fixture regeneration and updated evidence.
- `server-replay.v1`, `behavioral-diff-report.v1`, `live-shadow-report.v1`, `p031-phase-0-runtime-manifest-row.v1`, and `p031-p041-parity-evidence.v1` are versioned runtime contracts. Field removal, rename, or type change requires a version bump and consumer audit.
- The GraphQL readback subset and P031 contract-family naming are owned jointly by the GraphQL server owner and the P031 release owner. Any argument-name or subscription-name change used here requires same-change updates to reference docs, gates, and proposal closeout artifacts.
- The runtime row and runtime detail artifact are owned jointly by the P031 release owner and Rust control-plane owner. Ready publication cannot advance unless both artifacts and the live checkout agree.

## 7. Rollout and implementation phases

### Phase A: Proposal rerun completion

- Replace the run-local proposal payload with this revision.
- Emit `revision-summary.md` and `feedback-coverage.json` for this refine pass.

### Phase B: Runtime publication and operator-surface hardening

- Add the cleanup-safe `parity-control` root, atomic control-file publication contract, runtime row schema, shared status vocabulary, summary-grid contract, and narrow-terminal fallback contract.
- Add live descendant identity, bounded-drain plus forced-termination behavior, and quiescence proof before lease release or abandoned-root cleanup.
- Add generation-scoped work, reports, shadow, staging publication, and runtime current publication roots.

Exit criteria:

- Same-tree evidence cannot race another same-tree invocation.
- Dirty worktrees cannot publish ready evidence.
- Operators see one consistent status and active-state model across CLI, JSON, and the optional markdown companion.

### Phase C: Atomic P031 runtime cutover and reference promotion

- Add the runtime row and runtime detail artifact.
- Update every artifact named in Section 6.6 in one change, including `docs/reference/p031-schema-decision-record.json`.
- Add the explicit promotion helper so published runtime evidence can be copied into tracked `docs/reference` snapshots without changing generation metadata.
- Make downstream acceptance fail closed on stale checkout provenance even when row and detail still agree internally.

Exit criteria:

- P031 has one authoritative runtime acceptance switch and one authoritative runtime detail artifact.
- No repo-owned consumer still trusts tracked docs snapshots or runtime files directly as the live client integration seam.
- No repo-owned consumer accepts ready publication unless row, detail, and live checkout all match.

### Phase D: Maintenance closeout

- Publish `docs/reference/p041-generated-artifact-schemas.md`.
- Keep or remove the markdown companion based on reviewer need, but only as generated non-authoritative output if retained.
- Consider Time Machine exclusion or `CACHEDIR.TAG` after the core correctness contract is stable; this is optional hygiene, not a closeout blocker.

## 8. Validation and acceptance criteria

P041 is ready for implementation closeout and aggregate re-review when all of the following are true:

- `./scripts/test-gate.sh proposal-041` passes on the same clean tree for all seven required fixtures and all six required surfaces.
- The full gate is bounded by a 25 minute deadline, each fixture replay path is bounded by 60 seconds, each fixture readback path is bounded by 30 seconds, each fixture live-shadow path is bounded by 60 seconds, and timeout drain is bounded to 30 seconds before forced termination.
- Every generated parity JSON artifact carries top-level `overall_status`, nested provenance, and the active `publication_generation_id`.
- The runtime row and runtime detail artifact both carry `publication_state`, both are versioned, `row.schema_version` validates independently, `detail.schema_version` validates independently, and `row.detail_schema_version == detail.schema_version`.
- `row.validation_status == detail.overall_status`, and the CLI final summary status projects that same canonical status without introducing a parallel state machine.
- `scripts/p031-thin-ui-gate.py` and P041-consuming `scripts/test-gate.sh` closeout logic capture live `HEAD` and `HEAD^{tree}` at evaluation time and fail closed unless the published runtime artifacts still match the live checkout.
- `control-plane/target/parity-control/` survives cleanup, every canonical control file uses the same-directory temp-file plus durable flush plus atomic rename contract, and stale-owner reclaim uses PID plus `process_birth_unix_ms` plus recorded process-group identity rather than PID liveness alone.
- Timeout and interruption validation includes a stubborn-descendant case that proves no lease release or abandoned-root cleanup happens before descendant absence is established or forced termination completes.
- Stale-owner validation includes an alive-but-stalled owner case: PID and birth time still match, but heartbeat and `control_sequence` do not advance across two 30 second observations. Expected result is `blocked_manual_recovery`, preserved generation root, no cleanup, no lease reuse, and diagnostics naming the stale owner fields.
- Successor reclaim validation covers all four reclaim-matrix cases, including the manual-recovery park path for missing or unreadable process-group metadata.
- Abandoned-generation cleanup removes `.sqlite`, `.sqlite-wal`, and `.sqlite-shm` together.
- `docs/reference` files are not modified by ordinary gate execution. They change only through the explicit promotion step from a published runtime generation.
- After Phase C, `scripts/test-gate.sh`, `scripts/p031-thin-ui-gate.py`, `docs/reference/query-projections-and-client-consumption-contract.md`, `docs/reference/p031-schema-decision-record.json`, `docs/reference/p031-phase-0-artifact-manifest.json`, and `docs/reference/test-gates.md` agree on the same runtime-versus-reference handoff and GraphQL naming contract.
- No new app-owned persistence, SwiftUI authority, or filesystem-based readiness inference is introduced.

## 9. Metrics and operational guardrails

| Metric | Definition | Target | Owner |
|---|---|---|---|
| Ready publication from dirty tree | Ready publications where `tree_clean != true` or `status_snapshot_line_count != 0` | Zero | P031 release owner |
| Runtime row/detail/live-checkout agreement | Cases where row and detail agree with each other but not with the live checkout, or vice versa | Zero | P031 release owner |
| Lease or quiescence false release rate | Cases where lease release, reclaim, or cleanup occurs before descendant absence is proven | Zero | Rust control-plane owner |
| Manual-recovery false reclaim rate | Cases where a successor reclaimed despite missing, unreadable, or ambiguous owner metadata | Zero | Rust control-plane owner |
| Gate wall-clock duration | Full seven-fixture runtime including cleanup, readback, shadow validation, report synthesis, and publication | Warn above 20 minutes; fail above 25 minutes | Rust control-plane release owner |
| Readback diagnosis latency | Time from stuck readback to naming exact fixture and surface | Under 5 minutes via `current-step.json` and final diagnostics | Rust control-plane owner |
| Fixture freshness | Time from parity-contract change to regenerated checked-in fixture evidence | Within 5 business days | Rust engine owner |
| False-green closeout rate | Rollout decisions made while runtime evidence was stale, blocked, mismatched, interrupted, timed out, dirty-tree generated, or parked in manual recovery | Zero | P031 release owner |
| Artifact storage budget | Total retained parity DB and report artifact footprint for all fixtures | Under 500 MB unless amended by proposal | Rust engine owner |

## 10. Risks and mitigations

| Risk | Why it matters | Mitigation |
|---|---|---|
| Old consumers still trust tracked docs snapshots as live authority | P031 could progress while runtime evidence is blocked or revoked | Phase C updates the named repo-owned consumers and makes live-checkout comparison mandatory |
| Row, detail, and current checkout drift independently | Internally consistent but stale evidence can look green | Require row/detail/live-checkout agreement, independent schema validation, and fail closed on any mismatch |
| Timeout or abnormal exit leaves descendant replay or shadow work alive | The next rerun can hit WAL busy pages, cleanup races, or spurious missing evidence | Track process-group identity, force terminate after bounded drain, and forbid reclaim or cleanup until descendant absence is proven |
| Crash recovery leaves incomplete owner metadata | A successor could misclassify an unsafe generation as reclaimable | Park in `blocked_manual_recovery`, preserve the generation, and surface exact missing metadata instead of guessing |
| Atomic write assumptions degrade on macOS or APFS edge cases | Partial control files or copy-plus-delete renames can corrupt ownership truth | Require same-directory temp files, atomic rename, Darwin durability notes, and explicit control-file validation |
| Future Apple-platform UI reads runtime files directly | The thin-client GraphQL boundary and auth semantics erode | State a hard rule that any future readiness UI must use a daemon-owned GraphQL read surface derived from the runtime artifacts |
| Longer gate budget hides regressions | Slow failures could become normal | Keep per-fixture caps unchanged, make slack explicit, and warn above 20 minutes |

## 11. Explicit reviewer feedback resolution and disagreements

No reviewer disagreement is silently ignored in this revision.

- `API-P041-R9-01` and `SLB-P041-R9-01`: resolved by Section 6.2 and Section 8. The proposal now distinguishes `row.schema_version`, `detail.schema_version`, and `row.detail_schema_version`, and it states the exact compatibility rule explicitly.
- `API-P041-R9-02` and `SLB-P041-R9-04`: resolved by Section 5.1, Section 6.2, and Section 8. The proposal now states `row.validation_status == detail.overall_status`, and the CLI final summary is explicitly a projection of that same canonical status.
- `REL-P041-R9-01`, `SLB-P041-R9-02`, and `SLB-P041-R9-05`: resolved by Section 5, Section 6.3, and Section 8. The successor reclaim matrix now covers owner-still-alive, owner-gone with missing metadata, owner-gone with observable descendants, and owner-gone with proven descendant absence, and ambiguous cases park in `blocked_manual_recovery` rather than reclaiming optimistically.
- `REL-P041-R9-02`: resolved by Section 6.4 and Section 9. The proposal now adds a deterministic pruning rule and makes it explicit that storage pressure never authorizes deletion of manual-recovery evidence.
- `REL-P041-R10-01` and `SLB-P041-R10-01`: resolved by Section 6.3 and Section 8. The proposal now bounds live-but-stalled ownership with heartbeat and `control_sequence` freshness, escalates stale live owners to `blocked_manual_recovery`, requires detailed stale-owner diagnostics, and adds an explicit validation case.
- `AGG-P041-R9-THRESH-01` and `SLB-P041-R9-03`: resolved by Section 5.1. The proposal now adds the status-to-prefix-to-grid mapping, a normative grid layout, a narrow-terminal reference transcript, timeout and missing-evidence examples, a passing-run footer with row and detail paths, and an explicit statement that `SKIP` is reserved.
- `UI-P041-R9-07`: resolved by explicit disagreement in Section 5. The markdown companion is intentionally non-authoritative and structurally non-normative. That is a deliberate trade-off because reviewers care about runtime truth, not companion-format lock-in.
- `APPLE-P041-01` remains preserved, not weakened. Section 2, Section 6.6, and Section 8 continue to require any future SwiftUI or AppKit parity-readiness surface to consume daemon-owned GraphQL derived from runtime artifacts rather than runtime files.

## 12. Open questions

- Should the runtime row eventually become one entry inside a broader runtime phase-0 manifest JSON, or is a dedicated single-purpose row the better long-term seam?
- Should the promotion helper be a standalone `scripts/parity/promote-p041-reference.sh` command or a documented `scripts/test-gate.sh proposal-041 --promote-reference` mode?
- After all repo-owned consumers move off the markdown companion, should the companion be deleted entirely rather than retained as a generated mirror?
- If the gate remains too slow after the correctness contract lands, should Phase D add bounded fixture parallelism?

## 13. Explicit trade-offs

- Runtime authority moves out of tracked `docs/reference` paths to preserve same-checkout retry safety. The trade-off is one extra runtime publication seam plus an explicit promotion step, but that is preferable to a gate that dirties the checkout it later needs to certify.
- A dedicated `parity-control` root, a `blocked_manual_recovery` park state, and a reclaim matrix add operational detail and implementation cost, but they prevent optimistic cleanup from destroying the evidence needed to recover safely.
- The proposal keeps the markdown companion non-authoritative. That leaves some presentational freedom, but it avoids freezing a convenience report format into the same contract tier as the actual runtime truth.
- A 25 minute global budget reduces timeout-driven false negatives under moderate host pressure, but it also accepts slower failure feedback. The proposal keeps runtime pressure visible by preserving per-fixture caps and warning above 20 minutes.
