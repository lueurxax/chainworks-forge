# Proposal 089: Managed Temporary Artifact Lifecycle

| Field | Value |
|---|---|
| Date | 2026-05-16 |
| Status | Draft |
| Author | Codex |
| Depends on | P037 ACP execution supervision, P076 auto-retry observation ledger, P083 execution-truth ownership invariants, P088 code-writer completion receipts |
| Related | `docs/reference/rust-control-plane.md`, `docs/reference/test-gates.md`, `scripts/test-gate.sh`, Rust ACP provider runtime homes |
| Scope | Bound uncontrolled temporary file growth by making Chainworks temporary artifacts owned, discoverable, and lifecycle-managed. |
| Non-goal | No daemon-wide hard quota/watermark policy, no provider token-budget policy, no deletion of active worktrees, no weakening of failure evidence preservation. |

---

## 1. Problem

Long unattended run batches can consume disk space even when the control plane is behaving correctly.
The immediate observed source is not run artifacts or SQLite metadata; it is unmanaged temporary build and provider runtime output:

- timestamped Xcode `DerivedData` and `.xcresult` directories under `chainworks-test-gates`;
- copied provider toolchain/runtime homes under `chainworks-forge-toolchains`;
- occasional cached build outputs created outside a durable run-owned lifecycle.

These directories are expensive and currently weakly owned:

- the operator can see their paths only after disk pressure appears;
- cleanup has to infer safety from names, mtimes, and live process checks;
- successful attempts can leave large temporary outputs behind indefinitely;
- failed attempts need diagnostic preservation, but not full runtime-home preservation;
- automation can retry useful runs for hours while silently accumulating stale temporary files.

The result is operationally fragile: the system can make progress but still fill the disk before the operator returns.

## 2. Goals

- Route Chainworks-created temporary files through a managed temp root instead of scattered process-local paths.
- Attach first-class ownership metadata to every large temporary artifact tree.
- Make cleanup deterministic: delete by lifecycle state and manifest, not by ad-hoc filename guessing.
- Preserve useful failure diagnostics while deleting bulk runtime/build data that is no longer needed.
- Keep active runs, active test gates, active provider sessions, and active worktrees safe by construction.
- Give unattended automation a safe cleanup path before retries continue.

## 3. Non-Goals

- Do not implement hard disk quotas, watermarks, or global launch blocking in this proposal.
- Do not delete `.chainworks/worktrees` for active or nonterminal runs.
- Do not delete the daemon build target while a running daemon uses it.
- Do not replace P088/P090-style failure receipts; this proposal manages storage lifecycle, not output-contract semantics.
- Do not rely on provider-specific session formats as the only cleanup truth.

## 4. Evidence

The current observed disk pressure pattern:

- `/var/folders/.../T/chainworks-test-gates` reached roughly hundreds of GiB from repeated `test-gate` DerivedData and `.xcresult` folders.
- `/var/folders/.../T/chainworks-forge-toolchains` reached multiple GiB from provider runtime/toolchain temp copies.
- Active processes held only a small subset of those directories open.
- Manual cleanup had to exclude live `xcodebuild`, live `test-gate`, and a launched app binary by inspecting processes and open files.

This is enough to show the missing invariant:

> Every Chainworks-created temporary artifact tree must have an owner, lifecycle state, and cleanup rule at creation time.

## 5. Decision

Introduce a managed temporary artifact lifecycle with three parts:

1. a canonical Chainworks temp root;
2. a manifest written at temp tree creation;
3. lifecycle cleanup hooks tied to work item completion, failure, cancellation, and startup recovery.

This intentionally excludes hard quota/watermark enforcement. The first fix is ownership and lifecycle correctness; budget policy can be layered later if still needed.

## 6. Proposed Design

### 6.1 Managed temp root

All Chainworks-owned temporary trees should be created under a single configured root.

Recommended default:

```text
~/Library/Caches/Chainworks Forge/tmp
```

Required subtrees:

```text
tmp/test-gates/
tmp/provider-runtime/
tmp/provider-toolchains/
tmp/build-cache/
tmp/scratch/
```

Rules:

- new test-gate DerivedData and `.xcresult` outputs go under `tmp/test-gates`;
- copied provider homes/toolchains go under `tmp/provider-runtime` or `tmp/provider-toolchains`;
- per-attempt scratch directories go under `tmp/scratch`;
- legacy `/var/folders/.../T/chainworks-*` paths remain readable for cleanup migration but are no longer the preferred creation target.

### 6.2 Temporary artifact manifest

Every large temporary tree must include a machine-readable manifest at creation time:

```text
.chainworks-temp.json
```

Minimum schema:

```json
{
  "schema_version": "chainworks_temp_artifact.v1",
  "artifact_id": "uuid-or-stable-id",
  "kind": "test_gate_derived_data",
  "owner_kind": "work_item",
  "run_id": "optional-run-id",
  "stage_execution_id": "optional-stage-execution-id",
  "work_item_id": "optional-work-item-id",
  "agent_execution_id": "optional-agent-execution-id",
  "created_at": "2026-05-16T18:00:00Z",
  "last_touched_at": "2026-05-16T18:00:00Z",
  "lifecycle_state": "active",
  "preserve_on_failure": false,
  "diagnostic_extractors": ["xcode_result_summary"],
  "safe_to_delete_after": null,
  "root_path": "/absolute/path"
}
```

Allowed `kind` values initially:

- `test_gate_derived_data`
- `test_gate_xcresult`
- `provider_runtime_home`
- `provider_toolchain_copy`
- `build_cache`
- `scratch`

Allowed `lifecycle_state` values:

- `active`
- `completed_success`
- `completed_failure_preserved`
- `cancelled`
- `orphaned`
- `deleted`

### 6.3 Creation APIs

Add a shared temp manager in the Rust control plane.

Expected responsibilities:

- allocate managed temp paths;
- write the manifest before subprocess launch;
- update `last_touched_at`;
- attach run/stage/work/agent ownership when available;
- expose cleanup operations used by executor, recovery, and MCP diagnostics.

The Swift app and shell scripts should not invent unmanaged temp paths for Chainworks-owned heavy outputs.
For `scripts/test-gate.sh`, add an environment override such as:

```text
CHAINWORKS_TEMP_ROOT
CHAINWORKS_TEST_GATE_TEMP_ROOT
```

When unset, the script may keep a local fallback, but daemon-launched test gates must set the managed temp root.

### 6.4 Lifecycle cleanup

Cleanup must be event-driven first and startup-driven second.

On successful work item completion:

- delete bulk `provider_runtime_home` unless explicitly marked for retention;
- delete `test_gate_derived_data`;
- preserve only compact evidence such as summaries, logs, and result bundles explicitly promoted into durable run evidence;
- mark manifest as `deleted` or remove the tree entirely.

On failed work item completion:

- run diagnostic extractors first;
- preserve compact failure evidence in durable run evidence paths;
- preserve provider session/transcript subtrees when required by provider-specific failure capture;
- delete bulk caches, build intermediates, copied package stores, and toolchain copies;
- mark any preserved temp tree as `completed_failure_preserved` with a bounded diagnostic reason.

On cancellation or stale retry supersession:

- if no active process owns the temp tree, delete it after a short grace period;
- if still open, mark it `orphaned` and let startup recovery retry cleanup.

On daemon startup:

- scan managed temp root and legacy `chainworks-*` temp roots;
- for manifest-backed trees, reconcile lifecycle state against DB work item and agent execution truth;
- for legacy unmanifested trees, classify conservatively by name, mtime, and open-file check, then either migrate to an `orphaned` manifest or delete if clearly stale.

### 6.5 Failure evidence preservation

The cleanup path must not destroy evidence needed for root-cause analysis.

Provider-specific preservation rules:

- Codex: preserve `~/.codex/sessions` or the provider session archive when failure diagnostics require it; do not preserve the full runtime home by default.
- Junie: preserve `~/.junie/sessions` or equivalent session events when available; do not preserve copied caches/toolchains by default.
- Claude ACP: preserve ACP transcript/session events and compact stderr/stdout receipts where available; do not preserve package caches by default.
- Xcode/test-gate: preserve `.xcresult` only when it is promoted as failure evidence; otherwise extract summary logs and delete full DerivedData.

The invariant is:

> Failure keeps diagnostic facts, not unbounded temporary bulk.

### 6.6 Operator and automation readback

Add readback fields to run reports or diagnostics:

- current managed temp usage by run;
- active temporary artifacts for a run/stage;
- preserved failure temp artifacts and reason;
- orphaned temp artifacts pending cleanup;
- last cleanup action and deletion count.

The existing retry-triage automation can then clean safely through the managed lifecycle instead of ad-hoc `find` rules.

## 7. Implementation Plan

1. Add Rust temp manager module in the control plane.
2. Add temp manifest type and JSON serializer.
3. Route provider runtime home/toolchain temp creation through the temp manager.
4. Route daemon-launched test-gate temp paths through `CHAINWORKS_TEST_GATE_TEMP_ROOT`.
5. Update `scripts/test-gate.sh` to honor the managed temp root for DerivedData and `.xcresult`.
6. Add work item completion hooks for success/failure/cancel cleanup.
7. Add startup recovery scan for manifest-backed and legacy `chainworks-*` temp roots.
8. Add diagnostics readback to reports or a focused MCP diagnostic tool.
9. Update `chainworks-blocked-run-triage` and `chainworks-orchestrator-ops` skills to use managed cleanup readback instead of raw filesystem heuristics.

## 8. Acceptance Criteria

- A daemon-launched test gate creates DerivedData and `.xcresult` under the managed temp root with `.chainworks-temp.json`.
- A provider runtime home/toolchain temp tree is manifest-backed and linked to run/stage/work/agent identity when available.
- Successful work item completion deletes bulk temp directories and preserves only durable run evidence.
- Failed work item completion preserves compact diagnostics/session evidence and deletes unrelated bulk cache/build data.
- Startup recovery can clean stale manifest-backed temp trees without touching active processes.
- Legacy `/var/folders/.../T/chainworks-test-gates` and `chainworks-forge-toolchains` are detected and reported until migrated or cleaned.
- Reports expose enough temp ownership detail that an operator can see which run owns current temp usage.
- No cleanup path deletes active `.chainworks/worktrees`, active provider sessions, or a DerivedData directory currently held by a live `xcodebuild`/app process.

## 9. Tests

Add focused Rust tests for:

- manifest creation with run/stage/work/agent ownership;
- successful work item cleanup deletes bulk temp and keeps promoted evidence;
- failed work item cleanup preserves provider session diagnostics but deletes copied caches;
- startup recovery marks/deletes stale manifest-backed temp trees;
- active-process guard refuses deletion when a temp tree is still in use;
- legacy temp root scanner reports unmanifested `chainworks-test-gates` and `chainworks-forge-toolchains` trees.

Add shell/script tests for:

- `scripts/test-gate.sh` honors `CHAINWORKS_TEST_GATE_TEMP_ROOT`;
- DerivedData path remains unique per invocation but under the managed root;
- `.xcresult` preservation behavior follows success/failure outcome.

## 10. Rollout

1. Implement manifest-backed temp creation for new daemon-launched work only.
2. Add read-only diagnostics for existing legacy temp roots.
3. Enable success-path cleanup for managed temp trees.
4. Enable failure-path compact preservation and bulk cleanup.
5. Enable startup recovery for manifest-backed stale trees.
6. Enable conservative legacy cleanup only after diagnostics prove the classifier is safe.

## 11. Open Questions

- Should the managed temp root live under `~/Library/Caches/Chainworks Forge/tmp` or `.chainworks/tmp` for easier per-repo cleanup?
- Which failure classes require preserving full `.xcresult` versus only extracted summaries?
- Should preserved provider sessions be compressed immediately or left as directories for easier inspection?
- Should Swift app-local test gates use the same temp manager or only honor the script environment override?
