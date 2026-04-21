# Per-Run Workspace Isolation Proof

Current implementation and proof status for per-run artifact isolation in the Rust control-plane daemon, consolidated from Proposal 050.

## Status

| Field | Value |
|---|---|
| Slice | Per-Run Workspace Isolation |
| Source contract | [../reference/per-run-workspace-isolation.md](../reference/per-run-workspace-isolation.md) |
| Current implementation status | Implemented |
| Current readiness | Ready with Risks |
| Primary proof owners | Focused P050 integration tests, canonical `proposal-050` gate |
| Last consolidated audit | R3 on 2026-04-17 |

## Motivating incident

On 2026-04-16, a `full-mvp-live` run was started on a workspace (`CryptoSavingsTracker`) that already contained artifacts from a prior completed run (`ux-audit-remediation-close-gaps-2026-04-16`). That prior run had reached `implementation_complete_pending_burndown` with an approved proposal (score 9.55/10).

The new run's `proposal_writer` agent (state_2) was expected to draft a new proposal. Instead, it read the stale `.chainworks/state/run-state.json`, saw "proposal approved, implementation in progress", and began writing Swift code into production source files.

Root cause: the Rust daemon had no per-run artifact isolation. All runs shared a flat `.chainworks/` directory under the workspace root. The new run inherited stale truth from the prior run.

This incident is closed by the P050 implementation. New runs derive `.chainworks/runs/{run_id}` and never read shared-root artifacts.

## Audit trail

Three implementation audits were conducted on the same tree, each building on the prior round's findings.

### R1: Static conformance scan

- **Verdict:** Not Implemented
- **Findings:** Core domain/DB/engine/ACP/readback plumbing was present, but two critical gaps remained:
  - `exists()` and `artifact.field` transition checks still fell back to the shared flat `artifact_root` for post-P050 runs, allowing stale artifacts to satisfy transition conditions.
  - The `proposal-050|p050` gate was not registered in `scripts/test-gate.sh` or documented in `docs/reference/test-gates.md`.
  - Named focused tests from the proposal inventory were absent.

### R2: Post-fix conformance scan

- **Verdict:** Partial (14 of 17 requirements implemented)
- **Findings:** Transition condition fallback was legacy-gated (shared `artifact_root` fallback now limited to `chainworks_meta_root = None` rows). Gate was registered and documented. Remaining gaps:
  - Several proposal-named focused tests were absent or only indirectly covered.
  - The gate body ran `cargo test --workspace` without enumerating the focused proof inventory.
  - No same-tree gate execution was recorded.

### R3: Gate-green audit with proof quality assessment

- **Verdict:** Partial conformance, gate green
- **Findings:** `./scripts/test-gate.sh proposal-050` passed on the same tree, including focused P050 tests and `cargo test --workspace`. Remaining proof quality gaps:
  - No direct non-Codex adapter env proof (code inspection confirms injection, but no focused subprocess test).
  - No direct GraphQL readback proof (field mapping confirmed, but no query-level test).
  - No direct release writer/reader proof (code passes meta root, but no named focused test).
  - Cancellation isolation inferred from architecture, not directly tested.
  - `test_normalize_artifacts_ignores_stale_flat_artifact_root_for_post_p050_runs` proves path separation, not the copy/search function.

## What is considered proven

The proof set supports these claims:

- new runs derive `.chainworks/runs/{run_id}` at run creation and callers cannot override it,
- `resolve_path_template` uses the run meta root and does not consult process env for `CHAINWORKS_META_ROOT`,
- legacy NULL-meta-root rows use the template default `.chainworks` as a backward-compatible fallback,
- `exists()` and `artifact.field` transition checks resolve against the per-run meta root; post-P050 runs do not fall back to shared `artifact_root`,
- `normalize_path_for_worktree` exempts meta-root paths from worktree rewrite while still normalizing source paths,
- ACP `ExecutionRequest` carries `chainworks_meta_root` and all five adapters inject the env var,
- `normalize_artifacts` source-side isolation searches only `artifact_root/{run_id}` for post-P050 runs,
- legacy NULL-meta-root runs preserve the flat-root fallback for normalization,
- GraphQL, MCP detail, and MCP list projection readbacks carry `chainworks_meta_root`,
- prompt input paths point to the per-run meta root,
- the `proposal-050` gate is registered, documented, and green on the same tree.

## Focused test inventory

Tests located in `control-plane/crates/engine/tests/integration.rs` and covered by the `proposal-050` gate:

| Test | What it proves |
|---|---|
| `test_resolve_path_template_uses_run_meta_root` | `Some(meta_root)` overrides template default |
| `test_resolve_path_template_null_meta_root_uses_template_default` | `None` falls back to `.chainworks` |
| `test_resolve_path_template_does_not_consult_process_env_for_runs` | Process env is ignored when `meta_root` is provided |
| `test_normalize_path_for_worktree_skips_meta_root_paths` | Meta-root paths are not rewritten into worktree |
| `test_normalize_path_for_worktree_still_normalizes_source_paths` | Source paths still get worktree normalization |
| `test_exists_checks_per_run_meta_root` | `exists()` transition resolves per-run, not shared |
| `test_artifact_field_reads_per_run_meta_root` | `artifact.field` transition resolves per-run, not shared |
| `test_execution_request_carries_chainworks_meta_root` | ACP request struct carries the meta root |
| `test_normalize_artifacts_uses_run_scoped_source_dir_for_post_p050_runs` | Post-P050 normalization searches only `artifact_root/{run_id}` |
| `test_normalize_artifacts_ignores_stale_flat_artifact_root_for_post_p050_runs` | Stale shared-root files are not imported |
| `test_normalize_artifacts_preserves_flat_root_fallback_for_null_legacy_runs` | Legacy runs keep old source lookup |
| `test_graphql_run_exposes_chainworks_meta_root` | GraphQL readback carries the field |
| `test_mcp_runs_get_exposes_chainworks_meta_root` | MCP detail carries the field |
| `test_mcp_runs_list_projection_exposes_chainworks_meta_root` | MCP list projection carries the field |
| `test_runs_start_does_not_accept_chainworks_meta_root_override` | Start command does not accept caller override |
| `test_new_run_gets_isolated_meta_root` | New run creation derives the per-run path |
| `test_stale_workspace_artifacts_not_visible_to_new_run` | Stale `.chainworks/` artifacts are invisible to a new run |
| `test_prompt_input_paths_point_to_per_run_meta_root` | Agent prompt inputs use the per-run directory |

## Canonical gate command

```bash
./scripts/test-gate.sh proposal-050
```

The gate runs focused P050 tests followed by `cargo test --workspace` from `control-plane/`. It requires a local Rust toolchain. No UI host or simulator is needed.

The runner also accepts the `p050` alias.

## Remaining caution

The remaining caution is about proof granularity, not functional correctness:

- the gate is green and core isolation behaviors are tested,
- but several proposal-named proof lanes (non-Codex subprocess env receipt, direct GraphQL query, direct release writer/reader, cancellation-specific isolation) are covered by code inspection and broad workspace tests rather than by focused named tests.

That keeps readiness at `Ready with Risks` rather than fully frictionless `Ready`.

## Historical note

Proposal 050's raw proposal, review, evidence pack, and three implementation audit reports were implementation-trail artifacts. This document and the stable reference at [../reference/per-run-workspace-isolation.md](../reference/per-run-workspace-isolation.md) now replace them as the canonical documentation for this slice.

## Usage guidance

Use:

- [../reference/per-run-workspace-isolation.md](../reference/per-run-workspace-isolation.md) for the stable contract,
- this document for implementation/proof status,
- [../reference/test-gates.md](../reference/test-gates.md) for the canonical `proposal-050` verification lane.
