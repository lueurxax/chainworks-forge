# Repository Review Instructions

## Proposal Implementation Audits

Use `$proposal-implementation-audit` only for proposal-vs-implementation audits. Do not use it for generic code review.

Default flow:

1. Read the proposal/spec first.
2. Reuse prior proposal-review reviewer selection when valid.
3. Inspect current implementation or diff only as needed to verify the proposal contract.
4. Keep the audit read-only except for one versioned report beside the proposal.
5. Record tests run, tests found but not run, and commands intentionally skipped.

## Proposal Review Artifacts

Keep proposal-review outputs near the proposal when possible:

```text
<proposal>.review/evidence-pack.md
<proposal>.review/final-review.md
<proposal>.review/research-pack.md
<proposal>.review/integration-context.md
```

Implementation audits will search these locations first.

## Validation Commands

Document repo-specific focused commands here. Prefer narrow package/module/scheme tests over broad test suites unless the proposal requires a full validation pass.

## Safety

Do not modify source files, generated files, migrations, or tests during an audit. Do not run publish/deploy/push/destructive commands.
