# Manual Prompt Fixtures

## proposal-readiness
`Use $proposal-review-triad at /Users/user/.codex/skills/proposal-review-triad in proposal-readiness mode for <proposal-path>. Read the proposal, adjacent docs, `.review-baselines/current-system-baseline.md` if present, any `<proposal>.review/integration-context.md`, current repo surfaces, and any prior evidence pack. Reuse a fresh baseline instead of rebuilding it. If a narrow slice is stale, refresh only that slice, preferably with `integration_mapper`, and do not run the app by default. Return a proposal-first review with evidence gaps only if the proposal/doc/code/baseline evidence is insufficient.`

## research
`Use $proposal-review-triad at /Users/user/.codex/skills/proposal-review-triad in research mode for <proposal-path>. First read the proposal, adjacent docs, `.review-baselines/current-system-baseline.md` if present, any `<proposal>.review/integration-context.md`, any existing evidence pack or research pack, and current repo surfaces. Build or refresh the local evidence pack first, derive bounded research questions from real proposal gaps or host-system risks, then write `<proposal>.review/research-pack.md` with source-backed findings, applicability decisions, proposal deltas, freshness risks, and reused-versus-refreshed source notes. Do not use Xcode tools in this mode.`

## baseline-refresh
`Use $integration-context-baseline at /Users/user/.codex/skills/integration-context-baseline in baseline-refresh mode for <repo-or-surface-scope>. Build or refresh `.review-baselines/current-system-baseline.md` from repo-local docs, existing baseline artifacts, and code/module mapping, preferably with `integration_mapper` first. Use narrow targeted Xcode/build/run/simulator observation only when it materially reduces ambiguity about the current host system. Do not turn this into a feature/runtime validation review.`

## targeted-context-refresh
`Use $integration-context-baseline at /Users/user/.codex/skills/integration-context-baseline in targeted-context-refresh mode for <repo-or-surface-scope>. Read the current `.review-baselines/current-system-baseline.md`, refresh only the stale or missing slices for the affected surfaces, prefer `integration_mapper` for the first pass, and keep any runtime observation narrow and ambiguity-driven.`
