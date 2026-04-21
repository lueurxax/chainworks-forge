# Per-Run Workspace Isolation

Stable reference for per-run artifact isolation in the Rust control-plane daemon.

This document describes the implemented system. It is not a proposal or future-state design.

Related stable docs:

- [rust-control-plane.md](rust-control-plane.md)
- [runtime-contract.md](runtime-contract.md)
- [full-mvp-delivery.md](full-mvp-delivery.md)
- [project-workspace-contract.md](project-workspace-contract.md)
- [acp-runtime-transport.md](acp-runtime-transport.md)
- [test-gates.md](test-gates.md)

## Purpose

Each run in the Rust daemon gets its own isolated directory for workflow artifacts. Sequential runs on the same workspace cannot inherit stale state from a prior run. Parallel runs on the same workspace write to disjoint paths.

Without isolation, a new run on a workspace that already contains `.chainworks/state/run-state.json` from a prior run will read that stale state and misinterpret its current position in the workflow. The agent responsible for drafting a new proposal may instead see "implementation in progress" and begin writing code.

## Scope

This reference covers:

- the per-run meta-root directory layout
- path resolution rules for `CHAINWORKS_META_ROOT`
- ACP subprocess environment handoff
- worktree normalization exemption for meta-root paths
- transition condition isolation (`exists()` and `artifact.field`)
- artifact normalization source-side isolation
- legacy fallback semantics for pre-isolation runs
- northbound readback contract (GraphQL and MCP)

It does not cover steward analysis isolation (separate lifecycle), Swift app workspace isolation (already present via `RunWorkspace`), or cleanup of stale pre-isolation artifacts.

## Per-run meta-root layout

When the daemon creates a new run, it sets `chainworks_meta_root` to `.chainworks/runs/{run_id}`. All artifact path templates in the agent catalog reference `${CHAINWORKS_META_ROOT:-.chainworks}`, so every artifact lands in a per-run subdirectory with no YAML changes.

```text
/Users/user/Documents/CryptoSavingsTracker/         workspace_root (project)
  .chainworks/
    runs/
      2c003f1d-62df-4c9a-b0c3-1e32c30add45/         run A
        state/run-state.json
        proposals/current/proposal.md
        reviews/proposal/product-owner.json
        reviews/proposal/summary.json
        ...
      4294aa32-0834-4f73-8191-2decf5847c83/         run B (parallel or prior)
        state/run-state.json
        proposals/current/proposal.md
        ...
    acp-stderr.log                                    shared (transport log, not run-scoped)
```

The `chainworks_meta_root` value is daemon-owned. Callers do not provide it on `runs.start`, and it is derived internally at run creation time.

Implementation: `control-plane/crates/engine/src/command_handler.rs` (run creation), `control-plane/crates/domain/src/run.rs` (field definition), `control-plane/crates/db/migrations/013_per_run_meta_root.sql` (schema).

## Path resolution rules

`resolve_path_template` is the central function that expands `${CHAINWORKS_META_ROOT:-.chainworks}` in artifact path templates.

Resolution order:

1. If the run has `chainworks_meta_root = Some(val)`, use `val`.
2. If the run has `chainworks_meta_root = None` (legacy), use the template default `.chainworks`.
3. Process `std::env::var("CHAINWORKS_META_ROOT")` is **not** consulted for run artifact resolution. It is only used by steward (which operates outside the run lifecycle) and by tests.

```rust
pub fn resolve_path_template(
    template: &str,
    workspace_root: &str,
    meta_root: Option<&str>,
) -> String
```

All artifact-resolution call sites pass the run's `chainworks_meta_root`:

| Call site | Semantic |
|---|---|
| `check_artifact_exists` | Transition condition `exists()` |
| `read_artifact_field` | Transition condition `artifact.field` |
| Prompt input path resolution | Agent prompt building |
| Required output path resolution | Agent expected outputs |
| Companion output path resolution | Agent companion outputs |
| Release artifact path resolution (orchestrator) | Release receipts |
| `normalize_artifacts` (post-ACP) | Artifact normalization destinations |
| `resolve_release_artifact_path` (executor) | Native release writer paths |

Implementation: `control-plane/crates/engine/src/orchestrator.rs` (resolver function and call sites 1-6), `control-plane/crates/engine/src/executor.rs` (call sites 7-8).

## ACP subprocess environment handoff

ACP agent subprocesses need `CHAINWORKS_META_ROOT` in their process environment so that `${CHAINWORKS_META_ROOT:-.chainworks}` in YAML artifact paths resolves correctly when the agent's tool calls (Read, Write, Grep) operate on files.

The `ExecutionRequest` struct carries `chainworks_meta_root: Option<String>`. Each of the five ACP adapters reads this field and injects `CHAINWORKS_META_ROOT` as an absolute path into the subprocess environment at spawn time.

| Adapter | File |
|---|---|
| Claude | `control-plane/crates/acp/src/adapters/claude.rs` |
| Codex | `control-plane/crates/acp/src/adapters/codex.rs` |
| Gemini | `control-plane/crates/acp/src/adapters/gemini.rs` |
| Auggie | `control-plane/crates/acp/src/adapters/auggie.rs` |
| Junie | `control-plane/crates/acp/src/adapters/junie.rs` |

When `chainworks_meta_root` is a relative path (e.g. `.chainworks/runs/{run_id}`), each adapter resolves it to an absolute path by prepending `workspace_root` before injecting the env var.

This does not depend on ACP `session/new.params` env support. It uses standard OS process environment that every subprocess inherits.

Implementation: `control-plane/crates/acp/src/lib.rs` (request struct), adapter files listed above (env injection at spawn).

## Worktree normalization exemption

Paths under `chainworks_meta_root` are control-plane meta artifacts (proposals, reviews, run-state, reviewer scores, summary bundles). They are not source-code artifacts and must not be rewritten into the worktree.

`normalize_path_for_worktree` skips any path that starts with the run's resolved `chainworks_meta_root` absolute prefix. This is a single `starts_with` check.

The effect:

- Source-code paths (`{workspace_root}/ios/...`) still normalize to `{worktree_root}/ios/...` for write-enabled agents.
- Meta paths (`{workspace_root}/.chainworks/runs/{run_id}/...`) stay under `workspace_root`.
- Transition checks and prompt input paths point to the same physical files the agent wrote.

Without this exemption, a meta path would be rewritten into `{worktree_root}/.chainworks/runs/{run_id}/...`, splitting meta truth between two physical locations.

Implementation: `control-plane/crates/engine/src/orchestrator.rs` (`normalize_path_for_worktree` function).

## Transition condition isolation

`exists()` and `artifact.field` transition conditions resolve artifact paths through the run's `chainworks_meta_root`.

For post-isolation runs (`chainworks_meta_root = Some(...)`), the shared `artifact_root` fallback is disabled. If the canonical per-run path does not exist, the transition condition evaluates to false. It does not search the shared flat `.chainworks/` directory.

For legacy runs (`chainworks_meta_root = None`), the old shared `artifact_root` fallback is preserved.

This prevents a stale `run-state.json` or `proposal.md` from a prior run from satisfying a transition check for a new run.

Implementation: `control-plane/crates/engine/src/orchestrator.rs` (`check_artifact_exists`, `read_artifact_field`).

## Artifact normalization source isolation

`normalize_artifacts` has two responsibilities:

1. Resolve the canonical destination path from the YAML artifact map.
2. Find an ACP-produced source file and copy it to that canonical destination when the destination is missing.

For post-isolation runs (`chainworks_meta_root = Some(...)`):

- The canonical destination uses the run's `chainworks_meta_root`.
- The source lookup searches **only** `{artifact_root}/{run_id}/` for same-name ACP outputs.
- Stale files in the shared flat `{artifact_root}/` directory are ignored, even if they share the same YAML artifact name.

For legacy runs (`chainworks_meta_root = None`):

- The old source lookup behavior is preserved: `{artifact_root}/` plus `{artifact_root}/{run_id}/`.

This prevents a post-isolation run from importing stale flat-root artifacts that happen to share a YAML artifact name such as `proposal_current` or `run_state`.

Implementation: `control-plane/crates/engine/src/executor.rs` (`normalize_artifacts` function).

## Legacy fallback semantics

| Run type | `chainworks_meta_root` | Path resolution | Source lookup |
|---|---|---|---|
| Post-isolation (new) | `Some(".chainworks/runs/{run_id}")` | Uses the run's meta root | Only `artifact_root/{run_id}` |
| Legacy (pre-isolation) | `None` | Uses template default `.chainworks` | `artifact_root/` plus `artifact_root/{run_id}` |

Key rules:

- New runs always have a non-null `chainworks_meta_root`. It is set by construction at run creation.
- Legacy runs with `chainworks_meta_root = NULL` continue to work with the old shared-root behavior.
- Process env `CHAINWORKS_META_ROOT` is not consulted for run artifact resolution.
- When a legacy run is resumed after the isolation change, it continues with its NULL meta root.
- Two legacy runs on the same workspace share `.chainworks/` exactly as they did before isolation was added.

Implementation: `control-plane/crates/engine/src/orchestrator.rs` (resolver fallback logic), `control-plane/crates/engine/src/executor.rs` (normalization fallback logic).

## Northbound readback contract

`chainworks_meta_root` is exposed read-only through all northbound surfaces for debugging and operator verification.

| Surface | Field | Notes |
|---|---|---|
| `domain::Run` | `chainworks_meta_root: Option<String>` | Core domain model |
| SQLite `runs` table | `chainworks_meta_root TEXT` | Nullable for legacy rows |
| GraphQL `Run` | `chainworksMetaRoot: String` (nullable) | Single-run and list/projection reads |
| MCP `runs.get` | `chainworks_meta_root` in JSON | Full `Run` serialization |
| MCP `runs.list` | `chainworks_meta_root` in projection rows | List and detail surfaces agree |
| MCP `runs.start` | Not accepted as input | Daemon-owned, caller cannot override |

Implementation: `control-plane/crates/graphql-server/src/types/run.rs` (GraphQL type), `control-plane/crates/mcp-server/src/tools/runs.rs` (MCP tools), `control-plane/crates/db/src/repos/projections.rs` (projection rows).

## Implementation file inventory

| Crate | File | Change |
|---|---|---|
| `domain` | `crates/domain/src/run.rs` | `chainworks_meta_root: Option<String>` field |
| `db` | `crates/db/migrations/013_per_run_meta_root.sql` | `ALTER TABLE runs ADD COLUMN chainworks_meta_root TEXT` |
| `db` | `crates/db/src/repos/runs.rs` | Insert/select/parse for `chainworks_meta_root` |
| `db` | `crates/db/src/repos/projections.rs` | `RunProjectionRow` carries `chainworks_meta_root` |
| `engine` | `crates/engine/src/command_handler.rs` | Derives `.chainworks/runs/{run_id}` on `StartRun` |
| `engine` | `crates/engine/src/orchestrator.rs` | `resolve_path_template` accepts `meta_root`; all artifact call sites pass it; `normalize_path_for_worktree` exempts meta-root paths |
| `engine` | `crates/engine/src/executor.rs` | `ExecutionRequest` population; `normalize_artifacts` source isolation; `resolve_release_artifact_path` passes meta root |
| `acp` | `crates/acp/src/lib.rs` | `ExecutionRequest.chainworks_meta_root` field |
| `acp` | `crates/acp/src/adapters/*.rs` | All five adapters inject `CHAINWORKS_META_ROOT` env var |
| `graphql-server` | `crates/graphql-server/src/types/run.rs` | Read-only `chainworksMetaRoot` on GraphQL `Run` |
| `mcp-server` | `crates/mcp-server/src/tools/runs.rs` | Full and projection readback carry the field |

## Test gate

The canonical verification command is:

```bash
./scripts/test-gate.sh proposal-050
```

This gate runs focused per-run isolation tests followed by `cargo test --workspace` from `control-plane/`. See [test-gates.md](test-gates.md) for full scope documentation.

## Non-goals

The following items are explicitly out of scope:

- **Stale artifact cleanup.** Pre-isolation `.chainworks/` artifacts are orphaned but harmless. A future `compact` command may clean them.
- **Steward analysis isolation.** Steward operates outside the run lifecycle and has its own IO root.
- **Swift app changes.** The Swift app already has per-run isolation via `RunWorkspace`.
- **`artifact_root` restructuring.** The DB-level `artifact_root` field stays as-is. Isolation only affects the filesystem meta root that YAML artifact paths resolve through.
- **Legacy run backfill.** Pre-isolation runs keep `chainworks_meta_root = NULL` and use the old shared path.
