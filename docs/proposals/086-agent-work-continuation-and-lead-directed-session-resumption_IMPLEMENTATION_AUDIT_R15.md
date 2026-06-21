# P086 Implementation Audit R15 - Provider Session Resurrection Completion

## Metadata

| Field | Value |
|---|---|
| Proposal | [086-agent-work-continuation-and-lead-directed-session-resumption.md](086-agent-work-continuation-and-lead-directed-session-resumption.md) |
| Proposal title | Proposal 086: Provider Session Resurrection Completion |
| Audit type | Implementation audit |
| Audit round | R15 |
| Audit timestamp | 2026-06-20T21:57:14Z / 2026-06-21T00:57:14+0300 |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Target revision | `0e6482c82b588b74a76294a225e68286bfe37fa4` plus local working-tree changes |
| Proposal status | Draft |
| Prior proposal-review reuse | Not reused; helper search returned no applicable prior review artifacts |
| Overall conformance | Partially Implemented |
| Overall readiness | Not Ready |
| Audit confidence | High for the routed findings and gate result; medium for exhaustive behavioral coverage because the tree is dirty with unrelated P079/P080/P083 work |

## Target And Compare Base

The audit target is the current dirty working tree. The compare base is the P086 proposal contract, especially:

- scope/non-goal/goal at proposal lines 10-12;
- mode taxonomy and no silent fallback at lines 57-90;
- adapter capability contract at lines 92-130;
- Claude resurrection and session-store evidence at lines 164-223;
- generic resurrection flow at lines 225-262;
- output-only recovery at lines 264-289;
- durable `resurrection_phase` replay contract at lines 291-365;
- data/readback/metrics at lines 369-438;
- safety rules at lines 440-457;
- tests and acceptance criteria at lines 459-577.

The worktree contains unrelated modified/untracked files from adjacent proposals. This audit only evaluates P086 surfaces and does not treat older untracked P086 audit reports as implementation evidence.

## Prior Review Reuse

No prior proposal-review artifacts were reused. The discovery helper returned an empty artifact set for this proposal.

## Reviewer Selection

| Reviewer | Selected | Reason |
|---|---:|---|
| `chainworks_execution_truth_reviewer` | Yes | Mandatory repo-local lens for run/stage/agent/ACP/MCP/recovery truth |
| `rust_reliability_reviewer` | Yes | Crash/replay, prompt duplication prevention, side-effect ledger, process reaping |
| `api_contract_reviewer` | Yes | MCP/GraphQL/schema/report/readback compatibility |
| `rust_security_reviewer` | Yes | Raw receipt access, provider session ids, auth, subprocess/env boundaries |
| `observability_rollout_reviewer` | Yes | Metrics, rollout fixtures, operator evidence gates |

Rejected due to the five-reviewer cap: `rust_arch_reviewer` (covered by execution truth + API/reliability), `macos_ui_reviewer` and `apple_ux_reviewer` (Swift scope is passive readback and the proposal is primarily Rust control-plane), `rust_performance_reviewer` (no P086 performance SLO is decisive here), and `product_reviewer` (proposal acceptance criteria are the product contract for this audit).

## Proposal State And Scope

P086 asks to complete `provider_session_resurrection`: start a new Chainworks-managed ACP process, attach/resume a known provider session id for adapters that support it, verify the resumed id before prompt, preserve fail-closed unsupported behavior, and expose enough durable evidence to distinguish this from retry. P093 owns soak/scale only and must not absorb resurrection implementation.

The implementation has the core provider-session resurrection path in place and the current same-tree gates pass. It is still not ready for closeout because two proposal-level contracts remain unowned: the resurrection phase API is stringly typed and has drifted from the proposal vocabulary, and the output-only recovery source-edit allowance subcase is implemented as fail-closed despite being specified as required evidence.

## Platform And Product Scope

Primary scope is the Rust control-plane daemon: ACP adapters, engine continuation worker, SQLite migrations/repos, MCP tools, GraphQL readback, rollout fixtures, and daemon tests. Swift/macOS scope is read-only continuation presentation through the P031 read boundary and a passive Runs card. No Swift mutation surface is in scope.

## Primary Flows Audited

1. `agents.continue_work` admission for `provider_session_resurrection`, `live_handle_continuation`, and `output_only_recovery`.
2. Frozen catalog gating for `code_writer.continuation_capability`.
3. Claude ACP adapter capability and `resumeSessionId` attach request.
4. Engine provider-session attach, identity proof, v2 receipt persistence, prompt marker, provider-send ledger, terminal correlation, and settlement.
5. DB replay state, deadlines, terminal idempotency, cancellation, and process ownership.
6. MCP/GraphQL attach receipt readback and redaction.
7. Operator/readback fixture and negative rollout-contract gates.

## Fidelity And Divergence

### Implemented Or Mostly Implemented

- The ACP adapter boundary defines `ProviderSessionResurrectionCapability` with provider family, adapter id, capability version, request/result shapes, identity proof, write safety, and failure classes in `control-plane/crates/acp/src/adapters/mod.rs:30-79`; unsupported adapters default to no capability at `mod.rs:881-895`.
- Claude declares support through `claude-agent-acp`, `resumeSessionId`, and `session/new.result.sessionId` in `control-plane/crates/acp/src/adapters/claude.rs:57-70`, and injects `resumeSessionId` into `session/new` at `claude.rs:103-136`.
- The runtime attach path opens a fresh session, reads the provider-returned session id, compares requested vs actual, closes on mismatch, and only inserts the session into live state after identity proof in `control-plane/crates/acp/src/manager.rs:495-573`.
- MCP admission uses runtime adapter capability rather than static provider-name support in `control-plane/crates/mcp-server/src/tools/agents.rs:1229-1247`.
- Frozen catalog gating rejects missing/invalid snapshots, disabled continuation capability, trigger mismatch, missing provider session id, and unsupported adapter before enqueue in `tools/agents.rs:1249-1504`.
- MCP admission validates IDs, input caps, run/stage/session/provider matches, stage eligibility, unresolved side effects, pending approvals, and atomic idempotent admission in `tools/agents.rs:1889-2561`.
- The engine attach path uses a fresh `p086-resurrection-*` session, no prompt during attach, `reuse_existing_session=false` for attach, then writes v2 raw/redacted attach receipts before provider send in `control-plane/crates/engine/src/executor.rs:7608-7995`.
- Provider-send is recorded before prompt, the canonical request artifact is persisted, `prompting`/`prompt_sent` are durable, and the resumed session executes with `reuse_existing_session=true` in `executor.rs:8063-8178`.
- Terminal settlement rejects completed results missing prompt marker, request fingerprint, stage execution id, or agent execution id correlation in `executor.rs:8234-8286`.
- The v2 receipt schema requires the proposal's key identity/proof/process/correlation fields in `docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json:8-53`.
- MCP/GraphQL attach receipt readback now performs the same run-scope check for all principal classes before returning raw, reviewer, or guest projections in `tools/agents.rs:528-565` and `control-plane/crates/graphql-server/src/schema.rs:3295-3327`.
- Rollout/readback companion gates pass: `p086-continuation-readback`, `p086-continuation-negative-fixtures`, and `p086-continuation-operator-report`.

### Divergences

- `resurrection_phase` is stringly typed in the Rust domain/repo/API path despite the proposal requiring a typed Rust enum. `ContinuationRecord` stores `resurrection_phase: Option<String>` at `control-plane/crates/domain/src/continuation.rs:148-161`, raw receipt readback also stores it as `Option<String>` in `control-plane/crates/db/src/repos/p086_resurrection_raw_receipts.rs:26`, and `update_resurrection_phase` accepts `phase: &str` at `control-plane/crates/db/src/repos/agent_work_continuations.rs:1289-1321`.
- The implementation widened `resurrection_phase` with `cancelling`, but the proposal allowed list and compatibility mapping only include `admitted`, `launching`, `launched`, `attaching`, `attached_unprompted`, `prompting`, `settling`, `completed`, and `failed_closed`. Evidence: proposal lines 315-341 vs migration `control-plane/crates/db/migrations/081_p086_resurrection_phase_cancelling.sql:79-96` and schema enum `provider_session_attach_receipt_v2.schema.json:89-102`.
- Output-only recovery can use provider-session resurrection when no live handle exists (`executor.rs:7488-7508`), but the DB phase updater only writes rows with `mode = 'provider_session_resurrection'` (`agent_work_continuations.rs:1303-1310`). That leaves an unresolved contract question for phase readback on output-only-over-resurrection rows.
- The proposal requires a deliberately allowed output-only source-edit request to record explicit operator allowance and changed files; current admission rejects catalog `source_edit_allowance=true` as unsupported in `tools/agents.rs:1422-1437`, and the unit test locks that behavior at `tools/agents.rs:3483-3512`.

## Residual Scope And Follow-up Ownership

P086 still owns both routed findings below. They are not P093 soak/scale work and are not covered by an explicit follow-up proposal. The implementation can become ready either by implementing the missing contract pieces or by revising P086 before closeout so the proposal, reference docs, gates, and implementation agree.

## Requirement Summary

| REQ | Proposal area | Status | Notes |
|---|---|---|---|
| REQ-01 | Explicit mode taxonomy and no silent fallback | Mostly Implemented | Modes are explicit; ordinary retry/reuse taxonomy is less visible than resurrection/recovery but no silent fallback was observed |
| REQ-02 | ACP adapter capability contract | Implemented | Runtime capability and Claude support are present |
| REQ-03 | Frozen catalog gate | Implemented | Admission uses frozen snapshot and adapter capability |
| REQ-04 | Claude provider-session resurrection | Implemented | Fresh ACP child, requested/actual id proof, mismatch fail-closed, tests pass |
| REQ-05 | Generic resurrection flow and safety gates | Mostly Implemented | Target checks, side-effect checks, pending approvals, prompt marker, provider-send ledger present |
| REQ-06 | Output-only recovery | Partially Implemented | No-source-change path exists; source-edit-allowed subcase is fail-closed and unowned |
| REQ-07 | Durable state/replay with `resurrection_phase` | Partially Implemented | DB state exists, but Rust enum/type contract and phase vocabulary drift from proposal |
| REQ-08 | Data/evidence/readback | Mostly Implemented | v2 receipts, DB raw storage, MCP/GraphQL access, Swift readback, and fixtures pass |
| REQ-09 | Metrics/readback | Mostly Implemented | Durable counters and summary fields exist; no blocking metric gap found in this pass |
| REQ-10 | Safety/security | Mostly Implemented | No current Major/Critical SEC blocker found after readback/run-scope/redaction checks |
| REQ-11 | Tests and gates | Implemented for current gate set | Canonical and companion P086 gates pass on this tree |

## Detailed Requirement Notes

### REQ-01 - Mode Architecture

The Rust domain has typed `ContinuationMode` values for `live_handle_continuation`, `provider_session_resurrection`, and `output_only_recovery` at `domain/src/continuation.rs:3-34`; MCP validates those modes at `tools/agents.rs:1980-1985`. The implementation keeps resurrection and output-only separate from normal retry, and I did not find evidence of an automatic fresh retry fallback.

### REQ-02 - Adapter Capability

Implemented. The capability declaration and failure classes match the proposal minimum in `adapters/mod.rs:30-79`, and Claude is the first supported adapter in `claude.rs:57-70`. Unsupported providers remain absent from `provider_session_resurrection_capability_for_provider` at `adapters/mod.rs:81-93`.

### REQ-03 - Frozen Catalog Gate

Implemented. Admission decodes frozen catalog JSON and fails closed for missing catalog, invalid JSON, missing `code_writer`, disabled root capability, trigger mismatch, disabled mode subtree, missing provider session id, and unsupported adapter in `tools/agents.rs:1249-1504`.

### REQ-04 - Claude Resurrection

Implemented for the audited path. The manager obtains the actual provider session id after `session/new`, compares it to the requested id, closes on mismatch, and returns attach evidence at `manager.rs:531-573`. The daemon integration gate passed the resurrection success, identity mismatch, uncorrelated terminal response, attach receipt persistence failure, and Claude session-store recovery tests.

### REQ-05 - Generic Resurrection Flow

Mostly implemented. MCP validates target identifiers and safety gates before atomic admission at `tools/agents.rs:1889-2561`. The engine attaches, writes the receipt before prompt, records provider send, sends the prompt once, correlates terminal output, and settles via continuation artifacts at `executor.rs:7608-8370`.

### REQ-06 - Output-only Recovery

Partially implemented. Output-only recovery uses a shorter prompt that forbids source edits at `executor.rs:6009-6053`, records changed source files and fails the continuation if source files changed at `executor.rs:6278-6296`, and can route through provider-session resurrection when the live handle is gone at `executor.rs:7488-7508`. It does not implement the proposal's deliberately allowed source-edit request/receipt subcase; see REQ-OUTPUT-001.

### REQ-07 - Durable State And Replay

Partially implemented. Migrations add DB phases, deadlines, timeout classes, terminal idempotency, cancellation support, raw receipt storage, and deadline invariants. However, the Rust API remains stringly typed for phases, and `cancelling` exists as an implementation-only phase without proposal mapping. This blocks closeout until the contract is reconciled.

### REQ-08 - Data And Readback

Mostly implemented. `provider_session_attach_receipt_v2` requires the major identity/process/correlation/session-store fields (`provider_session_attach_receipt_v2.schema.json:8-53`). Raw receipt access is DB-backed and run-scoped before any projection is returned in MCP/GraphQL. Swift readback passed the targeted P086 test.

### REQ-09 - Metrics

Mostly implemented. The canonical gate statically checks durable metric helpers and summary fields in `scripts/test-gate.sh:10119-10135` and GraphQL metric fields in `scripts/test-gate.sh:10202-10210`; those checks passed in the full gate.

### REQ-10 - Security

Mostly implemented. I inspected raw receipt access after the security-sensitive scan flagged auth/public-ingress/session-id/subprocess surfaces. MCP and GraphQL now check the requested run for all principal classes before raw, reviewer, or guest projections (`tools/agents.rs:528-565`, `schema.rs:3295-3327`). Input caps and UUID validation are present in `tools/agents.rs:1889-1969`. Raw DB receipt storage and child-env isolation are documented in `docs/reference/agent-work-continuation.md:117-121`.

### REQ-11 - Tests

Implemented for the current gate set. The canonical gate definition runs static preflight, Rust domain/ACP/DB/engine/MCP/GraphQL/daemon tests, and Swift readback in `scripts/test-gate.sh:9885-10317`. The companion P086 gates cover operator readback fixtures and negative fixture inventory in `scripts/test-gate.sh:10319-10498`.

## Reviewer Scorecard

| Reviewer | Score | Rationale |
|---|---:|---|
| `chainworks_execution_truth_reviewer` | 4/5 | Runtime truth is strong, but phase typing/vocabulary drift blocks closeout |
| `rust_reliability_reviewer` | 4/5 | Same-tree gates pass and replay/idempotency coverage exists; string phase API remains fragile |
| `api_contract_reviewer` | 3/5 | Schemas/readback pass gates, but `resurrection_phase` vocabulary is not aligned with the proposal |
| `rust_security_reviewer` | 4/5 | No current Major/Critical security blocker found; receipt access and redaction are run-scoped |
| `observability_rollout_reviewer` | 4/5 | Companion rollout/readback fixture gates pass; closeout still blocked by contract drift |

## Security-sensitive Diff Scan

The security-sensitive scan is triggered by MCP/GraphQL ingress, auth checks, provider session identifiers, raw receipt storage/redaction, subprocess/session attach, child process env isolation, and parser/input boundaries.

Manual security review completed for:

- MCP and GraphQL attach receipt run-scope checks;
- raw/reviewer/guest projection separation;
- provider session id redaction and hashed lower-privilege projections;
- UUID and max-length validation before DB writes;
- child process env/DATABASE_URL isolation per reference doc;
- stale ACP process ownership/reap evidence;
- prompt marker and terminal correlation.

No Major or Critical security finding remains from this pass. Residual security risk is tied to the API contract finding: stringly typed phases make future replay/security policy changes easier to desynchronize.

## Routed Findings

### REQ-PHASE-001 - `resurrection_phase` is stringly typed and has drifted from the proposal vocabulary

- Severity: Major
- Owner: Execution truth / API contract
- Proposal evidence: P086 requires `resurrection_phase` to have a DB `CHECK` constraint, typed Rust enum, MCP/GraphQL/report readback, and receipt mirroring at lines 310-313. The allowed proposal phases are listed at lines 315-327, and the compatibility mapping at lines 331-341 does not include `cancelling`.
- Implementation evidence:
  - `ContinuationRecord` stores `resurrection_phase: Option<String>` at `control-plane/crates/domain/src/continuation.rs:148-161`.
  - Raw receipt context stores `resurrection_phase: Option<String>` in `control-plane/crates/db/src/repos/p086_resurrection_raw_receipts.rs:26`.
  - `update_resurrection_phase` accepts `phase: &str` and binds it directly to SQL at `control-plane/crates/db/src/repos/agent_work_continuations.rs:1289-1321`.
  - `rg` found no `ResurrectionPhase`/`ProviderSessionResurrectionPhase` enum in `domain`, `db`, `engine`, `mcp-server`, or `graphql-server`.
  - Migration `081_p086_resurrection_phase_cancelling.sql` widens the DB CHECK to include `cancelling` at lines 79-96.
  - The public v2 receipt schema includes `cancelling` in the phase enum at `docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json:89-102`.
- Impact: The replay/readback contract is no longer the one P086 specifies. Future code can pass arbitrary phase strings until SQLite rejects them, and clients may observe a public `cancelling` phase with no proposal-defined status compatibility or replay rule.
- Required action: Introduce a typed `ResurrectionPhase` Rust enum and use it across domain, DB repo APIs, engine transitions, MCP/GraphQL/report DTOs, and schemas. Also either remove `cancelling` from the resurrection phase vocabulary or revise P086/reference docs/gates to define `cancelling` as a supported phase with compatibility and replay semantics.

### REQ-OUTPUT-001 - The specified source-edit-allowed output-only recovery subcase is fail-closed, not implemented

- Severity: Major
- Owner: Execution truth / API contract
- Proposal evidence: Output-only repair must forbid source edits unless operator instruction allows them at lines 274-275, and if source edits are intentionally allowed, the request and receipt must explicitly say so and list changed source files at lines 284-289. Required test 16 says a deliberately allowed source-edit request must record the explicit allowance and changed file list at lines 506-508.
- Implementation evidence:
  - MCP admission rejects catalog `output_only_recovery.source_edit_allowance=true` with `output_only_source_edit_allowance_not_supported` in `control-plane/crates/mcp-server/src/tools/agents.rs:1422-1437`.
  - The unit test asserts that rejection in `tools/agents.rs:3483-3512`.
  - The engine records `source_edit_allowance = !output_only_recovery` in the attach receipt at `control-plane/crates/engine/src/executor.rs:7859-7866`, so output-only recovery cannot produce an allowed-source-edit receipt.
  - Output-only settlement fails when changed source files are detected at `executor.rs:6278-6296`.
- Impact: The safe fail-closed behavior is acceptable operationally, but it does not satisfy the proposal's required source-edit-allowed evidence path. Full-implementation closeout cannot treat this as implemented unless the proposal is revised to make source-edit-allowed output-only recovery a non-goal or explicitly owned follow-up.
- Required action: Either implement explicit operator allowance in the MCP request/catalog/admission, prompt, worktree diff summary, receipt schema, report/readback, and tests; or revise P086 before closeout to state that output-only recovery always forbids source edits and allowed source edits are out of scope.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Proposal file read | Pass | Proposal lines cited above |
| Prior review reuse checked | Pass | No artifacts found |
| Relevant implementation surfaces inspected | Pass | ACP, MCP, engine, DB, GraphQL, schemas, docs, gates |
| Security-sensitive diff hard gate | Pass with caveat | No current Major/Critical SEC blocker; phase contract drift remains |
| Specialist coverage hard gate | Pass | Five reviewer lenses applied |
| Canonical proposal gate | Pass | `bash -x ./scripts/test-gate.sh proposal-086` exited 0 after full gate pass |
| Companion P086 gates | Pass | Readback, negative fixtures, operator-report gates all passed |
| Full-implementation tail gate | Fail | REQ-PHASE-001 and REQ-OUTPUT-001 remain unowned |
| Ready for closeout | Fail | Contract and proposal/gate alignment required |

## Verification Log

| Command | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` | Pass; no prior review artifacts |
| `test ! -e docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R15.md` | Pass before report creation |
| `./scripts/test-gate.sh proposal-086` | Initial normal invocation exited 137 with no output; rerun with shell trace to obtain concrete gate evidence |
| `bash -x ./scripts/test-gate.sh proposal-086` | Pass; static preflight, Rust domain/ACP/DB/engine/MCP/GraphQL/daemon tests, and Swift P086 readback completed; Swift readback reported one passing test |
| `./scripts/test-gate.sh p086-continuation-readback` | Pass |
| `./scripts/test-gate.sh p086-continuation-negative-fixtures` | Pass; all 37 fixtures present and valid |
| `./scripts/test-gate.sh p086-continuation-operator-report` | Pass |

## Verdict And Required Actions

Verdict: Not Ready.

The core resurrection implementation is now substantially present and the relevant same-tree gates pass, including daemon resurrection and Swift readback proof. The remaining blockers are contract alignment blockers, not missing broad plumbing:

1. Make `resurrection_phase` a typed Rust enum and reconcile the `cancelling` phase with the P086 proposal vocabulary and compatibility mapping.
2. Implement or explicitly de-scope the source-edit-allowed output-only recovery subcase required by P086.

After those changes, rerun `./scripts/test-gate.sh proposal-086` plus the three companion P086 gates and update the audit with the same-tree results.
