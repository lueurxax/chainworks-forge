# Proposal 085 Implementation Audit R2

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/085-thin-client-read-model-parity-and-affordance-contract.md` |
| Proposal title | Thin-Client Read-Model Parity and Affordance Contract |
| Audit timestamp | 2026-05-09 18:52:03 EEST |
| Audit mode | proposal-implementation-audit |
| Report path | `docs/proposals/085-thin-client-read-model-parity-and-affordance-contract_IMPLEMENTATION_AUDIT_R2.md` |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer selection reuse | Not reused |
| Audit confidence | High for audited code/gate evidence; Medium for UI runtime behavior |

## Implementation Target

| Field | Value |
|---|---|
| Worktree | `.chainworks/worktrees/cw-implement-proposal-085-thin-cl-daa93eeb` |
| Branch | `cw/implement-proposal-085-thin-cl/daa93eeb` |
| HEAD | `45708e72f8d073f935ab4185b892989a3d1f84ea` |
| Compare base | `main...HEAD`, plus dirty working-tree implementation changes |
| Dirty tree before this report | 9 modified files plus prior untracked R1 implementation audit |
| Current dirty implementation files | `P031ThinGraphQLReadBoundary.swift`, `P085AffordancePresenter.swift`, `Proposal085Tests.swift`, Rust `command_handler.rs`, GraphQL `schema.rs`, GraphQL `types/p031.rs`, `docs/reference/test-gates.md`, `docs/reference/thin-client-read-model-affordance-contract.md`, `scripts/test-gate.sh` |

This audit covers the current worktree, not only the committed branch tip.

## Prior Proposal-Review Reuse

| Field | Value |
|---|---|
| Discovery result | No prior proposal-review artifacts found by the skill helper |
| Reuse state | Not reused |
| Prior implementation audit | R1 exists beside the proposal, but implementation audit reports are not valid reviewer-selection inputs unless explicitly requested |
| Effect | Reviewers were routed from the proposal text and current implementation evidence |

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `api_contract_reviewer` | P085 defines GraphQL fields, enum values, mutation payload shape, Swift decoding, and client/server affordance parity. |
| `apple_arch_reviewer` | Implementation adds a Swift presenter and production Swift read-boundary wiring for affordance ownership. |
| `rust_arch_reviewer` | Dirty implementation changes Rust command-handler errors and GraphQL resolver behavior. |
| `observability_rollout_reviewer` | P085 adds a canonical proposal gate, negative fixtures, and readiness documentation. |
| `chainworks_execution_truth_reviewer` | Approval actionability and conflict readback depend on durable approval state and command-journal truth. |

Rejected close alternatives:

| Reviewer | Reason rejected |
|---|---|
| `macos_ui_reviewer` | No visual layout or navigation implementation changed; P085 UI evidence is presenter/view-state level. |
| `apple_ux_reviewer` | Copy and interaction semantics are covered through the contract and architecture review; no runtime UX surface was exercised. |
| `rust_reliability_reviewer` | Approval conflict/idempotency is narrow enough for Rust architecture plus execution-truth review; no queue, retry worker, shutdown, or backpressure changes were added. |
| `security_reviewer` | Authorization denial is in scope and tested, but no new public auth mechanism, secret handling, unsafe code, or broad security boundary was introduced. |

## Proposal State And Contract Summary

Proposal metadata says `Status: Implemented` and points to `docs/reference/thin-client-read-model-affordance-contract.md` (proposal lines 3-10). The audit treats that as the target state to verify, not as proof of completion.

The proposal requires a single contract for every GraphQL-driven Swift affordance, including source GraphQL fields, local presentation state, actionable state, disabled reason, mutation availability, stale/list/detail behavior, unauthorized behavior, and tests (proposal lines 41-60). It also requires:

- honest artifact list/detail preview labels (proposal lines 64-73);
- freshness as projection recency only (proposal lines 75-78);
- approval actionability tied to durable state, caller policy, allowed approval mutations, and backend authorization fallback text (proposal lines 79-90);
- a `proposal-085|p085` proof gate with backend projection, Swift presenter, list/detail view-state, P081 boundary, and unauthorized/redacted readback tests (proposal lines 92-106);
- non-goals preserving thin-client boundaries and avoiding broad UI writes/local workflow truth (proposal lines 108-113);
- acceptance criteria around the reference contract, explicit rows, honest list/detail merging, and future contract-cited tests (proposal lines 115-122).

## Platform And Product Scope

| Dimension | Scope |
|---|---|
| Apple platform | macOS SwiftUI operator app |
| Backend/service | Rust control-plane GraphQL API, command handler, projection tests |
| Product surface | Operator read-side affordances for artifacts, freshness, diagnostics, and approval decisions |
| API surface | GraphQL read models plus `approveApproval` / `rejectApproval` mutation payloads |
| Data/persistence | Existing projection tables and `command_journal`; no new persisted projection fields claimed |
| Explicitly out of scope | iOS, Go support, broad UI writes, local workflow truth, forced payloads in list queries, P036 replacement |

## Primary Implementation Flows

1. Artifact list rows and detail panes show payload state honestly, merging selected detail over list state without stale detail overwrites.
2. Freshness badges communicate recency only and do not drive payload availability or approval actionability.
3. Approval rows enable approve/reject only from durable pending/requested state, caller policy, `availableActions`, `writePathState`, and backend mutation authorization.
4. Unauthorized or redacted diagnostic readback does not leak diagnostic IDs or debug detail to Swift affordances.
5. The canonical `proposal-085|p085` gate proves the contract with backend and Swift slices on the audited tree.

## Fidelity And Divergence Inventory

### Matches

- The reference contract exists with the required schema/version marker and affordance rows (`docs/reference/thin-client-read-model-affordance-contract.md` lines 1-6, 38-56, 61-225).
- `P085AffordancePresenter` owns immutable Swift DTOs and maps payload, approval, freshness, diagnostic, and conflict states (`P085AffordancePresenter.swift` lines 3-124).
- Artifact `payload_deferred` maps to "Open to preview" rather than unavailable (`P085AffordancePresenter.swift` lines 312-323).
- List/detail merge is selection-guarded (`P085AffordancePresenter.swift` lines 176-192).
- Freshness flags are diagnostic-only (`P085AffordancePresenter.swift` lines 218-228).
- Approval actionability now fails closed for caller-policy denial codes and resolved decisions (`P085AffordancePresenter.swift` lines 355-378).
- Production Swift reads `conflictResultCode` on approval mutations and decodes it fail-closed (`P031ThinGraphQLReadBoundary.swift` matches at lines 488-503, 2174, 2196).
- Backend conflict readback now uses a typed command-handler error and real failed journal ID for already-resolved approvals (`command_handler.rs` lines 144-159, 3565-3590; `schema.rs` lines 1387-1396, 6424-6459).
- The same-tree `proposal-085` gate passed: backend `proposal_085_` slice passed 2 tests and Swift `Proposal085Tests` passed 44 tests.

### Divergences

- Several contract rows do not include every required per-affordance column. For example, `report.payload.metadata` omits explicit actionable state, disabled reason, fallback text, and stale/list/detail behavior even though the row schema says rows include those fields (`thin-client-read-model-affordance-contract.md` lines 99-109 versus 38-56).
- The backend projection proof is not a fixture for each affordance state. The P085 backend test covers one pending approval path and one report metadata artifact path (`schema.rs` lines 6463-6607), while the proposal requires a GraphQL projection fixture for each affordance state (proposal lines 100-106).
- The GraphQL enum and contract expose `state_conflict` and `transient_error_retryable`, but the Rust resolver currently maps only `ApprovalResolutionConflict::AlreadyResolved` to a conflict result (`types/p031.rs` lines 72-78; `schema.rs` lines 1387-1396).
- The P085 gate checks negative fixtures for existence, JSON validity, and `contract_violation`, but does not execute semantic negative cases (`scripts/test-gate.sh` lines 6222-6244).
- The P081 boundary-matrix tie remains indirect through `ui-action-boundary.md` and existing approval policy tests, not an explicit P081 matrix test/citation in the P085 gate.

### Ambiguities / Evidence Gaps

- No runtime macOS UI, screenshot, or UI smoke evidence was collected; UI behavior is proven only at presenter/view-state level.
- P081 itself appears proposal-level in this repo snapshot, so the "tied to P081 boundary matrix" requirement cannot be fully closed without either a checked-in executable P081 matrix or a P085 clarification.
- Future affordance-change compliance cannot be proven by the current implementation beyond gate and documentation hooks.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 1 |
| Out of Scope | 0 |

## Detailed REQ Audit

| ID | Requirement | Source | Status | Evidence | Gap / note |
|---|---|---|---|---|---|
| REQ-001 | Canonical affordance contract exists under `docs/reference/`. | Proposal lines 41-47, 117-120 | Implemented | Contract file lines 1-6; gate checks lines 6184-6220 | None. |
| REQ-002 | Each affordance row defines source fields, local state, actionable state, disabled reason, fallback text, mutation availability, stale/list/detail behavior, unauthorized behavior, and tests. | Proposal lines 49-60 | Partially Implemented | Row schema lines 38-56; rows lines 61-225 | Some rows omit required dimensions, especially read-only/report/freshness rows. |
| REQ-003 | List rows must not label intentionally omitted payload/detail fields permanently unavailable. | Proposal lines 64-73 | Implemented | Presenter labels lines 312-323; Swift tests passed | `payload_deferred` maps to "Open to preview"; unavailable remains only for unavailable state. |
| REQ-004 | List/detail payload state merges without stale detail overwrite. | Proposal lines 64-73, 103-104 | Implemented | Merge guard lines 176-192; Swift tests passed | View-state evidence only, which the proposal allows. |
| REQ-005 | Freshness communicates projection recency only, not payload/action/permission availability. | Proposal lines 75-78 | Implemented | Freshness DTO lines 218-228; contract rows lines 113-163; Swift tests passed | None. |
| REQ-006 | Approval buttons appear only when durable approval state is actionable, caller policy allows mutation, mutation availability is approve/reject only, and disabled/fallback text matches backend authorization. | Proposal lines 79-87 | Implemented | Swift actionability lines 355-378; boundary doc lines 8-18; backend projection/auth test lines 6531-6607; gate passed | The core behavior is implemented; P081-specific matrix proof is tracked separately in REQ-011. |
| REQ-007 | Swift affordances do not infer mutation availability from display text, status, freshness, or local selection alone. | Proposal lines 88-90 | Implemented | Swift actionability lines 355-378; freshness diagnostic-only lines 218-228; production P085 wiring at read boundary matches | None. |
| REQ-008 | `proposal-085|p085` proof gate exists. | Proposal lines 92-98 | Implemented | Gate branch lines 6175-6356; docs lines 1853-1888; same-tree gate passed | None. |
| REQ-009 | GraphQL projection fixture exists for each affordance state. | Proposal lines 100-102 | Partially Implemented | Backend tests lines 6375-6607; gate runs backend slice lines 6351-6354 | Backend proof covers approval/report/authorization/conflict examples, not every payload/freshness/disabled state. |
| REQ-010 | Swift presenter tests cover label/fallback mapping. | Proposal lines 102-103 | Implemented | `Proposal085Tests` passed 44 tests; rg evidence shows label/fallback/conflict tests across lines 11-649 | None. |
| REQ-011 | Approval actionability test is tied to the P081 boundary matrix. | Proposal lines 104-105 | Partially Implemented | Backend policy tests and `ui-action-boundary.md` lines 8-18; P085 backend auth proof lines 6531-6607 | No explicit P081 matrix row IDs or P081 executable matrix are part of the P085 gate. |
| REQ-012 | Unauthorized/redacted readback test exists where applicable. | Proposal lines 105-106 | Implemented | Backend observer denial lines 6598-6607; Swift unauthorized diagnostic tests passed | None for the audited scope. |
| REQ-013 | Non-goals preserved: no broad UI writes, no Swift local workflow truth, no forced list payloads, no P036 replacement. | Proposal lines 108-113 | Implemented | Changed-file scope; `ui-action-boundary.md` lines 8-26; contract lines 271-274 | None found. |
| REQ-014 | Future UI affordance changes cite the contract and include backend + Swift presenter tests. | Proposal lines 121-122 | Not Verifiable | Gate docs lines 1865-1888; gate script lines 6175-6356 | Current gate encourages this, but future changes cannot be proven from the current implementation. |

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Backend projection and P081 matrix proof do not fully satisfy explicit test requirements. | High |
| API contract | Major risk | Conflict enum/contract exposes states the backend does not currently emit. | High |
| Apple architecture | Pass with risks | Presenter ownership and production wiring are clean; no runtime UI proof. | High |
| Rust architecture | Pass with risks | Typed already-resolved conflict is sound, but conflict taxonomy is incomplete. | High |
| Execution truth | Pass with risks | Real failed `command_journal` ID is now proved; reject/conflict variants remain thin. | High |
| Observability/rollout | Not ready | Gate passes but negative fixtures are structural and backend state coverage is incomplete. | High |
| Overall readiness | Not Ready | Major requirement/evidence gaps remain despite green proposal gate. | High |

## Routed Specialist Findings

### READY-001: Backend projection proof is not per affordance state

| Field | Value |
|---|---|
| Reviewer | `observability_rollout_reviewer` |
| Severity | Major |
| Confidence | High |
| Related requirements | REQ-009, REQ-008 |
| Evidence types | proposal, code, tests-run |
| Evidence references | Proposal lines 100-106; `schema.rs` lines 6463-6607; `scripts/test-gate.sh` lines 6351-6354 |

Why it matters: P085 explicitly requires a GraphQL projection fixture for each affordance state. The current backend proof is valuable but narrow: one pending approval, one metadata report artifact, observer denial, and already-resolved conflict readback. Server projection drift for payload `available`, `payload_deferred`, `generating`, `unavailable`, freshness variants, disabled reasons, and reject-specific actionability can still escape the P085 backend slice.

Recommended action: Add a data-driven backend projection fixture table for every state enumerated by the P085 contract rows, including payload availability states, freshness states, approval disabled reasons, approve/reject action availability, diagnostic authorized/redacted behavior, and report metadata states.

Acceptance criteria: `./scripts/test-gate.sh proposal-085` fails if any contract state lacks a backend projection fixture, and passes with backend assertions for every current P085 affordance state.

### API-001: Conflict result enum and contract overpromise backend behavior

| Field | Value |
|---|---|
| Reviewer | `api_contract_reviewer` |
| Severity | Major |
| Confidence | High |
| Related requirements | REQ-006, REQ-007 |
| Evidence types | schema, code, tests-run |
| Evidence references | Contract lines 244-246 and 299-302; `types/p031.rs` lines 72-78; `command_handler.rs` lines 144-159; `schema.rs` lines 1387-1396 |

Why it matters: The schema and Swift client recognize `already_resolved`, `state_conflict`, and `transient_error_retryable`, and the contract says stale/conflicting/transient approval outcomes should surface through typed conflict codes. The backend currently maps only the `AlreadyResolved` typed error. That leaves two advertised contract values as documentation/schema surface without resolver evidence.

Recommended action: Either implement and test `state_conflict` and `transient_error_retryable` for both approval mutations, or narrow the current schema/contract to only `already_resolved` and defer the remaining codes to a later proposal revision.

Acceptance criteria: P085 backend tests prove every exposed `MutationConflictResultCode` value can be emitted under a concrete resolver path, or the exposed enum/contract is reduced to values that are actually implemented.

### READY-002: P081 boundary-matrix linkage is indirect

| Field | Value |
|---|---|
| Reviewer | `chainworks_execution_truth_reviewer` |
| Severity | Major |
| Confidence | Medium |
| Related requirements | REQ-011 |
| Evidence types | proposal, code, docs, tests-run |
| Evidence references | Proposal lines 104-105; `ui-action-boundary.md` lines 8-18; P085 backend auth proof `schema.rs` lines 6531-6607 |

Why it matters: Approval actionability is now tied to durable state, `availableActions`, `writePathState`, and GraphQL authorization, which is the important behavior. However, the explicit P085 test requirement names the P081 boundary matrix. The current P085 gate does not cite P081 matrix row IDs or execute a P081 matrix artifact, so the requirement remains only indirectly satisfied through `ui-action-boundary.md` and policy tests.

Recommended action: Add a checked-in P081 row citation or executable matrix fixture to the P085 approval actionability tests, or clarify P085 to reference the current implemented `ui-action-boundary.md` until P081 becomes implemented truth.

Acceptance criteria: P085 approval actionability tests name the exact boundary matrix rows they prove, including allowed UI approval mutations and denied non-approval or observer paths.

### OPS-001: Negative fixtures are structural, not semantic

| Field | Value |
|---|---|
| Reviewer | `observability_rollout_reviewer` |
| Severity | Minor |
| Confidence | High |
| Related requirements | REQ-008, REQ-014 |
| Evidence types | code, config |
| Evidence references | `scripts/test-gate.sh` lines 6222-6244; `docs/reference/test-gates.md` lines 1860 and 1886 |

Why it matters: The gate proves the eight negative fixture files exist and contain `contract_violation`, but it does not use them to validate that the contract checker catches those failures. This weakens future-change guardrails, especially for the proposal's "future affordance changes cite the contract and include tests" acceptance criterion.

Recommended action: Add a semantic contract validator or fixture runner that expects each negative fixture to fail for a specific reason.

Acceptance criteria: Removing or mutating the checked behavior represented by a negative fixture makes `proposal-085` fail for the matching fixture-specific violation, not only for missing JSON structure.

## Readiness Checklist

| Check | Status | Evidence / note |
|---|---|---|
| Canonical proposal gate on audited tree | Passed | `./scripts/test-gate.sh proposal-085` on 2026-05-09 passed. |
| Backend contract tests | Passed, partial coverage | 2 Rust `proposal_085_` tests passed. Coverage gaps captured in READY-001. |
| Swift presenter/view-state tests | Passed | 44 `Proposal085Tests` passed. |
| Full regression suite | Not run | Canonical proposal gate was run; full repo gate was not required for this audit verdict because readiness remains Not Ready. |
| Core service flow validation | Partial | GraphQL unit/integration tests validate selected projection/auth/conflict paths; no daemon runtime validation. |
| UI runtime/screenshot validation | Not run | UI evidence is presenter/view-state only; no visual layout changes were audited. |
| Empty/loading/error/offline/permission states | Partial | Unauthorized diagnostic/readback behavior tested; no runtime UI error-state sweep. |
| Accessibility/localization | Not run | Help text is tested at presenter level; no accessibility or localization pass. |
| Privacy/permissions | Passed for audited path | Observer diagnostic query denial and Swift unauthorized diagnostic clearing are tested. |
| Entitlements | Not applicable | No entitlement changes. |
| Negative fixture enforcement | Partial | Fixture presence checked; semantic negative execution missing. |

## Verification Log

| Command / check | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py <proposal>` | Returned R2 report path. |
| `git status --short` | Dirty tree with 9 implementation files modified and prior R1 audit untracked before writing R2. |
| `git diff --stat` | Dirty implementation adds 533 insertions and 60 deletions across 9 files. |
| `./scripts/test-gate.sh proposal-085` | Passed. Static checks passed; Rust backend slice passed 2 tests; Swift `Proposal085Tests` passed 44 tests; result bundle `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-085-20260509-184715.xcresult`. |

## Final Verdict

Overall conformance is Partial. The implementation has materially improved since R1: the same-tree P085 gate now passes, the backend proof slice exists and runs, already-resolved approval conflicts return a typed `conflictResultCode` with a real failed `command_journal` ID, Swift production documents request the field, and unauthorized diagnostic state is fail-closed in Swift and backend tests.

Overall implementation readiness is Not Ready. The remaining blockers are not generic preferences: they are tied to explicit P085 requirements and the implemented contract surface. The backend projection proof is still not per affordance state, P081 matrix linkage remains indirect, and the conflict-code schema/contract advertises states the backend does not currently emit.

Recommended next actions:

1. Expand the P085 backend projection proof to cover every contract state and make the gate fail on missing state coverage.
2. Either implement/test `state_conflict` and `transient_error_retryable` resolver paths or narrow the current conflict enum/contract to `already_resolved`.
3. Add explicit P081 boundary matrix row linkage to approval actionability tests, or revise P085/reference wording to use the implemented `ui-action-boundary.md` as the current truth.
4. Turn P085 negative fixtures from structural JSON checks into semantic failure checks.
