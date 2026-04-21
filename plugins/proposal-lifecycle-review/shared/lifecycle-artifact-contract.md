# Lifecycle Artifact Contract

The plugin keeps proposal review and implementation audit separate, but the artifacts should line up.

## Proposal Review Outputs

`proposal-review-router` may produce or consume:

- `<proposal>.review/evidence-pack.md`
- `<proposal>.review/final-review.md`
- `<proposal>.review/research-pack.md`
- `<proposal>.review/reviewer-selection.yaml`
- `<proposal>.review/integration-context.md`
- sibling historical review files matching `<proposal-stem>_PROPOSAL_REVIEW_R*.md`, `<proposal-stem>_REVIEW_R*.md`, `<proposal-stem>_EVIDENCE_PACK*.md`, or `<proposal-stem>_RESEARCH_PACK*.md`

The durable routing handoff is `<proposal>.review/reviewer-selection.yaml`. Markdown artifacts remain supported as fallback discovery inputs.

## Implementation Audit Inputs and Outputs

`proposal-implementation-audit` must:

- read the proposal before implementation details
- discover prior proposal-review artifacts
- reuse reviewer selection when valid
- add delta reviewers only from implementation evidence
- audit atomic `REQ-*` proposal commitments
- write exactly one versioned `<proposal-stem>_IMPLEMENTATION_AUDIT_R<N>.md` report beside the proposal

## Handoff Fields

Reviewer selection handoff should include:

- proposal path and stable proposal identity
- proposal state
- selected reviewers
- rejected close alternatives
- stack, surface, and risk tags
- evidence ids and source references
- required changes before implementation
- metrics, rollout decision checkpoints, and research conclusions when present
- created/updated timestamps and source skill version

Use `reviewer-selection-state-template.yaml` as the repo-local artifact shape.
