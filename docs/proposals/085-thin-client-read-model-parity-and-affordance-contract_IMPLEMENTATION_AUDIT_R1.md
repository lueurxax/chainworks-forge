# Proposal 085 Implementation Audit R1

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/085-thin-client-read-model-parity-and-affordance-contract.md` |
| Audit timestamp | 2026-05-09 17:45:32 EEST |
| Audit mode | `implementation-audit` |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-085-thin-cl-daa93eeb` |
| Implementation branch | `cw/implement-proposal-085-thin-cl/daa93eeb` |
| Audited HEAD | `45708e72f8d073f935ab4185b892989a3d1f84ea` |
| Compare base | `main...HEAD`; merge base `83daf7e4a0c895e917f3efb4bc64e2b54ef840ab` |
| Working tree status at finalization | Dirty: five uncommitted source files plus this audit report |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Audit confidence | Medium-High |

## Implementation Target / Compare Base

The audited target is the existing run worktree for branch `cw/implement-proposal-085-thin-cl/daa93eeb`, not the caller's currently open `main` checkout. The committed implementation diff from `main...HEAD` contains 19 changed files:

- Swift implementation: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `Chainworks Forge/Support/P085AffordancePresenter.swift`
- Swift tests: `Chainworks ForgeTests/Proposal085Tests.swift`
- Gate and references: `scripts/test-gate.sh`, `docs/reference/test-gates.md`, `docs/reference/thin-client-read-model-affordance-contract.md`, `docs/reference/ui-action-boundary.md`, `README.md`, `docs/README.md`
- Negative fixtures: eight `docs/evidence/rollout-contract/negative/p085-*.json` files
- Run output marker: `CHAINWORKS_OUTPUT`

During the audit, additional uncommitted implementation edits appeared in the target worktree after the first validation pass. Final readiness is therefore based on the dirty final tree, not only on committed HEAD. The dirty source files are:

- `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- `Chainworks Forge/Support/P085AffordancePresenter.swift`
- `Chainworks ForgeTests/Proposal085Tests.swift`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/graphql-server/src/types/p031.rs`

## Prior Proposal-Review Reuse

| Item | Result |
|---|---|
| Prior artifacts discovered | None. `discover_prior_review.py` returned an empty artifact list. |
| Reviewer-selection reuse | Not reused |
| Reason | No P085 proposal-review directory, final review, reviewer-selection summary, evidence pack, or sibling review report was found. |

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `apple_arch_reviewer` | Swift presenter DTOs, P031 read-boundary model decoding, and production presenter wiring changed. |
| `api_contract_reviewer` | Proposal is centered on GraphQL read-model fields, mutation availability, schema/readback parity, and client/server contract drift. |
| `observability_rollout_reviewer` | The implementation adds a proposal gate, reference-gate documentation, and negative fixture evidence. |
| `chainworks_execution_truth_reviewer` | The touched contract governs approval truth, artifact truth, projection truth, and prevention of local workflow truth fallback. |

### Rejected Close Alternatives

| Reviewer | Reason rejected |
|---|---|
| `macos_ui_reviewer` | No SwiftUI view files changed in the implementation diff; UI-facing behavior is represented through existing views plus presenter/view-state evidence. |
| `apple_ux_reviewer` | User-facing labels are in scope, but the primary risks are contract/actionability/proof-gate risks already covered by the selected reviewers. |
| `rust_arch_reviewer` / `rust_reliability_reviewer` | No Rust files changed; existing GraphQL-server evidence was inspected as context only. |
| `rust_security_reviewer` | No new Rust auth/public-boundary implementation was added; auth proof gaps are handled as API/readiness findings. |
| `product_reviewer` | No metric, decision checkpoint, or product rollout experiment is central to P085. |

## Proposal State And Contract Summary

The audited proposal file declares status `Implemented` at line 6 and points to the new reference contract. It is not superseded or replaced, so the audit treats it as an active implementation-target proposal with implementation status asserted by the branch.

Proposal scope:

- Canonicalize the contract for every GraphQL field that drives a Swift affordance.
- Cover approval buttons, retry/cancel/start visibility, artifact preview state, report payload state, freshness badges, disabled/fallback copy, diagnostic copy actions, and future approval-only mutations.
- Prevent stale list/detail preview drift, freshness/actionability confusion, approval authorization drift, and schema/read-model/text review drift.

Platform/product scope:

- Apple: macOS thin SwiftUI client.
- Backend/service: GraphQL projection/readback and approval mutation contract, using existing Rust control-plane GraphQL surfaces as the server source.
- Cross-stack scope: Swift presenter + GraphQL read/mutation shape + proof gate + rollout/reference documentation.

Locked decisions and non-goals:

- Governed SwiftUI remains GraphQL read/subscription plus `approveApproval` and `rejectApproval` only.
- No broad UI writes.
- No Swift-local workflow truth.
- No forced payloads in bulk list queries.
- P085 does not replace P036 visual/navigation restoration.

## Primary Implementation Flows

1. Artifact list row renders payload states from server read-model fields and shows `Open to preview` for deferred payloads instead of a permanent unavailable state.
2. Artifact detail preview loads authorized payload text for a selected artifact, ignores stale async selection responses, and avoids local filesystem truth fallback.
3. Approval inbox row enables approve/reject only from durable approval state, `writePathState`, `availableActions`, and approved mutation vocabulary.
4. Freshness badges communicate projection recency only and do not drive payload availability or mutation actionability.
5. P085 gate checks the contract artifact, negative fixtures, Swift presenter symbols, P031 boundary symbols, and runs the P085 Swift test suite.

## Proposal Fidelity / Divergence Inventory

### Matches

- `docs/reference/thin-client-read-model-affordance-contract.md` exists and defines `thin_client_affordance_contract_v1` with rows for artifact preview, report payload metadata, freshness badges, approval approve/reject, diagnostics, and external command placeholders.
- `P085AffordancePresenter` centralizes immutable, `Equatable`, `Sendable` presenter DTOs for artifact, approval, freshness, and diagnostic affordances.
- `P031ArtifactPresenter` now delegates list payload labels through `P085AffordancePresenter.artifactListAffordance`.
- `P031ApprovalInboxPresenter` now derives `canApprove` and `canReject` from `P085AffordancePresenter.approvalAffordance`.
- `proposal-085|p085` is registered in `scripts/test-gate.sh` and documented in `docs/reference/test-gates.md`.
- The P085 gate is registered and did pass once on the clean committed HEAD before later dirty edits appeared.

### Divergences

- The current dirty tree fails `./scripts/test-gate.sh proposal-085` before Swift tests run because the gate still string-checks for the old literal `approval.decision != nil`.
- The P085 gate does not execute backend GraphQL projection tests even though proposal section 5 requires GraphQL projection fixture coverage for each affordance state.
- Dirty edits now start wiring `conflictResultCode` through Swift GraphQL documents and the Rust GraphQL schema, but that path is unverified on the final tree, covers only `already_resolved`, uses string-matched error classification, and returns a zero UUID journal id for conflict responses.
- Negative fixtures are checked for JSON validity and a `contract_violation` key only; the gate does not semantically prove that each fixture is rejected by a contract validator or backend projection test.

### Ambiguities / Evidence Gaps

- Existing P031/P043 GraphQL-server tests cover many underlying projection fields and states, but they are not newly tied to P085 or composed into the P085 proof gate.
- Existing UI code already guards stale artifact preview responses by selected artifact ID, but P085's own `mergedAffordance` helper is tested at presenter level rather than as a direct SwiftUI render test.
- No full regression or remote UI smoke was run. The first P085 gate passed on the pre-drift clean tree; the current dirty final tree has a failing P085 gate.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 7 |
| Missing | 0 |
| Not Verifiable | 1 |
| Out of Scope | 0 |

## Detailed Requirement Audit

### REQ-001: Add Canonical Affordance Contract Artifact

| Field | Value |
|---|---|
| Proposal source | Section 3, lines 41-60 |
| Status | Implemented |
| Evidence types | `proposal`, `code`, `diff` |
| Evidence references | `docs/reference/thin-client-read-model-affordance-contract.md:1-317`, `docs/README.md:75-76`, `README.md` reference update |
| Implementation mapping | New reference file and docs index/reference links. |
| Gap / note | None. |

### REQ-002: Define Explicit Rows For Artifact Preview, Approval Actionability, Freshness, And Report Payload States

| Field | Value |
|---|---|
| Proposal source | Section 4, lines 62-90; acceptance criteria lines 119-122 |
| Status | Implemented |
| Evidence types | `proposal`, `code` |
| Evidence references | `docs/reference/thin-client-read-model-affordance-contract.md:61-225` |
| Implementation mapping | Contract rows cover `artifact.preview.listLabel`, `artifact.preview.detail`, `report.payload.metadata`, four freshness badge rows, `approval.resolve.approve`, `approval.resolve.reject`, `diagnostic.copy`, and `external.command.placeholder`. |
| Gap / note | Rows exist for all acceptance-criteria state families. |

### REQ-003: Define Required Per-Affordance Dimensions In One Place

| Field | Value |
|---|---|
| Proposal source | Section 3, lines 49-60 |
| Status | Partially Implemented |
| Evidence types | `proposal`, `code` |
| Evidence references | `docs/reference/thin-client-read-model-affordance-contract.md:38-56`, `docs/reference/thin-client-read-model-affordance-contract.md:61-225` |
| Implementation mapping | The contract defines source fields, local presentation state, actionability, mutation availability, stale/detail behavior, unauthorized behavior, and proof tests across rows. |
| Gap / note | Some read-only rows omit several required dimensions rather than explicitly marking them `n/a`, and backend proof tests are not fully wired into the P085 gate. |

### REQ-004: List/Detail Preview Must Not Mislabel Deferred Payloads As Permanently Unavailable

| Field | Value |
|---|---|
| Proposal source | Section 4.1, lines 64-73; acceptance criterion line 121 |
| Status | Implemented |
| Evidence types | `code`, `tests-found`, `tests-run` |
| Evidence references | `Chainworks Forge/Support/P085AffordancePresenter.swift:126-174`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:5160-5192`, `Chainworks Forge/Views/RunsHomeView.swift:2161-2285`, `Chainworks ForgeTests/Proposal085Tests.swift:11-68`, `Chainworks ForgeTests/Proposal085Tests.swift:181-237`, first P085 gate run on pre-drift tree |
| Implementation mapping | Deferred list payloads map to `.deferred` and list labels contain `Open to preview`; production artifact rows use P085 list labels. Existing view code ignores stale async preview responses when selection changed. |
| Gap / note | Runtime UI screenshot evidence was not collected; proposal allowed view-state evidence and the gate covers presenter/view-state behavior. |

### REQ-005: Freshness Is Projection Recency Only

| Field | Value |
|---|---|
| Proposal source | Section 4.2, lines 75-78 |
| Status | Implemented |
| Evidence types | `code`, `tests-found`, `tests-run` |
| Evidence references | `Chainworks Forge/Support/P085AffordancePresenter.swift:114-120`, `Chainworks Forge/Support/P085AffordancePresenter.swift:218-229`, `Chainworks ForgeTests/Proposal085Tests.swift:155-177`, first P085 gate run on pre-drift tree |
| Implementation mapping | `P085FreshnessAffordanceState` hard-codes `canDrivePayloadAvailability = false` and `canDriveApprovalActionability = false`; tests exercise all P031 freshness states. |
| Gap / note | None for Swift presenter state. |

### REQ-006: Approval Buttons Only When Durable State, Caller Policy, Mutation Availability, And Disabled Copy Agree

| Field | Value |
|---|---|
| Proposal source | Section 4.3, lines 79-87 |
| Status | Partially Implemented |
| Evidence types | `code`, `tests-found`, `tests-run`, `inference` |
| Evidence references | `Chainworks Forge/Support/P085AffordancePresenter.swift:194-216`, `Chainworks Forge/Support/P085AffordancePresenter.swift:355-385`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:1220-1298`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:4538-4572`, `Chainworks ForgeTests/Proposal085Tests.swift:102-153`, `Chainworks ForgeTests/Proposal085Tests.swift:334-398`, first P085 gate run on pre-drift tree |
| Implementation mapping | Swift disables resolved approvals, checks `writePathState == .available`, and requires matching `availableActions`. Production approval rows consume this presenter. |
| Gap / note | Caller-policy/backend authorization parity is not executed by the P085 gate. Existing server tests cover approval mutation authorization separately, but that proof is not composed into P085. |

### REQ-007: No Mutation Availability Inference From Display Text, Status, Freshness, Or Local Selection Alone

| Field | Value |
|---|---|
| Proposal source | Section 4.4, lines 88-90 |
| Status | Implemented |
| Evidence types | `code`, `tests-found`, `tests-run` |
| Evidence references | `Chainworks Forge/Support/P085AffordancePresenter.swift:355-372`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:225-249`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:511-552`, `Chainworks ForgeTests/Proposal085Tests.swift:157-177`, first P085 gate run on pre-drift tree |
| Implementation mapping | Mutations are limited to `approveApproval` / `rejectApproval`; actionability comes from durable decision, write path, and available action vocabulary, not display labels or freshness. |
| Gap / note | None for Swift boundary behavior. |

### REQ-008: Add `proposal-085|p085` Proof Gate

| Field | Value |
|---|---|
| Proposal source | Section 5, lines 92-99 |
| Status | Partially Implemented |
| Evidence types | `config`, `tests-run` |
| Evidence references | `scripts/test-gate.sh:211-213`, `scripts/test-gate.sh:2225`, `scripts/test-gate.sh:6175-6326`, `docs/reference/test-gates.md:1853-1887`, first P085 gate run passed on pre-drift tree; current P085 gate rerun failed |
| Implementation mapping | Gate aliases exist and run static Python checks plus `Chainworks ForgeTests/Proposal085Tests`. |
| Gap / note | Gate exists, but the current dirty final tree fails before Swift tests run: `proposal-085: P085AffordancePresenter.swift missing required term: 'approval.decision != nil'`. Its coverage is also incomplete for backend projection proof. |

### REQ-009: GraphQL Projection Fixture For Each Affordance State

| Field | Value |
|---|---|
| Proposal source | Section 5, lines 100-102 |
| Status | Partially Implemented |
| Evidence types | `tests-found`, `tests-run`, `code` |
| Evidence references | Existing server evidence in `control-plane/crates/graphql-server/src/schema.rs:3920-3965`, `control-plane/crates/graphql-server/src/schema.rs:4248-4290`, `control-plane/crates/graphql-server/src/schema.rs:4430-4473`, `control-plane/crates/graphql-server/src/schema.rs:4528-4565`, `control-plane/crates/graphql-server/src/schema.rs:4818-4868`; P085 gate definition at `scripts/test-gate.sh:6175-6326` |
| Implementation mapping | Existing backend tests cover enum domains and some projection readback states such as `metadata_only`, `payload_deferred`, `available`, approval diagnostic fields, and approval mutation readback. |
| Gap / note | No new P085 backend projection fixture was added, and the P085 gate does not execute the existing GraphQL-server tests. It also does not prove each affordance state listed in the P085 contract rows. |

### REQ-010: Swift Presenter Test For Label/Fallback Mapping

| Field | Value |
|---|---|
| Proposal source | Section 5, lines 100-104 |
| Status | Implemented |
| Evidence types | `tests-found`, `tests-run` |
| Evidence references | `Chainworks ForgeTests/Proposal085Tests.swift:11-177`, `Chainworks ForgeTests/Proposal085Tests.swift:312-365`, first P085 gate run on pre-drift tree |
| Implementation mapping | Presenter tests cover deferred labels, metadata-only labels, available/unavailable payload mapping, disabled approval help text, projection-lag help text, and unknown-state fail-closed behavior. |
| Gap / note | None for Swift presenter mapping. |

### REQ-011: UI Render Snapshot Or View-State Test For List/Detail Payload Merge

| Field | Value |
|---|---|
| Proposal source | Section 5, lines 100-105 |
| Status | Implemented |
| Evidence types | `tests-found`, `tests-run`, `code` |
| Evidence references | `Chainworks ForgeTests/Proposal085Tests.swift:179-237`, `Chainworks Forge/Views/RunsHomeView.swift:2161-2285`, first P085 gate run on pre-drift tree |
| Implementation mapping | P085 tests cover view-state merge and stale detail rejection; existing `RunsHomeView` code also guards stale preview responses by selected artifact ID. |
| Gap / note | No screenshot/snapshot runtime proof was collected; the proposal allowed view-state testing. |

### REQ-012: Approval Actionability Test Tied To P081 Boundary Matrix

| Field | Value |
|---|---|
| Proposal source | Section 5, line 105 |
| Status | Partially Implemented |
| Evidence types | `tests-found`, `tests-run`, `code` |
| Evidence references | `docs/reference/ui-action-boundary.md:8-26`, `docs/reference/ui-action-boundary.md:73-77`, `Chainworks ForgeTests/Proposal085Tests.swift:102-153`, `Chainworks ForgeTests/Proposal085Tests.swift:367-398`, first P085 gate run on pre-drift tree |
| Implementation mapping | Tests cover approval actionability through write-path state, available actions, and durable decision state. `ui-action-boundary.md` cites the new affordance contract. |
| Gap / note | The tests do not directly cite or execute a P081 boundary-matrix artifact, and backend authorization matrix tests are not part of the P085 gate. |

### REQ-013: Unauthorized/Redacted Readback Test Where Applicable

| Field | Value |
|---|---|
| Proposal source | Section 5, line 106 |
| Status | Partially Implemented |
| Evidence types | `tests-found`, `tests-run`, `code` |
| Evidence references | `Chainworks ForgeTests/Proposal085Tests.swift:241-270`, `control-plane/crates/graphql-server/tests/proposal_031_authorization.rs:45-167`, `control-plane/crates/graphql-server/tests/proposal_031_authorization.rs:170-208`, first P085 gate run on pre-drift tree |
| Implementation mapping | Swift P085 tests invalidate diagnostic affordance on unauthorized freshness. Existing server tests verify query/subscription unauthorized/forbidden behavior. |
| Gap / note | P085 gate does not execute the server unauthorized/redaction tests, and no new P085-specific unauthorized/redacted projection fixture was added. |

### REQ-014: Preserve Non-Goals

| Field | Value |
|---|---|
| Proposal source | Section 6, lines 108-113 |
| Status | Implemented |
| Evidence types | `diff`, `code` |
| Evidence references | `git diff --name-status main...HEAD`, `docs/reference/ui-action-boundary.md:8-26`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:225-249`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:2195-2246` |
| Implementation mapping | No SwiftUI view write paths were added. Non-approval mutations remain forbidden. Bulk artifact list queries do not force `payloadText`; detail query owns payload readback. |
| Gap / note | None found. |

### REQ-015: Future Affordance Changes Cite Contract And Include Backend + Swift Presenter Tests

| Field | Value |
|---|---|
| Proposal source | Acceptance criterion line 122 |
| Status | Not Verifiable |
| Evidence types | `config`, `code`, `inference` |
| Evidence references | `docs/reference/test-gates.md:1864-1887`, `README.md`, `docs/README.md` |
| Implementation mapping | Documentation tells future work to use the contract and the gate fails if the contract artifact/symbols are missing. |
| Gap / note | Future behavior cannot be proven from current code alone. The current P085 gate does not enforce backend test inclusion, so the backend half of this future-change rule remains weak. |

## Reviewer / Lens Scorecard

| Lens | Conformance | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Backend projection proof and P081/unauthorized proof are not composed into the P085 gate. | Medium-High |
| Apple architecture | Mostly passes | Presenter DTOs and production wiring are coherent; some P085 helper behavior is presenter-level rather than direct UI runtime proof. | Medium |
| API contract | At risk | Typed conflict/idempotency contract is documented and decoded but not selected by production mutation documents or evidenced server-side. | High |
| Observability/readiness | At risk | Current gate fails on brittle static checks; when green on the earlier tree, it validated negative fixtures structurally, not semantically, and lacked backend projection execution. | High |
| Execution truth | Mostly passes | Swift avoids local workflow truth and broad UI writes; approval/actionability proof still needs backend matrix composition. | Medium-High |

## Routed Specialist Findings

### READY-001: P085 Gate Does Not Execute Required Backend Projection And Authorization Proof

| Field | Value |
|---|---|
| Reviewer | `observability_rollout_reviewer` |
| Severity | Major |
| Confidence | High |
| Related requirements | REQ-009, REQ-012, REQ-013, REQ-015 |
| Evidence types | `proposal`, `config`, `tests-run`, `tests-found` |
| Evidence references | Proposal lines 94-106; `scripts/test-gate.sh:6175-6326`; `docs/reference/test-gates.md:1853-1887`; first P085 gate run passed on pre-drift tree; current P085 gate rerun failed; existing GraphQL tests in `control-plane/crates/graphql-server/src/schema.rs` and `control-plane/crates/graphql-server/tests/proposal_031_authorization.rs` |
| Why it matters | On the current dirty final tree, `proposal-085` is red before Swift tests run. Even when it was green on the earlier clean tree, it proved the new contract document, JSON fixture presence, Swift symbols, and Swift presenter behavior, but not the proposal's backend projection fixture, P081 boundary-matrix, or unauthorized/redacted readback proof. This can let Swift affordance behavior drift from the actual GraphQL server while the proposal-specific gate remains green or fails for brittle string-check reasons. |
| Recommended action | Extend `proposal-085|p085` to run focused GraphQL-server projection/auth tests or add P085-specific backend tests that exercise each contract row/state. Also make the p085 negative fixtures semantic: they should be rejected by a validator or by targeted backend/contract tests, not just parsed as JSON with `contract_violation`. |
| Acceptance criteria | `./scripts/test-gate.sh proposal-085` runs and reports backend projection coverage for each P085 affordance state, P081 approval mutation availability/auth matrix coverage, and unauthorized/redacted readback coverage; deliberately broken negative fixtures fail the gate for semantic reasons. |

### API-001: Typed Mutation Conflict Contract Is Partially Wired But Red And Semantically Incomplete

| Field | Value |
|---|---|
| Reviewer | `api_contract_reviewer` |
| Severity | Major |
| Confidence | High |
| Related requirements | REQ-006, REQ-007, REQ-012 |
| Evidence types | `code`, `schema`, `tests-found`, `inference` |
| Evidence references | `docs/reference/thin-client-read-model-affordance-contract.md:167-196`, `docs/reference/thin-client-read-model-affordance-contract.md:290-293`; dirty diff in `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`; dirty diff in `control-plane/crates/graphql-server/src/schema.rs`; dirty diff in `control-plane/crates/graphql-server/src/types/p031.rs`; current P085 gate failure |
| Why it matters | The dirty final tree starts wiring `conflictResultCode` through Swift mutation documents and the Rust GraphQL payload type, which addresses part of the earlier API gap. It is still not ready: the canonical P085 gate fails before tests, the server maps conflicts by string-matching error text, only `already_resolved` is actually returned, `state_conflict` and `transient_error_retryable` are enum-only, and conflict responses use a fabricated zero UUID `journalId`. That creates a new contract ambiguity for consumers that expect journal IDs to identify real command journal entries. |
| Recommended action | Keep the server/client `conflictResultCode` path only if it is backed by typed command-handler errors and real readback semantics. Replace string matching and dummy journal IDs with an explicit result model, add tests for already-resolved, state-conflict, and transient retryable cases, and update the gate's brittle static check so pending/requested decisions can be represented without making `proposal-085` red. |
| Acceptance criteria | `./scripts/test-gate.sh proposal-085` passes on the dirty final tree; server-side tests prove `approveApproval`/`rejectApproval` return typed conflict codes for already-resolved/conflicting/transient cases; Swift production GraphQL documents request the field; response `journalId` is either a real journal id or the schema makes it nullable/absent for conflicts; the P085 gate runs the server proof. |

### OPS-001: Negative Fixtures Are Presence Checks, Not Contract Regression Tests

| Field | Value |
|---|---|
| Reviewer | `observability_rollout_reviewer` |
| Severity | Minor |
| Confidence | High |
| Related requirements | REQ-008, REQ-009, REQ-015 |
| Evidence types | `config`, `tests-run` |
| Evidence references | `scripts/test-gate.sh:6222-6244`; eight `docs/evidence/rollout-contract/negative/p085-*.json` files |
| Why it matters | The negative fixtures look useful as documentation, but the gate only requires valid JSON and a `contract_violation` field. That means the fixtures do not guard against future regressions in schema symbols, fallback mapping, or unsafe local-truth fallback. |
| Recommended action | Convert each P085 negative fixture into an input to a real contract checker or targeted test, and fail the gate when the negative condition is accepted. |
| Acceptance criteria | Removing a required affordance row, changing `payload_deferred` to unavailable, omitting an unauthorized readback behavior, or allowing unknown enum optimistic actionability causes `proposal-085` to fail through a semantic fixture assertion. |

## Readiness Checklist

| Item | Status | Evidence / note |
|---|---|---|
| Canonical proposal gate | Failed on final dirty tree | Current rerun failed with `proposal-085: P085AffordancePresenter.swift missing required term: 'approval.decision != nil'`. |
| Build integration | Stale pass | First gate run on the clean pre-drift tree built the app and embedded control-plane daemon successfully; dirty final source changes were not built because the gate failed in static checks. |
| Swift presenter tests | Stale pass | 38 P085 Swift tests passed on the clean pre-drift tree; they did not run on the current dirty final tree. |
| Backend projection tests | Partial / not run in P085 gate | Existing P031/P043 tests found, not composed into `proposal-085`. |
| UI runtime/screenshot | Not run | Not required for this local non-UI gate; proposal allowed view-state proof. |
| Empty/loading/error/offline/permission states | Partial | Permission/unauthorized presenter state covered; backend unauthorized/redacted readback not run in P085 gate. |
| Accessibility/localization/privacy/entitlements | Low direct risk | No SwiftUI view, string catalog, privacy, permission, or entitlement file changed. |
| Full regression or full sign-off | Not run | Not required for a successful verdict because this audit does not report `Implemented`/`Ready`; readiness remains Not Ready. |

## Verification Log

| Command / check | Result | Notes |
|---|---|---|
| `git worktree list --porcelain` | Passed | Located target branch in `.chainworks/worktrees/cw-implement-proposal-085-thin-cl-daa93eeb`. |
| `git branch --show-current && git rev-parse HEAD` | Passed | Branch `cw/implement-proposal-085-thin-cl/daa93eeb`, HEAD `45708e72f8d073f935ab4185b892989a3d1f84ea`. |
| `git diff --name-status main...HEAD` | Passed | 19 committed branch changes inspected. |
| `git status --short` after validation drift | Dirty | Five source files changed after the first gate run: P031 boundary, P085 presenter, Proposal085 tests, GraphQL schema, and P031 GraphQL types. |
| `discover_prior_review.py <proposal>` | Passed | Returned no prior review artifacts. |
| Focused source/reference inspection | Passed | Inspected proposal, P085 presenter, P031 boundary/presenters, RunsHome artifact preview state, test gate, reference docs, negative fixtures, and existing GraphQL-server tests. |
| `./scripts/test-gate.sh proposal-085` on clean pre-drift tree | Passed | Static contract checks passed; xcodebuild test succeeded; 38 P085 Swift tests passed; result bundle `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-085-20260509-174242.xcresult`. |
| `./scripts/test-gate.sh proposal-085` on final dirty tree | Failed | Static check failed before build/tests: `proposal-085: P085AffordancePresenter.swift missing required term: 'approval.decision != nil'`. |

## Final Verdict

Overall conformance is **Partial**. The Swift presenter, production Swift wiring, and reference artifact are real. However, the current dirty final tree does not pass the canonical P085 gate, and even the earlier green gate did not execute the backend projection, P081 approval actionability, or unauthorized/redacted readback evidence required by the proposal.

Overall implementation readiness is **Not Ready**. The branch should not be handed off as implemented until the current dirty tree has a passing `proposal-085` gate, backend projection/auth proof is composed into that gate, and the typed approval mutation conflict contract is backed by typed server semantics instead of brittle error-string matching and dummy journal IDs.

## Recommended Next Actions

1. Fix the current P085 gate failure caused by the stale literal check for `approval.decision != nil`, then rerun `./scripts/test-gate.sh proposal-085`.
2. Extend `proposal-085|p085` to run focused GraphQL-server tests for every P085 affordance state and the approval authorization/mutation matrix.
3. Turn the eight P085 negative fixtures into semantic regression tests instead of presence checks.
4. Finish or remove the `conflictResultCode` contract: typed server errors, real or nullable journal semantics, and tests for `already_resolved`, `state_conflict`, and `transient_error_retryable`.
5. After the proposal gate is green on the final tree, run the repo's broader canonical readiness gate before claiming full readiness.
