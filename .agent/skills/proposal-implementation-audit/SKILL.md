---
name: proposal-implementation-audit
description: Dual-track multi-lens audit of an iOS/macOS proposal or spec against the current repository implementation. Produce an evidence-based proposal-vs-implementation report with atomic REQ-* conformance statuses plus architecture, product, UI, UX, and readiness findings, then write exactly one new versioned report beside the proposal. Use only when explicitly asked to audit implementation against a proposal/spec or evaluate proposal-aligned readiness; do not use for generic code review, feature implementation, proposal editing, or architecture brainstorming.
---

# Proposal Implementation Audit

Run a proposal-vs-implementation audit for Apple-platform work. Keep the audit proposal-anchored, evidence-first, low-thrash, and split into two separate tracks:

- Track 1: objective `REQ-*` proposal-conformance audit
- Track 2: expert multi-lens review across Architecture, Product, UI, UX, and Delivery / Readiness

Do not collapse these tracks together. A requirement can be implemented while the shipped flow is still risky, incoherent, or unready.

## Trigger Boundaries

Use this skill only when it is explicitly invoked or when the user explicitly asks for an implementation audit of a proposal/spec.

Do not use this skill for:
- generic code review
- feature implementation
- proposal rewriting or editing
- architecture brainstorming without an implementation-audit request
- unrelated UX critique detached from a proposal/spec contract

Because this workflow writes a report file, it must not be invoked implicitly.

## Read-Only Boundary

The audit may:
- inspect code, tests, configs, docs, and proposal-adjacent artifacts
- inspect linked or nearby design references, screenshots, PDFs, diagrams, preview assets, test artifacts, and related review docs when they help clarify proposal intent or evidence
- run focused searches, builds, tests, previews, UI checks, and runtime validation
- write exactly one new versioned audit report beside the proposal

The audit must not:
- modify the proposal
- modify implementation files, tests, configs, or assets
- write additional docs or sidecar files beyond the single generated report
- broaden into implementation work unless the user separately asks for that work

## Audit Model

### Track 1: Objective Proposal-Conformance Audit

Convert the proposal contract into atomic `REQ-*` items and classify each with the normalized status model:

- `Implemented`
- `Partially Implemented`
- `Missing`
- `Not Verifiable`

This track answers: did the current implementation satisfy what the proposal explicitly committed to?

### Track 2: Expert Multi-Lens Review

Add stable finding IDs for:

- `ARCH-*` Architecture
- `PROD-*` Product
- `UI-*` UI
- `UX-*` UX
- `READY-*` Delivery / Readiness

This track answers: even if the proposal is implemented, does the result still hold up as architecture, product, UI, UX, and ship/handoff quality?

### Separation Rule

- Do not downgrade a `REQ-*` status because of a generic preference or a subjective expert opinion.
- Change `REQ-*` status only when the proposal explicitly promised the behavior, constraint, decision, flow, UI, UX, or evidence.
- If something is risky, awkward, or non-native but not explicitly promised by the proposal, record it as an expert finding instead.

## Workflow

1. Resolve the proposal path and confirm it is an existing Markdown file.
2. Determine the repository root and gather reproducibility metadata:
   - repository root
   - git SHA
   - working tree status
   - full audit timestamp
3. Create the report path with the bundled helper, resolved relative to this skill bundle:
   ```bash
   python3 <skill-dir>/scripts/report_path.py /abs/path/to/proposal.md
   ```
   Preserve the existing naming convention unless there is a compelling reason to change it.
4. Determine proposal state before auditing code:
   - `Active`
   - `Superseded`
   - `Deprecated`
   - `Replaced`
   Search the proposal itself plus nearby proposals/reviews/reference docs for markers such as `superseded`, `deprecated`, `replaced by`, `obsolete`, or explicit replacement links.
5. Determine platform scope early:
   - `iOS`
   - `macOS`
   - `Universal`
   - `Ambiguous`
   Rules:
   - do not invent requirements for a platform the proposal does not target
   - when both iOS and macOS are in scope, review platform conventions separately
   - when scope is ambiguous, say so explicitly and lower confidence where appropriate
6. Extract the proposal contract before inspecting code. Record:
   - scope
   - locked decisions
   - acceptance criteria
   - test / evidence requirements
   - explicit exclusions / non-goals
   - user flows / jobs-to-be-done
   - UI commitments
   - UX commitments
   - platform-specific commitments
7. Derive the 1-5 Primary User Flows implied by the proposal.
   - These are the most important end-to-end tasks a user should be able to complete.
   - Use them to anchor Product, UI, UX, and Readiness review.
8. Convert explicit proposal commitments into atomic `REQ-*` items.
   - Assign stable IDs: `REQ-001`, `REQ-002`, ...
   - Record proposal source precisely: section heading plus closest line/anchor/reference.
   - Audit requirement-by-requirement, never paragraph-by-paragraph.
9. Start a Proposal Fidelity / Divergence inventory with three buckets:
   - Matches
   - Divergences
   - Ambiguities / Evidence Gaps
10. Inspect only implementation surfaces relevant to:
   - the proposal contract
   - the primary user flows
   - linked/nearby artifacts that clarify scope or proof
   - disputed or high-risk findings
11. Verify with the strongest practical evidence.
   - Prefer targeted `rg`, focused file reads, and narrow builds/tests over broad scans.
   - For critical UI/UX claims, prefer runtime checks, screenshots, or executed UI tests when practical.
   - Use previews, snapshot baselines, storyboards, or design assets as supporting evidence, not as a full substitute for runtime behavior when the claim is about end-to-end UX.
12. Audit every `REQ-*` item with the normalized status model.
13. Produce expert findings across the Architecture/Product/UI/UX/Readiness lenses.
14. Roll up:
   - `Overall Conformance`
   - `Overall Readiness`
   - `Audit Confidence`
15. Write the versioned report beside the proposal.
16. In chat, state the verdict directly and point to the generated report path.

## Contract Extraction Guidance

Use these meanings consistently:

- User flows / jobs-to-be-done: the primary tasks the user should be able to complete
- UI commitments: screens, layout expectations, navigation IA, controls, components, sidebars, toolbars, badges, labels, visual states
- UX commitments: discoverability, task clarity, onboarding, empty/loading/error states, recovery, confirmation flows, accessibility-related interaction behavior
- Platform-specific commitments: explicit or strongly implied iOS/macOS behaviors or conventions

If the proposal is vague:
- do not invent missing requirements
- place uncertainty in `Ambiguities / Evidence Gaps`
- or record a low/medium-confidence expert finding
- do not fabricate a failed `REQ-*` item from personal preference

## Requirement Model

For every `REQ-*` item include:

- short title
- proposal source
- status
- evidence type(s)
- evidence references
- gap / note

Status rules:

- `Implemented`: proven by direct evidence; never by `inference` alone
- `Partially Implemented`: some meaningful portion exists, but the committed behavior is incomplete
- `Missing`: the committed behavior, constraint, or evidence is absent
- `Not Verifiable`: the behavior may exist, but this audit could not prove it with available evidence

Roll-up rules for Track 1:

- `Overall Conformance = Implemented` if every in-scope requirement is `Implemented`
- `Overall Conformance = Not Implemented` if any in-scope requirement is `Missing`
- otherwise `Overall Conformance = Partial`

## Expert Findings Model

Every `ARCH-*`, `PROD-*`, `UI-*`, `UX-*`, and `READY-*` finding must include:

- Title
- Severity: `Critical`, `Major`, `Minor`, or `Note`
- Confidence: `High`, `Medium`, or `Low`
- Related proposal items and/or `REQ-*` IDs when applicable
- Evidence type(s)
- Evidence references
- Why It Matters
- Recommended Action

Do not emit findings that are pure taste. Ground them in proposal text, code, assets, tests, runtime evidence, platform conventions, or clearly stated ambiguity.

## Evidence Model

Allowed evidence types:

- `code`
- `tests-found`
- `tests-run`
- `runtime`
- `screenshot`
- `design-reference`
- `inference`

Evidence rules:

- Never mark `Implemented` from `inference` alone.
- Distinguish `tests-found` from `tests-run`.
- Use `runtime` only when you actually validated live behavior.
- For screen-level UI/UX claims, prefer `runtime`, `screenshot`, or executed UI tests when practical.
- If only code inspection was possible for a UI/UX claim, lower confidence or use `Not Verifiable` where appropriate.
- If the proposal includes mockups, screenshots, diagrams, or visual states, compare implementation against them explicitly.

## Apple-Platform Review Guidance

Stay grounded in the targeted platform(s).

### Architecture Lens

Assess when relevant:
- module / feature boundaries
- state ownership and data flow
- navigation structure / coordinators / routers
- dependency flow and side-effect isolation
- async / concurrency correctness and main-thread UI safety
- persistence / sync model
- testability
- separation between shared and platform-specific code
- alignment with locked architectural decisions

### Product Lens

Assess when relevant:
- whether the primary user job is actually achievable
- completeness of the happy path
- empty / loading / error / offline / permission-denied states
- business-rule correctness
- whether the implementation delivers intended user value, not just mechanics
- rollout / measurement readiness when the proposal implies analytics, flags, experiments, or observability

### UI Lens

Assess when relevant:
- visual hierarchy
- information density
- component consistency
- design-system reuse
- navigation chrome appropriateness for iOS vs macOS
- layout resilience under size changes
- truncation and overflow risk
- Dynamic Type / text scaling risk
- dark mode / appearance handling
- safe areas
- window resizing and split-view behavior
- accessibility-affecting visuals such as contrast, tap target size, and focus visibility

### UX Lens

Assess when relevant:
- task clarity
- discoverability
- navigation friction
- feedback for loading, success, error, and destructive actions
- keyboard, focus, pointer, VoiceOver, and platform-appropriate interaction support
- onboarding / first-run clarity
- empty states
- recovery / retry / undo / back-navigation
- continuity across lifecycle transitions
- whether macOS interactions behave like real desktop UX rather than stretched mobile UX

### Delivery / Readiness Lens

Assess when relevant:
- buildability on the targeted platform(s)
- key flows validated in this audit
- what was inferred vs executed
- test coverage quality for critical flows
- accessibility risk
- localization risk
- performance risk signals
- privacy / permissions / entitlements readiness
- known blockers
- unknowns
- overall confidence

## Platform-Conventions Rule

Review iOS and macOS conventions separately when both are in scope.

iOS examples:
- navigation stack / tab bar / sheet / full-screen patterns
- touch ergonomics
- Dynamic Type
- safe areas
- compact vs regular adaptation

macOS examples:
- sidebar / split-view / window behavior
- menu and toolbar conventions
- keyboard shortcuts
- pointer / hover / focus behavior
- multiwindow behavior when relevant
- selection models and desktop affordances

Do not assume an iOS pattern is correct for macOS or vice versa.

Use Apple-platform conventions as a heuristic baseline, not as a magical override:
- if the proposal explicitly chooses a nonstandard pattern, do not silently treat that as a failed `REQ-*`
- call out the tension between proposal fidelity and platform fit
- record it as a Product/UI/UX finding unless it clearly harms usability or violates an explicit requirement

## Verification Strategy

Prefer the narrowest proof that closes the claim:

- `rg` for identifiers, screens, settings, tags, feature entry points, and linked artifacts
- focused file reads for behavior, navigation, layout, and data contracts
- targeted unit/UI tests for proposal acceptance criteria and primary flows
- targeted `xcodebuild` build/test for the relevant Apple platform with external DerivedData
- runtime validation and screenshots for critical user-facing flows when practical

Example iOS build:

```bash
DERIVED_DATA="$(mktemp -d "${TMPDIR:-/tmp}/proposal-audit-derived-data.XXXXXX")"
xcodebuild \
  -scheme MyApp \
  -destination 'platform=iOS Simulator,name=iPhone 16' \
  -derivedDataPath "$DERIVED_DATA" \
  build
```

Example macOS test:

```bash
DERIVED_DATA="$(mktemp -d "${TMPDIR:-/tmp}/proposal-audit-derived-data.XXXXXX")"
xcodebuild \
  -scheme MyMacApp \
  -destination 'platform=macOS' \
  -derivedDataPath "$DERIVED_DATA" \
  test
```

If runtime validation is not practical:
- say so clearly
- lower confidence
- do not overclaim readiness or UX quality

Avoid broad full-suite runs unless the proposal explicitly requires them or a critical readiness claim depends on them.

## Roll-Up Model

Report all three:

- `Overall Conformance`: `Implemented`, `Partial`, or `Not Implemented`
- `Overall Readiness`: `Ready`, `Ready with Risks`, or `Not Ready`
- `Audit Confidence`: `High`, `Medium`, or `Low`

Guidance:

- `Overall Conformance` is driven only by `REQ-*` results.
- `Overall Readiness` is driven by critical/major findings, missing critical evidence, violated locked decisions, and unverified primary user flows.
- Missing runtime proof for critical UI flows should lower readiness or confidence.
- A repo can be `Implemented` but still only `Ready with Risks`.
- A repo can be `Partial` but still reasonably close to shippable.
- Use `Not Ready` when critical flows, critical evidence gaps, or critical findings make ship/handoff risky.

## Output Contract

Write a Markdown report beside the proposal using the generated versioned path. Keep it flat, reproducible, and evidence-first.

Use this shape:

```md
# <Proposal Title> Multi-Lens Audit R<n>

| Field | Value |
|---|---|
| Proposal | docs/proposals/example.md |
| Repository Root | . |
| Git SHA | abc1234 |
| Working Tree | clean |
| Audited At | 2026-03-21T10:15:42+02:00 |
| Platform Scope | iOS / macOS / Universal / Ambiguous |
| Proposal State | Active / Superseded / Deprecated / Replaced |
| Overall Conformance | Implemented / Partial / Not Implemented |
| Overall Readiness | Ready / Ready with Risks / Not Ready |
| Audit Confidence | High / Medium / Low |

## Executive Verdict

Direct answer in one short paragraph.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Missing acceptance flow | High |
| Architecture | Acceptable | State ownership leaks across feature boundary | Medium |
| Product | At Risk | Happy path exists but failure states are incomplete | Medium |
| UI | Acceptable | Layout truncation on smaller sizes | Low |
| UX | At Risk | Navigation and recovery are unclear | Medium |
| Readiness | Ready with Risks | Critical flow not runtime-validated on macOS | Medium |

## Proposal Contract

### Scope
...

### Locked Decisions
...

### Primary User Flows
...

### UI Commitments
...

### UX Commitments
...

### Acceptance Criteria
...

### Test / Evidence Requirements
...

### Explicit Exclusions
...

## Proposal Fidelity / Divergence

### Matches
- ...

### Divergences
- ...

### Ambiguities / Evidence Gaps
- ...

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 0 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 <short title>
- Proposal Source: ...
- Status: ...
- Evidence Type: ...
- Evidence:
  - path/to/file.swift:42
  - `xcodebuild ...`
- Gap / Note: ...

## Architecture Review

**Summary:** Strong / Acceptable / At Risk / Weak

### ARCH-001 <finding title>
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: ...
- Evidence Type: code, tests-found
- Evidence:
  - ...
- Why It Matters: ...
- Recommended Action: ...

## Product Review

**Summary:** Strong / Acceptable / At Risk / Weak

### PROD-001 <finding title>
- Severity: ...
- Confidence: ...
- Related Proposal Items / Requirements: ...
- Evidence Type: ...
- Evidence:
  - ...
- Why It Matters: ...
- Recommended Action: ...

## UI Review

**Summary:** Strong / Acceptable / At Risk / Weak

### UI-001 <finding title>
- Severity: ...
- Confidence: ...
- Related Proposal Items / Requirements: ...
- Evidence Type: ...
- Evidence:
  - ...
- Why It Matters: ...
- Recommended Action: ...

## UX Review

**Summary:** Strong / Acceptable / At Risk / Weak

### UX-001 <finding title>
- Severity: ...
- Confidence: ...
- Related Proposal Items / Requirements: ...
- Evidence Type: ...
- Evidence:
  - ...
- Why It Matters: ...
- Recommended Action: ...

## Delivery / Readiness Review

**Summary:** Ready / Ready with Risks / Not Ready

### READY-001 <finding title>
- Severity: ...
- Confidence: ...
- Related Proposal Items / Requirements: ...
- Evidence Type: ...
- Evidence:
  - ...
- Why It Matters: ...
- Recommended Action: ...

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass / Partial / Fail / Not Checked | ... |
| Core user flow runtime-validated | ... | ... |
| Empty/loading/error states covered | ... | ... |
| Accessibility risk acceptable | ... | ... |
| Localization risk acceptable | ... | ... |
| Critical tests executed | ... | ... |
| Privacy/permissions/entitlements reviewed | ... | ... |

## Verification Log

- `rg ...`
- `xcodebuild ...`
- runtime steps executed

## Recommended Next Actions

Ordered by severity and leverage. Keep concise and concrete.
```

Prefer repository-relative paths in the report body. Absolute paths are acceptable internally for shell commands but should not be the report default.

See `references/example-implementation-audit-report.md` for an example.

## Done Condition

The skill is done only when:
- exactly one new versioned report has been written beside the proposal
- the report clearly separates proposal conformance from expert multi-lens findings
- the report includes platform scope, primary user flows, fidelity/divergence, lens scorecard, readiness roll-up, and the `REQ-*` audit
- the chat reply states the verdict directly
- the chat reply points to the generated report path

## What Not To Do

- Do not turn the audit into a generic app code review.
- Do not rewrite or patch the proposal.
- Do not modify implementation files, tests, configs, or assets.
- Do not invent missing requirements from vague proposal language.
- Do not claim `Implemented` from inference alone.
- Do not hide `Not Verifiable`.
- Do not conflate UI/UX/Product concerns with `REQ-*` failures unless the proposal explicitly committed to them.
- Do not thrash across unrelated files, modules, or gates.
