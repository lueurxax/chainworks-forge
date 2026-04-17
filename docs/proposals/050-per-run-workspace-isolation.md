# Proposal 050: Per-Run Workspace Isolation

| Field | Value |
|---|---|
| Date | 2026-04-16 |
| Status | Draft |
| Author | Claude |
| Depends on | None. Fixes a live production defect in the current Rust daemon. |
| Scope | Isolate each run's workflow artifacts under `.chainworks/runs/{run_id}/` so that sequential and parallel runs on the same idea/workspace cannot contaminate each other's state, proposals, reviews, or transition conditions. |
| Goal | A new run on a workspace that already contains artifacts from a prior run must start clean — it must not inherit stale `run-state.json`, stale `proposal.md`, or stale reviewer outputs. Parallel runs on the same workspace must write to disjoint paths. |

---

## 1. Context and Motivation

### 1a. The incident

On 2026-04-16, a `full-mvp-live` run was started on `/Users/user/Documents/CryptoSavingsTracker`. The `proposal_writer` agent (state_2) was expected to draft a new proposal. Instead, it immediately began writing Swift code — editing `AssetDetailView.swift`, `AddTransactionView.swift`, and other production files.

Root cause: the workspace contained artifacts from a **prior completed run** (`ux-audit-remediation-close-gaps-2026-04-16`). That run had reached `implementation_complete_pending_burndown` with an approved proposal (score 9.55/10). Its artifacts were still on disk:

- `.chainworks/state/run-state.json` → `status: "implementation_complete_pending_burndown"`
- `.chainworks/proposals/current/proposal.md` → 749-line approved proposal (revision v7)
- `.chainworks/reviews/proposal/` → 10 reviewer artifacts

The new run's state_1 (`normalize_idea_and_open_run`) did not clear these artifacts. State_2's `proposal_writer` read the stale `run-state.json` as its input, saw "proposal approved, implementation in progress", and logically concluded its job was to continue implementing — not to draft a new proposal.

### 1b. The architectural gap

In the Swift app, each run is isolated through `RunWorkspace`:
- `workspaceRoot` → project directory (shared, read-access for agents)
- `artifactRoot` → `{workspaceRoot}/artifacts/` scoped per run (ARCH-026)

In the Rust daemon, there is no equivalent isolation:
- `workspace_root` → project directory (shared, agents get `cwd` here)
- `artifact_root` → flat shared directory, not run-scoped
- `.chainworks/` → **mutable shared state** inside the project, overwritten by every run

This means:
1. **Sequential runs** inherit stale state from the prior run.
2. **Parallel runs** on the same idea would overwrite each other's `run-state.json`, `proposal.md`, and reviewer artifacts in real time.
3. **Cancelled runs** leave orphan artifacts that the next run treats as truth.

### 1c. Why this is not a prompt bug

The `proposal_writer` prompt is correct: *"Draft and refine the proposal from the current idea and all reviewer feedback."* The agent correctly read its inputs (`idea_brief`, `run_state`) and acted on them. The inputs were wrong because the workspace was polluted, not because the prompt was ambiguous.

---

## 2. Design

### 2a. Per-run `CHAINWORKS_META_ROOT`

The existing YAML artifact path templates already use `${CHAINWORKS_META_ROOT:-.chainworks}` as the root for all workflow artifacts (80 occurrences in `agents.yaml`). The default value `.chainworks` resolves to `{workspace_root}/.chainworks/` — a flat, shared directory.

**Fix:** when the orchestrator creates a run, set `CHAINWORKS_META_ROOT` to `.chainworks/runs/{run_id}` instead of `.chainworks`. All artifact path templates resolve through this variable, so every artifact lands in a per-run subdirectory with zero YAML changes.

Layout after the fix:

```
/Users/user/Documents/CryptoSavingsTracker/           ← workspace_root (project)
  .chainworks/
    runs/
      2c003f1d-62df-4c9a-b0c3-1e32c30add45/          ← run A
        state/run-state.json
        proposals/current/proposal.md
        reviews/proposal/product-owner.json
        reviews/proposal/summary.json
        ...
      4294aa32-0834-4f73-8191-2decf5847c83/          ← run B (parallel or prior)
        state/run-state.json
        proposals/current/proposal.md
        ...
    acp-stderr.log                                     ← shared (transport log, not run-scoped)
```

### 2b. Where the change lands

The `CHAINWORKS_META_ROOT` variable is resolved in exactly two places:

1. **`engine/src/orchestrator.rs::resolve_path_template`** (line 2054) — resolves `${CHAINWORKS_META_ROOT:-.chainworks}` via `std::env::var`. This is the orchestrator's path resolver for transition conditions (`exists('artifact_name')`), prompt building, and artifact normalization.

2. **`engine/src/executor.rs`** (lines 250-252) — string-replaces `${CHAINWORKS_META_ROOT:-.chainworks}` in artifact path templates for steward and ACP agent invocations.

Both read the variable from the process environment or fall back to the default `.chainworks`. P050 changes the resolution so it reads from a **per-run field on the `Run` record**, not from the process environment.

### 2c. Implementation

#### Step 1: Add `chainworks_meta_root` field to `Run`

Add to `domain/src/run.rs`:
```rust
pub chainworks_meta_root: Option<String>,
```

Add a corresponding nullable column in a new migration:
```sql
ALTER TABLE runs ADD COLUMN chainworks_meta_root TEXT;
```

#### Step 2: Set `chainworks_meta_root` at run creation

In `engine/src/command_handler.rs`, when processing `Command::StartRun`:
```rust
let meta_root = format!(".chainworks/runs/{}", run_id);
// ... set on Run record:
run.chainworks_meta_root = Some(meta_root.clone());
```

The orchestrator creates the directory on first access (mkdir -p semantics).

#### Step 3: Use run's `chainworks_meta_root` in path resolution

In `resolve_path_template`, change the `CHAINWORKS_META_ROOT` resolution from:
```rust
std::env::var(var_name).unwrap_or_else(|_| default_val.to_string())
```
to a signature that accepts the run's `chainworks_meta_root`:
```rust
pub fn resolve_path_template(template: &str, workspace_root: &str, meta_root: Option<&str>) -> String
```

When resolving `${CHAINWORKS_META_ROOT:-.chainworks}`:
1. If the run has `chainworks_meta_root` → use it.
2. Else if `std::env::var("CHAINWORKS_META_ROOT")` is set → use it.
3. Else → use the default value from the template (`.chainworks`).

In `executor.rs`, the same: replace the string replacement with the run's `chainworks_meta_root`.

#### Step 4: Pass `chainworks_meta_root` to ACP subprocess

The ACP subprocess needs to know the per-run meta root so it writes artifacts to the right place. Two options:

**Option A (env var injection):** Set `CHAINWORKS_META_ROOT={workspace_root}/.chainworks/runs/{run_id}` in the ACP subprocess environment. The agent's YAML artifact paths resolve naturally through `${CHAINWORKS_META_ROOT}`.

**Option B (cwd-relative resolution):** No change — the agent writes to `{cwd}/.chainworks/...` where `cwd` is `workspace_root`. But this defeats isolation.

**Decision: Option A.** The executor sets `CHAINWORKS_META_ROOT` as an environment variable on the ACP subprocess command. Agents that use `${CHAINWORKS_META_ROOT:-.chainworks}` in their paths (all of them) will resolve to the per-run directory. The ACP transport already supports per-session env vars via `session/new.params`.

#### Step 5: Create directory structure on first write

The orchestrator creates `.chainworks/runs/{run_id}/` when the run starts. Subdirectories (`state/`, `proposals/`, `reviews/`) are created on demand by the agent when it writes its first artifact — this matches current behavior where agents mkdir -p their output paths.

### 2d. Transition condition resolution

`exists('git_push_receipt')` and similar transition conditions call `resolve_path_template` with the artifact's YAML path template. Since that template contains `${CHAINWORKS_META_ROOT}`, it resolves to the per-run directory. **No change needed in transition evaluation logic** — the existing `check_artifact_exists` and `evaluate_condition` paths already go through `resolve_path_template`.

### 2e. Backward compatibility

- **Existing runs** created before P050 have `chainworks_meta_root = NULL`. The resolver falls back to `std::env::var` or the template default (`.chainworks`). These runs continue to work exactly as before.
- **New runs** created after P050 always have `chainworks_meta_root = ".chainworks/runs/{run_id}"`. Their artifacts are isolated.
- **No data migration** is needed. Stale artifacts from old runs remain in `.chainworks/` at the project root. They are orphaned but harmless — new runs never look there.

### 2f. What this does NOT change

- **`workspace_root`** — still the project directory. Agents still get `cwd = workspace_root` (or `worktree_root` for write-enabled agents). Code reading/writing happens in the project, not in `.chainworks/`.
- **`artifact_root` on the Run record** — still exists, still used for the daemon's own artifact persistence in the DB. P050 does not change how `artifact_root` works for DB-persisted artifacts.
- **YAML artifact path templates** — zero changes. All 80 `${CHAINWORKS_META_ROOT:-.chainworks}/...` references work as-is.
- **Agent prompts** — zero changes.
- **Worktree provisioning** — unchanged; worktrees are already per-run by design (P007).

---

## 3. Files to Modify

| File | Change | Lines (approx.) |
|---|---|---|
| `domain/src/run.rs` | Add `chainworks_meta_root: Option<String>` field | ~2 |
| `db/migrations/012_per_run_meta_root.sql` | `ALTER TABLE runs ADD COLUMN chainworks_meta_root TEXT` | ~1 |
| `db/src/repos/runs.rs` | Include `chainworks_meta_root` in insert/find/update queries | ~10 |
| `engine/src/command_handler.rs` | Set `chainworks_meta_root = ".chainworks/runs/{run_id}"` on `StartRun` | ~3 |
| `engine/src/orchestrator.rs` | Change `resolve_path_template` to accept `meta_root: Option<&str>`; pass run's `chainworks_meta_root` at all call sites | ~20 |
| `engine/src/executor.rs` | Use run's `chainworks_meta_root` for artifact path resolution; inject `CHAINWORKS_META_ROOT` env var into ACP subprocess | ~15 |
| `acp/src/transport.rs` | Accept and forward `CHAINWORKS_META_ROOT` env var to subprocess | ~5 |
| `engine/tests/integration.rs` | Add focused isolation tests | ~60 |

---

## 4. Acceptance Criteria

1. A new run on a workspace that already has `.chainworks/state/run-state.json` from a prior run does not read the stale state. The new run's `state_2` agent sees an empty/fresh meta root.
2. Two concurrent runs on the same workspace write artifacts to disjoint directories (`.chainworks/runs/{run_id_A}/` and `.chainworks/runs/{run_id_B}/`).
3. `exists('proposal_current')` transition condition resolves against the current run's meta root, not the shared `.chainworks/` root.
4. Cancelling a run leaves its artifacts under `.chainworks/runs/{cancelled_run_id}/` without affecting other runs.
5. Existing runs with `chainworks_meta_root = NULL` continue to work (backward compatible fallback to `.chainworks`).
6. The `proposal-050` gate is green on the same tree.

---

## 5. Test Gate

### test-gates.md Entry

```
### `proposal-050`

Per-run workspace isolation gate.

Command:

\`\`\`bash
./scripts/test-gate.sh proposal-050
\`\`\`
```

### Focused test inventory

```
test_new_run_gets_isolated_meta_root
test_resolve_path_template_uses_run_meta_root_over_env
test_stale_workspace_artifacts_not_visible_to_new_run
test_transition_condition_resolves_against_run_meta_root
test_null_meta_root_falls_back_to_default
```

---

## 6. Out of Scope

- **Cleanup of stale `.chainworks/` artifacts from pre-P050 runs.** These are orphaned but harmless. A future `compact` command (P038) may clean them.
- **Steward analysis isolation.** Steward operates outside the run lifecycle and has its own IO root. Not affected by P050.
- **Swift app changes.** The Swift app already has per-run isolation via `RunWorkspace`. P050 is daemon-only.
- **`artifact_root` restructuring.** The DB-level `artifact_root` field stays as-is. P050 only isolates the filesystem meta root that YAML artifact paths resolve through.
