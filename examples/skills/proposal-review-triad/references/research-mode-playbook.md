# Research Mode Playbook

Use `research` only after the local proposal evidence gate is complete. Research mode augments proposal review with current external knowledge; it does not replace proposal, baseline, or code inspection.

## 1. Local-First Entry Gate

Before researching, read:

- the proposal
- adjacent repo-local docs
- `.review-baselines/current-system-baseline.md` when present
- `<proposal>.review/integration-context.md` when present
- any existing proposal evidence pack
- any existing `<proposal>.review/research-pack.md`
- current code / module mapping for the affected surfaces

If local evidence is too weak to frame bounded questions, stop and return an `Evidence Gap Review` or switch to `$integration-context-baseline` for missing host-system mapping.

## 2. Derive Questions From Real Local Issues

Only research questions that trace back to one of these:

- proposal gaps
- baseline constraints
- host-system integration risks
- unresolved tradeoffs

Record each question with its triggering local evidence IDs before browsing.

## 3. Source Selection Order

Prefer sources in this order:

1. official platform docs / primary documentation / standards
2. strong technical or regulatory primary sources
3. reputable engineering writeups
4. high-quality industry analysis

Avoid filler or SEO listicles unless no stronger source exists.

## 4. Bounded Research Execution

- Keep the source set tight.
- Do not harvest links without a concrete question.
- Separate source-backed findings from model inference.
- Do not use research to override verified repo or baseline reality.
- Do not require Xcode MCP, build, run, or simulator work for research mode.

If repo-local custom agents exist, `research_scout` may gather the source-backed findings, but the main thread still owns synthesis and file writing.

## 5. Deterministic Artifact Writing

If the proposal is `path/to/Foo.md`, create or reuse:

- `path/to/Foo.review/`

Write:

- `path/to/Foo.review/research-pack.md`

Use [assets/research-pack-template.md](../assets/research-pack-template.md).

Default to a single research pack. Add a separate source-ledger file only when it materially improves reuse or auditability.

## 6. Reuse vs Refresh Rules

- Reuse prior research only when it is still fresh enough.
- Re-check temporally unstable or version-sensitive sources each round.
- When the current-system baseline changed for the affected surfaces, re-evaluate applicability instead of carrying prior recommendations forward unchanged.
- Explicitly mark which sources were reused and which were refreshed.

## 7. Output Standard

The research pack must contain:

- local context consumed
- bounded research questions
- source ledger
- themed findings
- host-system applicability matrix
- proposal deltas / recommended updates
- freshness risks / recheck triggers
- remaining open questions
