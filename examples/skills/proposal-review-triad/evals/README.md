# Proposal Review Suite Evals

These evals cover the current proposal-review-triad plus the linked integration-context-baseline workflow and repo-local subagent policy.

## How to run manually

1. Open [scenarios.yaml](/Users/user/.codex/skills/proposal-review-triad/evals/scenarios.yaml).
2. Pick one scenario.
3. Run the listed prompt against the named skill and mode.
4. Check the response or generated artifact against the `success_signals`.
5. For static checks, inspect the named file paths directly with `rg`, `sed`, or manual review.

## Success signals

- Proposal-first review consumes the latest reusable baseline when present and does not demand build/run evidence by default.
- A fresh baseline is reused instead of rebuilt.
- Missing or stale baseline slices trigger only a narrow targeted context refresh, not a full runtime gate.
- `integration-context-baseline` may use `integration_mapper` first and `xcode_operator` only when docs/code still leave a host-system ambiguity.
- Baseline refresh stays a host-system mapping task and does not pretend to validate whether a new feature works.
- Missing rollout, analytics, or non-happy-path coverage is called out from proposal and local evidence alone.
- `research` mode starts only after local proposal/baseline/code evidence is assembled and writes `<proposal>.review/research-pack.md`.
- Research findings stay source-backed, bounded, and explicit about reused versus refreshed sources.
- Integration-context-baseline writes or refreshes `.review-baselines/current-system-baseline.md` and optionally `<proposal>.review/integration-context.md`.
- Product findings keep `Leading metric`, `Guardrail metric`, and `Decision checkpoint`.
- Repo-local reviewer agents stay read-only, and only `xcode_operator` is allowed to touch Xcode or simulator workflows during baseline refresh when targeted ambiguity reduction is justified.
- Round-2 behavior distinguishes reused vs refreshed evidence and baseline slices.

## Regressions to watch for

- Proposal review silently drifting back to runtime-heavy gating.
- Fresh reusable baseline being ignored and rebuilt from scratch.
- Research mode starting before local context extraction or turning into generic link harvesting.
- Proposal review demanding a full baseline rebuild instead of a narrow targeted refresh.
- Specialist modes spawning recursive review trees.
- Integration-context-baseline drifting into feature/runtime validation instead of host-system mapping.
- `integration_mapper` being skipped for easy targeted slices that do not need runtime observation.
- UI/UX/product reviewers using Xcode or simulator tools.
- `xcode_operator` being used in normal proposal-readiness reviews.
- Final review templates losing the product metrics fields or baseline provenance hooks.

## Drift Back Toward Runtime-Validation Behavior

The suite is drifting in the wrong direction if any of these become common:

- proposal review is blocked because the app was not run
- baseline refresh claims a feature is working rather than mapping host-system facts
- `xcode_operator` is used before docs, baseline artifacts, and code mapping are exhausted
- `research` mode starts with web browsing instead of local evidence extraction
