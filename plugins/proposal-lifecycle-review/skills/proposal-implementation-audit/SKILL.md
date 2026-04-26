---
name: proposal-implementation-audit
description: Universal proposal-vs-implementation audit for iOS, macOS, Rust backend/services, Go backend/microservices, and cross-stack contract/rollout changes. Compares a proposal/spec and any prior proposal-review routing against the current implementation or diff, reuses selected reviewers when valid, produces atomic REQ-* conformance statuses plus routed implementation findings, and writes exactly one versioned audit report beside the proposal. Use only when explicitly asked to audit implementation against a proposal/spec or evaluate proposal-aligned implementation readiness; do not use for generic code review, proposal editing, feature implementation, or architecture brainstorming.
---

# Proposal Implementation Audit

Run a proposal-anchored implementation audit. The job is not “review this code in general.” The job is to answer a narrower question with evidence: **does this implementation satisfy the proposal, and does the implemented slice hold up under the specialist reviewers that the proposal warranted?**

The skill is universal across these families:

- iOS
- macOS
- Rust backend / services
- Go backend / server / microservices
- cross-stack API, schema, rollout, telemetry, product, reliability, performance, and security seams

Keep the audit low-thrash. Read the proposal first, reuse prior proposal-review routing when it still fits, inspect only the implementation surfaces needed to verify the contract, and write exactly one new versioned report beside the proposal.

## Trigger Boundaries

Use this skill only when it is explicitly invoked or when the user explicitly asks for an implementation audit of a proposal/spec.

Do not use this skill for:

- generic code review without a proposal/spec anchor
- implementation work
- proposal rewriting or editing
- pure architecture brainstorming
- broad repo health review
- unrelated UX critique detached from the proposal contract

Because this workflow writes a report file, it must not be invoked implicitly.

## Read-Only Boundary

The audit may:

- inspect the proposal, adjacent docs, implementation code, tests, configs, manifests, migrations, API schemas, generated sources, assets, screenshots, diagrams, logs, and previous review artifacts
- inspect `.review-baselines/current-system-baseline.md` and `<proposal>.review/integration-context.md` when present
- inspect prior proposal-review reports, evidence packs, research packs, and reviewer-selection summaries
- run focused searches, builds, tests, benchmarks, linters, UI checks, service-level checks, and runtime validation when practical and relevant
- write exactly one new versioned audit report beside the proposal

The audit must not:

- modify implementation files, tests, configs, generated sources, or assets
- modify the proposal or prior review artifacts
- write additional sidecar files beyond the single generated report
- broaden into implementation work unless the user separately asks for that work
- mark a requirement implemented from inference alone

## Input Model

The primary input is a proposal path. The implementation target is one of:

- current worktree / current branch, when the user gives only a proposal path
- a PR branch or diff, when the user gives a compare target
- a specific commit/range, when the user gives one
- a manually supplied implementation directory, when the repo layout requires it

If the implementation target is implicit, audit the current worktree and record that the compare base was implicit.

Supported modes:

- `auto`: default; reuse prior proposal-review routing when valid, then audit implementation
- `implementation-audit`: explicit alias of `auto`
- `implementation-readiness`: conformance plus ship/handoff gates; use this when the user asks “is this ready?”
- `conformance-only`: only `REQ-*` proposal-contract audit; no specialist findings unless they explain a requirement status
- `reuse-proposal-review-selection`: prefer prior selected reviewers and only add delta reviewers for new implementation risks
- `reroute`: ignore prior reviewer selection except as context; route from current proposal and implementation evidence
- `diff-only`: audit only changed files and required adjacent context
- specialist modes: `ui-only`, `ux-only`, `architecture-only`, `reliability-only`, `performance-only`, `security-only`, `api-contract-only`, `observability-rollout-only`, `product-only`

## Core Audit Model

The audit has two tracks. Do not collapse them.

### Track 1: Objective Proposal-Conformance Audit

Convert explicit proposal commitments into atomic `REQ-*` items and classify each with the normalized status model:

- `Implemented`
- `Partially Implemented`
- `Missing`
- `Not Verifiable`
- `Out of Scope`

This track answers: **did the current implementation satisfy what the proposal explicitly committed to?**

### Track 2: Routed Specialist Implementation Review

Use the reviewer registry to select only relevant implementation reviewers. Findings use stable prefixes:

- `ARCH-*` architecture and ownership boundaries
- `UI-*` iOS/macOS visual, navigation, accessibility, and platform-fit issues
- `UX-*` user journey, recovery, trust, clarity, accessibility interaction issues
- `PROD-*` user value, product completeness, metric/decision quality
- `REL-*` reliability, idempotency, cancellation, shutdown, overload, recovery
- `PERF-*` performance, latency, throughput, allocations, contention, benchmarks
- `SEC-*` security, auth, privacy, unsafe/FFI, validation, public boundary issues
- `API-*` API/schema/backward-compatibility/contract issues
- `OPS-*` observability, rollout, migration, rollback, feature flag, operational readiness
- `READY-*` release/handoff/testing/readiness issues

This track answers: **even if the proposal is implemented, does the implemented slice hold up under the disciplines it actually touches?**

### Separation Rule

- Do not downgrade a `REQ-*` status because of a generic preference or subjective specialist opinion.
- Change `REQ-*` status only when the proposal explicitly promised the behavior, constraint, decision, flow, API, UI, UX, migration, rollout, telemetry, test, or evidence.
- If something is risky, awkward, non-native, fragile, or hard to operate but not explicitly promised by the proposal, record it as a specialist finding instead.
- A feature can have `Overall Conformance = Implemented` and still have `Overall Implementation Readiness = Not Ready`.

## Prior Proposal-Review Reuse

The implementation audit should try to reuse reviewer selection from the proposal review. This keeps the proposal and implementation conversations aligned and avoids a fresh round of random reviewer roulette.

### Discovery Order

Look for prior proposal-review artifacts in this order:

1. A path explicitly supplied by the user.
2. `<proposal>.review/` beside the proposal, including files such as:
   - `evidence-pack.md`
   - `final-review.md`
   - `research-pack.md`
   - `reviewer-selection.md`
   - `integration-context.md`
3. Sibling files beside the proposal matching names like:
   - `<proposal-stem>_PROPOSAL_REVIEW_R*.md`
   - `<proposal-stem>_REVIEW_R*.md`
   - `<proposal-stem>_EVIDENCE_PACK*.md`
   - `<proposal-stem>_RESEARCH_PACK*.md`
4. Repo-local review directories such as `.review/`, `.reviews/`, `docs/reviews/`, or `docs/proposal-reviews/` when they clearly refer to the proposal.
5. The helper script when available:
   ```bash
   python3 <skill-dir>/scripts/discover_prior_review.py /abs/path/to/proposal.md
   ```

Ignore prior `IMPLEMENTATION_AUDIT` reports for reviewer selection unless the user explicitly asks to compare against a previous implementation audit.

### What to Extract

From prior proposal-review artifacts, extract:

- selected reviewers
- rejected close alternatives
- detected stacks
- detected surfaces
- detected risks
- proposal completeness gaps
- required changes before implementation
- current repo contradictions found during proposal review
- research conclusions that affected implementation choices
- leading metric, guardrail metric, decision checkpoint, rollout recommendation when present
- evidence IDs and their provenance

### Reuse Validity States

Classify reviewer-selection reuse as one of:

- `Reused exactly`: prior reviewer set still matches proposal and implementation evidence
- `Reused with delta`: prior reviewer set is still relevant, but implementation introduced new surfaces or risks requiring additional reviewers
- `Partially reused`: some prior reviewers are still relevant, others are stale or over-broad
- `Not reused`: no prior selection found or it conflicts with current evidence
- `Forced reroute`: user asked to ignore prior selection

### Reuse Rules

Reuse prior selected reviewers when all are true:

- the prior review clearly refers to the same proposal or direct predecessor
- the proposal state is still `Active` or the superseding proposal is included in the audit scope
- the implementation touches the same primary stacks/surfaces/risks
- no new high-risk implementation-only surface appears in the diff or current code path
- the prior reviewer set does not exceed the current hard cap unless the user explicitly requests full carry-over

Add delta reviewers when implementation evidence introduces new risks not present in the proposal review, for example:

- proposal selected `go_service_arch_reviewer`, but implementation adds auth middleware: add `go_security_reviewer`
- proposal selected `rust_arch_reviewer`, but implementation adds retry/backpressure worker behavior: add `rust_reliability_reviewer`
- proposal selected `ios_ui_reviewer`, but implementation changes API schema: add `api_contract_reviewer`
- proposal selected `api_contract_reviewer`, but implementation includes migration/feature-flag rollout: add `observability_rollout_reviewer`

Do not blindly reuse prior reviewers when:

- the implementation touches a different stack than the proposal review assumed
- the prior review was marked under-routed or evidence-insufficient
- the proposal has been superseded and the new proposal changes the implementation surface
- the implementation contains security, data migration, public API, unsafe/FFI, auth, or performance hot-path risks not represented in the prior reviewer set

### Reviewer Count

Target reviewer count: 2–4. Hard cap: 5.

When reusing a prior set that already contains 5 reviewers, adding a delta reviewer requires dropping a lower-relevance reviewer and recording why.

## Plugin and Registry Model

Load reviewer definitions and routing overrides in this order:

1. Built-in implementation reviewers from `assets/implementation-reviewer-registry.yaml`.
2. Repo-local reviewer definitions from `.codex/reviewers/*.yaml`.
3. Repo-local implementation reviewer definitions from `.codex/implementation-reviewers/*.yaml`.
4. Repo-local router overrides from `.codex/review-router.yaml`.
5. Repo-local implementation audit overrides from `.codex/implementation-audit-router.yaml`.

Repo-local agents under `.codex/agents/` should be preferred when their name exactly matches the reviewer id or one of the reviewer’s `preferred_agent_names`.

Do not copy the whole global registry into a repo plugin. Good plugins are thin: a few route overrides, a few precise custom reviewers, and a small number of discipline-scoped agents.

## Workflow

1. Resolve the proposal path and confirm it is an existing Markdown file.
2. Determine the repository root and implementation target:
   - repo root
   - current git SHA
   - working tree status
   - compare base or PR/diff target when provided
   - full audit timestamp
3. Create the report path with the bundled helper:
   ```bash
   python3 <skill-dir>/scripts/report_path.py /abs/path/to/proposal.md
   ```
4. Determine proposal state before auditing implementation:
   - `Active`
   - `Superseded`
   - `Deprecated`
   - `Replaced`
   - `Ambiguous`
5. Discover prior proposal-review artifacts and classify reviewer-selection reuse.
6. Extract the proposal contract before inspecting implementation details. Record:
   - scope
   - target stacks/platforms/services
   - platform/product scope:
     - Apple: `iOS`, `macOS`, `Universal`, or `Ambiguous`
     - backend/service: service, worker, API, data, rollout, or cross-stack scope
   - locked decisions
   - acceptance criteria
   - test / evidence requirements
   - explicit exclusions / non-goals
   - user flows / jobs-to-be-done
   - UI commitments
   - UX commitments
   - API/schema commitments
   - data/persistence commitments
   - security/auth/privacy commitments
   - reliability/performance commitments
   - rollout/telemetry/migration commitments
   - when platform or service scope is ambiguous, say so explicitly and lower confidence where appropriate
7. Derive 1-5 primary user/service implementation flows:
   - use the most important end-to-end tasks a user, caller, operator, worker, or integration should be able to complete
   - Apple: user-visible end-to-end journeys, navigation entries, states, permissions
   - Rust/Go services: request/worker/event paths, persistence paths, retry/idempotency paths, shutdown/deploy paths
   - Cross-stack: client ↔ API ↔ service ↔ storage ↔ telemetry/rollout paths
8. Convert explicit proposal commitments into atomic `REQ-*` items.
   - Assign stable IDs: `REQ-001`, `REQ-002`, ...
   - Record proposal source precisely: heading plus closest line/anchor/reference when possible
   - Audit requirement-by-requirement, never paragraph-by-paragraph
9. Build an implementation evidence pack:
   - changed files / touched modules / generated files
   - proposal-to-code mapping
   - tests found and tests run
   - validation commands and results
   - runtime/screenshot evidence when actually obtained
   - API/schema/migration/flag/telemetry evidence
   - security/perf/reliability evidence when relevant
10. Build an implementation fingerprint:
    - stack tags
    - surface tags
    - risk tags
    - evidence IDs for every tag
    - proposal fidelity / divergence buckets:
      - Matches
      - Divergences
      - Ambiguities / Evidence Gaps
11. Route reviewers:
    - reuse prior proposal-review selection when valid
    - add or remove reviewers only based on current evidence
    - record selected reviewers and rejected close alternatives
12. Inspect only implementation surfaces relevant to:
    - the proposal contract
    - the primary implementation flows
    - prior proposal-review findings that should have been addressed
    - selected reviewer disciplines
    - disputed or high-risk findings
13. Verify with the strongest practical evidence:
    - prefer targeted `rg`, focused file reads, and narrow tests over broad scans
    - distinguish `tests-found` from `tests-run`
    - use `runtime` only when live behavior was actually validated
    - for UI/UX claims, prefer runtime, screenshots, executed UI tests, previews, or snapshot tests when practical
    - for services, prefer focused unit/integration tests, contract tests, logs/traces, migration checks, or benchmark output when relevant
    - if the audit is trending toward a successful roll-up, run the repository's full regression suite or canonical full/proposal gate on the same tree/HEAD before locking the verdict
    - treat `Overall Conformance = Implemented`, `Overall Implementation Readiness = Ready`, and `Overall Implementation Readiness = Ready with Risks` as successful outcomes that require passing same-tree full regression evidence
    - if full regression is unavailable, red, stale, or from a different tree/HEAD, fail closed and downgrade the verdict
14. Audit every `REQ-*` item with the normalized status model.
15. Produce routed specialist findings only from selected reviewers, plus mandatory `READY-*` findings when readiness is blocked.
16. Roll up:
    - `Overall Conformance`
    - `Overall Implementation Readiness`
    - `Reviewer Selection Reuse`
    - `Audit Confidence`
17. Write the versioned report beside the proposal.
18. In chat, state the verdict directly and point to the generated report path.

## Contract Extraction Guidance

Use these meanings consistently:

- User flows / jobs-to-be-done: primary tasks users or callers should be able to complete
- Service flows: request, worker, stream, cron, queue, event, migration, deploy, rollback, and shutdown paths
- UI commitments: screens, layout expectations, navigation IA, controls, components, sidebars, toolbars, badges, labels, visual states
- UX commitments: discoverability, task clarity, onboarding, empty/loading/error states, recovery, confirmation flows, trust, accessibility-related interaction behavior
- API/schema commitments: endpoints, methods, proto/OpenAPI/schema fields, request/response shape, versioning, compatibility, pagination, validation
- Reliability commitments: retries, idempotency, deadlines, cancellation, backpressure, error handling, shutdown, replay/recovery
- Performance commitments: latency, throughput, allocation, locking, serialization, batching, benchmark targets
- Security commitments: auth, authorization, input validation, secrets, PII, privacy, unsafe/FFI, rate limits, public boundary behavior
- Rollout commitments: flags, dark launch, migration, rollback, observability, alerts, decision gates

If the proposal is vague:

- do not invent missing requirements
- place uncertainty in `Ambiguities / Evidence Gaps`
- record low/medium-confidence specialist findings when useful
- do not fabricate a failed `REQ-*` item from personal preference

## Requirement Model

For every `REQ-*` item include:

- short title
- proposal source
- status
- evidence type(s)
- evidence references
- implementation mapping
- gap / note

Status rules:

- `Implemented`: proven by direct evidence; never by inference alone
- `Partially Implemented`: some meaningful portion exists, but committed behavior is incomplete
- `Missing`: committed behavior, constraint, or evidence is absent
- `Not Verifiable`: behavior may exist, but the audit could not prove it with available evidence
- `Out of Scope`: proposal explicitly excluded it or the implementation target does not cover that slice

Roll-up rules for Track 1:

- `Overall Conformance = Implemented` if every in-scope requirement is `Implemented` and passing same-tree full regression or canonical full/proposal gate evidence exists
- `Overall Conformance = Not Implemented` if any in-scope requirement is `Missing`
- `Overall Conformance = Partial` if at least one is `Partially Implemented` or `Not Verifiable` and none are `Missing`
- `Overall Conformance = Not Verifiable` only when most critical in-scope requirements cannot be verified
- `Overall Implementation Readiness = Ready` or `Ready with Risks` requires passing same-tree full regression or canonical full/proposal gate evidence
- If the audit did not execute successful same-tree full regression/canonical gate evidence, do not report a successful audit verdict

Readiness values:

- `Ready`: no blocking `REQ-*` gaps, critical/major reviewer findings, or missing critical evidence; same-tree full regression/canonical gate passed
- `Ready with Risks`: no unresolved critical blocker, but bounded major/minor risks remain; same-tree full regression/canonical gate passed
- `Not Ready`: missing critical evidence, blocked primary flow, failed/missing gate evidence, or unresolved major/critical findings make ship/handoff unsafe
- `Blocked`: the audit cannot establish readiness because the proposal, implementation target, or required evidence is inaccessible or contradictory

## Evidence Model

Allowed evidence types:

- `proposal`
- `prior-review`
- `code`
- `diff`
- `tests-found`
- `tests-run`
- `benchmark-found`
- `benchmark-run`
- `runtime`
- `screenshot`
- `design-reference`
- `schema`
- `migration`
- `config`
- `telemetry`
- `log-or-trace`
- `inference`

Evidence rules:

- Never mark `Implemented` from `inference` alone.
- Distinguish `tests-found` from `tests-run` and `benchmark-found` from `benchmark-run`.
- Use `runtime` only when live behavior was actually validated.
- Use `prior-review` as context, not as proof that implementation is correct.
- If prior proposal review identified required changes, verify whether the implementation actually addressed them.
- If implementation evidence contradicts prior review assumptions, prefer current implementation evidence and record the drift.
- If only code inspection was possible for a behavior that needs runtime proof, lower confidence or use `Not Verifiable`.

## Specialist Reviewer Guidance

### iOS / macOS UI and UX

Use `ios_ui_reviewer`, `macos_ui_reviewer`, and `apple_ux_reviewer` only when user-visible client behavior is in scope.

Assess:

- proposal-mandated screens, navigation, controls, and states
- iOS and macOS conventions separately when both platforms are in scope
- do not assume an iOS pattern is correct for macOS, or a macOS pattern is correct for iOS
- if the proposal explicitly chooses a nonstandard platform behavior, preserve the `REQ-*` status and record the platform-fit risk as a `UI-*`, `UX-*`, or `PROD-*` finding unless it violates an explicit requirement
- accessibility, dynamic type, keyboard behavior, focus, VoiceOver, localization risk
- empty/loading/error/offline/permission states
- whether runtime, screenshots, previews, or UI tests actually support UI/UX claims

### Apple Architecture

Use `apple_arch_reviewer` for Swift/SwiftUI/UIKit/AppKit implementation boundaries.

Assess:

- state ownership and data flow
- navigation/coordinator/router/deep-link behavior
- dependency flow and side-effect isolation
- concurrency and main-thread safety
- persistence/sync model
- shared vs platform-specific code boundaries
- feature flags, telemetry, tests, and locked architectural decisions

### Rust Backend / Services

Use Rust reviewers only for Rust implementation surfaces.

Architecture checks:

- workspace/crate/module boundaries
- trait/API design and error semantics
- async runtime boundaries and blocking work isolation
- persistence, schema, queue, and protocol seams
- testability and operability hooks

Reliability checks:

- idempotency, retries, deadlines, cancellation, backpressure
- task ownership, graceful shutdown, replay/recovery
- queue semantics, worker lifecycle, overload behavior

Performance checks:

- latency/throughput hot paths, allocations, locks, serialization, batching
- benchmark presence and whether benchmark claims were actually run

Security checks:

- auth boundaries, input validation, secrets, PII, unsafe/FFI, deserialization, rate limits

### Go Backend / Microservices

Use Go reviewers only for Go implementation surfaces.

Architecture checks:

- package boundaries, `cmd/`, `internal/`, `pkg/`, transport/domain/persistence separation
- interface placement, dependency direction, generated code ownership
- context propagation and lifecycle boundaries

Reliability checks:

- context cancellation, deadlines, retry behavior, goroutine leaks, graceful shutdown
- queue/worker/idempotency/backpressure behavior

Performance checks:

- allocation/GC pressure, JSON/protobuf encoding, pooling, lock contention, p99-sensitive paths
- benchmarks and profiles when the proposal claimed performance behavior

Security checks:

- authN/authZ, request validation, SSRF-like outbound calls, secrets, TLS, PII, rate limits

### Cross-Cutting Reviewers

Use `api_contract_reviewer` for schema/API compatibility, generated types, client/server contract drift, versioning, migrations, and deprecation behavior.

Use `observability_rollout_reviewer` for feature flags, metrics, logs, traces, migrations, rollback, kill switches, alerts, health checks, and deploy readiness.

Use `product_reviewer` only when user value, metrics, rollout decisions, experiment gates, or proposal acceptance criteria are central. Product review should not become a default catch-all.

## Findings Model

Every specialist finding must include:

- Finding ID
- Reviewer
- Severity: `Critical`, `Major`, `Minor`, or `Note`
- Confidence: `High`, `Medium`, or `Low`
- Related proposal items and/or `REQ-*` IDs when applicable
- Evidence type(s)
- Evidence references
- Why it matters
- Recommended action
- Acceptance criteria

Do not emit findings that are pure taste. Ground them in proposal text, code, diff, assets, tests, runtime evidence, platform conventions, service correctness constraints, API compatibility, operational requirements, or clearly stated ambiguity.

## Output Report Requirements

The generated report must include, at minimum:

- metadata table
- implementation target / compare base
- prior proposal-review reuse summary
- selected reviewers
- rejected close alternatives
- proposal state and contract summary
- platform/product scope:
  - Apple: `iOS`, `macOS`, `Universal`, or `Ambiguous`
  - backend/service: service, worker, API, data, rollout, or cross-stack scope
- 1-5 primary user/service implementation flows
- proposal fidelity / divergence inventory with `Matches`, `Divergences`, and `Ambiguities / Evidence Gaps`
- requirement summary
- detailed `REQ-*` audit
- reviewer/lens scorecard covering conformance, selected reviewer disciplines, readiness, top risk, and confidence
- routed specialist findings
- readiness checklist including:
  - build or canonical gate status
  - core user/service flow runtime or integration validation when relevant
  - empty/loading/error/offline/permission states when UI/UX is in scope
  - accessibility, localization, privacy, permissions, and entitlements risk when relevant
  - critical tests executed
  - full regression suite or canonical full/proposal gate passed on the audited tree/HEAD
- verification log
- final verdict and recommended next actions

If product review is selected or product metrics were present in the prior proposal review, preserve:

- `Leading metric:`
- `Guardrail metric:`
- `Decision checkpoint:`

## Report Path

Write exactly one new versioned report beside the proposal using:

```bash
python3 <skill-dir>/scripts/report_path.py /abs/path/to/proposal.md
```

The helper preserves the existing naming convention:

```text
<proposal-stem>_IMPLEMENTATION_AUDIT_R<N>.md
```

Do not overwrite an existing report.

## Done Condition

The audit is complete only when:

- exactly one versioned report was written beside the proposal
- Track 1 `REQ-*` conformance and Track 2 routed specialist review remain separate
- the report records implementation target, compare base, proposal state, prior-review reuse, selected reviewers, and rejected close alternatives
- the report includes platform/product scope, 1-5 primary user/service flows, proposal fidelity/divergence buckets, requirement audit, reviewer/lens scorecard, readiness checklist, verification log, and recommended next actions
- any successful verdict (`Implemented`, `Ready`, or `Ready with Risks`) is backed by passing same-tree full regression or canonical full/proposal gate evidence recorded in the verification log
- stale or different-tree regression evidence was not reused for a successful verdict

## Chat Response

After writing the report, keep the chat response short:

- direct verdict
- overall conformance
- overall readiness
- reviewer-selection reuse status
- highest-risk blockers
- generated report path

Do not paste the whole report into chat unless the user explicitly asks.
