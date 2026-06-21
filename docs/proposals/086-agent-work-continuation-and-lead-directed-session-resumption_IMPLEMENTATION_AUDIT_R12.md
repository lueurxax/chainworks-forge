# Implementation Audit R12: Proposal 086 Provider Session Resurrection Completion

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` |
| Proposal title | Proposal 086: Provider Session Resurrection Completion |
| Proposal state | Draft (`Status: Draft`) |
| Audit date | 2026-06-20 |
| Audit report | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R12.md` |
| Repo root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current working tree at `0e6482c8` plus dirty workspace changes |
| Worktree state | Dirty; 194 changed/untracked paths at audit time |
| Audit mode | Read-only implementation audit, except this report file |
| Verdict | **Not Ready / Partially Implemented** |

## Implementation Target And Compare Base

Compare base is the proposal contract in `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md`, not prior audit reports. The audited implementation target is the current local tree because the P086 implementation is present as dirty workspace changes across Rust ACP, MCP, DB, engine, GraphQL, Swift readback, schemas, examples, and test gates.

The target includes the expected implementation surfaces:

- ACP adapter capability and Claude attach request: `control-plane/crates/acp/src/adapters/mod.rs`, `control-plane/crates/acp/src/adapters/claude.rs`, `control-plane/crates/acp/src/manager.rs`
- Frozen catalog parsing and default catalog opt-in: `control-plane/crates/workflow/src/catalog.rs`, `examples/agents/agents.yaml`
- Continuation persistence/replay: `control-plane/crates/db/migrations/065_*`, `079_*`, `081_*`, `083_*`, `085_*`, `control-plane/crates/db/src/repos/agent_work_continuations.rs`
- Worker attach/prompt/settlement path: `control-plane/crates/engine/src/executor.rs`
- MCP/GraphQL readback and access control: `control-plane/crates/mcp-server/src/tools/agents.rs`, `control-plane/crates/graphql-server/src/schema.rs`
- Reference schemas/readback docs: `docs/reference/p086/schemas/*`, `docs/reference/agent-work-continuation.md`

## Prior-Review Reuse

`discover_prior_review.py` found no reusable proposal-review artifacts:

```json
{"artifacts":[]}
```

Existing `IMPLEMENTATION_AUDIT_R*.md` files were ignored for reviewer selection, per skill instructions. They were not used as authority for this verdict.

## Selected Reviewers

| Reviewer lens | Included because | Evidence focus |
|---|---|---|
| Architecture / workflow | P086 changes run continuation semantics and adapter ownership | Explicit modes, frozen catalog truth, live-handle compatibility |
| Reliability / replay | Core requirement is no duplicate prompt and no silent retry after crashes | `resurrection_phase`, side-effect ledger, prompt-sent guards, stale worker recovery |
| API contract | `agents.continue_work`, `agents.attach_receipt.get`, GraphQL receipt readback changed | JSON schemas, MCP admission, GraphQL raw/reviewer/minimal projections |
| Security | Session IDs, raw receipts, subprocesses, transcript recovery, and auth boundaries are sensitive | Principal access matrix, raw receipt storage, redaction, process reaping |
| Observability / rollout | Proposal requires metrics, readback, receipts, and report surfaces | metric events, operator fixture, report/readback fields |
| macOS product scope | SwiftUI is read-only but proposal gate includes Swift readback | `Proposal031ThinGraphQLReadBoundaryTests`, Xcode gate result |

Rejected alternatives: UI/UX deep review and performance-only review were not primary because this proposal is mostly daemon/control-plane behavior. They are covered only where Swift readback or runtime cost affects readiness.

## Product And Platform Scope

P086 is a Rust control-plane implementation with a read-only SwiftUI readback surface. The product behavior under audit is: an operator can issue `agents.continue_work` for a stage-owned `code_writer` execution after the live ACP handle is gone, and Chainworks starts a new managed ACP process attached to the old provider-native session id, proves identity, writes durable evidence, and settles through continuation artifacts rather than normal retry.

## Audited Flows

1. Operator MCP admission for `provider_session_resurrection` validates agent/run/stage/session identity, frozen catalog opt-in, adapter support, unsafe lanes, pending approvals, side effects, and idempotency.
2. Worker claims the continuation row, starts a new Claude ACP process, sends `resumeSessionId`, verifies the returned session id before prompt, writes raw/redacted attach receipt evidence, then sends the mode-reset prompt.
3. Unsupported adapters and mismatched resumed provider sessions fail closed before prompt.
4. `output_only_recovery` uses the same continuation machinery with source-edit prohibition and post-run changed-source detection.
5. Readback exposes continuation status, raw/reviewer/minimal attach receipt projections, metrics, and operator fixture evidence.

## Fidelity Summary

Substantial P086 implementation is present:

- Adapter-owned resurrection capability exists with typed failure classes and Claude support.
- Claude `session/new` includes `resumeSessionId`; manager rejects actual/requested session mismatches before prompt.
- MCP admission uses frozen catalog snapshot truth and runtime adapter support, not static provider-family assumptions.
- Worker uses `resurrection_phase`, side-effect ledger rows, prompt-sent guards, DB-backed raw receipts, redacted filesystem receipt artifacts, and no normal retry fallback.
- Raw receipt access moved to DB-backed storage with audited raw/reviewer/minimal read paths.
- Direct Rust slices pass.

The implementation is not yet proposal-complete:

- Prompt-turn marker correlation is recorded but not enforced before successful settlement.
- Receipt persistence failures after attach are not handled as typed `attach_receipt_persist_failed` fail-closed outcomes with cleanup and `failed_closed` phase.
- Claude session-store recovery has helper-level code and tests, but the P086 resurrection worker/readback path records `not_attempted`/null fields and the canonical P086 gate does not run the session-store recovery tests.
- The canonical proposal gate does not pass on this tree because the Swift readback test runner exits before establishing the XCTest connection.

## Findings

### P1: Settlement accepts an uncorrelated terminal response after resurrection

Proposal requirement: every terminal response or recovered transcript must be correlated with the persisted prompt-turn marker before settlement, and missing/contradictory fields must fail closed (`docs/proposals/086-...md:245-261`, `493-499`, `567-569`).

Implementation writes the marker into the prompt and raw receipt (`control-plane/crates/engine/src/executor.rs:5967-5997`, `7634-7758`), but after `acp.execute` it settles success based only on `result.status == AgentStatus::Completed` (`control-plane/crates/engine/src/executor.rs:7943-8024`). There is no check that the returned terminal text, provider turn id, request id, or recovered transcript contains the prompt marker, request fingerprint, stage execution id, or agent execution id.

The daemon resurrection fixture demonstrates the gap: its terminal response contains only `stopReason` and `sessionId`, while the emitted chunk is just `"resurrection turn ran ./scripts/test-gate.sh proposal-086 passed"` (`control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs:159-164`). The test still expects and observes `row.status == "succeeded"` and `resurrection_phase == "completed"` (`control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs:809-843`).

Impact: a stale or unrelated terminal response from the resumed provider session can be attributed to the current continuation. This violates the proposal's core anti-misattribution safety rule.

Required action: before settlement, require a machine-checkable correlation proof for resurrection terminal output. Accept provider request/turn id when exposed, or require transcript/output to include the prompt marker plus request fingerprint/target ids. Add negative tests where the provider returns success without the marker and with contradictory marker data.

### P1: Attach receipt persistence failures do not fail closed with typed cleanup

Proposal requirement: attach receipt must be persisted before the prompt is sent, and failure classes include `attach_receipt_persist_failed`; fail-closed means no prompt and no fresh retry fallback (`docs/proposals/086-...md:115-127`, `242-244`, `456-457`).

After successful attach, the worker advances to `attached_unprompted`, records provider process binding, and then persists the raw DB receipt and redacted artifact using `?` propagation (`control-plane/crates/engine/src/executor.rs:7601-7815`). If either persistence operation fails, control returns to the generic work-item handler, which only sets continuation `status = 'failed'` with `failure_reason = 'worker_error'` (`control-plane/crates/engine/src/executor.rs:13797-13808`).

That path does not close the newly attached ACP session, does not set `resurrection_phase = 'failed_closed'`, does not record the typed `attach_receipt_persist_failed` failure class, and does not write the required failure evidence. Because the live session was inserted in `AcpRuntimeManager` before receipt persistence (`control-plane/crates/acp/src/manager.rs:531-562`), this can leave an attached session alive after the worker error.

Impact: a persistence failure at the required before-prompt receipt boundary is not a durable P086 fail-closed outcome and can leak a managed provider process/session until later recovery.

Required action: wrap raw/redacted receipt persistence in a local error branch that closes the resurrection session, records `failed_closed` with `attach_receipt_persist_failed`, writes/attempts failure metrics, and settles without prompt. Add a fault-injection test for raw receipt DB failure and redacted artifact write failure.

### P2: Claude session-store recovery is not end-to-end complete for P086 readback

Proposal requirement: Claude session-store recovery is first-class evidence; it must bind recovered transcript content to the target request and record transcript path, digest, read timestamp, latest turn/activity, ownership source, and failure reason (`docs/proposals/086-...md:199-223`, `496-502`, `570-571`).

There is helper code and unit coverage for Claude session-store transcript recovery (`control-plane/crates/acp/src/session.rs:425-492`, `818-978`, `1545-1686`). However, the resurrection attach receipt written by the P086 worker always records `session_store_recovery_result = "not_attempted"` and null transcript fields (`control-plane/crates/engine/src/executor.rs:7760-7785`), and that raw receipt is not updated after a recovered provider result. The canonical P086 Rust gate slice `cargo test -p acp claude_resurrection` ran only the two `claude_resurrection_*` adapter tests, not the `claude_session_store_*` recovery tests.

Impact: the implementation cannot yet prove the proposal's lost-terminal-response recovery path through the full MCP/worker/readback flow. Operators get no session-store path/digest/ownership evidence from P086 receipt/readback, even when helper recovery is used.

Required action: integrate Claude transcript recovery metadata into the P086 worker and raw receipt/readback surfaces, add daemon-level tests that corrupt the ACP terminal response after prompt and settle only from matching transcript evidence, and include those tests in `proposal-086`.

### P2: Canonical proposal gate does not pass on the same tree

Proposal acceptance requires same-tree canonical proposal gate evidence (`docs/proposals/086-...md:525-526`, `577`).

Verification result:

- Default `./scripts/test-gate.sh proposal-086` exited `137` at the first Rust cargo slice under the local gate cargo wrapper. Disk was at 99% with 13 GiB free and the gate cargo target cache was 77 GiB.
- With the cargo wrapper disabled and `CHAINWORKS_PROPOSAL_086_CARGO_TARGET_DIR=target/p086-focused`, the canonical gate reached Swift readback but exited `65`.
- The Swift failure was: `Chainworks Forge ... encountered an error (Early unexpected exit, operation never finished bootstrapping - no restart will be attempted. (Underlying Error: The test runner exited with code 0 before establishing connection.))`

Direct Rust slices passed, but the canonical gate is still red. This blocks `Implemented/Ready` closeout.

Required action: fix or quarantine the Swift readback runner failure through the canonical gate, then rerun `./scripts/test-gate.sh proposal-086` in the same tree without bespoke direct-slice substitution. Also address the local cargo-wrapper kill or document the required wrapper-disabled invocation in the gate if that is the intended supported path.

## Requirements Audit

| ID | Requirement | Status | Evidence |
|---|---|---|---|
| REQ-01 | Explicit distinguishable continuation modes | Partial | Modes exist in domain/MCP schemas; normal retry mode selection/report rejection reasons are not fully audited here. |
| REQ-02 | Adapter-owned capability with failure classes | Met | `ProviderSessionResurrectionCapability` and failure class list in `acp/src/adapters/mod.rs:30-79`. |
| REQ-03 | Claude adapter resumes requested provider session id | Met | `resumeSessionId` in `claude.rs:103-136`; attach identity check in `manager.rs:531-547`. |
| REQ-04 | Frozen catalog opt-in before enqueue | Met | MCP reads frozen catalog and rejects missing/disabled/malformed capability in `tools/agents.rs:1145-1398`; default catalog opts in at `examples/agents/agents.yaml:2053-2058`. |
| REQ-05 | Unsafe lanes, pending approvals, side effects fail closed | Met | Forbidden stage and pending approval/side-effect gates in `tools/agents.rs:1089-1123`, `2231-2329`. |
| REQ-06 | Attach receipt before prompt | Partial | Success path persists raw/redacted receipts before prompt, but persistence failures are generic worker errors rather than typed fail-closed cleanup. |
| REQ-07 | Prompt-turn marker persisted and enforced | Missing | Marker is persisted, but terminal response settlement does not require marker correlation. |
| REQ-08 | No silent fallback to retry | Met for inspected path | Worker settles continuation artifacts and returns; no normal retry enqueue observed in resurrection path. |
| REQ-09 | Output-only recovery forbids source edits and proves no source changes | Partial | Prompt forbids source edits and settlement detects changed source files; receipt is not post-updated with changed-source list/evidence. |
| REQ-10 | Durable `resurrection_phase` without overloading `status` | Met | DB insert/update phase logic and CHECK migrations present; direct DB tests pass. |
| REQ-11 | Crash/replay no duplicate prompt | Partial | Prompt-sent guard exists; phase-specific crash coverage for every required phase was not fully proven by the passing direct slices. |
| REQ-12 | Raw/reviewer/minimal receipt access control | Met | MCP/GraphQL readback paths and DB-backed raw receipt storage are present; tests passed. |
| REQ-13 | Claude session-store recovery and ambiguity evidence | Partial | Helper/unit behavior exists, but not end-to-end in P086 worker/readback/gate. |
| REQ-14 | Metrics/readback/report surfaces distinguish resurrection | Partial | Metrics and fixture readback exist; prompt correlation/session-store gaps mean evidence is incomplete. |
| REQ-15 | P095 minimal continuation prompt | Met | P086 prompt has mode reset/objective/identity fields and no `CHAINWORKS_OUTPUT` instructions. |
| REQ-16 | Same-tree canonical gate passes | Missing | `proposal-086` gate failed at Swift readback. |

## Specialist Coverage Matrix

| Lens | Coverage | Result |
|---|---|---|
| API contract | MCP schemas, handler validation, GraphQL receipt query, fixture checks | Mostly pass; prompt correlation/readback evidence gaps remain |
| Architecture | Adapter capability, manager attach, worker flow, status/phase model | Partial; core shape is right, but settlement correlation is missing |
| Reliability | direct Rust lifecycle/replay tests, process-group reaping, gate result | Partial; duplicate prompt guard exists, receipt-persist failure is weak |
| Security | mandatory scan, raw receipt DB storage, redaction, principal matrix | Partial; raw receipt storage is improved, but uncorrelated settlement is a high-risk integrity gap |
| Observability | metrics summary, operator fixture, raw/reviewer/minimal receipt fields | Partial; session-store and post-prompt receipt evidence incomplete |
| macOS product | proposal Swift readback gate | Failing locally |

## Security Scan

Mandatory `security_sensitive_diff.py` result:

```text
triggered: true
categories:
- auth
- dos_resource_limits
- filesystem_subprocess_boundary
- parser_boundary
- public_ingress
- secrets_redaction_privacy
- unsafe_crypto_dependency
files_count: 194
```

Security-sensitive areas reviewed:

- Raw provider session ids are hashed in mismatch messages (`acp/src/manager.rs:122-127`, `1231-1245`).
- Raw v2 attach receipts are DB-backed, not provider-visible filesystem artifacts (`db/src/repos/p086_resurrection_raw_receipts.rs:1-12`; migration `085_p086_raw_receipt_db_storage.sql:1-13`).
- MCP/GraphQL raw receipt access uses principal-class projections and audit rows.
- Process-group reaping validates UID and actual process group before signaling (`engine/src/recovery.rs:125-240`).

Residual security risk: the P1 prompt-correlation gap is an integrity/authorship problem. It can attribute stale provider output to the wrong continuation target.

## Surface Fingerprint

Mandatory `implementation_surface_fingerprint.py` result:

```text
required_lenses:
- api-contract
- apple-ui-ux
- architecture
- observability-rollout
- performance
- reliability
- security
files_count: 194
```

This matched the reviewer set above. No additional specialist review was required beyond the selected lenses.

## Verification Log

| Check | Result |
|---|---|
| `report_path.py` | Produced `..._IMPLEMENTATION_AUDIT_R12.md` |
| `discover_prior_review.py` | No reusable artifacts |
| `security_sensitive_diff.py --json` | Triggered, categories listed above |
| `implementation_surface_fingerprint.py --json` | Required lenses listed above |
| `./scripts/test-gate.sh proposal-086` default | Failed, exit `137`, no output; trace showed kill at first `cargo test -p domain continuation` under gate cargo wrapper |
| `proposal-086` with wrapper disabled and focused Rust target | Failed, exit `65`; Rust slices ran, Swift readback failed before XCTest connection |
| Direct `CARGO_TARGET_DIR=target/p086-focused cargo test -p domain continuation` | Passed, 4 tests |
| Direct `cargo test -p acp claude_resurrection` | Passed, 2 tests; warnings only |
| Direct `cargo test -p db --test proposal_086_continuation_lifecycle` | Passed, 11 tests |
| Direct `cargo test -p engine --lib p086` | Passed, 7 tests; warnings only |
| Direct `cargo test -p mcp-server "tools::agents"` | Passed, 40 tests; warnings only |
| Direct `cargo test -p graphql-server --test proposal_086_continuation_readback` | Passed, 2 tests |
| Direct `cargo test -p daemon --test proposal_086_mcp_continuation_live_reuse -- --test-threads=1` | Passed, 3 tests |
| Direct Python fixture check for `p086-continuation-readback`/negative/operator fields | Passed: operator fixture exists, `rollout_contract_status=pass`, 37 negative fixtures, no placeholders |

Observed compiler warnings include unused variables/dead code in ACP/engine/MCP/GraphQL. They did not cause direct Rust test failures, but they are cleanup candidates before closeout.

## Readiness Checklist

- [x] Proposal contract read and mapped to implementation surfaces.
- [x] Prior review discovery completed.
- [x] Mandatory security-sensitive diff run.
- [x] Mandatory implementation surface fingerprint run.
- [x] Direct Rust proof slices pass.
- [x] Static operator/negative fixture content is populated.
- [ ] Prompt-turn correlation enforced before settlement.
- [ ] Attach receipt persistence failures produce typed fail-closed cleanup/evidence.
- [ ] Claude session-store recovery is end-to-end through worker/readback and gate.
- [ ] Canonical `./scripts/test-gate.sh proposal-086` passes on the same tree.

## Scorecard

| Category | Score | Notes |
|---|---:|---|
| Functional completeness | 6/10 | Main happy path exists; correlation and session-store recovery incomplete |
| Safety / fail-closed behavior | 5/10 | Strong admission and identity mismatch handling; weak receipt-persist failure and terminal-output attribution |
| Persistence / replay | 7/10 | Phase model and side-effect ledger are solid; phase-specific crash coverage incomplete |
| API/readback | 7/10 | MCP/GraphQL receipt surfaces exist; session-store and post-prompt receipt evidence incomplete |
| Test coverage | 6/10 | Many Rust slices pass; required negative correlation/session-store E2E tests missing; canonical gate red |
| Closeout readiness | 3/10 | Cannot close while P1 findings and gate failure remain |

## Residual Scope And Follow-Up Ownership

Recommended owner: Rust control-plane implementation owner for P086.

Required before another audit:

1. Enforce prompt-turn correlation before any resurrection settlement.
2. Add fail-closed typed handling for raw/redacted receipt persistence failures.
3. Integrate Claude session-store recovery metadata into P086 receipt/readback and add daemon-level tests.
4. Fix the canonical `proposal-086` Swift readback failure and rerun the same-tree gate.

Do not move P086 to closeout until those are complete and the canonical proposal gate passes.

## Final Verdict

P086 is **Partially Implemented / Not Ready**.

The implementation has the right architecture and a passing direct Rust happy-path proof for Claude provider-session resurrection, but it does not yet satisfy the proposal's strongest safety guarantees. In particular, successful settlement is not tied to the persisted prompt-turn marker, receipt persistence failures are not durable typed fail-closed outcomes, session-store recovery is not end-to-end/readback complete, and the canonical same-tree gate is failing.
