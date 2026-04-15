# Deterministic Release Operations (former P045)

## Status
- **Implemented and stabilized**: 2026-04-15
- **Primary contract owners**: Rust control-plane executor, release services, release artifact-path resolution
- **Evidence source**: `./scripts/test-gate.sh proposal-045` and `control-plane/crates/engine/tests/release.rs`

## Purpose

This document replaces the proposal-level text for deterministic release execution.
It defines the production contract for repo-backed git push and sandbox/staging
publish work in the Rust control plane.

The goal of this slice is simple:

- release agents do not run through ACP,
- release side effects do not depend on free-form LLM shelling,
- frozen delivery configuration reaches the executor unchanged,
- canonical release artifacts exist at catalog-defined paths,
- and receipt truth survives happy paths, release-attempt failure paths, and
  eligible terminal backfill.

## Scope

This reference covers:

- frozen `delivery_configuration_json` input truth at run start,
- native `commit_and_push_to_github` execution,
- native `build_archive_and_push_connect` execution in safe mode,
- canonical release artifact persistence for workflow transition truth,
- structured `delivery_receipt` persistence and preserve semantics,
- lineage-gated terminal backfill,
- and northbound readback of release configuration and evidence.

It does not replace:

- the broader repo-backed workflow in [full-mvp-delivery.md](full-mvp-delivery.md),
- the daemon architecture in [rust-control-plane.md](rust-control-plane.md),
- the workflow/executor substrate in [workflow-execution-engine.md](workflow-execution-engine.md),
- or the proof inventory in [test-gates.md](test-gates.md).

## Core Rules

### Release execution is native, not ACP-driven

`commit_and_push_to_github` and `build_archive_and_push_connect` are executor-owned
release operations. They bypass ACP completely and run through Rust release
services.

This is the hard safety boundary for the release slice: agents may recommend
release, but they do not improvise commit, push, archive, or upload mechanics.

### Delivery configuration is frozen at run start

Repo-backed release execution depends on the run's frozen
`delivery_configuration_json`.

That payload is accepted at the northbound start surfaces, persisted on `Run`,
and deserialized fail-closed when the executor enters a release step.

The frozen configuration is the only release input owner for:

- repository identity,
- repository root,
- base branch,
- target branch,
- release target ID,
- and release mode.

### Canonical artifact paths are part of release truth

Release artifacts are not written to arbitrary executor-local filenames.
They resolve through the compiled workflow/catalog artifact map so transition
conditions such as `exists('git_push_receipt')` and operator readback stay on
one authority lane.

Current canonical release artifacts are:

| Artifact | Canonical path |
|---|---|
| `release_manifest` | `.chainworks/release/release-manifest.json` |
| `git_push_receipt` | `.chainworks/release/git-push-receipt.json` |
| `release_bundle_manifest` | `.chainworks/release/release-bundle-manifest.json` |
| `connect_upload_receipt` | `.chainworks/release/connect-upload-receipt.json` |
| `delivery_receipt` | `.chainworks/release/delivery-receipt.json` |

### The first valid `delivery_receipt` writer wins

`delivery_receipt` is preserved, not endlessly regenerated.

The stable rule is:

- git failure may write it,
- publish failure may write it,
- publish success may write it,
- terminal finalization may backfill it only when still absent.

Once the canonical receipt path already exists, later write sites must preserve
the existing file instead of overwriting it.

### Terminal backfill is lineage-gated

`state_12` is only a fallback writer.
It may backfill `delivery_receipt` only when finalization still has the full
eligibility chain:

- frozen delivery config,
- worktree root,
- and prior release-agent lineage strong enough to derive release-result truth.

Pre-release failures without release lineage do not get a metadata-only receipt.

### Publish remains safe-mode only

This slice supports `sandbox` and `staging` release modes only.

`build_archive_and_push_connect` performs deterministic local build/archive
evidence work and writes a safe-mode upload receipt. It does not perform real
App Store Connect communication and does not enable production release mode.

## Execution Model

### 1. Start-run persistence

At run creation time the daemon accepts and stores frozen
`delivery_configuration_json`. GraphQL and MCP both feed the same persisted run
truth.

### 2. Git release step

The git release service:

1. inspects worktree status and diff stats,
2. stages all changes,
3. creates the release commit,
4. resolves `HEAD`,
5. pushes to the configured target branch,
6. persists `release_manifest` and `git_push_receipt`.

Protected branches such as `main` and `master` are rejected by the release path.

### 3. Publish step

The publish service consumes prior git artifacts and frozen delivery config.
In the current contract it:

1. verifies git success,
2. attempts a local `xcodebuild build` compilability check,
3. records build warnings without treating sandbox build failure as fatal,
4. derives deterministic archive/checksum metadata,
5. persists `release_bundle_manifest` and `connect_upload_receipt`.

### 4. Receipt settlement

`delivery_receipt` summarizes the release outcome for operator surfaces, proof
gates, and readback APIs.

It can represent:

- success with full git/publish lineage,
- git failure with structured failure stage,
- publish failure with preserved git lineage,
- or eligible terminal backfill when earlier execution never wrote the receipt.

## Failure Semantics

### Missing delivery configuration fails closed

If a repo-backed release step starts without valid `delivery_configuration_json`,
the executor fails closed.

That pre-release failure path does not synthesize an executor-side
`delivery_receipt`.

### Git failure is terminal for publish

If `commit_and_push_to_github` fails:

- publish is not attempted,
- git/publish happy-path artifacts are not fabricated,
- `delivery_receipt` records `failure_stage = "commit_and_push"` when eligible,
- the run blocks with operator-visible failure truth.

### Publish failure preserves git truth

If publish fails after git succeeds:

- git artifacts remain authoritative,
- `delivery_receipt` records `failure_stage = "build_archive_and_push"`,
- the run blocks rather than rolling back silently.

### Receipt preservation outranks later convenience writes

If a canonical `delivery_receipt` already exists, later writer sites must skip.
This keeps failure-path truth and earlier success-path truth stable under
subsequent finalization or retry bookkeeping.

## Northbound Readback

The current daemon exposes release truth northbound through the same read stack
used by other run data:

- GraphQL run reads,
- MCP `runs.get`,
- MCP `reports.get`,
- and collection/resource readers tied to run projections and report material.

The northbound contract for this slice is:

- frozen delivery configuration remains readable after start,
- release artifacts remain discoverable at canonical paths,
- and structured release-result truth survives into operator/report surfaces.

## Proof Owners

The canonical same-tree proof lane for this slice is:

```bash
./scripts/test-gate.sh proposal-045
```

That gate currently proves:

- frozen input-path persistence,
- native release-agent routing,
- deterministic git push behavior,
- sandbox/staging publish behavior,
- canonical artifact-path persistence,
- structured git-failure and publish-failure receipts,
- missing-config fail-close with no receipt,
- preserve-without-overwrite behavior,
- lineage-gated terminal backfill,
- and northbound GraphQL/MCP readback.

Focused runtime coverage lives primarily in
`control-plane/crates/engine/tests/release.rs`.

## Archived Proposal History

The proposal draft, review pack, evidence pack, and intermediate audit rounds for
P045 are retained under [../archive/proposals/README.md](../archive/proposals/README.md)
for provenance only. They are not the canonical current-head contract anymore.

## Related Stable Docs

- [full-mvp-delivery.md](full-mvp-delivery.md)
- [rust-control-plane.md](rust-control-plane.md)
- [workflow-execution-engine.md](workflow-execution-engine.md)
- [test-gates.md](test-gates.md)
