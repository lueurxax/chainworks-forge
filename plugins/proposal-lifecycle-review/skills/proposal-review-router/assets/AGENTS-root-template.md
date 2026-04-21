# Repository Review Conventions

Use this file to document repo-local proposal review conventions for `proposal-review-router`.

## Baselines

- Reusable baseline path: `.review-baselines/current-system-baseline.md`
- Proposal integration context path: `<proposal>.review/integration-context.md`
- Evidence pack path: `<proposal>.review/evidence-pack.md`
- Research pack path: `<proposal>.review/research-pack.md`

## Routing overrides

- Reviewer plugins: `.codex/reviewers/*.yaml`
- Routing overrides: `.codex/review-router.yaml`
- Repo-local agents: `.codex/agents/`

## Review rules

- Read proposal and local evidence before routing.
- Refresh only affected stale baseline slices.
- Do not require build/run evidence for proposal-readiness.
- Keep reviewer selection selective and evidence-backed.
