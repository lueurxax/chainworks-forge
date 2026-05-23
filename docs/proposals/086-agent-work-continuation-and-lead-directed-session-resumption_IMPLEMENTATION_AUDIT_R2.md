# Proposal 086 Implementation Audit R2

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` |
| Audit report | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R2.md` |
| Audit timestamp | 2026-05-22 21:54:49 EEST |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b` |
| Implementation target | Current worktree, branch `cw/implement-proposal-086-agent-w/976f3d1b` |
| Audited HEAD | `9b79b0667ed9ea0c67659fe4f47e47a60118feab` |
| Compare base | Implicit current staged worktree |
| Working tree state | Staged implementation changes present; previous audit R1 is untracked and ignored for reviewer reuse |
| Proposal state | Draft, treated as active implementation target because this worktree implements it |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | High for static/code contract findings and focused gate results; medium for live runtime behavior because no end-to-end MCP-to-worker continuation run was executed |

## Implementation Target / Compare Base

This R2 audit rechecks the same worktree requested by the user after additional staged changes landed since R1. The relevant deltas add or update:

- ACP worker dispatch now calls the live-session reuse path.
- `agents.continue_work` request schema and handler now accept run/stage/session/provider IDs, operator instruction, max turns, wall-clock budget, and blockers.
- `lead_auto` now reaches decision-artifact validation instead of unconditional early rejection.
- Frozen catalog continuation opt-in is modeled in the workflow catalog and `examples/agents/agents.yaml`.
- Release/publish/git-push/upload/distribution stage rejection and unresolved side-effect rejection are added.
- The P086 prompt builder now includes an explicit mode-reset section and admitted instruction/budget context.

The implementation remains a Rust control-plane/service change: `domain`, `db`, `engine`, `mcp-server`, `graphql-server`, `workflow`, JSON schemas, fixtures, and reference docs. No SwiftUI command surface was found in scope.

## Prior Proposal-Review Reuse

Reviewer-selection reuse: Not reused.

The prior-review discovery helper returned no proposal-review artifacts. Existing implementation audit R1 is not a proposal-review artifact and was ignored for reviewer selection per the skill rules.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_arch_reviewer` | ACP runtime dispatch, background worker ownership, catalog-driven policy, and daemon crate boundaries are central. |
| `rust_reliability_reviewer` | P086 depends on idempotency, replay, side-effect safety, worker recovery, cancellation, and fail-closed modes. |
| `api_contract_reviewer` | The implementation changes MCP request/response schemas, GraphQL readback, canonical fingerprints, and JSON artifacts. |
| `observability_rollout_reviewer` | The proposal commits evidence readback, metrics, rollout fixtures, recovery proof, and closeout gates. |

Rejected close alternatives:

- `macos_ui_reviewer` / `apple_ux_reviewer`: the proposal explicitly forbids an in-app Continue command; the audited UI scope is read-only GraphQL consumption.
- `rust_security_reviewer`: auth/validation matters, but observed risk is contract/reliability/readiness rather than a distinct security issue requiring a fifth reviewer.
- `performance_reviewer`: write-budget is covered by observability/reliability; no concrete benchmark target is proposed.
- `product_reviewer`: product value depends on the still-incomplete runtime/evidence path, so product metrics are covered under readiness.

## Proposal State And Contract Summary

Proposal 086 is marked `Draft` but its implementation scope is concrete: add server-owned work continuation that is distinct from retry and output repair, supports live-handle continuation first, models provider-session resurrection by provider `session_id`, fails closed when unsupported, supports operator MCP and lead-directed automatic continuation, records Chainworks truth/evidence, and keeps SwiftUI read-only (`docs/proposals/...086...md:6-12`, `:52-127`).

Important commitments:

- Live-handle continuation sends another prompt into an existing `AcpRuntimeManager` live handle and validates provider-session identity (`:86-101`, `:113-121`, `:1193`).
- Provider-session resurrection is explicit and must fail closed if unsupported, without retry/checkpoint fallback (`:92-101`, `:231-242`, `:1194`).
- Lead-directed continuation uses `lead_continuation_decision_v1`; the server validates it before execution (`:297-331`, `:721-762`, `:1195`).
- Eligibility checks include role/capability, session generation, provider session id, run/agent/worktree/runtime compatibility, no unresolved side effects, forbidden release/external stages, policy limits, prompt reset, and mode-specific checks (`:205-244`).
- MCP input includes run/stage/agent/session IDs, continuation mode, optional provider session id, operator instruction, optional budgets, and blockers (`:273-293`, `:609-654`).
- Evidence must include ACP transcript evidence, tool trace, worktree diff, changed files, tests, generated artifacts, and summary while keeping high-volume evidence out of SQLite (`:551-583`).
- Every continuation prompt must explicitly reset away from output-contract repair mode and use the canonical template intent (`:766-867`).
- Agent catalog opt-in via `continuation_capability` is required; absent field disables continuation (`:1038-1075`).
- Metrics and tests are explicitly listed (`:1120-1185`).

## Platform/Product Scope

Apple scope: macOS readback only. SwiftUI must not invoke continuation or render a Continue command.

Backend/service scope: Rust control-plane daemon, MCP API, GraphQL read model, SQLite persistence, ACP runtime manager integration, background worker, recovery/replay, rollout fixtures, and evidence artifacts.

Product scope: reduce wasted agent work by preserving useful provider-session context while preserving operator trust, safety gates, provenance, and readback.

## Primary Implementation Flows

1. Operator admission: Operator calls `agents.continue_work`; server validates schema, identity, role, frozen catalog capability, stage safety, unresolved side effects, idempotency, active continuation, and saturation; it persists a continuation row and enqueues a worker item.
2. Live-handle worker continuation: worker claims the row, validates the recorded live ACP handle/provider session id, writes ordered side-effect rows, materializes canonical request evidence, sends the prompt through ACP live-session reuse, and settles terminal evidence.
3. Lead-directed continuation: caller provides a lead decision artifact id/hash and instruction hash; server verifies artifact bytes and payload before admission.
4. Provider-session resurrection: request is represented as a distinct mode and currently fails closed as unsupported for all adapters.
5. Readback: MCP/GraphQL expose continuation status/candidates/artifact pointers; SwiftUI remains read-only.

## Fidelity And Divergence Inventory

Matches:

- Distinct continuation model and tables exist (`domain/src/continuation.rs:3-80`; `db/migrations/065_p086_agent_work_continuations.sql:11-185`).
- MCP request schema now accepts proposal-relevant operator/session/budget/blocker fields (`mcp-server/src/tools/agents.rs:65-150`; `docs/reference/p086/schemas/mcp/agents.continue_work.request.schema.json:1-94`).
- Server-side additional-properties enforcement matches the tool schema (`mcp-server/src/tools/agents.rs:845-878`, `:2065-2124`).
- The worker now calls `self.acp.execute(...)` with `reuse_existing_session: true`, which routes to `prompt_session` rather than `start_session` (`engine/src/executor.rs:4682-4736`; `acp/src/manager.rs:603-681`, `:733-746`).
- Lead-auto artifact verification is reachable and verifies artifact id, file bytes SHA-256, and instruction hash (`mcp-server/src/tools/agents.rs:676-825`, `:1042-1128`).
- Frozen catalog `continuation_capability` is parsed and declared in the example code writer (`workflow/src/catalog.rs:155-192`, `:201-225`; `examples/agents/agents.yaml:1667-1691`).
- Admission now rejects forbidden release/external stage names and unresolved side effects (`mcp-server/src/tools/agents.rs:540-553`, `:1236-1289`; `db/src/repos/agent_work_continuations.rs:228-250`).

Divergences:

- Lead-auto is validated through an Operator-only MCP command, not an automatic server action directly triggered by a lead-emitted artifact.
- Provider-session resurrection is still unconditionally rejected for all adapters; attach/resume and orphan-reap proof remain unimplemented (`mcp-server/src/tools/agents.rs:1005-1018`; `docs/reference/proposal-086-api-contracts.md:63-75`).
- The prompt includes a mode-reset section, but it is a reduced implementation-specific prompt rather than the full proposal template with all canonical anti-output-repair lines and closeout sections (`engine/src/executor.rs:4011-4098`; proposal `:778-867`).
- Terminal evidence artifacts are still synthetic: successful continuation result payloads contain empty changed files, tests, and provider transcript artifact ids (`engine/src/executor.rs:4181-4302`).
- MCP command response remains an admission receipt (`accepted`/`replay`/`rejected`) rather than the full proposal output with session, provider session, attach receipt, evidence bundle, worktree readback, and continuation report ids (`docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json:1-44`; proposal `:637-654`).
- Some rollout fixtures are stale relative to the current implementation. For example, `lead-decision-missing-or-changed.json` still claims `lead_auto` is unconditionally blocked, while code now reaches artifact validation (`docs/evidence/rollout-contract/p086/negative/lead-decision-missing-or-changed.json:5-18`).

Ambiguities / Evidence Gaps:

- No end-to-end runtime proof was run from MCP admission through worker execution against a real live ACP session.
- No worker-level integration test proves that the P086 worker does not open a fresh ACP session; ACP manager reuse is tested separately.
- Same-worktree and runtime-profile compatibility are represented in admitted context/catalog policy but not fully proven as concrete admission checks.
- Continuation count policy is not clearly implemented beyond active-row exclusion and global queue/concurrency caps.
- Full regression/canonical full gate was not run; no successful readiness verdict is claimed.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 9 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 1 |

Overall conformance is Partial because multiple in-scope requirements are still incomplete or not backed by required live/recovery evidence.

## Detailed Requirement Audit

| ID | Requirement | Status | Proposal source | Evidence and mapping | Gap / note |
|---|---|---|---|---|---|
| REQ-001 | Distinct server-owned continuation primitive separate from retry/output repair/checkpoint rehydration | Implemented | `:52-84`, `:107-127`, `:1199-1204` | Domain modes/statuses, continuation tables, MCP tools, and worker item kind exist | Runtime mode separation is represented; resurrection remains fail-closed. |
| REQ-002 | Operator MCP live-handle continuation uses the existing live ACP session and does not create a fresh session generation | Implemented | `:90-101`, `:113-121`, `:1145-1146`, `:1193` | Worker checks live handle then calls `acp.execute` with `reuse_existing_session: true` (`engine/src/executor.rs:4568-4736`); ACP manager routes reuse to `prompt_session` (`acp/src/manager.rs:603-681`, `:733-746`); ACP reuse tests passed | No end-to-end MCP-to-worker runtime proof was run, so readiness remains blocked, but the code path no longer has the R1 fresh-session defect. |
| REQ-003 | Provider-session resurrection is explicit and fails closed when unsupported, with supported attach/orphan behavior when enabled | Partially Implemented | `:41-48`, `:92-101`, `:231-269`, `:1194` | Mode exists; unsupported requests return `provider_session_resurrection_unsupported` (`mcp-server/src/tools/agents.rs:1005-1018`) | Supported attach/resume, orphan reap evidence, and attach receipt path are not implemented; all adapters remain disabled. |
| REQ-004 | Lead-directed automatic continuation through structured decision artifact | Partially Implemented | `:297-331`, `:721-762`, `:1195` | Lead decision artifact verification is reachable and checks artifact id/file hash/instruction hash (`mcp-server/src/tools/agents.rs:676-825`, `:1042-1128`) | This is still an Operator-only MCP admission path, not automatic consumption of a lead-emitted artifact by orchestration. Stale negative fixture text contradicts the current code. |
| REQ-005 | All eligibility and safety checks fail closed | Partially Implemented | `:205-244`, `:295`, `:1196` | Checks include role/owner/status, expected run/stage/session/provider ids, catalog opt-in, forbidden stage kind, unresolved side effects, pending approvals, active continuation, idempotency, and saturation (`mcp-server/src/tools/agents.rs:827-1428`) | Remaining gaps: same-worktree compatibility, runtime profile/adapter family compatibility, continuation count policy, and explicit compatible continuation family checks are not fully proven. |
| REQ-006 | MCP request/response contract carries operator context, budgets, session IDs, and evidence/readback ids | Partially Implemented | `:273-293`, `:609-654`, `:1172-1185` | Request side now accepts the key proposal fields and includes them in fingerprint/budget context (`mcp-server/src/tools/agents.rs:65-150`, `:1316-1382`) | Response remains an admission receipt and omits the proposal's full output/evidence id set; terminal ids are available later through readback. |
| REQ-007 | Continuation prompts use the canonical mode-reset template | Partially Implemented | `:766-867`, `:1197` | Prompt builder includes a P086 mode-reset header, not-retry/output-repair/checkpoint wording, instruction, blockers, bounds, and safety rules (`engine/src/executor.rs:4011-4098`); focused engine test passed | It does not reproduce the full proposal template, including exact `This is NOT output-contract repair` wording, `CHAINWORKS_OUTPUT` prohibitions, known-completed/review findings sections, and closeout requirements. |
| REQ-008 | Durable data model, lifecycle, atomic idempotency replay/conflict | Implemented | `:335-548`, `:1205` | Migration includes continuation, ledger, worker tables; atomic admission handles replay/conflict/active/saturation (`db/migrations/...:11-185`; `db/src/repos/agent_work_continuations.rs:411-577`) | Focused DB lifecycle tests passed. |
| REQ-009 | Duplicate after `prompt_sent` never resends and uses reconciliation | Partially Implemented | `:475-506`, `:1176-1181`, `:1207-1208` | Provider-send ledger row is inserted before `prompt_sent`; duplicate provider send moves to `needs_continuation_reconciliation`; claim refuses prompt-sent replay | Reconciliation evidence-window settlement is still documented as pending (`docs/reference/proposal-086-api-contracts.md:75`). |
| REQ-010 | Evidence/readback truth captures transcript/tool trace/worktree diff/changed files/tests/artifacts while respecting write budget | Partially Implemented | `:551-583`, `:1199-1200` | Terminal artifacts are materialized and SQLite stores artifact pointers (`engine/src/executor.rs:4181-4302`) | Result payloads still contain empty changed-files/tests/transcript arrays; no real transcript/tool/worktree/test capture was proven. |
| REQ-011 | GraphQL read-only UI inspection; no GraphQL mutation or SwiftUI invocation | Implemented | `:123-127`, `:675-717`, `:1201` | GraphQL exposes read-only continuation status/candidates; no mutation or SwiftUI command surface found (`graphql-server/src/schema.rs:1146-1202`) | Evidence richness is covered under REQ-010 and REQ-006. |
| REQ-012 | Release and side-effect stages fail closed | Implemented | `:587-605`, `:1198` | Forbidden stage kind matching and unresolved `side_effects` lookup reject admission (`mcp-server/src/tools/agents.rs:540-553`, `:1236-1289`; `db/src/repos/agent_work_continuations.rs:228-250`); focused tests passed | Stage-kind detection is string-based but covers the committed forbidden lane names. |
| REQ-013 | Agent catalog `continuation_capability` opt-in controls eligibility; absent field disables continuation | Implemented | `:1038-1075` | Workflow catalog structs and example `code_writer` opt-in exist; MCP admission checks frozen catalog capability (`workflow/src/catalog.rs:155-192`, `:201-225`; `examples/agents/agents.yaml:1667-1691`; `mcp-server/src/tools/agents.rs:556-674`, `:1258-1266`) | Old snapshots without the field fail closed as intended. |
| REQ-014 | Metrics and observability track continuation volume, avoided fresh sessions, progress/no-progress, tests/files, trigger success, budget, orphan reap, attach success/failure | Partially Implemented | `:1120-1137` | Some runtime/readback fields and queue/concurrency caps exist; docs acknowledge most metrics/log correlation remain pending (`docs/reference/proposal-086-api-contracts.md:75`) | The proposal's metrics list is not fully implemented. |
| REQ-015 | Required tests and evidence gates cover the proposal's critical behavior list | Partially Implemented | `:1141-1185` | Proposal gate, fixtures, DB lifecycle, ACP manager reuse, workflow catalog, and prompt unit test passed | Missing coverage includes end-to-end worker live-session continuation, no-new-generation at worker level, automatic lead orchestration, real worktree readback, supported resurrection, orphan recovery, cancellation proof, reconciliation settlement, and full regression. |
| REQ-016 | Phase 5 expansion/soak | Out of Scope | `:10-11`, `:131-135` | Proposal 093 split exists | Correctly excluded from P086 audit. |

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
|---|---|---|---|---|
| Proposal conformance | Partial | Not Ready | Evidence/recovery/lead automation remain incomplete | High |
| Rust architecture | Mostly aligned | Not Ready | Worker reuse path is fixed, but no worker-level live-session integration proof exists | Medium |
| Rust reliability | Partial | Not Ready | Reconciliation, cancellation, recovery, and continuation count limits remain incomplete | High |
| API contract | Partial | Not Ready | Request side improved; response/readback contract still omits full evidence ids and some schema/docs are stale | High |
| Observability/rollout | Partial | Not Ready | Evidence artifacts and fixtures are not yet live-runtime proof | High |

## Routed Specialist Findings

### REL-001: No worker-level proof that P086 continuation reuses the live ACP session end to end

Reviewer: `rust_reliability_reviewer`  
Severity: Major  
Confidence: Medium  
Related requirements: REQ-002, REQ-015  
Evidence types: code, tests-run  
Evidence references: `engine/src/executor.rs:4682-4736`; `acp/src/manager.rs:603-681`, `:733-746`

Why it matters: The R1 architectural defect is fixed in code, and ACP manager reuse is tested. The remaining readiness gap is that no test drives `agents.continue_work` admission through `ProcessContinuation` into a live fake ACP session and asserts no fresh session is opened.

Recommended action: Add a daemon/engine integration test with a fake live ACP handle registered under a session generation. Admit continuation, drain the worker item, and assert `prompt_session`/reuse was used and no fresh session generation was created.

Acceptance criteria: Test proves reused provider session id, `reused_existing_session=true`, no new generation, and fail-closed behavior for missing/mismatched live handle.

### API-001: `agents.continue_work` response/readback contract is still narrower than the proposal output

Reviewer: `api_contract_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-006, REQ-010, REQ-011  
Evidence types: proposal, schema, code  
Evidence references: proposal `:637-654`; `docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json:1-44`; `graphql-server/src/types/continuation.rs:57-155`

Why it matters: The request side now matches the proposal much better, but callers still do not receive or read the full proposal output shape: session generation id, provider session id, attach receipt, evidence bundle, worktree readback, and continuation report ids are not first-class in the command response and are only partly visible through GraphQL.

Recommended action: Either extend response/readback to expose the committed fields or revise the proposal/API reference to define `agents.continue_work` as an admission receipt plus a separate full readback query.

Acceptance criteria: Contract tests validate the accepted/replay response and readback together provide every proposal output field or the proposal explicitly documents the split contract.

### REL-002: Lead-directed continuation is validated but not automatic

Reviewer: `rust_reliability_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-004, REQ-015  
Evidence types: proposal, code  
Evidence references: proposal `:297-331`, `:721-762`; `mcp-server/src/tools/agents.rs:676-825`, `:827-1128`

Why it matters: The implementation now validates lead decision artifacts when `trigger_kind=lead_auto` is supplied to MCP, but the proposal describes a lead emitting a decision artifact and the server validating it before execution. There is no orchestration path that watches/consumes the lead artifact automatically and enqueues continuation without an Operator MCP command.

Recommended action: Add the lead decision ingestion path or explicitly re-scope P086 to "operator submits a lead decision artifact through MCP."

Acceptance criteria: A test shows a lead-produced `lead_continuation_decision_v1` artifact causes server-side validation and admission under policy, or the proposal/reference is changed to the MCP-submitted artifact model.

### REL-003: Recovery/reconciliation/cancellation lifecycle remains incomplete

Reviewer: `rust_reliability_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-003, REQ-009, REQ-014, REQ-015  
Evidence types: code, docs, tests-run  
Evidence references: `docs/reference/proposal-086-api-contracts.md:71-75`; `db/src/repos/agent_work_continuations.rs:580-633`; `engine/src/executor.rs:4640-4665`

Why it matters: P086 explicitly includes crash windows, no-resend reconciliation, orphan ACP recovery, and cancellation proof. The no-resend guard exists, but the durable evidence-window settlement, heartbeat refresh/release, cancellation worktree-lease termination proof, and orphan-reap/attach path are still pending.

Recommended action: Complete the recovery paths before implementation closeout.

Acceptance criteria: Tests cover crash after prompt delivery, prompt-sent replay without provider resend, reconciliation from transcript/worktree evidence, cancellation timeout/termination proof, daemon restart orphan reap success/failure, and provider resurrection fail-closed on unverified reap.

### OPS-001: Evidence artifacts are materialized but still synthetic

Reviewer: `observability_rollout_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-010, REQ-014, REQ-015  
Evidence types: code, tests-run  
Evidence references: proposal `:551-583`; `engine/src/executor.rs:4181-4302`

Why it matters: P086's value depends on Chainworks truth being more than a lifecycle receipt. The current success artifact writes generic summary text and empty changed-files/tests/provider-transcript lists. That does not prove the continuation captured the actual worktree diff, tool trace, transcript, tests, or generated artifacts.

Recommended action: Wire actual ACP transcript/tool spool references, worktree diff/readback, changed file manifest, tests/gates, and provider summary into terminal evidence.

Acceptance criteria: A live or integration continuation produces non-empty evidence for a worktree-changing continuation and stores only compact pointers/metadata in SQLite.

### OPS-002: Rollout fixtures/docs are stale relative to the current code

Reviewer: `observability_rollout_reviewer`  
Severity: Minor  
Confidence: High  
Related requirements: REQ-004, REQ-015  
Evidence types: docs, code  
Evidence references: `docs/evidence/rollout-contract/p086/negative/lead-decision-missing-or-changed.json:5-18`; `mcp-server/src/tools/agents.rs:1042-1128`; `docs/reference/proposal-086-api-contracts.md:63-75`

Why it matters: The gate still passes because it checks fixture presence/shape, but at least one fixture describes a pre-change behavior: unconditional `lead_auto_unsupported`. The code now validates lead artifacts. Stale evidence can make closeout look stronger or weaker than the actual implementation.

Recommended action: Update negative fixtures and reference docs so evidence descriptions match current behavior.

Acceptance criteria: Fixture text names current failure modes, includes a valid lead-artifact acceptance/negative hash proof, and no longer says verification code is unreachable.

### READY-001: P086 is not ready for closeout despite major R1 fixes

Reviewer: `observability_rollout_reviewer`  
Severity: Critical  
Confidence: High  
Related requirements: REQ-003, REQ-004, REQ-009, REQ-010, REQ-014, REQ-015  
Evidence types: code, docs, tests-run  
Evidence references: `docs/reference/proposal-086-api-contracts.md:63-75`; verification log below

Why it matters: The core live-session dispatch issue is fixed, but P086 still lacks full lead automation, live evidence capture, reconciliation/recovery/cancellation proof, metrics, provider resurrection attach/orphan behavior, and full regression evidence.

Recommended action: Treat this as a substantially improved partial landing, not closeout.

Acceptance criteria: All in-scope REQs are Implemented, live end-to-end continuation proof exists, fixtures/docs match behavior, and `./scripts/test-gate.sh full` or the repository-approved sign-off gate passes on the audited tree.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Build or canonical proposal gate status | Pass for focused P086 gates | `./scripts/test-gate.sh proposal-086` passed |
| Core service flow runtime/integration validation | Partial | Code uses live reuse path; ACP reuse tests passed; no end-to-end MCP-to-worker runtime proof |
| MCP contract validation | Partial | Request contract improved; response/readback still narrower than proposal |
| GraphQL read-only status | Pass for no-mutation boundary | Read-only continuation status/candidates exist; no mutation found |
| UI/UX empty/loading/error/offline/permission states | Not applicable | No SwiftUI command surface in scope |
| Accessibility/localization | Not applicable | No UI implementation change audited |
| Privacy/permissions/entitlements | Partial | Operator-only command boundary and Observer redaction tests exist; lead automation path still unclear |
| Critical tests executed | Pass for focused tests | Proposal gate, fixture gates, DB lifecycle, ACP reuse, workflow catalog, engine prompt unit test passed |
| Required proposal tests fully covered | No | Missing live worker end-to-end, automatic lead orchestration, worktree readback, supported resurrection, orphan recovery, cancellation proof, reconciliation settlement |
| Full regression or canonical full sign-off passed on audited tree | No | Not run; readiness remains Not Ready |

## Verification Log

| Command | Result | Notes |
|---|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py <proposal>` | Pass | Returned R2 report path |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py <proposal>` | Pass, no artifacts | Reviewer reuse: Not reused |
| `./scripts/test-gate.sh proposal-086` | Pass | P086 migration/schema/domain/MCP gate passed; 30 MCP agent unit tests passed |
| `./scripts/test-gate.sh p086-continuation-readback && ./scripts/test-gate.sh p086-continuation-negative-fixtures && ./scripts/test-gate.sh p086-continuation-operator-report` | Pass | Fixture/readback/operator-report gates passed |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p db --test proposal_086_continuation_lifecycle` | Pass | 6 DB continuation lifecycle tests passed |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p workflow --test proposal_066_toolchain_cache_policy` | Pass | 13 workflow tests passed after catalog struct change |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p acp --test integration test_runtime_manager_reuses_live_session_handle` | Pass | Proves ACP manager reuse path prompts an existing live session |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p acp --test integration test_runtime_manager_healthcheck_rejects_exited_live_session` | Pass | Proves exited live session is rejected |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p engine p086_continuation_prompt_uses_mode_reset_contract` | Pass | Prompt builder unit test passed |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p acp --test integration test_runtime_manager_reuses_live_session_handle test_runtime_manager_healthcheck_rejects_exited_live_session` | Invalid invocation | Cargo accepts one test filter; rerun as two separate commands above and both passed |
| `./scripts/test-gate.sh full` | Not run | Required before any Ready/Implemented closeout verdict |

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

R2 materially improves on R1. The most important R1 blocker, fresh-session dispatch from the continuation worker, is fixed in code. MCP request coverage, lead-artifact validation, catalog opt-in, side-effect/release rejection, and prompt reset are also substantially better.

The implementation is still not closeout-ready. Remaining blockers are evidence fidelity, reconciliation/recovery/cancellation, metrics, provider-session resurrection beyond fail-closed unsupported mode, automatic lead-directed ingestion, stale rollout fixtures, and missing end-to-end live worker proof.

Recommended next actions:

1. Add a live end-to-end MCP admission → worker → reused ACP prompt integration test.
2. Decide whether lead-auto means automatic server ingestion of lead artifacts or operator-submitted lead artifacts through MCP, then align proposal/reference/code/tests.
3. Replace synthetic terminal evidence with real transcript/tool/worktree/test artifacts.
4. Complete prompt-sent reconciliation, cancellation proof, heartbeat refresh/release, and restart/orphan recovery evidence.
5. Update stale P086 negative fixtures and reference docs.
6. Run the repository's full/canonical sign-off gate before claiming P086 closeout readiness.
