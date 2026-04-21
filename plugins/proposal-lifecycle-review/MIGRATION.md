# Migration

This plugin combines two existing skill packages without collapsing them into one review engine.

## Source Packages

| Source | Destination | Migration |
|---|---|---|
| `/Users/user/.codex/skills/proposal-review-router` | `skills/proposal-review-router` | Copied as a first-class skill. |
| `/Users/user/.codex/skills/proposal-implementation-audit` | `skills/proposal-implementation-audit` | Copied as a first-class skill. |

Excluded non-functional files:

- `.DS_Store`
- `__pycache__/`
- `*.pyc`

## What Changed

- Added `.codex-plugin/plugin.json`.
- Added thin dispatcher skill `skills/proposal-lifecycle-review`.
- Added shared lifecycle docs and `reviewer-selection-state-template.yaml`.
- Added root-level repo-local templates under `assets/templates`.
- Added root-level copies of audit helper scripts in `scripts/` while preserving the skill-local originals.
- Added plugin-level union eval suite in `evals/scenarios.yaml`.
- Added README, install notes, migration notes, and parity map.

## What Did Not Change

- Proposal review remains proposal-first and router-first.
- Implementation audit remains proposal-anchored and proof-oriented.
- Reviewer ids are preserved.
- Existing skill-local paths are preserved so relative links, templates, scripts, and tests keep working.
- No MCP servers, apps, or hooks were added.

## Migration For Existing Users

Existing prompts keep working with the primary skill names:

```text
Use $proposal-review-router for docs/proposals/example.md
Use $proposal-implementation-audit for docs/proposals/example.md
```

New convenience prompt:

```text
Use $proposal-lifecycle-review for docs/proposals/example.md
```

The convenience skill dispatches to the correct primary skill and does not replace either workflow.

## Artifact Compatibility

Keep existing proposal review artifacts in place:

```text
<proposal>.review/evidence-pack.md
<proposal>.review/final-review.md
<proposal>.review/research-pack.md
<proposal>.review/reviewer-selection.yaml
<proposal>.review/integration-context.md
```

Implementation audit still discovers these artifacts and still writes versioned audit reports beside the proposal:

```text
<proposal-stem>_IMPLEMENTATION_AUDIT_R<N>.md
```
