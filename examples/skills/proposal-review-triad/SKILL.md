---
name: proposal-review-triad
description: "Review repo-local proposals for consumer finance iOS Xcode apps using evidence-first UI, UX, and iOS architecture critique, with an optional product overlay and an optional second-stage research mode. Use for proposal readiness before implementation, for proposal/code/doc-based review that should first consume the latest reusable host-system baseline at `.review-baselines/current-system-baseline.md`, or when Codex should extract local context from the proposal, adjacent docs, baseline artifacts, current repo reality, and only then add targeted external research. Use $integration-context-baseline instead when the missing task is to build or refresh reusable understanding of the host app/repo/system rather than judge the proposal itself."
---

# Proposal Review Triad

This skill is proposal-first. Keep the review grounded in repo-local proposal/docs evidence, the latest reusable host-system baseline when present, targeted code/module mapping, and current repo reality. Do not force build/run attempts or simulator screenshots in the default mode. External research is allowed only as a second-stage augmentation after the local evidence gate is complete.

## Modes

Choose one mode before doing any analysis.

- `proposal-readiness`
- `research`
- `ui-only`
- `ux-only`
- `architecture-only`
- `product-only`

Legacy compatibility:

- `full-review` is a deprecated alias of `proposal-readiness`

Default to `proposal-readiness` unless the user explicitly requests a specialist mode.

Trigger the optional product overlay only when the request explicitly asks for product review or materially depends on prioritization, KPIs, rollout, instrumentation, business value, or scope sequencing.

## Skill Boundary

Use `proposal-review-triad` when the main question is:

- Is this proposal complete enough to implement?
- Does the proposal match current repo/module reality?
- Does the proposal align with the latest reusable host-system baseline?
- Are the UI, UX, architecture, and optional product implications well specified?
- What is missing, contradictory, risky, deferred, or now stale against current repo reality?
- Which unresolved proposal gaps, host-system tensions, or architecture tradeoffs need targeted external research before the next review round?

Use `$integration-context-baseline` instead when the main task is:

- building or refreshing `.review-baselines/current-system-baseline.md`
- creating `<proposal>.review/integration-context.md` for future rounds
- mapping the host app/repo/system after significant changes even without a specific proposal review
- reducing ambiguity about current architecture, screens, navigation, shared components, state patterns, or integration constraints before proposal judgment begins

Do not fail proposal review merely because runtime evidence was not collected.

## Shared Local Context Stage

Complete this stage before `proposal-readiness` or `research`.

1. Read the proposal itself.
2. Read adjacent repo-local materials relevant to the reviewed flow: tickets, architecture notes, rollout notes, constraints, API notes, analytics notes, and related docs when present.
3. Read `.review-baselines/current-system-baseline.md` when it exists.
4. Read `<proposal>.review/integration-context.md` when it exists.
5. Read any existing proposal evidence pack, prior review artifact, or `<proposal>.review/research-pack.md` that materially affects the same proposal.
6. Inspect the current codebase only enough to map impacted modules, affected screens/entry points, data/API/persistence/auth touchpoints, feature flags, analytics surfaces, and current repo contradictions.
7. If the reusable baseline is missing or stale for the affected surfaces, do a narrow targeted context refresh for those surfaces only. Do not force a full runtime workflow.
8. Fill or refresh [references/pre-review-evidence-playbook.md](references/pre-review-evidence-playbook.md) into [assets/evidence-pack-template.md](assets/evidence-pack-template.md) with evidence IDs.

If the repo is missing local review conventions, use [assets/AGENTS-root-template.md](assets/AGENTS-root-template.md) and [assets/AGENTS-proposals-template.md](assets/AGENTS-proposals-template.md) as fill-in templates instead of guessing build commands, proposal directory layout, or review expectations.

## Baseline Freshness Rules

Treat the reusable baseline as an accelerator, not a magical override.

Baseline is usually `Fresh` when:

- the affected modules and entry points are already covered
- no major repo restructuring changed ownership of the touched surfaces
- no proposal-critical contradictions were discovered between the baseline and current code

Baseline is `Stale` or `Partial` when:

- the affected surfaces are missing from the baseline
- current code or docs materially diverge from the baseline artifact
- recent repo changes renamed navigation, modules, state boundaries, or shared components
- the proposal depends on a subsystem the baseline has not mapped yet

If the baseline is missing or stale:

- refresh only the affected surfaces first
- write `<proposal>.review/integration-context.md` when the proposal needs extra local context that future rounds should reuse
- use `$integration-context-baseline` when the reusable repo-level baseline itself needs to be created or materially refreshed

Do not escalate straight from stale baseline to full runtime-heavy investigation inside this skill.

## Optional Repo-Local Subagents

If the current repository defines custom agents under `.codex/agents/`, use them selectively instead of hand-rolling new review trees.

Allowed agents for this skill:

- `proposal_explorer`
- `integration_mapper`
- `ui_reviewer`
- `ux_reviewer`
- `arch_reviewer`
- `product_reviewer`
- `research_scout`

Coordination rules:

- Keep `max_depth = 1`.
- Keep fan-out modest: usually `proposal_explorer`, or `proposal_explorer` plus one specialist reviewer, or one `integration_mapper` when a narrow baseline slice needs refresh.
- Keep evidence-pack assembly, artifact writing, and final synthesis in the main thread.
- Never use `xcode_operator` in normal `proposal-readiness` mode.
- Never use `xcode_operator` in `research` mode.
- Use `research_scout` only in `research` mode and only after the shared local context stage is complete.
- Use `integration_mapper` only for narrow targeted context refresh on affected host-system surfaces. If the reusable baseline needs broader refresh, switch to `$integration-context-baseline`.
- If current context cannot be refreshed narrowly inside proposal review, switch to `$integration-context-baseline` rather than building a runtime gate here.
- Specialist modes stay single-discipline and never spawn more subagents.

Narrow prompt shapes:

- `proposal_explorer`: `Use the repo-local proposal_explorer agent. Read <proposal>, adjacent docs, the latest baseline artifacts, current code paths, and any existing evidence pack. Stay read-only. Return impacted modules, baseline gaps, contradictions, evidence skeletons, and open questions only.`
- `integration_mapper`: `Use the repo-local integration_mapper agent. Refresh only the named host-system slice for <proposal> using the existing baseline, adjacent docs, and current code paths. Stay read-only. Do not use Xcode or runtime tools. Return reused vs refreshed slices, provenance-labeled facts, remaining unknowns, and artifact rows to update.`
- `ui_reviewer`: `Use the repo-local ui_reviewer agent. Review only visual hierarchy, states, and platform-fit using <proposal or evidence-pack>. Stay read-only. Do not discuss product or architecture.`
- `ux_reviewer`: `Use the repo-local ux_reviewer agent. Review only task flow, trust, recovery, and accessibility using <proposal or evidence-pack>. Stay read-only.`
- `arch_reviewer`: `Use the repo-local arch_reviewer agent. Review only architecture, data flow, state boundaries, testability, and operability using <proposal or evidence-pack>. Stay read-only.`
- `product_reviewer`: `Use the repo-local product_reviewer agent. Review only metrics, rollout, prioritization, business value, and trust tradeoffs using <proposal or evidence-pack>. Stay read-only.`
- `research_scout`: `Use the repo-local research_scout agent. Start only after local context, baseline intake, code mapping, and proposal evidence-pack assembly are complete. Stay read-only. Answer only the supplied research questions with source-backed findings, applicability notes, freshness risks, and reused-versus-refreshed source notes.`

## Proposal-Readiness Workflow

1. Build the proposal evidence pack first.

- Complete the shared local context stage.

2. Enforce the proposal evidence gate.

A defensible `proposal-readiness` review requires all of the following:

- proposal file reviewed
- adjacent repo-local docs reviewed when they materially affect the flow
- latest reusable baseline consumed when present, or an explicit note that it was missing/stale
- impacted modules / code-path map or an explicit note that mapping was not possible
- affected screen / navigation slice mapped from baseline or targeted refresh
- state coverage matrix completed
- proposal completeness matrix completed

This mode does not require:

- build/run attempts
- simulator screenshots
- exhaustive runtime state capture

Optional targeted runtime observations from a reusable baseline are allowed only when already present in the baseline or proposal-specific integration context. They are not a default gate.

3. Return an `Evidence Gap Review` only when proposal/doc/code/baseline evidence is insufficient.

Use an `Evidence Gap Review` when:

- the proposal is missing critical sections
- adjacent docs are required but unavailable
- current repo reality cannot be mapped for a critical surface
- baseline and current code are both too weak to support a defensible call
- data/API/persistence/auth touchpoints remain too unclear for a defensible review
- the completeness matrix or state coverage matrix cannot be filled without guesswork

Do not use an `Evidence Gap Review` merely because the app was not run.

4. Lock review scope and assumptions.

- Confirm target flow, reviewed surface, and out-of-scope areas.
- Record explicit assumptions, contradictions, baseline freshness notes, open questions, blockers, and intentional deferrals in the evidence pack.
- Decide whether the optional product overlay is in scope before specialist review begins.

5. Run discipline tracks.

When the request is substantial and subagents are available, proposal mode may use specialist subagents. Keep orchestration and evidence-pack assembly in the main thread.

- `proposal_explorer` first when the proposal, baseline slice, adjacent docs, or code-path mapping are still fuzzy
- `integration_mapper` only when a narrow baseline slice is stale or missing but can be refreshed without switching to the full baseline skill
- `ui-only`
- `ux-only`
- `architecture-only`
- `product-only` only when the product overlay is in scope

Do not spawn every reviewer by default. Use only the disciplines that materially help the current review.

Pass each specialist only:

- the skill path
- the proposal file
- the prepared evidence pack path
- the discipline rubric

Do not pass intended findings or a prewritten diagnosis.

Recommended specialist prompt shape:

`Use $proposal-review-triad in <ui-only | ux-only | architecture-only | product-only> mode at <skill-path>. Review <proposal-file> using <rubric-file>, the latest reusable baseline if referenced in <evidence-pack-file>, and the prepared evidence pack <evidence-pack-file>. Do not spawn subagents. Do not require build/run or simulator evidence. If proposal/doc/code/baseline evidence is insufficient, report evidence gaps explicitly. Return severity-ranked findings with finding IDs, evidence IDs, fixes, acceptance criteria, and confidence.`

6. Consolidate one report in the main thread.

- Merge specialist findings into one report using [assets/final-review-template.md](assets/final-review-template.md).
- Keep findings traceable to evidence IDs.
- Preserve evidence gaps explicitly.
- Keep readiness semantic and operational. Do not add 1-10 scores.

## Research Workflow

Use `research` only when the local proposal/baseline/code review surfaced real unresolved questions that would benefit from modern external guidance. Research mode is not a generic link-harvesting pass.

1. Complete the shared local context stage first.

- Do not start web research before reading the proposal, adjacent docs, the latest reusable baseline when present, any proposal-local integration context, current code/module mapping, and any existing proposal evidence pack.
- Build or refresh the proposal evidence pack first. Record the research triggers in section `O` before you browse.

2. Derive bounded research questions from local evidence only.

Every research question must trace back to at least one local issue:

- proposal gap
- baseline constraint
- host-system integration risk
- unresolved tradeoff

If a question cannot be traced to local evidence, do not research it in this mode.

3. Write to a deterministic artifact next to the proposal.

If the proposal is `path/to/Foo.md`, create or reuse:

- `path/to/Foo.review/`

Write the research artifact to:

- `path/to/Foo.review/research-pack.md`

Default to a single research pack. Only add a separate source-ledger file when it clearly improves reliability or reuse.

4. Reuse prior research carefully.

- Reuse prior research only when the sources are still fresh enough for the current round.
- Re-check temporally unstable or policy-sensitive sources each round.
- When `.review-baselines/current-system-baseline.md` or `<proposal>.review/integration-context.md` changed for the affected surfaces, re-evaluate applicability instead of carrying prior conclusions forward unchanged.
- Be explicit about what was reused versus refreshed in the new research pack.

5. Run bounded external research.

Source preference order:

1. official platform docs / primary documentation / standards
2. strong technical or regulatory primary sources
3. reputable engineering writeups
4. high-quality industry analysis

Rules:

- Use a bounded number of sources. Enough to answer the real questions; not a dump.
- Avoid filler or SEO listicles when better sources exist.
- Separate source-backed facts from model inference.
- Do not let external research override verified repo reality.
- Do not require Xcode MCP or runtime validation for research mode.
- Mark time-sensitive claims so later rounds know what must be rechecked.

6. Write the research pack in the main thread.

- Use [assets/research-pack-template.md](assets/research-pack-template.md).
- Keep findings tied to research-question IDs, source IDs, and local evidence IDs.
- Include a host-system applicability matrix with `Adopt`, `Adapt`, `Watch`, or `Reject`.
- Make proposal deltas concrete enough that a follow-up proposal-readiness review can reuse them directly.

7. Feed research back into review without replacing local truth.

- Use research to strengthen or update the proposal, not to overwrite current repo or baseline evidence.
- When a consolidated proposal review includes research, summarize what changed, which sources were reused or refreshed, and which recommendations depend on time-sensitive guidance.

## Specialist Modes

The specialist modes are `ui-only`, `ux-only`, `architecture-only`, and `product-only`.

Specialist mode rules:

- Never spawn subagents.
- Never restart the full triad.
- Analyze only the assigned discipline.
- Use the prepared proposal evidence pack and any already-prepared baseline artifacts referenced there.
- Do not require runtime evidence.
- If evidence is partial, return partial-confidence findings or evidence gaps instead of inflating certainty.

Mode-specific references:

- `ui-only`: [references/ui-liquid-glass-rubric.md](references/ui-liquid-glass-rubric.md)
- `ux-only`: [references/ux-financial-rubric.md](references/ux-financial-rubric.md)
- `architecture-only`: [references/ios-architecture-rubric.md](references/ios-architecture-rubric.md)
- `product-only`: [references/product-review-rubric.md](references/product-review-rubric.md)

## Product Overlay Rules

The product overlay is optional and evidence-driven.

- Trigger it only when explicitly requested or when the request materially depends on prioritization, scope sequencing, KPIs, adoption, retention, rollout, instrumentation, or business value.
- Use repo-local proposal/docs, the reusable baseline, targeted code inspection, mapped dependencies, and current repo evidence when available.
- Do not invent product conclusions when product evidence is weak.
- Report product evidence gaps explicitly instead.

## Severity, Confidence, and Evidence Completeness

Severity glossary:

- `Critical` = implementation should not start until the issue is resolved
- `High` = major proposal risk, strong contradiction, or missing contract on a core path
- `Medium` = meaningful ambiguity or incompleteness that should be fixed before handoff
- `Low` = localized or non-blocking issue

Confidence levels:

- `High`
- `Medium`
- `Low`

Evidence completeness levels:

- `Complete`
- `Partial`
- `Insufficient`

## Operating Standards

- Keep the workflow evidence-first.
- Anchor claims to proposal text, repo docs, reusable baseline artifacts, code inspection, or cited external sources.
- Prefer current repo reality over proposal intent when they diverge, but record the divergence clearly.
- Keep runtime evidence optional and contextual in proposal review.
- Keep external research second-stage, bounded, and traceable to real local gaps.
- Prefer authoritative and fresh sources, and mark time-sensitive guidance explicitly.
- Use `$integration-context-baseline` when the missing task is host-system baseline building or refresh, not proposal judgment.
- Keep findings crisp, traceable, and implementation-oriented.
