# Proposal 085 Implementation Audit R3

## Metadata

- Proposal: `docs/proposals/085-thin-client-read-model-parity-and-affordance-contract.md`
- Proposal title: Thin client read-model parity and affordance contract
- Proposal format: `proposal_json_v1` stored in a `.md` file
- Proposal claimed status: `implemented`
- Audit report: `docs/proposals/085-thin-client-read-model-parity-and-affordance-contract_IMPLEMENTATION_AUDIT_R3.md`
- Audit date: 2026-05-09
- Target branch requested: `main`
- Audited branch: `main`
- Audited HEAD: `924669e87694d3f0172bf6a2df7b028a47f77f7c`
- Working tree state at audit start: dirty, with 22 modified tracked files and untracked P086 evidence directories
- Canonical gate run: `./scripts/test-gate.sh proposal-085`
- Gate result: failed with exit code 65; Swift build failed before `Proposal085Tests` ran

## Target And Base

The audit target is the current `main` working tree at HEAD `924669e87694d3f0172bf6a2df7b028a47f77f7c`, including uncommitted implementation changes present in the tree. No separate merge base, PR base, or implementation branch was supplied for comparison.

Important dirty-tree context: the working tree contains Proposal 085 changes, but it also contains unrelated-looking changes in P015, P086 evidence, workflow compiler, executor, and `RunsHomeView.swift`. The canonical Proposal 085 gate failure is caused by Swift compile errors in this same tree, including errors outside the narrow P085 presenter surface.

## Prior Review Reuse

- Prior proposal review artifacts discovered by the helper: none.
- Prior implementation audit reports present: R1 and R2.
- Prior implementation audits were not used for reviewer selection, per audit workflow rules. They were only noted as historical artifacts.

## Reviewers

Selected reviewers:

- `api_contract_reviewer`: GraphQL schema, DTO, enum, and mutation/read-model contract parity.
- `apple_arch_reviewer`: Swift read boundary, presenter mapping, fail-closed behavior, and build viability.
- `rust_reliability_reviewer`: stale, duplicate, conflict, retry, and idempotency behavior for durable approval/write paths.
- `observability_rollout_reviewer`: gate registration, negative fixtures, rollout hold conditions, and evidence quality.
- `chainworks_execution_truth_reviewer`: durable approval/artifact/report truth and UI action boundary consistency.

Rejected reviewers:

- `macos_ui_reviewer`: UI runtime review is blocked by the Swift build failure; static Swift architecture review covers the current issue.
- `rust_arch_reviewer`: Rust architectural scope is narrow enough to be covered by API contract and reliability review in this pass.
- Product review: not selected; the proposal is an implementation parity contract and does not center new product decision checkpoints.

## Proposal Contract Summary

Proposal 085 requires a canonical affordance contract so the Swift thin client does not infer actionability or payload availability from partial local state. Required surfaces include:

- Artifact preview list and detail rows.
- Report payload metadata rows.
- Run, stage, agent, and approval freshness rows.
- Approval approve and reject rows.
- Diagnostic copy rows.
- External command placeholder rows.

The proposal also requires each row to carry a full schema of contract columns: surface, GraphQL entrypoint, source fields, nullability and enum domains, local state, actionable state, disabled reason, fallback text, mutation availability and idempotency, staleness deadline, cancellation behavior, stale-list/detail behavior, unauthorized behavior, supported interactions, and proof tests.

The implementation must prove GraphQL/schema parity, typed approval actionability, stale/duplicate/idempotent behavior, fail-closed enum handling, redacted unauthorized readback, server-owned generating/deferred deadlines or stalled states, P081 UI action boundary linkage, and a Proposal 085 gate that catches known-bad fixtures.

## Platform And Product Scope

In scope:

- macOS SwiftUI operator shell thin-client behavior.
- Swift GraphQL read boundary and P085 presenter mapping.
- Rust GraphQL schema and approval mutation behavior.
- Reference docs and rollout/test-gate evidence.
- P081 UI action boundary linkage.

Out of scope:

- New external command execution from SwiftUI.
- Raw payload fetching without server authorization.
- Local UI inference of backend truth.
- iOS behavior.
- Manual lifecycle cleanup or orchestration state mutation.

## Flow Audit

### Artifact Preview Flow

Evidence found:

- `docs/reference/thin-client-read-model-affordance-contract.md` defines artifact list/detail rows.
- `Chainworks Forge/Support/P085AffordancePresenter.swift` maps artifact preview states into list labels, detail payload authorization, cancellation, and merge behavior.
- `control-plane/crates/graphql-server/src/schema.rs` includes a Proposal 085 artifact projection matrix test covering available, metadata-only, payload-deferred, unavailable, and live freshness cases.

Assessment:

The artifact preview flow is mostly present for static availability states, but the server-owned generating/deferred deadline or stalled transition requirement is not proven by backend behavior. The current gate passes static checks and Rust tests for the implemented matrix, but it does not prove the explicit Proposal 085 stuck generating/deferred contract.

### Report Payload Metadata Flow

Evidence found:

- Reference docs include `report.payload.metadata`.
- Backend tests include report payload metadata projection coverage.
- Swift presenter separates metadata-only, deferred, generating, unavailable, and unknown payload states.

Assessment:

The basic report metadata affordance exists, but the reference row does not contain the full required row schema columns and the stuck generating/deferred deadline proof is absent.

### Freshness Flow

Evidence found:

- Reference docs include run, stage, agent, and approval freshness rows.
- Swift presenter maps freshness to recency copy and explicitly disables actionability.
- Proposal text requires distinct freshness states including disconnected/refreshing-style states and recency-only behavior.

Assessment:

The thin-client recency-only behavior is present in Swift code, but the current Swift target does not compile, so this behavior is not verified in the same-tree gate. The reference rows also omit several required row schema columns.

### Approval Approve/Reject Flow

Evidence found:

- GraphQL exposes `conflictResultCode` on approval mutation payloads.
- Rust enum `GqlMutationConflictResultCode` currently emits `AlreadyResolved`.
- Backend tests prove approve and reject already-resolved conflicts use a real failed journal ID.
- Swift decodes `already_resolved` and fail-closes unknown conflict result codes.
- P031 approval row presentation delegates actionability to `P085AffordancePresenter`.
- `docs/reference/ui-action-boundary.md` now contains explicit P081 approval approve/reject boundary rows.

Assessment:

The narrowed already-resolved conflict contract is substantially implemented in backend code and static Swift mapping. However, the same-tree Swift build failure blocks verification of the Swift presenter tests and UI read boundary tests.

### Diagnostic Copy Flow

Evidence found:

- Reference docs include diagnostic copy availability, redaction, and invalidation behavior.
- Swift presenter clears run IDs and debug payloads when diagnostics are unavailable or unauthorized.

Assessment:

The mapping exists, but it is not same-tree verified because the Swift test target fails to build.

### External Command Placeholder Flow

Evidence found:

- Reference docs include an `external.command.placeholder` row.
- `docs/reference/ui-action-boundary.md` preserves the GraphQL-only observer and external command placeholder boundary.
- P085 gate static checks require external command placeholder terms.

Assessment:

The placeholder contract is present and does not appear to introduce SwiftUI mutation capability.

## Fidelity And Divergence

High-fidelity areas:

- Required top-level affordance rows are present in the canonical reference contract.
- Approval conflict result naming is aligned across GraphQL, Rust tests, Swift decoding, and docs for `already_resolved`.
- P081 boundary rows now directly name approval approve/reject, read-only, and external command surfaces.
- The Proposal 085 gate is registered and runs static contract checks, Rust backend proof tests, and Swift tests.

Divergences:

- The canonical Proposal 085 gate fails in the current tree because Swift does not build.
- Server-owned generating/deferred deadlines or typed stalled/timed-out transitions are documented as required but not proven by backend implementation or tests.
- Several reference contract rows do not include all columns required by the proposal's row schema contract.
- One negative fixture still mentions `state_conflict` in a hold condition after the current schema/contract narrowed conflict result codes to `already_resolved`.
- The audit target is a dirty `main` tree with unrelated changes that affect gate readiness.

## Requirement Audit

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Canonical Proposal 085 affordance contract exists and is marked implemented. | Implemented | `docs/reference/thin-client-read-model-affordance-contract.md` status section, Proposal 085 JSON status. |
| REQ-002 | Required affordance surfaces are present: artifact list/detail, report metadata, freshness, approve/reject, diagnostic copy, external placeholder. | Implemented | Reference contract rows cover all required surfaces. |
| REQ-003 | Every required row carries the full row schema columns required by the proposal. | Partially Implemented | Reference rows exist, but report/freshness/diagnostic/external rows omit several required columns such as actionable state, disabled reason, fallback text, stale behavior, supported interactions, or proof tests. |
| REQ-004 | GraphQL schema/read-model proof covers required fields, enum domains, and approval mutations. | Partially Implemented | Backend tests and static gate checks pass for current fields/enums, but the overall gate fails and server-owned stalled/deadline states are not proven. |
| REQ-005 | Artifact list/detail affordances distinguish available, metadata-only, payload-deferred, generating, unavailable, and unknown without treating deferred/generating as unavailable. | Partially Implemented | Swift mapping exists and backend matrix covers several states, but Swift tests did not run and backend proof omits generating/stalled deadline behavior. |
| REQ-006 | Report metadata affordance stays metadata-only unless server-owned payload authorization is present. | Partially Implemented | Swift and backend evidence exist, but same-tree Swift verification is blocked and the reference row is incomplete. |
| REQ-007 | Freshness is recency-only and cannot create local actionability. | Partially Implemented | Swift mapping sets freshness actionability to false; same-tree Swift verification is blocked by build failure. |
| REQ-008 | Approve/reject actionability comes from durable backend state, write-path availability, and authorized actions. | Partially Implemented | Backend and Swift mapping evidence exist; Swift verification is blocked by compile failure. |
| REQ-009 | Stale/duplicate approval submissions return typed idempotent/already-resolved behavior rather than silent retry or ambiguous success. | Implemented | Rust approve and reject tests pass for real failed journal ID plus `already_resolved`; Swift decodes `already_resolved` and fail-closes unknown codes. This is implemented for the current narrowed conflict-code contract. |
| REQ-010 | Transient transport/server failures remain GraphQL errors and are not silently retried as success. | Missing | No same-tree gate evidence was produced because Swift tests did not run; static review did not find a dedicated Proposal 085 transient failure proof. |
| REQ-011 | Generating/deferred previews have server-owned deadline proof, explicit no-deadline justification, or typed stalled/timed-out state. | Missing | The proposal requires this; current evidence is limited to docs/negative fixture checks. Backend projection tests do not prove generating/deferred deadline or stalled transitions. |
| REQ-012 | Unknown enum values fail closed in Swift. | Partially Implemented | Swift mapping uses unknown cases for payload/write/conflict states, but Swift target build failure blocks current verification. |
| REQ-013 | Unauthorized/redacted readback prevents raw payload or diagnostic leakage. | Partially Implemented | Backend auth denial and Swift diagnostic redaction evidence exist; Swift verification is blocked by compile failure. |
| REQ-014 | P081 UI action boundary linkage is explicit and enforced by gate/reference docs. | Implemented | `docs/reference/ui-action-boundary.md` includes P081 approval/read-only/external rows and the P085 gate requires those terms. |
| REQ-015 | Proposal 085 canonical gate passes in the implementation tree. | Missing | `./scripts/test-gate.sh proposal-085` failed with exit code 65 due Swift compile errors. |

## Scorecard

- Contract coverage: Partial
- GraphQL/backend parity: Partial
- Swift thin-client parity: Partial, currently not buildable
- Approval conflict/idempotency parity: Mostly implemented for `already_resolved`
- Server-owned deadline/stalled semantics: Missing
- Negative fixture/gate quality: Partial
- Same-tree verification: Failed
- Rollout readiness: Not ready

Overall conformance per audit rules: Not Implemented, because at least one in-scope required behavior is missing and the canonical same-tree Proposal 085 gate does not pass.

## Findings

### FINDING-001: Canonical Proposal 085 Gate Fails Because The Swift Target Does Not Build

- Severity: Critical
- Reviewers: `apple_arch_reviewer`, `observability_rollout_reviewer`
- Requirement IDs: REQ-005, REQ-006, REQ-007, REQ-008, REQ-012, REQ-013, REQ-015
- Evidence:
  - Command: `./scripts/test-gate.sh proposal-085`
  - Result: exit code 65
  - Result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-085-20260509-200830.xcresult`
  - Failure messages include:
    - `Cannot convert value of type '@Sendable (P031StageReadModel, Date) -> P031StageTransitionPresentation' to expected argument type '(P031StageReadModel) -> P031StageTransitionPresentation'`
    - `Missing arguments for parameters 'startedLabel', 'completedLabel', 'durationLabel' in call`
    - `Testing cancelled because the build failed.`
  - Files implicated by the build output include `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift` and `Chainworks Forge/Views/RunsHomeView.swift`.

Impact:

The implementation cannot be considered ready because the canonical Proposal 085 gate fails before Swift tests run. Static code evidence for P085 Swift mapping is useful, but it is not deployable or verifiable in the current tree.

Recommended action:

Fix the Swift compile errors in the audited tree, especially the stage transition presenter map call and all `P031StageTransitionPresentation` initializers that lack `startedLabel`, `completedLabel`, and `durationLabel`. Re-run `./scripts/test-gate.sh proposal-085` in the same tree and attach passing evidence.

### FINDING-002: Server-Owned Generating/Deferred Deadline Or Stalled-State Semantics Are Not Implemented

- Severity: Major
- Reviewers: `api_contract_reviewer`, `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`
- Requirement IDs: REQ-005, REQ-006, REQ-011
- Evidence:
  - Proposal 085 requires payload-deferred/generating states to carry a server-owned deadline, an explicit no-deadline justification, or a typed stalled/timed-out state after the deadline.
  - `docs/reference/thin-client-read-model-affordance-contract.md` describes the expectation, but the implementation notes also say there are no new persisted fields.
  - The backend Proposal 085 artifact projection matrix covers available, metadata-only, payload-deferred, unavailable, and live freshness cases, but not a generating/deferred deadline expiry or stalled/timed-out transition.
  - The P085 gate checks a negative fixture for missing deadline evidence, but static fixture validation is not behavioral implementation proof.

Impact:

The client can present deferred/generating states, but the durable server-owned truth needed to decide whether a stuck preview is still pending, explicitly indefinite, stalled, or timed out is not proven. This is one of the proposal's explicit hold conditions.

Recommended action:

Either implement server-owned deadline/no-deadline/stalled semantics in the backend read model and add Proposal 085 backend tests for the transition, or revise the proposal/reference contract to remove or explicitly defer that requirement. The gate should fail if the behavioral proof is absent, not only if a fixture lacks a static field.

### FINDING-003: Several Contract Rows Do Not Carry The Full Required Row Schema

- Severity: Major
- Reviewers: `api_contract_reviewer`, `observability_rollout_reviewer`
- Requirement IDs: REQ-003
- Evidence:
  - Proposal 085 requires every row to name columns including actionable state, disabled reason, fallback text, mutation availability/idempotency, staleness deadline, cancellation behavior, stale behavior, unauthorized behavior, supported interactions, and proof tests.
  - `artifact.preview.listLabel`, `artifact.preview.detail`, and approval rows are comparatively complete.
  - `report.payload.metadata`, freshness rows, diagnostic copy, and external placeholder rows omit multiple required columns or express them only implicitly.

Impact:

The reference contract is useful but does not yet satisfy the proposal's own "every row must name" schema discipline. Future client changes could still reintroduce local inference because some rows do not make disabled, stale, interaction, and proof expectations explicit.

Recommended action:

Normalize each required reference row to the full row schema. Where a column does not apply, state `not_applicable` with a reason rather than omitting it.

### FINDING-004: Negative Fixture Text Still References A Removed Conflict Code

- Severity: Minor
- Reviewers: `api_contract_reviewer`, `observability_rollout_reviewer`
- Requirement IDs: REQ-009
- Evidence:
  - `docs/evidence/rollout-contract/negative/p085-approval-stale-double-submit-conflict.json` now uses `conflictResultCode`, but its hold condition still references `already_resolved/state_conflict`.
  - The current backend enum and Swift conflict mapping only support `already_resolved` plus fail-closed unknown handling.
  - The gate's semantic fixture checks do not catch this stale hold-condition wording.

Impact:

The fixture can confuse reviewers or future implementers into believing `state_conflict` remains part of the current emitted contract.

Recommended action:

Update the fixture hold condition to the narrowed `already_resolved` contract, or reintroduce and prove `state_conflict` if the broader proposal contract is still desired. Add a gate assertion that fixture hold conditions do not reference removed enum values.

## Readiness Checklist

- [x] Proposal file located and parsed.
- [x] Current branch and HEAD captured.
- [x] Reference contract inspected.
- [x] Swift presenter/read-boundary mapping inspected.
- [x] Rust GraphQL mutation and backend tests inspected.
- [x] P081 action-boundary linkage inspected.
- [x] Negative fixtures inspected.
- [x] Canonical Proposal 085 gate executed.
- [ ] Canonical Proposal 085 gate passed.
- [ ] Swift Proposal 085 tests executed in the audited tree.
- [ ] Server-owned generating/deferred deadline or stalled-state behavior proven.
- [ ] Full row schema parity proven for every required row.

## Verification Log

Commands and observations:

- `git rev-parse --show-toplevel`
  - Confirmed repository root: `/Users/user/Documents/Chainworks Forge`.
- `git branch --show-current`
  - Confirmed audited branch: `main`.
- `git rev-parse HEAD`
  - Confirmed audited HEAD: `924669e87694d3f0172bf6a2df7b028a47f77f7c`.
- `git status --short`
  - Dirty tree with P085, P015, P086, reference docs, fixtures, scripts, Swift, and Rust changes.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/085-thin-client-read-model-parity-and-affordance-contract.md`
  - Returned this report path with R3 suffix.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/prior_reviews.py /Users/user/Documents/Chainworks Forge/docs/proposals/085-thin-client-read-model-parity-and-affordance-contract.md`
  - Returned no reusable prior proposal-review artifacts.
- Static inspection with fast file search and line-numbered reads.
  - Inspected Proposal 085 JSON contract, P085 reference docs, P081 boundary docs, Swift presenter/read-boundary code, GraphQL schema/types/command handling, negative fixtures, and P085 test-gate wiring.
- `./scripts/test-gate.sh proposal-085`
  - Static Proposal 085 gate checks passed.
  - Rust backend Proposal 085 tests passed: 5 tests.
  - Swift build failed before Swift Proposal 085 tests ran.
  - Overall result: failed, exit code 65.

No full gate, build gate, UI smoke test, simulator run, daemon startup, benchmark, fuzz, or load test was run. Repository policy says UI tests are remote-only unless explicitly requested.

## Verdict

Verdict: Not ready. The current `main` tree does not satisfy Proposal 085 implementation readiness.

The implementation has meaningful contract and backend progress, especially around `already_resolved` approval conflict parity and P081 boundary linkage. However, the same-tree canonical Proposal 085 gate fails because Swift does not build, and the proposal's server-owned generating/deferred deadline or stalled-state requirement remains unimplemented or unproven. The reference contract also needs row-schema normalization before it fully matches the proposal's "every row must name" requirement.

Required actions before closeout:

1. Fix the Swift compile failures and re-run `./scripts/test-gate.sh proposal-085` successfully in the same tree.
2. Implement or explicitly revise/defer the server-owned generating/deferred deadline or stalled-state contract, with backend proof tests and gate enforcement.
3. Normalize all required affordance rows to the full Proposal 085 row schema.
4. Clean up the stale `state_conflict` fixture wording or restore/prove that conflict code if it is still intended.
5. Re-audit after the gate passes and the missing behavioral proof is resolved.
