# Pre-Review Evidence Playbook

Collect proposal, baseline, and repo-local evidence before any `proposal-readiness` or `research` review. Keep it tight: consume the reusable baseline first, refresh only the affected local slice, and avoid turning proposal review into a runtime workflow.

## 1. Proposal and Document Inventory

- Read the proposal itself first.
- Read adjacent repo-local materials that materially affect the reviewed flow:
  - issue notes
  - architecture notes
  - rollout notes
  - analytics notes
  - API docs
  - constraints and acceptance criteria
- Record each item as `DOC-*`.
- Extract:
  - target flow
  - explicit scope
  - explicit non-goals
  - deferrals
  - dependencies
  - assumptions
  - blockers

## 2. Reusable Baseline Intake

Before doing fresh mapping, consume reusable local context when present.

Capture `BASE-*` evidence for:

- `.review-baselines/current-system-baseline.md`
- `<proposal>.review/integration-context.md`
- proposal-local evidence packs or prior reviews that still materially matter
- `<proposal>.review/research-pack.md` when a prior research round exists and is still relevant

For each baseline input, record:

- whether it was `Reused`, `Partially refreshed`, or `Missing`
- which surfaces it covers
- why it is fresh enough or stale
- which parts of the proposal still need targeted refresh

If the baseline is missing or stale, do a narrow targeted refresh for the affected surfaces only. Do not force a full runtime workflow inside proposal review.

## 3. Affected Screens / Navigation / Entry Points

Capture `NAV-*` evidence for:

- screens, scenes, or SwiftUI surfaces the proposal touches
- current navigation entry points
- shell surfaces and toolbars
- shared flows or modal entry paths

Use the reusable baseline first when it already covers the affected surfaces. Refresh only the missing or changed slices.

## 4. Code-Path and Module Mapping

Map the reviewed flow through the codebase before judging it.

Capture `MAP-*` evidence for:

- views, view controllers, or SwiftUI screens
- view models, presenters, or coordinators
- domain or use-case layer
- networking touchpoints
- persistence touchpoints
- feature flags
- analytics or telemetry touchpoints

For each mapped area, record:

- file path or module
- relevant symbol or screen
- why it matters to the reviewed flow
- risk if the mapping is wrong

If mapping is not possible, record the blocker explicitly. Do not guess.

## 5. Data / API / Persistence / Auth Touchpoints

Capture `DATA-*` and `AUTH-*` evidence for:

- APIs touched by the proposal
- local persistence or cache changes
- auth/session boundaries
- rollback or cancellation paths

Record what the proposal says, what the current repo already has, and where the mismatches are.

## 6. Current Host-System Integration Surfaces

Capture `INT-*` evidence for:

- current ownership seams the proposal will cross
- reusable host-system surfaces the proposal will attach to
- shell, navigation, shared-component, rollout, or observability seams
- likely conflict surfaces where future implementation can drift

Use the reusable baseline first when it already maps these seams. Refresh only the affected slice.

## 7. State Coverage Matrix

Complete the proposal state coverage matrix for:

- entry
- happy path
- loading
- empty
- validation error
- backend error
- offline / degraded
- retry / recovery
- auth / permission expiry
- rollback / cancellation

Use only these status labels:

- `Specified`
- `Partial`
- `Missing`
- `Contradicted by repo`
- `Deferred intentionally`

If a state is only implied, mark that explicitly in notes instead of upgrading it to `Specified`.

## 8. Feature Flags / Rollout / Rollback

Capture `FLAG-*` evidence for:

- feature flags
- staged rollout
- rollback / hold criteria
- migration or compatibility concerns

If the proposal is silent on rollout but the feature is risky, record a completeness gap.

## 9. Analytics / Instrumentation

Capture `METRIC-*` evidence for:

- existing telemetry hooks
- proposed analytics events
- leading indicators
- guardrails
- review checkpoints

If the product overlay is in scope and these are absent, record the evidence gap. Do not invent metrics.

## 10. Testing Strategy

Capture `TEST-*` evidence for:

- proposal-expected unit coverage
- integration coverage
- UI coverage
- failure-state coverage
- proposal-critical regression gates

Proposal mode does not require that tests are run. It requires that the proposal's intended testing strategy is explicit enough to judge.

## 11. Current Repo Reality / Contradictions

Capture `REAL-*` evidence for:

- current entry points
- current state handling
- current module ownership
- contradictions between proposal and repo reality
- contradictions between the reusable baseline and current repo reality

Prefer current repo reality over proposal intent when they diverge, but record the divergence clearly.

## 12. Proposal Completeness Gate

A `proposal-readiness` review is allowed when all of the following are true:

- the proposal itself was reviewed
- adjacent docs were reviewed when materially relevant
- the latest reusable baseline was consumed when present, or its absence/staleness was recorded
- the affected screen/navigation slice is complete enough to reason about feasibility
- the impacted modules / code path map is complete enough to reason about feasibility
- the state coverage matrix was filled without guesswork
- the proposal completeness matrix was completed

Set evidence completeness to:

- `Complete` when the gate is satisfied
- `Partial` when useful evidence exists but critical sections remain unclear
- `Insufficient` when the available proposal/doc/code/baseline evidence cannot support a defensible review

## 13. Evidence Gap Review Fallback

If the gate is not met, return an `Evidence Gap Review`.

The fallback must state:

- what was reviewed
- what evidence is missing
- blockers
- confidence level
- what can still be said with partial confidence
- what evidence is required to finish the review

## 14. Optional Research Handoff

After the local evidence gate is met, switch into `research` mode only when external knowledge would materially improve the next review round.

Capture `RSH-*` triggers for:

- proposal gaps that need modern guidance
- baseline constraints that create design tension
- host-system integration risks that need external best-practice context
- unresolved tradeoffs where local evidence is real but incomplete

Rules:

- Do not start research before the proposal evidence pack is assembled.
- Every research question must trace back to local evidence IDs.
- Reuse prior research only when it is still fresh enough for the current round.
- Re-check time-sensitive sources when the host-system baseline changed or when the sources are policy/version sensitive.

## 15. When to Switch Skills

If the missing task is reusable host-system context rather than proposal judgment, switch to `$integration-context-baseline`.
