# Proposal Review Conventions

Use `proposal-review-router` for proposal readiness, cross-stack routing, and bounded research.

## Artifact layout

For `docs/proposals/NNN-title.md`, write reusable review artifacts under:

`docs/proposals/NNN-title.review/`

Expected artifacts:

- `integration-context.md` for reusable local system mapping
- `evidence-pack.md` for proposal-local evidence and routing
- `research-pack.md` for bounded external research
- `proposal-readiness-review.md` when a durable Markdown report is requested

## Proposal-readiness default

- Do not require builds, app launches, service startup, benchmarks, load tests, or fuzzing.
- Prefer current proposal lines, adjacent docs, baseline facts, and narrow code-path mapping.
- Findings must cite evidence IDs.
