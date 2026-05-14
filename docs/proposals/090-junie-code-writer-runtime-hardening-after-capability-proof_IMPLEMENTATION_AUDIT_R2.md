# Implementation Audit R2: Proposal 090 - Junie Code Writer Runtime Hardening After Capability Proof

## Verdict

- Overall Conformance: Not Implemented
- Overall Implementation Readiness: Not Ready
- Reviewer Selection Reuse: Reused exactly
- Audit Confidence: Medium-High

The current implementation is materially closer than the previous audit snapshot: it now has runtime rollout flags, Junie adapter preflight checks, concrete negative fixture validation, subtype helper coverage for partial/narrative repair, and staged repair rows that are persisted before canonical commit. It still does not fully satisfy P090. The remaining blockers are the engine-owned provider-claim readback contract, full staged-settlement recovery/active-pointer semantics, complete preflight remediation/capacity lifecycle, final-payload artifact separation, API vocabulary compatibility, and missing P089/long-running Junie canary evidence.

The roll-up remains `Not Implemented` because explicit in-scope commitments are still Missing, not merely unpolished.

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
| Report path | `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof_IMPLEMENTATION_AUDIT_R2.md` |
| Audit date | 2026-05-14 |

Existing implementation audit reports were ignored for reviewer selection per the skill. R2 exists because R1 was already present beside the proposal.

## Worktree Scope

The worktree is dirty. The audited P090 implementation surfaces are Rust control-plane ACP, engine, DB, GraphQL/MCP readback, evidence fixtures, and test gates. An unrelated Swift change in `Chainworks Forge/Support/DaemonLifecycleClient.swift` was present and was not treated as P090 evidence.

Primary P090 files inspected:

- `control-plane/crates/acp/src/adapters/junie.rs`
- `control-plane/crates/acp/src/adapters/mod.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/db/migrations/053_p090_code_writer_runtime_hardening.sql`
- `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs`
- `control-plane/crates/domain/src/code_writer_completion.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/graphql-server/src/types/run.rs`
- `control-plane/crates/graphql-server/tests/proposal_088_code_writer_completion_readback.rs`
- `control-plane/crates/mcp-server/tests/proposal_088_code_writer_completion_readback.rs`
- `docs/evidence/090/junie-runtime-hardening/*`
- `scripts/test-gate.sh`

## Prior Proposal-Review Reuse

Prior proposal-review artifacts were found at `docs/proposals/090-junie-code-writer-runtime-hardening-after-capability-proof.review/evidence-pack.md`. They clearly refer to P090 but were produced for older md5 `e6f4a176751fffe415aeed362041a0bb`; current proposal md5 is `f98170c78ca39398e9aaed497180c057`. The reviewer set still matches the current implementation surfaces and risks, so reuse is valid.

Selected reviewers:

- `chainworks_execution_truth_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`

Rejected close alternatives:

- `rust_security_reviewer`: provider spoofing is security-sensitive, but P090 frames it as execution-truth and public-boundary authority, covered by selected reviewers.
- `rust_arch_reviewer`: architecture risks are narrower than execution-truth/reliability in this implementation slice.
- `apple_arch_reviewer`: no SwiftUI/app architecture behavior is in scope.
- `product_reviewer`: no product metric or prioritization decision is central to this implementation audit.

## Proposal Contract

Source highlights:

- P090 is a runtime/completion-boundary hardening proposal, not a provider-capability proposal: proposal lines 13-32, 34-55, 207-217.
- Strict final completion envelope and engine-owned trust boundary: lines 221-315.
- Separate final payload capture from forensic transcript: lines 317-352.
- Provider-neutral `completion_boundary_subtype` public contract and seven Junie subtype values: lines 354-409.
- Per-output staged repair settlement, durable rows, idempotency, and crash recovery: lines 411-513.
- Final size budget, progress-without-handoff, transcript absence, and Junie preflight/remediation: lines 527-625.
- Additive receipt/readback fields and compatibility: lines 627-741.
- Operator readback agreement across GraphQL/MCP/report: lines 801-843.
- Acceptance criteria and gate expectations: lines 845-970.
- Rollout controls: lines 972-994.

Platform/product scope:

- Apple: N/A for implementation. No user-facing iOS/macOS UI behavior is audited.
- Backend/service: service, worker, API, data, rollout, and runtime control-plane scope.
- Product/operator scope: make blocked Junie `code_writer` attempts diagnosable and safely repairable without re-debating Junie structured-output capability.

Primary service flows:

1. Junie `code_writer` emits a valid final `CHAINWORKS_OUTPUT`; the engine settles outputs and records completion truth.
2. Junie returns no final handoff, a truncated/narrative final response, or progress without terminal payload; the receipt exposes a precise boundary subtype.
3. A repair turn returns a mixed-validity payload; accepted outputs are staged/settled independently and malformed siblings do not overwrite canonical truth.
4. Junie runtime path preflight catches missing/wrong/unreadable project/output paths before provider launch when enforcement is enabled.
5. DB, GraphQL, MCP, and run-report readback expose the same completion-boundary and settlement facts while old clients continue to read legacy rows.

Product metrics:

- Leading metric: N/A.
- Guardrail metric: N/A.
- Decision checkpoint: N/A.

## Evidence Pack

| ID | Type | Evidence |
| --- | --- | --- |
| EV-001 | proposal | Proposal lines 221-315 require strict envelope and engine-owned failure truth. |
| EV-002 | proposal | Proposal lines 426-513 require staged per-output settlement rows, transaction semantics, and crash recovery. |
| EV-003 | proposal | Proposal lines 563-625 require Junie preflight, remediation, no-launch behavior, and capacity timing. |
| EV-004 | proposal | Proposal lines 801-843 require operator readback agreement across GraphQL, MCP, and reports. |
| EV-005 | migration | `control-plane/crates/db/migrations/053_p090_code_writer_runtime_hardening.sql:1-69` adds additive receipt fields and `code_writer_output_settlement_rows`. |
| EV-006 | code | `control-plane/crates/domain/src/code_writer_completion.rs:6-123` adds P090 receipt and settlement-row domain fields. |
| EV-007 | code | `control-plane/crates/engine/src/executor.rs:1314-1467` stages repair candidates and can commit accepted staged outputs. |
| EV-008 | code | `control-plane/crates/engine/src/executor.rs:11123-11146` persists staged rows, commits canonical files, then persists committed rows. |
| EV-009 | code | `control-plane/crates/engine/src/executor.rs:11628-11657` wires P090 rollout flags. |
| EV-010 | code | `control-plane/crates/engine/src/executor.rs:11659-11758` maps P090 boundary subtypes. |
| EV-011 | code | `control-plane/crates/engine/src/executor.rs:11761-11857` records preflight phase/facts. |
| EV-012 | code | `control-plane/crates/engine/src/executor.rs:11873-11887` emits `progress_before_handoff` values that diverge from proposal vocabulary. |
| EV-013 | code | `control-plane/crates/acp/src/adapters/junie.rs:81-166` adds enforcement-gated Junie preflight checks. |
| EV-014 | code | `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:322-430` validates and writes settlement rows. |
| EV-015 | code | `control-plane/crates/graphql-server/src/types/run.rs:218-233`, `387-448`, and `647-675` expose P090 summary, receipt, and settlement row fields. |
| EV-016 | tests-found | `control-plane/crates/engine/src/executor.rs:13799-13961`, `14241-14370` cover preflight mapping, rollout flag parsing, subtype helper cases, and staged repair materialization helper behavior. |
| EV-017 | tests-found | `control-plane/crates/acp/src/adapters/junie.rs:269-307` covers adapter preflight project/output checks. |
| EV-018 | tests-found | `control-plane/crates/db/tests/proposal_088_persistence.rs:815-965` covers P090 receipt/readback and settlement-row persistence/idempotency. |
| EV-019 | tests-run | `./scripts/test-gate.sh proposal-090` passed on the audited tree. |
| EV-020 | evidence | `docs/evidence/090/junie-runtime-hardening/evidence-index.json` lists all seven subtype fixtures and concrete negative fixtures. |

## Fidelity Inventory

Matches:

- Additive DB migration preserves legacy compatibility and adds P090 fields (`053_p090...:1-69`).
- Domain and GraphQL expose provider-neutral subtype/readback wrappers and settlement rows.
- The P090 gate validates evidence index fixture paths/SHA-256 and runs focused Rust/API tests.
- Runtime flags now exist for strict final payload, Junie preflight enforcement, staged repair settlement, and staged settlement disable.
- Staged repair helper writes valid candidates to staging paths and avoids canonical mutation until commit.
- Malformed repair siblings are not committed in the focused helper test.

Divergences:

- Provider-authored failure-envelope rejection is not surfaced as `provider_claim_rejected` in DB/GraphQL/MCP/report readback, despite proposal lines 253-259 requiring that operator truth.
- `progress_before_handoff` emits values such as `provider_completed`, `current_attempt_diff_without_handoff`, and `observed`, while proposal lines 644-648 specify `none`, `session_updates_only`, `meaningful_progress`, and `worktree_diff_detected`.
- Staged settlement still lacks proven crash recovery. A crash between the first DB row write and the second committed-row update can leave durable rows marked `staged` after canonical files were written.
- Active artifact pointer publication from accepted settlement rows is represented by `active_pointer_generation_id`, but no active artifact index update or same-boundary publication is shown.
- Final payload capture JSON lacks the proposal's explicit `redacted_text_artifact_path` and distinct durable final-payload artifact requirement.
- Junie preflight implements useful project/output/temp checks, but not the full remediation/capacity/work-queue lifecycle required by P090.
- GraphQL/MCP tests mainly exercise compatibility/non-`none` fields through old P088 paths rather than an end-to-end Junie non-`none` P090 receipt/report agreement.

Ambiguities / Evidence Gaps:

- No P089 capability proof gate was rerun as part of this audit.
- No live long-running Junie refine-like canary was run or found as a required regression gate.
- No transcript-persistence-failure fixture proves completion truth survives transcript storage failure.
- No crash-recovery test exercises staged rows before/after canonical mutation failure windows.
- No implementation evidence shows `provider_claim_rejected` readback beyond evidence fixtures and proposal gate term checks.

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 4 |
| Partially Implemented | 12 |
| Missing | 2 |
| Not Verifiable | 2 |
| Out of Scope | 0 |

## Detailed REQ Audit

| REQ | Requirement | Source | Status | Evidence and Gap |
| --- | --- | --- | --- | --- |
| REQ-001 | Preserve the P089 capability baseline and do not replace/reframe Junie as incapable. | Proposal lines 34-55, 207-217 | Implemented | The implementation keeps Junie ACP paths and does not swap providers. P089 live proof was not rerun; that proof gap is tracked separately in REQ-020. |
| REQ-002 | Strict final completion boundary: valid `CHAINWORKS_OUTPUT`, engine failure envelope, or repair failure envelope; prose is not ordinary success. | Lines 221-235 | Partially Implemented | P090 subtype/final-payload helpers exist (`executor.rs:11659-11871`) and strict mode is flag-gated (`11628-11637`). End-to-end strict narrative rejection is not proven. |
| REQ-003 | Engine owns failure-envelope authority; provider-authored failure claims are untrusted and must read back as engine synthesized or provider claim rejected. | Lines 237-315 | Partially Implemented | ACP test rejects provider-authored failure envelope extraction (`transport.rs:4529`). Domain includes spoof subtype values (`code_writer_completion.rs:296-308`). No runtime/readback field or code path was found for `provider_claim_rejected`; grep finds it only in evidence/gate/proposal material. |
| REQ-004 | Separate final payload capture from full forensic transcript. | Lines 317-352, 713-727 | Partially Implemented | P090 capture JSON is recorded (`executor.rs:11890-11910`) and P088 text-capture records retain redacted paths. The P090 final-payload JSON itself lacks `redacted_text_artifact_path`, and no test proves transcript persistence failure does not affect settlement. |
| REQ-005 | Provider-neutral public `completion_boundary_subtype` wrapper with unknown raw round-trip. | Lines 354-409, 729-740 | Partially Implemented | Domain and GraphQL expose wrappers/fields (`code_writer_completion.rs:198-235`, `348-352`; `run.rs:218-233`). Unknown/raw behavior is not specifically proven for P090 GraphQL/MCP. |
| REQ-006 | Support the seven required Junie subtype values. | Lines 376-398 | Partially Implemented | Known values exist and helper emits all seven in code paths (`executor.rs:11673-11758`). Tests cover partial and narrative repair helper cases, but not all seven values end to end through receipt/API/report. |
| REQ-007 | Per-output repair settlement row schema, receipt linkage, and idempotency. | Lines 411-480 | Partially Implemented | Migration has row shape and uniqueness (`053...:34-69`), repository validates receipt linkage and digest conflict (`code_writer_completion_receipts.rs:322-370`), and DB tests cover idempotency. Validation does not compare row `stage_id` to receipt context, and replay semantics are proven only at DB-row level. |
| REQ-008 | Repair candidates are staged, validated, then materialized per-output without malformed sibling overwrite. | Lines 482-499, 508-513, 791-799 | Partially Implemented | Staging helper writes candidates to a staging path and the focused test confirms canonical outputs are untouched before commit and malformed sibling remains unchanged (`executor.rs:1314-1467`, `14241-14370`). Active artifact pointer publication is not implemented/proven, and crash windows remain. |
| REQ-009 | Crash recovery for staged/committed/failed settlement rows. | Lines 501-506 | Missing | No recovery service or test was found for staged settlement rows. The code persists staged rows, commits files, then persists committed rows (`executor.rs:11123-11146`), leaving an unproven crash window. |
| REQ-010 | Repair payloads prefer output-name keys with canonical paths only as fallback. | Lines 514-525 | Not Verifiable | Existing discovery accepts output names, but no focused P090 test proves repair prompts prefer the named keys. |
| REQ-011 | Final response size budget and truncation classification. | Lines 527-537, 754-762 | Partially Implemented | Capture metadata and subtype mapping handle truncation (`executor.rs:11678-11680`, `11859-11870`). No deterministic large narrative fixture was run end to end through settlement/readback. |
| REQ-012 | Progress-without-handoff is distinct from empty execution/startup failure. | Lines 538-552, 763-770 | Partially Implemented | Subtype helper can return `junie_progress_without_terminal_handoff` (`executor.rs:11741-11757`). The public `progress_before_handoff` value vocabulary diverges from proposal lines 644-648 (`executor.rs:11873-11887`). |
| REQ-013 | Transcript absence must not erase completion-boundary truth. | Lines 553-562, 909-916 | Not Verifiable | Transcript and capture statuses exist, but no fixture or test proves final completion truth and settled outputs survive transcript storage failure. |
| REQ-014 | Junie runtime preflight fail-before-launch and persisted facts. | Lines 563-625, 781-789 | Partially Implemented | Junie adapter preflight checks execution root, readable proof file, output parent writeability, and temp writeability behind `CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE` (`junie.rs:81-166`). Engine maps failure facts to `failed_no_launch` (`executor.rs:11761-11857`). Remediation, provider-capacity timing, and work-queue ack semantics are not proven. |
| REQ-015 | Rollout controls and downgrade behavior. | Lines 972-994 | Partially Implemented | Runtime flag helpers exist (`executor.rs:11628-11657`) and adapter preflight reads the enforcement flag (`junie.rs:93-103`). Tests cover parsing only, not full enabled/disabled/downgrade behavior or startup validation. |
| REQ-016 | Additive receipt/readback fields with historical compatibility. | Lines 627-740 | Implemented | Migration adds nullable/default fields, domain/readback structs carry them, and GraphQL/MCP compatibility tests pass. |
| REQ-017 | Operator/API readback answers provider start, progress, final handoff, truncation, repair, and per-output settlement consistently across GraphQL/MCP/report. | Lines 801-843 | Partially Implemented | Fields exist in domain and GraphQL (`run.rs:218-233`, `387-448`). Tests still mostly use P088 compatibility fixtures with `completion_boundary_subtype = none`; non-`none` Junie agreement across GraphQL/MCP/report is not proven. |
| REQ-018 | Evidence inventory maps every observed subtype and validates concrete negative fixtures. | Lines 57-68, 845-860, 946-970 | Implemented | Evidence index contains all seven subtype fixtures and five negative fixtures with SHA-256; gate validates paths, hashes, schema, and expected semantics (`scripts/test-gate.sh:7080-7127`). |
| REQ-019 | Canonical proposal-090 gate exists and runs focused Rust/API checks. | Lines 950-970 | Implemented | `./scripts/test-gate.sh proposal-090` exists and passed. It runs DB, ACP, engine, GraphQL, and MCP focused tests (`scripts/test-gate.sh:7129-7137`). |
| REQ-020 | Acceptance proof includes unchanged P089 proof and a long-running Junie refine-like canary. | Lines 845-850, 864-871, 972-979 | Missing | No same-tree P089 proof rerun or required long-running Junie refine-like canary execution was found or run. Adapter has an environment-gated live smoke, but it is not the required long-running canary/regression gate. |

## Reviewer Scorecard

| Lens | Conformance | Readiness | Top Risk | Confidence |
| --- | --- | --- | --- | --- |
| Proposal conformance | Not Implemented | Not Ready | Missing canary/recovery/readback commitments prevent closeout. | Medium-High |
| `chainworks_execution_truth_reviewer` | Partial | Not Ready | Provider-authored failure envelope claims are not exposed as `provider_claim_rejected` operator truth. | Medium |
| `rust_reliability_reviewer` | Partial | Not Ready | Staged settlement lacks crash recovery and active pointer publication proof. | High |
| `api_contract_reviewer` | Partial | Not Ready | Public `progress_before_handoff` vocabulary diverges from proposal; non-`none` API agreement is under-tested. | High |
| `observability_rollout_reviewer` | Partial | Not Ready | Flags exist, but full rollout/downgrade/canary behavior is not proven. | Medium-High |

## Routed Specialist Findings

### READY-001 - Missing P089 and long-running Junie canary evidence

- Reviewer: `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related REQs: REQ-001, REQ-020
- Evidence types: proposal, tests-run, tests-found
- Evidence references: proposal lines 849-850 and 864-871; `./scripts/test-gate.sh proposal-090` output; `control-plane/crates/acp/src/adapters/junie.rs:374-380`

Why it matters: P090 explicitly builds on P089 and requires a long-running Junie refine-like canary with fresh settled outputs. The focused P090 gate passed, but it did not rerun P089 and did not execute a long-running Junie canary. An optional environment-gated Junie smoke is not equivalent to the acceptance criterion.

Recommended action: Add the P089 capability proof and a long-running Junie code-writer canary to the P090 readiness path, or document a separate required gate and run it on the audited tree.

Acceptance criteria: same-tree evidence shows P089 proof passes unchanged and the long-running Junie canary completes with fresh settled outputs under P090 hardening.

### REL-001 - Staged settlement still lacks crash recovery and active-pointer publication proof

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-007, REQ-008, REQ-009
- Evidence types: proposal, code, tests-found
- Evidence references: proposal lines 482-506; `control-plane/crates/engine/src/executor.rs:11123-11146`, `1314-1467`, `14241-14370`; `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:372-430`

Why it matters: The implementation now persists staged rows before canonical commit, which is a meaningful improvement. But canonical filesystem writes happen between two DB upserts. If the process dies after `commit_p090_staged_repair_materialization` starts and before committed rows are persisted, recovery can see durable `staged` rows while canonical files may already have changed. The proposal requires crash recovery and active artifact pointers derived from accepted settlement rows.

Recommended action: Add recovery logic for staged/committed/failed rows, active pointer updates derived from accepted rows, and tests that inject failures before commit, during commit, and before the second row update.

Acceptance criteria: a crash during staged settlement cannot promote unaccepted staged files, cannot lose committed canonical truth, and readback reports accepted/failed rows deterministically after restart.

### API-001 - `progress_before_handoff` exposes non-proposal vocabulary

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-012, REQ-016, REQ-017
- Evidence types: proposal, code
- Evidence references: proposal lines 644-648; `control-plane/crates/engine/src/executor.rs:11873-11887`; GraphQL exposure at `control-plane/crates/graphql-server/src/types/run.rs:224-233`, `412-423`

Why it matters: P090 makes `progress_before_handoff` a public readback field. The proposal defines `none`, `session_updates_only`, `meaningful_progress`, and `worktree_diff_detected`. The implementation emits `provider_completed`, `current_attempt_diff_without_handoff`, and `observed`. That creates API drift and makes subtype-aware clients branch on values not in the proposal.

Recommended action: Map runtime facts to the proposal vocabulary or amend the proposal before closeout. Add DB/GraphQL/MCP tests for each public value.

Acceptance criteria: readback never emits values outside the P090 public vocabulary unless the field is explicitly converted to an enum-wrapper known/raw model.

### API-002 - Provider-claim rejection is not exposed as required operator truth

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related REQs: REQ-003, REQ-017, REQ-018
- Evidence types: proposal, code, tests-found
- Evidence references: proposal lines 253-259; `control-plane/crates/acp/src/transport.rs:4529`; `control-plane/crates/domain/src/code_writer_completion.rs:296-308`; grep found `provider_claim_rejected` only in evidence/gate/proposal material.

Why it matters: Transport-level non-extraction prevents a spoofed failure envelope from materializing outputs, but P090 also requires GraphQL/MCP/report readback to say whether an envelope was `engine_synthesized` or `provider_claim_rejected`. Operators still need to know a provider-authored envelope was rejected rather than silently ignored or collapsed into a generic failure.

Recommended action: Persist an envelope-authority/readback field or equivalent receipt fact, populate it when provider-authored envelope-shaped JSON is rejected, and add GraphQL/MCP/report tests for spoof, identity mismatch, and unknown schema.

Acceptance criteria: P090 negative fixtures produce readback containing `provider_claim_rejected` and no materialized outputs.

### REL-002 - Junie preflight is useful but does not implement the full lifecycle

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related REQs: REQ-014, REQ-015
- Evidence types: proposal, code, tests-found
- Evidence references: proposal lines 563-625; `control-plane/crates/acp/src/adapters/junie.rs:81-166`, `269-307`; `control-plane/crates/engine/src/executor.rs:11761-11857`

Why it matters: The adapter now checks project root, proof file, output parent writeability, and temp writeability when enforcement is on. P090 also requires one remediation attempt for cwd/runtime-home failures, capacity acquisition only after preflight passes, work-item ack semantics, and redacted persisted attempts. Those lifecycle pieces are not proven.

Recommended action: Add explicit preflight attempt records, remediation tests for wrong cwd/runtime-home, provider-capacity timing tests, and work-queue ack/fail-before-launch integration tests.

Acceptance criteria: permission-denied fixtures fail before launch, cwd/runtime-home fixtures remediate once, no provider capacity is consumed before passed preflight, and receipts expose attempt/remediation facts.

### API-003 - Final payload capture is not a distinct durable artifact contract

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: Medium
- Related REQs: REQ-004, REQ-011, REQ-013
- Evidence types: proposal, code
- Evidence references: proposal lines 317-352 and 713-727; `control-plane/crates/engine/src/executor.rs:11890-11910`; text capture records at `executor.rs:11433-11468`

Why it matters: P090 separates final extraction input from forensic transcript and requires a redacted final payload artifact path or explicit absence policy. The implementation records capture metadata, but the P090 JSON does not carry `redacted_text_artifact_path`; it relies on P088 text-capture rows for artifact paths. That may be acceptable only if the final payload artifact is explicitly represented and linked from the P090 receipt shape.

Recommended action: Include the redacted final-payload artifact path or a pointer to the exact capture record in `final_completion_payload_capture_json`, and test transcript persistence failure independently.

Acceptance criteria: a receipt identifies the exact bounded final payload artifact used for settlement, even when transcript capture is missing or failed.

### READY-002 - API agreement is not proven for non-`none` Junie P090 readback

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: Medium
- Related REQs: REQ-005, REQ-006, REQ-017, REQ-019
- Evidence types: code, tests-found, tests-run
- Evidence references: `control-plane/crates/graphql-server/tests/proposal_088_code_writer_completion_readback.rs:172-230`; `control-plane/crates/mcp-server/tests/proposal_088_code_writer_completion_readback.rs:170-230`; `./scripts/test-gate.sh proposal-090`

Why it matters: GraphQL and MCP tests pass, but their seed receipts use compatibility-heavy P088 scenarios, mostly with `completion_boundary_subtype = none` and Codex provider values. P090 requires GraphQL, MCP, and run-report agreement for the same non-`none` receipt and settlement rows.

Recommended action: Add a Junie P090 fixture receipt with `junie_repair_outputs_partially_materialized`, settlement rows, preflight facts, and final payload metadata, then assert GraphQL, MCP, and report readback match.

Acceptance criteria: same receipt returns identical P090 subtype, final payload status, preflight facts, repair summary, and settlement rows through all public readback paths.

## Readiness Checklist

| Item | Status | Evidence / Note |
| --- | --- | --- |
| Proposal path resolved | Passed | Markdown exists at target path. |
| Report path generated by helper | Passed | Helper selected R2 path. |
| Prior proposal-review routing reused | Passed | Reused exactly. |
| Focused proposal gate | Passed | `./scripts/test-gate.sh proposal-090` passed on audited tree. |
| Full regression suite | Not run | Not required for a failed verdict; required before any Ready/Implemented verdict. |
| P089 capability proof | Not run / Missing | Explicit acceptance criterion not satisfied in this audit. |
| Long-running Junie refine-like canary | Missing | No required canary run/evidence found. |
| Core service flow integration validation | Partial | Focused unit/API tests pass; no full live runtime flow. |
| Staged settlement crash recovery | Missing | No recovery path/test found. |
| GraphQL/MCP/report agreement for non-`none` P090 subtype | Partial | Compatibility tests pass; Junie non-`none` agreement not proven. |
| UI empty/loading/error/offline states | N/A | No UI scope. |
| Accessibility/localization | N/A | No UI scope. |
| Privacy/permissions/entitlements risk | Partial | Junie preflight covers permission-like failures, but macOS TCC/sandbox remediation behavior is not proven. |
| Rollout/downgrade behavior | Partial | Flags exist; behavior matrix not fully tested. |

## Verification Log

Executed:

```bash
./scripts/test-gate.sh proposal-090
```

Result: Passed.

Observed covered checks:

- Evidence inventory validation passed.
- DB P090 tests passed:
  - `proposal_090_receipt_round_trips_boundary_subtype_and_preflight_readback`
  - `proposal_090_settlement_rows_are_receipt_linked_and_idempotent_by_candidate_digest`
- ACP P090 tests passed:
  - Junie preflight project/output checks.
  - Missing project root fail-before-launch helper.
  - Provider-authored failure envelope not extracted as output.
- Engine P090 tests passed:
  - preflight failure subtype mapping;
  - successful preflight readback without new AgentStatus;
  - rollout flag parsing;
  - partial/narrative repair subtype helper cases;
  - staged repair materialization helper avoids malformed sibling overwrite.
- GraphQL/MCP P088 readback tests with P090 additive fields passed.

Not executed:

- Full regression suite.
- P089 capability gate.
- Live long-running Junie refine-like canary.
- Crash-recovery simulation for staged settlement.

## Final Recommendation

Do not close P090 as Implemented/Ready yet. The implementation is on the right path and the current gate is valuable, but the proposal still requires stronger runtime truth and recovery guarantees than the code proves today.

Recommended next actions:

1. Add `provider_claim_rejected` / `engine_synthesized` readback and tests for provider-authored engine/repair envelope claims.
2. Add staged-settlement crash recovery and active-pointer publication from accepted rows, with failure-injection tests.
3. Align `progress_before_handoff` with the proposal vocabulary or update the proposal before implementation closeout.
4. Expand Junie preflight to remediation, provider-capacity timing, and work-item ack semantics.
5. Add non-`none` Junie P090 GraphQL/MCP/report agreement tests.
6. Run P089 proof and a long-running Junie canary as same-tree readiness evidence.

