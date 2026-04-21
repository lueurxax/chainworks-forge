# Proposal Lifecycle Review Repo Guidance

Use `proposal-review-router` for proposal-readiness, proposal research, and reviewer routing before implementation.

Use `proposal-implementation-audit` for implementation-vs-proposal audits after or during implementation.

Keep the phases separate:

- Proposal review selects the specialist reviewer set from proposal and local evidence.
- Implementation audit reuses that reviewer set when valid and adds delta reviewers only from implementation evidence.

Recommended local inputs:

- `.review-baselines/current-system-baseline.md`
- `<proposal>.review/integration-context.md`
- `<proposal>.review/evidence-pack.md`
- `<proposal>.review/final-review.md`
- `<proposal>.review/research-pack.md`
- `<proposal>.review/reviewer-selection.yaml`

Do not require broad builds, service startup, simulator runs, benchmarks, load tests, or fuzzing for proposal-readiness mode unless the proposal explicitly asks for that evidence.

For implementation-audit successful verdicts, require passing same-tree full regression or the repository's canonical full/proposal gate.
