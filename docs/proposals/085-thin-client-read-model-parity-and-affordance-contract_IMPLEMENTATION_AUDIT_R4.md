# Proposal 085 Implementation Audit R4

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/085-thin-client-read-model-parity-and-affordance-contract.md` |
| Proposal title | Thin client read-model parity and affordance contract |
| Proposal format | `proposal_json_v1` stored in a `.md` file |
| Proposal claimed status | `implemented` |
| Audit report | `docs/proposals/085-thin-client-read-model-parity-and-affordance-contract_IMPLEMENTATION_AUDIT_R4.md` |
| Audit date | 2026-05-09 |
| Target branch | `main` |
| Audited HEAD | `924669e87694d3f0172bf6a2df7b028a47f77f7c` |
| Implementation target | Current worktree, including uncommitted changes |
| Compare base | Implicit current worktree audit; no PR base or commit range supplied |
| Canonical gate | `./scripts/test-gate.sh proposal-085` |
| Gate result | Passed |
| Result bundle | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-085-20260509-202331.xcresult` |

## Implementation Target

The audit target is the current `main` worktree at HEAD `924669e87694d3f0172bf6a2df7b028a47f77f7c`.

The tree is dirty. At audit start it contained Proposal 085 implementation changes plus other modified files under P015/P086/workflow/engine surfaces, and an untracked R3 implementation audit report. The P085 gate was run against this exact dirty tree and passed.

## Prior Proposal-Review Reuse

- Prior proposal-review artifacts discovered by the skill helper: none.
- Prior implementation audit reports present: R1, R2, and R3.
- Reviewer-selection reuse status: Not reused.
- Prior `IMPLEMENTATION_AUDIT` reports were not used for reviewer selection, per the audit workflow.

## Selected Reviewers

- `api_contract_reviewer`: GraphQL schema, enum, DTO, mutation, and read-model parity.
- `apple_arch_reviewer`: Swift thin-client read boundary, presenter mapping, and fail-closed behavior.
- `rust_reliability_reviewer`: approval conflict/idempotency, stale submit handling, and deadline/no-deadline semantics.
- `observability_rollout_reviewer`: gate wiring, negative fixtures, rollout hold conditions, and evidence quality.
- `chainworks_execution_truth_reviewer`: durable approval/artifact/report truth and UI action boundary consistency.

Rejected close alternatives:

- `macos_ui_reviewer`: user-visible macOS behavior is present, but the proposal's audited commitments are contract/presenter/read-model behavior rather than visual layout runtime.
- `rust_arch_reviewer`: Rust architecture concerns were sufficiently covered by API contract and reliability lenses.
- `product_reviewer`: no central product metric or decision checkpoint required a separate product-readiness review.

## Proposal State And Contract Summary

Proposal state: Implemented, based on the proposal JSON status and the current reference contract.

The proposal requires a canonical affordance contract so Swift does not infer truth from partial local state. The implemented slice must:

- Define all required affordance rows and row-schema columns.
- Keep artifact/report payload availability honest across available, metadata-only, payload-deferred, generating, unavailable, and unknown states.
- Keep freshness diagnostic-only and unable to drive local actionability.
- Gate approve/reject actions on durable backend approval state, caller policy, `availableActions`, and write-path availability.
- Return typed approval conflict/idempotency results for already-resolved stale/duplicate submits.
- Preserve transient server/transport failures as GraphQL errors, not silent retries or success.
- Fail closed on unknown enum values.
- Redact or invalidate unauthorized diagnostics and payload readback.
- Keep non-approval external commands out of SwiftUI mutation paths.
- Prove the contract through the P085 gate, including schema/static checks, backend proof tests, negative fixtures, and Swift parity tests.

## Platform And Product Scope

- Apple platform scope: macOS.
- Backend/service scope: Rust GraphQL API, read-model projection, approval mutation, and rollout/test-gate evidence.
- Cross-stack scope: Swift thin client to GraphQL API to durable command/read-model truth.
- Product scope: operator affordance correctness and trust; no new product metric or experiment gate.

Out of scope:

- iOS behavior.
- New external command execution from SwiftUI.
- Raw local payload fallback.
- UI smoke/runtime screenshot validation unless separately requested.
- Full repo release validation outside the Proposal 085 gate.

## Primary Flows

1. Artifact preview list/detail flow: GraphQL artifact state is mapped through the P085 presenter, keeping deferred/generating/metadata/unavailable/unknown states distinct.
2. Report metadata flow: report rows expose metadata and server-owned diagnostics without raw payload fallback unless an authorized server payload query exists.
3. Freshness flow: run/stage/agent/approval/artifact freshness badges remain recency diagnostics and do not create actionability.
4. Approval resolve flow: approve/reject buttons are enabled only from durable backend state plus caller/write-path authorization, and stale duplicate submits return typed already-resolved readback.
5. Diagnostic/external boundary flow: diagnostic copy is redacted or invalidated when unauthorized, while external command placeholders stay outside SwiftUI mutation authority.

## Fidelity Inventory

### Matches

- All required affordance rows are present in `docs/reference/thin-client-read-model-affordance-contract.md`, and the gate checks every required row for the required row-schema fields.
- P081 UI boundary rows are explicit in `docs/reference/ui-action-boundary.md` lines 32-35 and referenced by the affordance contract.
- Backend approval mutation payloads include `conflictResultCode` and return `already_resolved` only for typed already-resolved approval conflicts.
- Non-conflict approval mutation errors return GraphQL errors.
- Backend artifact projection code now emits `P085_NO_DEADLINE_JUSTIFICATION` for bounded deferred/metadata states.
- Backend P085 tests prove conflict readback, enum parity, artifact/report projection states, authorization denial, and no-deadline justification.
- Swift P085 tests prove payload mapping, stale detail merge behavior, approval actionability, freshness diagnostic-only behavior, diagnostic invalidation, fail-closed enum handling, mutation conflict decoding, and production presenter wiring.
- Negative fixtures are present, semantically checked, and the gate now rejects stale `state_conflict` references.

### Divergences

- The implementation uses an explicit no-deadline justification for current bounded deferred/metadata states rather than adding persisted deadline fields. This is permitted by the proposal contract, which allowed explicit no-deadline justification.
- The audited tree is dirty and includes non-P085 changes. This is a handoff/traceability risk, not a Proposal 085 conformance failure.

### Ambiguities / Evidence Gaps

- UI runtime smoke/screenshots were not collected. The proposal's audited commitments are covered by presenter/read-model tests and the canonical P085 gate, but visual runtime evidence remains out of scope for this pass.
- Full repository regression was not run. The canonical Proposal 085 gate passed, which is sufficient for a successful Proposal 085 audit verdict under this skill.

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 12 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

Overall Conformance: Implemented.

Overall Implementation Readiness: Ready with Risks, because the canonical Proposal 085 gate passed, but the worktree is dirty and mixes P085 with unrelated proposal work.

## Detailed Requirement Audit

| ID | Requirement | Proposal source | Status | Evidence | Implementation mapping | Gap / note |
| --- | --- | --- | --- | --- | --- | --- |
| REQ-001 | Canonical affordance contract exists with all required rows and row-schema fields. | `affordance_contract.required_rows`, `affordance_contract.row_schema_columns` | Implemented | docs, tests-run | `docs/reference/thin-client-read-model-affordance-contract.md` rows; `scripts/test-gate.sh` required row/field checks at lines 6244-6267 | Gate enforces all required row fields. |
| REQ-002 | GraphQL schema/read model exposes required payload, freshness, approval, diagnostic, conflict, and authorization fields. | `architecture.backend_read_model`, `gate` | Implemented | schema, code, tests-run | `control-plane/crates/graphql-server/src/schema.rs`; P085 backend proof tests at lines 6375, 6463, 6496, 6590, 6795 | Rust P085 proof slice passed. |
| REQ-003 | Artifact preview list/detail distinguishes available, metadata-only, payload-deferred, generating, unavailable, and unknown states without collapsing deferred to unavailable. | `artifact.preview.listLabel`, `artifact.preview.detail` | Implemented | code, tests-run | `P085AffordancePresenter.artifactListAffordance` and `.artifactDetailAffordance`; Swift tests including `payloadDeferredMapsToDeferred` and `staleDetailDoesNotOverwriteAfterSelectionChange` | Swift P085 slice passed. |
| REQ-004 | Current deferred/metadata payload states include server-owned deadline, stalled state, or explicit no-deadline justification. | payload deadline/stalled hold conditions | Implemented | code, tests-run | `P085_NO_DEADLINE_JUSTIFICATION` in `types/artifact.rs` lines 15, 161, 213; `mark_payload_deferred` in `schema.rs` line 1272; backend assertions at lines 6738 and 6748 | Implemented using explicit no-deadline justification for bounded projection states. |
| REQ-005 | Report payload remains metadata-only unless server-owned payload authorization is present. | `report.payload.metadata` | Implemented | docs, code, tests-run | Reference row at lines 103-119; backend projection/auth test at line 6795; Swift metadata tests at line 35 | No raw local payload fallback found. |
| REQ-006 | Freshness badges are diagnostic/recency-only and cannot drive payload availability or approval actionability. | freshness rows | Implemented | docs, code, tests-run | Reference freshness rows beginning at line 124; `freshnessAffordance` in Swift presenter line 214; Swift tests at lines 158-171 | Freshness actionability is false in tests and mapping. |
| REQ-007 | Approve/reject actionability is driven by durable approval state, caller policy, available actions, and write-path availability. | `approval.resolve.approve`, `approval.resolve.reject` | Implemented | docs, code, tests-run | Reference approval rows at lines 208 and 230; `actionAvailability` in Swift presenter line 351; Swift tests at lines 105, 579, 638, 651 | Production presenter wiring is tested. |
| REQ-008 | Stale/duplicate approval submits return typed idempotent/already-resolved behavior with real journal truth. | approval mutation idempotency and conflict contract | Implemented | code, tests-run | GraphQL mutation payloads at `schema.rs` lines 1376, 1384, 1404-1535; backend tests at lines 6375 and 6496 | Both approve and reject conflict tests passed. |
| REQ-009 | Transient server/transport failures stay GraphQL errors and are not silently retried as success. | approval mutation error policy | Implemented | code, tests-run | `approve_approval`/`reject_approval` return typed payload only for `approval_resolution_conflict_code`; all other errors return `Err(Error::new(...))` at `schema.rs` lines 1456-1468 and 1526-1538 | Direct code evidence plus gate coverage of conflict-only typed payload behavior. |
| REQ-010 | Unknown enum values fail closed in Swift. | unknown enum fail-closed contract | Implemented | code, tests-run | `payloadPresentation(fromRaw:)` line 242; conflict code mapping; Swift tests at lines 446, 493, and surrounding unknown enum tests | Swift P085 slice passed fail-closed tests. |
| REQ-011 | Diagnostic copy redacts or invalidates unavailable/unauthorized diagnostic details. | `diagnostic.copy` | Implemented | docs, code, tests-run | Reference row at line 252; `diagnosticAffordance` line 227; Swift tests at lines 565 and related diagnostics tests | Unauthorized diagnostic state clears ID/detail. |
| REQ-012 | External command placeholder and P081 UI action boundary prevent SwiftUI from owning non-approval mutations. | `external.command.placeholder`, P081 boundary linkage | Implemented | docs, tests-run | `docs/reference/ui-action-boundary.md` lines 32-35; reference external row at line 273; static gate checks | P085 gate passed boundary-row checks. |

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Implemented | None blocking | High |
| API contract | Pass | Future enum additions must preserve fail-closed behavior and gate updates | High |
| Apple architecture | Pass | UI runtime not smoke-tested in this audit | Medium-High |
| Rust reliability | Pass | No-deadline policy depends on bounded projection semantics staying true | High |
| Observability/rollout | Pass | Dirty tree can confuse handoff if P085 is not isolated at closeout | Medium-High |
| Execution truth | Pass | Approval/read-model truth remains tied to GraphQL command journal/readback | High |
| Overall readiness | Ready with Risks | Dirty-tree co-tenancy | High for P085 gate-backed scope |

## Routed Specialist Findings

### OPS-001: Dirty Worktree Mixes P085 With Other Proposal Work

- Reviewer: `observability_rollout_reviewer`
- Severity: Note
- Confidence: High
- Related proposal items: rollout/closeout evidence
- Related REQ IDs: none
- Evidence types: diff, config
- Evidence references: `git status --short`; dirty files include P085 code/docs/tests as well as P015/P086/workflow/engine changes and prior audit report R3.
- Why it matters: Proposal 085 conformance is implemented and the P085 gate passes, but handoff or closeout can become ambiguous if unrelated dirty changes are bundled without an operator decision.
- Recommended action: For closeout, either isolate/stage/commit the P085 slice separately or explicitly accept the broader dirty-tree bundle and run the relevant additional gates for non-P085 changes.
- Acceptance criteria: P085 closeout points to a coherent commit/branch or an operator-approved bundled change set, with applicable gates recorded.

No `READY-*`, `API-*`, `ARCH-*`, `REL-*`, `SEC-*`, `UI-*`, or `UX-*` blocker was found for the Proposal 085 contract scope.

## Readiness Checklist

- [x] Proposal path resolved and parsed.
- [x] Current branch and HEAD recorded.
- [x] Prior proposal-review discovery executed.
- [x] Reviewers routed.
- [x] Required affordance rows inspected.
- [x] Required row-schema field enforcement inspected.
- [x] Swift presenter/read-boundary mapping inspected.
- [x] Rust GraphQL mutation/read-model implementation inspected.
- [x] Negative fixtures inspected.
- [x] P081 action-boundary linkage inspected.
- [x] Canonical Proposal 085 gate executed.
- [x] Canonical Proposal 085 gate passed on the audited tree/HEAD.
- [x] Rust Proposal 085 backend proof slice passed.
- [x] Swift `Proposal085Tests` slice passed.
- [x] Empty/loading/error/offline/permission-state proxy coverage reviewed where represented by payload/freshness/authorization states.
- [x] Privacy/authorization redaction behavior reviewed for diagnostics and payload fallback.
- [ ] UI runtime screenshot/smoke validation collected. Not required by the Proposal 085 gate and not requested.
- [ ] Full repository regression suite run. Not required because the canonical Proposal 085 gate passed for this proposal audit.

## Verification Log

- `git rev-parse --show-toplevel`
  - Confirmed repo root: `/Users/user/Documents/Chainworks Forge`.
- `git branch --show-current`
  - Confirmed branch: `main`.
- `git rev-parse HEAD`
  - Confirmed HEAD: `924669e87694d3f0172bf6a2df7b028a47f77f7c`.
- `git status --short`
  - Dirty worktree, including P085 implementation files and unrelated modified/untracked files.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/085-thin-client-read-model-parity-and-affordance-contract.md`
  - Returned R4 report path.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py /Users/user/Documents/Chainworks Forge/docs/proposals/085-thin-client-read-model-parity-and-affordance-contract.md`
  - Returned no prior proposal-review artifacts.
- Static inspection:
  - Proposal JSON, reference contract, P081 boundary docs, gate wiring, negative fixtures, Swift P085 presenter/tests, Swift P031 read boundary, Rust GraphQL schema/types, and backend P085 tests.
- `./scripts/test-gate.sh proposal-085`
  - Static Proposal 085 checks passed.
  - Rust backend tests passed: 5 tests.
  - Swift `Proposal085Tests` passed: 45 tests in 1 suite.
  - Overall result: Proposal 085 gate passed.
  - Result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-085-20260509-202331.xcresult`.

No UI smoke test, daemon startup outside the gate build, benchmark, fuzz, load test, or full suite was run.

## Verdict

Overall Conformance: Implemented.

Overall Implementation Readiness: Ready with Risks.

Reviewer Selection Reuse: Not reused.

Audit Confidence: High for the Proposal 085 contract scope, because the same-tree canonical Proposal 085 gate passed and the audited requirements have direct code, documentation, and test evidence.

Recommended next actions:

1. Close out Proposal 085 from a coherent staged/committed slice, or explicitly accept the broader dirty-tree bundle.
2. If the non-P085 dirty changes are included in the same handoff, run their applicable proposal gates before merging.
3. Preserve the P085 gate evidence path above in closeout notes.
