# Implementation Audit R3: Proposal 090 - Junie Code Writer Runtime Hardening After Capability Proof

## Verdict

- Overall Conformance: Not Implemented
- Overall Implementation Readiness: Not Ready
- Reviewer Selection Reuse: Reused exactly
- Audit Confidence: Medium-High

The current implementation has advanced beyond R2. It now includes P090 startup recovery code for staged settlement rows, P089 evidence validation passes, `progress_before_handoff` uses the proposal vocabulary, final-payload capture JSON includes artifact paths and failure-envelope authority, and GraphQL/MCP tests exercise a non-`none` Junie P090 readback fixture.

The implementation still does not close P090. The remaining hard blockers are the missing long-running Junie refine-like canary under the hardened boundary, incomplete provider failure-envelope identity/mismatch enforcement, unproven active artifact pointer publication, incomplete Junie preflight remediation/capacity lifecycle, and missing full-regression/readiness evidence.

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.md` |
| Proposal state | Active draft (`Status: Draft`) |
| Proposal md5 | `f98170c78ca39398e9aaed497180c057` |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| Audited HEAD | `225bac4e47b135d92c4fe2de243dd13c4647be19` |
| Implementation target | Current dirty worktree on `main` |
| Compare base | Implicit current worktree; no PR/range supplied |
| Report path | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof_IMPLEMENTATION_AUDIT_R3.md` |
| Gate evidence | `./scripts/test-gate.sh proposal-090` passed; `./scripts/test-gate.sh proposal-089` default evidence validation passed |

Existing `IMPLEMENTATION_AUDIT_R1/R2` files were ignored for reviewer selection, per skill rules.

## Prior Proposal-Review Reuse

Prior proposal-review evidence was found at `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.review/evidence-pack.md`. It was produced for older proposal md5 `e6f4a176751fffe415aeed362041a0bb`, but the selected disciplines still match the current backend/runtime/API/rollout implementation surfaces.

Reuse status: Reused exactly.

Selected reviewers:

- `chainworks_execution_truth_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`

Rejected close alternatives:

- `rust_security_reviewer`: provider spoofing is security-sensitive, but the proposal frames it as execution-truth and public readback authority.
- `rust_arch_reviewer`: covered by the execution-truth and reliability lenses for this slice.
- `apple_arch_reviewer`: no SwiftUI/app architecture behavior is in scope.
- `product_reviewer`: no product metrics or prioritization checkpoint drive this implementation audit.

## Proposal Contract And Scope

P090 is a runtime-boundary hardening proposal, not a provider replacement proposal. It depends on P089's conclusion that Junie can emit strict structured output, then addresses long-running `code_writer` reliability and final handoff behavior.

Key proposal commitments:

- strict final completion envelope and engine-owned failure authority: proposal lines 221-315;
- separate final payload capture and forensic transcript capture: lines 317-352 and 713-727;
- provider-neutral `completion_boundary_subtype` with seven Junie values: lines 354-409;
- staged per-output repair settlement, durable rows, idempotency, active pointer behavior, and crash recovery: lines 411-513;
- final response overflow, progress-without-handoff, transcript degradation, and runtime preflight/remediation: lines 527-625;
- additive DB/API/readback compatibility: lines 627-843;
- acceptance and gate requirements: lines 845-970;
- rollout controls and downgrade behavior: lines 972-994.

Platform/product scope:

- Apple: N/A. No iOS/macOS UI implementation behavior is audited.
- Backend/service: service, worker, API, data, rollout, runtime diagnostics, and recovery.
- Operator scope: blocked Junie code-writer attempts must become precisely diagnosable and safely repairable.

Primary service flows:

1. Junie returns valid final `CHAINWORKS_OUTPUT`, and outputs settle normally.
2. Junie returns missing/truncated/narrative completion, and the receipt/readback exposes a precise subtype.
3. Repair returns mixed-validity outputs, and accepted outputs settle without malformed siblings overwriting canonical truth.
4. Runtime path preflight catches known path failures before Junie launch when enforcement is active.
5. GraphQL, MCP, and run-report readback expose consistent P090 completion and settlement facts.

Product metrics:

- Leading metric: N/A.
- Guardrail metric: N/A.
- Decision checkpoint: N/A.

## Evidence Pack

| ID | Type | Evidence |
| --- | --- | --- |
| EV-001 | proposal | Lines 849-860 define the P090 done criteria. |
| EV-002 | tests-run | `./scripts/test-gate.sh proposal-090` passed on the audited worktree. |
| EV-003 | tests-run | `./scripts/test-gate.sh proposal-089` default evidence validation passed. |
| EV-004 | code | `control-plane/crates/engine/src/executor.rs:10940-11025` builds P090 receipt fields including final payload capture JSON and failure-envelope authority. |
| EV-005 | code | `control-plane/crates/engine/src/executor.rs:11673-11768` maps P090 subtype values. |
| EV-006 | code | `control-plane/crates/engine/src/executor.rs:11887-11902` maps `progress_before_handoff` to proposal vocabulary. |
| EV-007 | code | `control-plane/crates/engine/src/executor.rs:11904-11931` writes final-payload capture JSON with raw/redacted artifact paths. |
| EV-008 | code | `control-plane/crates/engine/src/executor.rs:11934-11962` derives `provider_claim_rejected` / `engine_synthesized` authority from captured text markers. |
| EV-009 | code | `control-plane/crates/engine/src/executor.rs:1314-1467` stages and commits accepted repair outputs. |
| EV-010 | code | `control-plane/crates/engine/src/recovery.rs:333-351` runs P090 settlement recovery during startup repair. |
| EV-011 | code | `control-plane/crates/engine/src/recovery.rs:597-670` reconciles staged/committed P090 rows from canonical/staging digests. |
| EV-012 | code | `control-plane/crates/acp/src/adapters/junie.rs:81-166` implements enforcement-gated Junie tool-path preflight checks. |
| EV-013 | tests-found | `control-plane/crates/db/tests/proposal_088_persistence.rs:976-1059` covers recoverable staged rows and digest conflict behavior at repository level. |
| EV-014 | tests-found | GraphQL/MCP P088 readback tests now seed and assert Junie P090 fields, final payload artifact path, failure-envelope authority, and settlement rows. |
| EV-015 | evidence | `docs/evidence/090/junie-runtime-hardening/evidence-index.json` lists all seven subtype fixtures and negative fixtures. |
| EV-016 | evidence | P089 `live-gate-run.json` records a prior live run for git SHA `6089a7b85ca524849ace1b4367eabbf71e424dfc`; the default P089 gate validates the recorded evidence and current proof-critical files. |

## Fidelity Inventory

Matches:

- Additive P090 schema fields and settlement row table exist.
- P090 gate validates subtype fixture coverage, negative fixtures, and focused Rust/API tests.
- P089 default evidence validation passes.
- Final payload capture JSON now includes raw/redacted artifact paths and `failure_envelope_authority`.
- `progress_before_handoff` now emits proposal values such as `worktree_diff_detected` and `meaningful_progress`.
- GraphQL and MCP tests cover non-`none` Junie P090 summary/readback values.
- Startup recovery has a P090 settlement-row reconciliation path.

Divergences:

- No required long-running Junie refine-like canary under P090 hardening was found or run.
- Provider-authored failure envelope handling is marker-based; no implementation path was found that validates `run_id`, `stage_execution_id`, `agent_execution_id`, or `session_generation_id` and emits `provider_envelope_identity_mismatch`.
- Active artifact pointer behavior is represented by `active_pointer_generation_id`, but no active artifact index update or preservation test proves proposal lines 491, 499, and 907.
- Junie preflight does not yet show wrong-cwd/runtime-home remediation, provider-capacity timing, or work-item ack semantics.
- P089 live evidence is recorded, but it was not live-rerun on the current worktree during this audit.

Ambiguities / Evidence Gaps:

- `proposal-090` does not call `proposal-089`; both gates were run separately in this audit.
- Transcript persistence degradation is represented by readback fields, but no fixture proves final completion truth survives a storage failure.
- Recovery tests prove repository helpers and recovery code exists, but no focused startup integration test injects a file-system crash window through the recovery service.
- Full regression was not run.

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 5 |
| Partially Implemented | 12 |
| Missing | 1 |
| Not Verifiable | 2 |
| Out of Scope | 0 |

## Detailed REQ Audit

| REQ | Requirement | Source | Status | Evidence and Gap |
| --- | --- | --- | --- | --- |
| REQ-001 | Preserve P089 capability baseline and do not reframe Junie as incapable. | Lines 34-55, 207-217, 849 | Implemented | Junie remains the provider path; `./scripts/test-gate.sh proposal-089` default evidence validation passed. Live P089 evidence itself was not rerun. |
| REQ-002 | Strict final completion boundary and no ordinary success from free-form prose. | Lines 221-235 | Partially Implemented | Strict/staged flags and subtype helpers exist. End-to-end large narrative strict rejection is not fully proven. |
| REQ-003 | Engine-owned envelope authority; provider-authored envelopes must fail closed and read back as rejected claims. | Lines 237-315, 933-944 | Partially Implemented | Transport refuses to extract provider-authored failure envelopes; final payload JSON can expose `provider_claim_rejected`. Identity/mismatch validation and `provider_envelope_identity_mismatch` emission were not found in runtime code. |
| REQ-004 | Separate final payload capture from forensic transcript. | Lines 317-352, 713-727 | Partially Implemented | Final payload JSON now includes capture refs and artifact paths. Transcript persistence failure independence is still not proven. |
| REQ-005 | Provider-neutral subtype wrapper with unknown raw compatibility. | Lines 354-409, 729-740 | Partially Implemented | Domain/API wrappers exist and non-`none` Junie readback is tested. Unknown subtype raw round-trip for P090 is not specifically proven. |
| REQ-006 | Support seven Junie subtype values. | Lines 376-398 | Partially Implemented | Helper contains mappings and evidence index covers all seven. The gate does not exercise every subtype end to end through DB/API/report. |
| REQ-007 | Durable per-output settlement rows with receipt linkage and idempotency. | Lines 411-480 | Implemented | Migration, repository validation, DB tests, and readback rows cover the row model and digest idempotency. |
| REQ-008 | Staged validate-before-materialize repair with malformed sibling protection. | Lines 482-499, 508-513, 791-799 | Partially Implemented | Staging helper and tests prove canonical protection for focused helper behavior. Active artifact index publication is not proven. |
| REQ-009 | Crash recovery for staged/committed/failed settlement rows. | Lines 501-506 | Partially Implemented | Startup recovery code and DB helper tests exist. No startup integration test proves recovery through real canonical/staging file states. |
| REQ-010 | Repair payloads prefer output-name keys. | Lines 514-525 | Not Verifiable | Existing discovery supports output names, but no focused prompt/repair test proves key preference. |
| REQ-011 | Final response size budget and truncation classification. | Lines 527-537, 872-879 | Partially Implemented | Capture metadata and subtype mapping handle truncation. A large narrative fixture is not exercised end to end. |
| REQ-012 | Progress-without-handoff is distinct from empty/startup failure. | Lines 538-552, 763-770 | Partially Implemented | Subtype and public progress vocabulary exist. A realistic no-terminal ACP progress fixture is not run end to end. |
| REQ-013 | Transcript absence must not erase completion truth. | Lines 553-562, 909-916 | Not Verifiable | Separate fields exist, but no transcript persistence degradation fixture proves the committed behavior. |
| REQ-014 | Junie runtime preflight fail-before-launch with facts. | Lines 563-625, 917-931 | Partially Implemented | Preflight checks project proof file/output/temp paths and engine records failure facts. Wrong-cwd/runtime-home remediation, capacity timing, and work-item ack semantics are not proven. |
| REQ-015 | Rollout controls and downgrade behavior. | Lines 972-994 | Partially Implemented | Env flag helpers and preflight enforcement flag exist. Full downgrade matrix and startup/config validation are not tested. |
| REQ-016 | Additive receipt/readback compatibility. | Lines 627-740 | Implemented | Migration/domain/API fields are additive; GraphQL/MCP tests pass. |
| REQ-017 | GraphQL/MCP/report readback agreement for P090 fields. | Lines 801-843 | Implemented | GraphQL and MCP tests now assert non-`none` Junie subtype, final payload path, authority, and settlement rows. |
| REQ-018 | Evidence inventory maps all observed subtypes and negative fixtures. | Lines 57-68, 946-970 | Implemented | Evidence index and gate validate all subtype and negative fixture files/hashes. |
| REQ-019 | Canonical `proposal-090` gate exists and runs focused Rust/API checks. | Lines 950-970 | Implemented | `./scripts/test-gate.sh proposal-090` passed and runs DB, ACP, engine, GraphQL, and MCP tests. |
| REQ-020 | Long-running Junie refine-like canary completes under the hardened P090 boundary. | Lines 850, 979 | Missing | No required P090 long-running refine-like canary run or regression gate was found. P089 ACP canary is not the same acceptance item. |

## Reviewer Scorecard

| Lens | Conformance | Readiness | Top Risk | Confidence |
| --- | --- | --- | --- | --- |
| Proposal conformance | Not Implemented | Not Ready | Missing long-running P090 canary and partial runtime lifecycle proof. | Medium-High |
| `chainworks_execution_truth_reviewer` | Partial | Not Ready | Provider envelope identity/mismatch handling is not implemented beyond marker rejection. | Medium-High |
| `rust_reliability_reviewer` | Partial | Not Ready | Recovery exists, but active pointer publication and startup crash-window proof are incomplete. | High |
| `api_contract_reviewer` | Partial | Not Ready | API readback improved, but unknown subtype and identity-mismatch semantics are not fully proven. | Medium |
| `observability_rollout_reviewer` | Partial | Not Ready | Flags/gates exist, but long-running canary and full rollout matrix are absent. | High |

## Routed Specialist Findings

### READY-001 - Missing P090 long-running Junie refine-like canary

- Reviewer: `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related REQs: REQ-020
- Evidence types: proposal, tests-run, code search
- Evidence references: proposal lines 850 and 979; `scripts/test-gate.sh:6981-7137`; search results only show P089 ACP canary and no P090 long-running refine-like gate.

Why it matters: P090's core problem is long-running code-writer completion after P089 proved one-shot capability. Passing P089 and focused P090 unit/API tests does not prove the long-running hardened boundary.

Recommended action: Add a required P090 canary that runs a Junie refine-like `code_writer` path under strict/preflight/staged settings and settles fresh outputs.

Acceptance criteria: `proposal-090` or a documented required companion gate runs the long-running canary and records same-tree evidence.

### API-001 - Failure-envelope identity/mismatch handling is still not implemented end to end

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-003, REQ-005, REQ-018
- Evidence types: proposal, code, tests-found
- Evidence references: proposal lines 253-259 and 933-944; `control-plane/crates/acp/src/transport.rs:4529-4562`; `control-plane/crates/engine/src/executor.rs:11934-11962`; repository search found no runtime emission of `provider_envelope_identity_mismatch`.

Why it matters: P090 requires mismatched `run_id`, `stage_execution_id`, `agent_execution_id`, or `session_generation_id` to fail closed as `provider_envelope_identity_mismatch`. The current implementation detects provider-authored failure-like markers and reports `provider_claim_rejected`, but does not validate envelope identifiers or surface identity-mismatch subtype from runtime facts.

Recommended action: Parse provider-authored engine/repair failure envelopes as untrusted diagnostics, validate identifiers, and persist `provider_envelope_identity_mismatch` or `provider_envelope_unrecognized` as appropriate.

Acceptance criteria: spoof, identity mismatch, and unknown schema fixtures fail closed through DB, GraphQL, MCP, and report readback with no materialized outputs.

### REL-001 - Active artifact pointer publication is not proven from accepted settlement rows

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related REQs: REQ-008, REQ-009
- Evidence types: proposal, code, tests-found
- Evidence references: proposal lines 491, 499, and 907; `control-plane/crates/engine/src/executor.rs:1314-1467`; GraphQL/MCP tests assert `activePointerGenerationId` values but do not prove active artifact index mutation or preservation.

Why it matters: P090 requires active artifact pointers to be derived from accepted settlement rows and malformed siblings to leave existing active pointers unchanged. Recording an `active_pointer_generation_id` field is not the same as updating and verifying the active artifact index.

Recommended action: Wire accepted settlement rows to the active artifact index update path, and add tests proving rejected outputs do not move active pointers while accepted outputs do.

Acceptance criteria: after mixed repair, the active artifact index points to accepted fresh generations only, and the malformed sibling still points to the previous valid generation.

### REL-002 - Preflight remediation and capacity lifecycle remain incomplete

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related REQs: REQ-014, REQ-015
- Evidence types: proposal, code, tests-found
- Evidence references: proposal lines 574-617; `control-plane/crates/acp/src/adapters/junie.rs:81-166`; `control-plane/crates/engine/src/executor.rs:11775-11872`.

Why it matters: The preflight checks are useful, but P090 requires one remediation attempt for wrong cwd/runtime-home, provider capacity only after preflight passes, and work-item ack after terminal failure or handoff. Those lifecycle guarantees are not covered by the current focused tests.

Recommended action: Add remediation state recording, wrong-cwd/runtime-home fixtures, provider-capacity timing tests, and work-queue ack/fail-before-launch integration tests.

Acceptance criteria: permission-denied fails before launch, cwd/runtime-home failures remediate once, provider capacity is not consumed before pass, and receipts expose attempts/remediation.

### READY-002 - Full regression was not run on the audited tree

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: readiness gate
- Evidence types: tests-run
- Evidence references: verification log below.

Why it matters: A successful closeout verdict requires same-tree full regression or canonical full/proposal gate evidence. This audit is not claiming success, so full regression was not required, but it remains mandatory before any Ready/Implemented closeout.

Recommended action: After the missing canary and major gaps are fixed, run the repository's required full gate on the same tree.

Acceptance criteria: full regression/canonical full gate passes on the same tree and HEAD used for closeout.

## Readiness Checklist

| Item | Status | Evidence / Note |
| --- | --- | --- |
| Proposal resolved | Passed | Markdown exists. |
| Report path generated by helper | Passed | Helper selected R3 path. |
| Prior review routing reused | Passed | Reused exactly. |
| P090 focused gate | Passed | `./scripts/test-gate.sh proposal-090`. |
| P089 default evidence gate | Passed | `./scripts/test-gate.sh proposal-089`. |
| P089 live rerun on current tree | Not run | Recorded live evidence exists, but not rerun during this audit. |
| Long-running P090 Junie refine-like canary | Missing | Required by proposal lines 850 and 979. |
| Core service flow integration | Partial | Focused Rust/API tests pass; long-running live flow absent. |
| Staged settlement recovery | Partial | Recovery code exists; startup crash-window integration test absent. |
| GraphQL/MCP/report non-`none` readback | Passed | Tests assert Junie P090 readback fields. |
| UI empty/loading/error/offline | N/A | No UI scope. |
| Accessibility/localization | N/A | No UI scope. |
| Privacy/permissions/entitlements | Partial | Preflight covers permission-like path failures; remediation/TCC behavior is not fully proven. |
| Full regression | Not run | Required before any successful closeout verdict. |

## Verification Log

Executed:

```bash
./scripts/test-gate.sh proposal-090
./scripts/test-gate.sh proposal-089
```

Results:

- `proposal-090`: Passed.
- `proposal-089`: Passed default evidence validation.

Not executed:

- P089 live rerun with `CHAINWORKS_PROPOSAL_089_LIVE=1`.
- Long-running P090 Junie refine-like canary.
- Full regression suite.

## Final Recommendation

Do not close P090 as Implemented/Ready yet. The implementation is substantially improved, but it still lacks the proposal's key long-running canary and has major proof gaps around provider envelope identity handling, active artifact pointer publication, and preflight remediation/capacity lifecycle.

Recommended next actions:

1. Add and run the long-running P090 Junie refine-like canary.
2. Implement untrusted failure-envelope identifier validation and identity-mismatch readback.
3. Prove active artifact index updates are derived only from accepted settlement rows.
4. Add preflight remediation/capacity/work-item lifecycle tests.
5. Add startup recovery integration tests for staged settlement crash windows.
6. Run full regression before any closeout verdict.

