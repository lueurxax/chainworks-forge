# Blocked-Run Carry-Forward Live Rollout

Date: 2026-09-21. Status: published and deployed; P095 preview held before
reservation. Full migration and proposal retirement remain incomplete.

## Authorized Scope

The operator authorized merge to main, GitHub push, a compatible daemon update
with a verified backup, and one P095 carry-forward ending at a fresh approval.
This does not authorize that approval, project trust, Apple native consent,
release, source cancellation or cleanup. The operator separately authorized
`p039-migration-operator` with six continuation capabilities and state reads.
It was created read-only for the old binary, extended only after the supporting
binary started, then removed after the held preview. Existing principals and
their policies were preserved. Removal was read back from the principal source;
an initialize-only probe is not treated as proof of authenticated revocation.

## Integration Identity

- Accepted offline implementation: `59869a1d008d364c52da9d8d76c170de20d0decf`.
- Current main/project-trust fix: `da3992474c2d753a5c298f73c6b65915d3b98d1a`.
- Clean automatic merge: `366643e2244f4e8820f16a8a5279eb2b9de018a2`.
- Independent focused merge review found no actionable findings in the
  executor, orchestrator, recovery, report and gate overlap. This is source
  review, not live acceptance.
- The original checkout remains on its preservation branch with unrelated
  local edits. Integration uses the separate P039 and clean main worktrees.
  Only its previously clean `examples/workflows/workflow.yaml` was synchronized
  with main's eight-line continuation opt-in. Both live definition files were
  then byte-identical to main. The P095 checkout was not changed.

## Pre-Deployment Readback

The installed daemon reports `ready`, build `da3992474c2d753a5c298f73c6b65915d3b98d1a`,
schema/binary schema 100, PID 54061, and a healthy Xcode broker with zero active
leases, queued leases or backend sessions. Its app parent is PID 54044. No daemon
child processes were present. Initial MCP readback reported eight blocked runs, zero pending
approvals and no unresolved release effects for P095.

Immediately before restart P070 C became `running`. Fresh `runtime.health`
still showed zero active sessions/continuations/unresolved effects, and the
daemon had no children. The app exited normally; SIGTERM drained daemon 54061
and port 4000 was confirmed free. No P070 retry, cancellation or approval was
issued by this rollout.

Storage writer health is healthy, queue depth zero. The pre-existing
`run_summaries` projection poison and rollout freshness hold are not repaired or
presented as caused by carry-forward.

P095 source: `bd83a310-360f-4d45-b4c6-a94873de3733`, idea
`d61d59b4-254a-4cb6-aea3-3a315d171c69`, blocked at
`state_7_implementation_started`. Its dedicated checkout remains clean at
`82b1d72581e6503e5e19a40e5a1164b2b9f2ae3b`. Both current and approved proposal
files have SHA-256
`04ecdd50e44cbcae1d0f1ce1dab6cc31280cce615551cda0dd61c25ba4523d44`.
This does not establish historical finding completeness or preview eligibility.

## Verification

- Merged-source formatting and whitespace checks pass.
- `xcode-headless-project-trust`: exit 0, 22 tests passed.
- `proposal-039`: exit 0, 348 Rust tests across 37 selections, zero failures or
  ignored tests; three checker tests; 29 cases/164 distinct mapped tests; lint PASS.
- Managed offline daemon build and workspace/all-targets check: exit 0.
- Updated app and daemon retain the existing Developer ID/team identity and
  entitlements. `codesign --verify --deep --strict` passes before and after install.
- Final documentation whitespace and changed links pass. The full link-check
  output is byte-identical to the prior baseline: 40 existing repository issues
  plus three ignored local review-report file-line links unsupported by the
  helper. Active spec lengths remain 250, 509 and 167 lines; none were retired.

| Private local log | SHA-256 |
| --- | --- |
| `/private/tmp/p039-i4-integrated-gate.log` | `98f4e0288629a64edb80ee37333661f4f776881d24457e0f6ce9b6ed0559973a` |
| `/private/tmp/p039-i4-project-trust-gate.log` | `5b429753e2f7b16480219debfccf184127332e77187e493992ec0252300603bc` |
| `/private/tmp/p039-i4-daemon-build.log` | `f6c497b84ed57b217955c91e061d03737205bff93b5fb78a1c5e2bceacc8df5e` |
| `/private/tmp/p039-i4-all-targets-check.log` | `5632af2a74592b974017b6a4d4ebd3290f21162d80783e46bec200c2385b4fa0` |

The earlier broad-suite baseline failures remain classified in
[offline integration evidence](p039-durable-integration.md); no full-workspace
green result is claimed here.

## Publication And Deployment

Main was fast-forwarded and pushed to GitHub. Remote readback confirmed
`366643e2244f4e8820f16a8a5279eb2b9de018a2`. The later rollout record is docs-only;
it does not change this deployed source identity.

At `2026-09-21T19:24:05Z`, `/ready` reported that exact build, schema and binary
schema 101, state `ready`, PID 94365 (app parent 94356). The Xcode broker is
healthy with no active/queued leases or backend sessions. Storage readback is
fresh/HEALTHY, writer alive, queue zero, transaction P95 1 ms, lock-wait P95 0 ms.
The pre-existing `run_summaries` poison remains visible, not silently cleared.

Admission is explicitly enabled in both installed daemon launch configurations.
The operation root is owned, private mode 0700. Default operator continuation
capabilities were not granted. Project-trust storage remains absent and the
Xcode authority file retains its pre-update SHA-256
`6399cc6cd6f4714c3b2b406197ba951a1491f896aee773fa60696b7af3175f05`.

The preflight created this mode-0600 backup under Application Support:
`control-plane.db.backup-1790018641-v100-to-v101-5b1b07d5-6ed1-48ea-a5ee-d81c4cabdbb7.sqlite`.
An independent copy at
`/private/tmp/cw-p039-i4-deployment-366643e/verified-restore-v100.sqlite`
opens read-only, passes full `PRAGMA integrity_check`, retains schema 100 and
49 run rows, including P095's original blocked status and snapshot hashes.
Both copies have SHA-256
`a78176417f5c3076b79f7f8f74978e5c13d5f15b3dcad1e951e1a44481db7a38`.
The old app is retained at
`/private/tmp/cw-p039-i4-deployment-366643e/previous-Chainworks Forge.app`.
It is not a schema-compatible rollback candidate for the now-upgraded live DB.

## P095 Preview Hold

One live `runs.continuation_preview` returned
`run_carry_forward_preview_hold_v1`, `source_busy`, `next_action: resolve_hold`.
The actual MCP catalog exposed all six strict continuation tools. The preview
selected original artifact `5d4d3b17-cc1b-44f4-8f06-31a120806788`, retained dirty
work, requested no exclusions and used current opt-in definitions and unchanged
sandbox delivery policy. No prepare, activate, abort or reconcile was invoked.

Because public hold readback does not identify contributing rows, bounded
read-only SQL diagnostics inspected the current schema and source-owned records:

| Observation | Result |
| --- | --- |
| Effective-resource peers | P095 only; P070 is not this source's resource owner |
| Source work items | 783 completed, 82 failed, six cancelled; no active work item |
| Stage-owned agent executions | 177 completed, 95 failed, one cancelled |
| Provider-session records | 272 `live` / `running`, all missing PID and process-start identity |
| Provider record timestamps | Created from 2026-08-11; newest update 2026-09-20T11:32:29Z |
| Escalation ledgers | 159 active: 84 on completed stages, 75 on skipped stages; six paused |
| Continuation operation/fence counts for P095 | Zero / zero |
| Directional successor readback | No incoming/outgoing successor |

These provider and escalation rows independently satisfy the current quiescence
guard. Zero live manager sessions or child processes does not authenticate the
fate of historical records without process identities. Conversely, `active`
historical ledgers do not prove useful current execution. No bulk SQL updates,
inferred absence, source cancellation or weakened guard were used.

The existing `provider_session.mark_process_absent` command requires a specific
held identity-ambiguous cancellation intent/epoch and reopens its settlement. It
is not a generic legacy-session cleanup API. Its prerequisites and scope were
not fabricated to make this canary pass.

Both proposal hashes and the clean source checkout remain unchanged after the
preview. Its frozen workflow/catalog hashes still match the verified backup.
Private request/response files are retained under `/private/tmp/p039-i4-*`;
credentials are not included in this evidence or those MCP payloads.

## Remaining Acceptance

Publication, signed deployment, schema migration/backup verification, exact
principal authorization, preview, preparation, activation, fresh review/approval
and post-approval headless execution are separate evidence steps. They are not
inferred from an offline gate or an MCP registration.

The next prerequisite is a bounded, evidence-backed legacy reconciliation path:
establish process fate and settle obsolete escalation ownership without signals
to unidentified processes, fabricated receipts, revived retries or source loss.
Then rerun read-only preview and review any subsequent provenance/policy holds;
those later checks have not been reached and are not claimed to pass.

No P095 continuation operation has been reserved or activated. Preparation,
fresh review/approval and post-approval headless execution remain unproved.
Proposal retirement remains pending applicable live acceptance.
