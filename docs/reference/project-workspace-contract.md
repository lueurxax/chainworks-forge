# Project Workspace Contract

Stable reference for idea-owned working-directory truth and frozen run workspace ownership.

## Purpose

Project-backed execution must not guess its repository from ambient process state.

The operator must be able to trust:

- which project an idea belongs to,
- whether a workflow needs project access,
- which directory was frozen into the run,
- and whether resume is still operating on that same frozen workspace contract.

## Scope

This reference covers:

- the `requires_project_access` selector,
- idea-owned workspace root state,
- fail-closed start/preflight behavior,
- frozen workspace state on the run,
- resume-time revalidation.

Related stable docs:

- [runtime-contract.md](runtime-contract.md)
- [run-control.md](run-control.md)

## Core rule

Every project-backed idea owns one explicit workspace root / project root.

This path belongs to the idea itself, not only to a transient start sheet.

If a workflow requires project access and the idea has no valid workspace contract, the run must not start.

## Shared selector

The single authoritative selector is:

```yaml
workflow:
  execution:
    requires_project_access: true
```

It compiles into `RunPlan.requiresProjectAccess`.

Consumers of that one typed answer:

- Start Run (Diagnostic placeholder in UI; enforced at mutation/MCP start)
- Preflight
- Run compilation
- Resume

If the YAML field is absent, it defaults to `false`.

## Start and preflight rules

When `requiresProjectAccess == true`:

- diagnostic Start Run placeholder blocks if the idea has no valid workspace root,
- preflight includes workspace readiness,
- the operator must fix the workspace contract before external live start.

When `requiresProjectAccess == false`:

- workspace readiness is optional,
- start may proceed without a project root,
- repo-agnostic flows remain directory-free by design.

## Frozen run contract

At run creation time, the product freezes project-location truth into the run:

- idea-owned workspace root,
- run workspace root,
- worktree root when repo-backed execution provisions one,
- related repo metadata such as base branch / base revision when available.

Later edits to the idea's workspace root do not mutate an already-started run.

## Resume contract

Resume trusts the frozen run/workspace state, not the current idea editor field.

Rules:

- resume revalidates the frozen workspace contract,
- project-backed resume fails closed when the frozen directory is no longer valid,
- no code path is allowed to re-infer the project from ambient app cwd.

## Operator ownership

The canonical owner path is in `Ideas`.

The operator can:

- view workspace root,
- edit workspace root,
- see when the workflow requires project access,
- fail fast before run start if the contract is missing.

This workspace contract is not a hidden engine detail.

## Non-goals

This document does not define:

- worktree provisioning internals for repo-backed release flows,
- multi-project routing for one idea,
- shared multi-user repo assignment,
- ambient cwd fallback behavior for unsupported legacy paths.

Those are either forbidden or handled in more specific runtime docs.
