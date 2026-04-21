---
name: proposal-review-router
description: Universal proposal-first, router-first review skill for proposal-driven changes across iOS, macOS, Rust backend/services, Go backend/microservices, and cross-stack API/rollout/product seams. Use when Codex must read a proposal and local repo evidence, fingerprint stack/surface/risk tags, selectively route 2-4 specialist reviewers, produce proposal-readiness findings, run bounded research after local evidence is complete, or judge cross-stack proposal completeness without defaulting to builds, simulator runs, service startup, benchmarks, load tests, or fuzzing.
---

# Proposal Review Router

Run this as a disciplined router with specialists, not as one generic reviewer.

Core invariants:

- Proposal-first: read the proposal and repo-local evidence before judging.
- Fingerprint-first: build evidence-backed stack, surface, and risk tags before routing.
- Router-first: select only reviewers that fit the evidence.
- Selective: target 2-4 reviewers; hard cap 5.
- Evidence-first: every tag, reviewer, finding, and research question traces to evidence IDs.
- Local-first: research starts only after the local evidence pack is complete enough to justify precise questions.

## Supported modes

Default to `auto` unless the user specifies a mode.

Supported modes:

- `auto`
- `proposal-readiness`
- `research`
- `ui-only`
- `ux-only`
- `architecture-only`
- `reliability-only`
- `performance-only`
- `security-only`
- `api-contract-only`
- `observability-rollout-only`
- `product-only`

Compatibility:

- Treat `full-review` as a deprecated alias of `proposal-readiness`.

Mode rules:

- `auto`: build local evidence, fingerprint, route specialists, then synthesize readiness findings.
- `proposal-readiness`: same as `auto`; do not require runtime/build evidence.
- `research`: complete local evidence first, then answer only evidence-backed external questions.
- Specialist-only modes: run the requested discipline only; do not auto-add reviewers unless the user asks for routing.

## Required local intake

Before routing in `auto`, `proposal-readiness`, or `research`, inspect only the evidence needed for the proposal:

1. Read the proposal file.
2. Read adjacent docs that materially affect the change: ADRs, RFCs, tickets, rollout notes, schemas, API docs, analytics docs, incident notes, or linked proposals.
3. Read `.review-baselines/current-system-baseline.md` when present.
4. Read `<proposal>.review/integration-context.md` when present.
5. Read prior `<proposal>.review/evidence-pack.md`, `<proposal>.review/research-pack.md`, or prior review artifacts when they materially affect the current proposal.
6. Inspect manifests and current code-path slices only enough to map impacted screens, modules, crates, packages, handlers, workers, APIs, persistence seams, feature flags, auth boundaries, telemetry, and rollout surfaces.
7. Record all facts in an evidence pack using [assets/evidence-pack-template.md](assets/evidence-pack-template.md) and [references/pre-review-evidence-playbook.md](references/pre-review-evidence-playbook.md).

Do not turn a partial baseline into a full repo remap. Refresh only stale or missing affected slices. If reusable host-system context is missing and cannot be refreshed narrowly, recommend creating or refreshing `<proposal>.review/integration-context.md` instead of expanding this review into a runtime investigation.

## Fingerprint before routing

Build the fingerprint before selecting reviewers.

Allowed `stack_tags`:

- `ios`
- `macos`
- `apple-client`
- `rust-backend`
- `go-backend`
- `microservice`
- `shared-api`
- `cross-stack`

Allowed `surface_tags`:

- `ui`
- `ux`
- `navigation`
- `architecture`
- `state-management`
- `concurrency`
- `background-work`
- `api-contract`
- `persistence`
- `migration`
- `auth`
- `telemetry`
- `feature-flag`
- `rollout`
- `rollback`
- `performance-hot-path`
- `security-boundary`

Allowed `risk_tags`:

- `backward-compatibility`
- `idempotency`
- `data-loss`
- `privacy-sensitive`
- `security-sensitive`
- `latency-sensitive`
- `availability-sensitive`
- `operability-sensitive`
- `multi-service-coordination`
- `user-trust`
- `platform-mismatch`

Rules:

- Attach evidence IDs to every tag.
- Use repo symbols, manifests, proposal sections, adjacent docs, or baseline facts as tag evidence.
- Do not infer a tag merely because it seems plausible.
- Record unknowns as evidence gaps, not guessed tags.

## Reviewer registry and plugin order

Use the built-in registry at [assets/reviewer-registry.yaml](assets/reviewer-registry.yaml).

Merge reviewer definitions in this order:

1. Built-in reviewers from `assets/reviewer-registry.yaml`.
2. Repo-local reviewer plugins from `.codex/reviewers/*.yaml`.
3. Repo-local routing overrides from `.codex/review-router.yaml`.

Repo-local agents:

- Prefer a repo-local agent under `.codex/agents/` when it matches the reviewer id exactly.
- Also match any `preferred_agent_names` listed for that reviewer.
- If no repo-local agent exists, run the reviewer in the main thread using its rubric.
- Do not invent a reviewer when a registry reviewer already fits.

Plugin authors should start from [assets/reviewer-plugin-template.yaml](assets/reviewer-plugin-template.yaml) and [assets/repo-routing-config-template.yaml](assets/repo-routing-config-template.yaml).

## Reviewer selection

Use [references/reviewer-selection-playbook.md](references/reviewer-selection-playbook.md) for detailed scoring.

Selection algorithm:

1. Start from evidence-backed fingerprint tags.
2. Add mandatory reviewers triggered by hard stack/surface/risk evidence.
3. Score optional reviewers using explicit user request, stack match, surface match, risk match, repo signal match, cross-stack dependency, and overlap penalty.
4. Prefer one primary architecture reviewer per active implementation stack.
5. Add cross-cutting reviewers only when contract, rollout, telemetry, migration, security, or product evidence warrants them.
6. De-duplicate reviewers with overlapping scope.
7. Keep the normal set at 2-4 reviewers; hard cap 5.
8. Record selected reviewers with evidence IDs.
9. Record close alternatives rejected and why.

Built-in reviewer families:

- Apple: `ios_ui_reviewer`, `macos_ui_reviewer`, `apple_ux_reviewer`, `apple_arch_reviewer`.
- Rust: `rust_arch_reviewer`, `rust_reliability_reviewer`, `rust_performance_reviewer`, `rust_security_reviewer`.
- Go: `go_service_arch_reviewer`, `go_reliability_reviewer`, `go_performance_reviewer`, `go_security_reviewer`.
- Cross-cutting: `api_contract_reviewer`, `observability_rollout_reviewer`, `product_reviewer`.

Product review remains opt-in unless metrics, product decision checkpoints, experiment design, prioritization, or adoption risk are central to the proposal.

## Proposal-readiness evidence gate

A normal `auto` or `proposal-readiness` review requires:

- proposal file reviewed
- relevant adjacent docs reviewed or explicitly marked absent
- reusable baseline consumed when present or marked missing/stale with scope
- proposal integration context consumed when present
- prior evidence/research artifacts consumed when relevant
- current code-path map or explicit mapping gap
- manifests inspected for stack signals when needed
- fingerprint table populated with evidence IDs
- selected and rejected reviewer rationale recorded
- state/failure coverage matrix completed
- proposal completeness matrix completed
- findings tied to evidence IDs and file/line references when possible

This mode must not require:

- build/run attempts
- simulator runs
- service startup
- benchmarks
- load tests
- fuzzing

Mention these only as optional later validation when the proposal itself needs them.

## Evidence-gap fallback

Return an `Evidence Gap Review` instead of speculative findings when any of these block a defensible judgment:

- proposal text is unavailable or too incomplete to identify target behavior
- no repo-local evidence exists for a claimed subsystem
- current code-path ownership cannot be mapped narrowly
- baseline and proposal contradict each other and current code cannot resolve it
- external research is needed but local evidence is not complete enough to ask precise questions

Evidence gap output must state:

- missing evidence
- why it blocks routing or findings
- exact local files or artifacts to collect next
- whether targeted integration-context refresh is needed

## Research mode

Use [references/research-mode-playbook.md](references/research-mode-playbook.md).

Research starts only after the local evidence pack has:

- proposal and adjacent docs inventory
- baseline status
- code-path or manifest map
- fingerprint tags with evidence IDs
- research triggers with local evidence IDs

Use web research only for narrow questions that local evidence cannot settle, such as current platform guidance, active library behavior, protocol compatibility, security guidance, or version-sensitive rollout practice. Write findings into [assets/research-pack-template.md](assets/research-pack-template.md) as `<proposal>.review/research-pack.md` when producing artifacts.

## Repeat-run md5 guard

For every final review answer, compute the MD5 hash of the reviewed proposal file contents and include it near the synthesis metadata:

```text
Reviewed proposal md5: <32-hex-md5>
```

When the user repeats a review request for the same proposal in the same conversation:

1. Compute the current MD5 of the proposal file before deciding whether to reuse prior findings.
2. Compare it with the most recent `Reviewed proposal md5` previously emitted for that proposal.
3. If the hash differs, treat the proposal as changed: reread the proposal and run a fresh review pass.
4. If the hash matches, do not rerun local evidence gathering or reread the proposal unless the user explicitly asks for a fresh review. Re-emit the prior review answer and preserve the same hash.
5. If no prior hash is available for that proposal, run the normal proposal-first intake and include the new hash in the result.

## Final synthesis

Use [assets/final-review-template.md](assets/final-review-template.md).

Final output must include:

- reviewed proposal md5
- mode
- selected reviewers
- rejected close alternatives
- fingerprint summary
- baseline status
- proposal completeness judgment
- severity-ranked findings
- evidence gaps or open questions
- product fields when product review is selected: `Leading metric`, `Guardrail metric`, and `Decision checkpoint`

For code review UIs that support inline comments, emit one finding per issue with tight file and line ranges. For plain Markdown reports, use the final review template.

## Rubric loading

Load only rubrics for selected reviewers:

- `ios_ui_reviewer`: [references/rubrics/ios-ui-rubric.md](references/rubrics/ios-ui-rubric.md)
- `macos_ui_reviewer`: [references/rubrics/macos-ui-rubric.md](references/rubrics/macos-ui-rubric.md)
- `apple_ux_reviewer`: [references/rubrics/apple-ux-rubric.md](references/rubrics/apple-ux-rubric.md)
- `apple_arch_reviewer`: [references/rubrics/apple-architecture-rubric.md](references/rubrics/apple-architecture-rubric.md)
- `rust_arch_reviewer`: [references/rubrics/rust-architecture-rubric.md](references/rubrics/rust-architecture-rubric.md)
- `rust_reliability_reviewer`: [references/rubrics/rust-reliability-rubric.md](references/rubrics/rust-reliability-rubric.md)
- `rust_performance_reviewer`: [references/rubrics/rust-performance-rubric.md](references/rubrics/rust-performance-rubric.md)
- `rust_security_reviewer`: [references/rubrics/rust-security-rubric.md](references/rubrics/rust-security-rubric.md)
- `go_service_arch_reviewer`: [references/rubrics/go-service-architecture-rubric.md](references/rubrics/go-service-architecture-rubric.md)
- `go_reliability_reviewer`: [references/rubrics/go-reliability-rubric.md](references/rubrics/go-reliability-rubric.md)
- `go_performance_reviewer`: [references/rubrics/go-performance-rubric.md](references/rubrics/go-performance-rubric.md)
- `go_security_reviewer`: [references/rubrics/go-security-rubric.md](references/rubrics/go-security-rubric.md)
- `api_contract_reviewer`: [references/rubrics/api-contract-rubric.md](references/rubrics/api-contract-rubric.md)
- `observability_rollout_reviewer`: [references/rubrics/observability-rollout-rubric.md](references/rubrics/observability-rollout-rubric.md)
- `product_reviewer`: [references/rubrics/product-review-rubric.md](references/rubrics/product-review-rubric.md)

## Output discipline

Findings must be concrete and auditable:

- Cite evidence IDs from the evidence pack.
- Cite proposal lines or current-code paths when available.
- State the behavioral or implementation risk.
- Give a specific fix and acceptance criteria.
- Prefer fewer high-confidence findings over broad commentary.
- Do not report lack of runtime validation as a finding in proposal-readiness mode.
