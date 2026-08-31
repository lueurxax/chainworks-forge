# Proposal Area Conventions

Use `proposal-review-router` for proposal-readiness reviews in this directory.

## Artifact layout

For `docs/proposals/NNN-title.md`, reusable review artifacts live under:

```text
docs/proposals/NNN-title.review/
├── integration-context.md
├── evidence-pack.md
├── research-pack.md
└── proposal-readiness-review.md
```

Implementation audits, when requested, use versioned files beside the proposal:

```text
docs/proposals/NNN-title_IMPLEMENTATION_AUDIT_RN.md
```

## Proposal-readiness rules

- Read the proposal first.
- Measure the proposal with `wc -l` before refinement or review. At `>= 2,000` physical lines, report a scope blocker and require decomposition; do not request or add further substantive detail to the same file.
- An over-limit parent may retain only the problem statement, shared invariants, dependency order, cheapest useful hypothesis test, minimal acceptance criteria, and links. Independently implementable contracts, proof obligations, migrations, UI slices, and rollout phases belong in child proposals with their own scope, acceptance criteria, proof gate, and lifecycle.
- Existing over-limit proposals may receive only decomposition corrections. Generated evidence, implementation-audit output, and canonical reference documentation are outside this active-proposal line budget.
- Reuse `.review-baselines/current-system-baseline.md` and `docs/reference/current-system-baseline.md` before source archaeology.
- Prefer current `docs/reference/` docs over stale proposal dependencies.
- Inspect only current code slices needed for the proposal's claimed seams.
- Build stack/surface/risk fingerprints before selecting reviewers.
- Record selected reviewers and rejected close alternatives.
- Do not require builds, Xcode, simulator, daemon startup, cargo tests, benchmarks, load tests, or fuzzing.

## Routing notes

- macOS UI and operator-flow proposals use `macos_ui_reviewer` and possibly `apple_ux_reviewer`.
- Swift app architecture proposals use `apple_arch_reviewer`.
- Rust control-plane proposals use `rust_arch_reviewer`; add `rust_reliability_reviewer` for retry, resume, work queue, cancellation, recovery, or idempotency.
- Contract proposals touching GraphQL, MCP, ACP, workflow YAML, agent catalog YAML, reports, resources, or future Go APIs use `api_contract_reviewer`.
- Migration, test-gate, rollout, rollback, release receipt, telemetry, or support-debug proposals use `observability_rollout_reviewer`.
- Durable execution semantics use `chainworks_execution_truth_reviewer`.
- Go/Temporal extraction proposals use Go reviewers, but do not invent Go implementation facts unless Go code exists.
- Product review is opt-in unless metrics or launch decision checkpoints are central.

## Finding quality

Findings must cite proposal lines, evidence IDs, and current repo owners where possible. Prefer narrow P1/P2 issues over broad commentary.
