# Proposal 086 Implementation Audit R6

| Field | Value |
|---|---|
| Proposal | `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md` |
| Audit timestamp | 2026-05-23 18:41:32 EEST |
| Auditor | Codex, proposal-implementation-audit skill |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b` |
| Branch | `cw/implement-proposal-086-agent-w/976f3d1b` |
| HEAD | `9b79b0667ed9ea0c67659fe4f47e47a60118feab` |
| Working tree | Dirty implementation worktree; audit is against current files at this worktree, not a clean commit |
| Compare base | Implicit current worktree, no PR/base range supplied |
| Proposal state | Draft in proposal metadata; treated as Active because it remains in `docs/proposals/` and was explicitly targeted |
| Overall conformance | Implemented |
| Overall implementation readiness | Ready with Risks |
| Reviewer-selection reuse | Not reused |
| Audit confidence | High for Rust/API/readback contract; Medium for live UI visuals and future provider-adapter resurrection enablement |

## Implementation Target

The implementation target is the supplied worktree:

`/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b`

`git status --short --branch` shows a dirty P086 implementation branch with Rust control-plane, Swift readback, docs/reference, evidence fixtures, schema, and test-gate changes. Existing `IMPLEMENTATION_AUDIT_R1` through `R5` files were present but were not used for reviewer selection or as proof.

## Prior Review Reuse

Reviewer-selection reuse: Not reused.

The helper `discover_prior_review.py` returned no prior proposal-review artifacts for this proposal. Prior implementation audit files are not proposal-review routing artifacts and were ignored for reviewer selection per the skill rule.

Selected reviewers:

- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`
- `macos_ui_reviewer`

Rejected close alternatives:

- `rust_security_reviewer`: relevant to principal enforcement, but the audited changes reuse existing MCP auth classes and the observed risks are primarily API contract/admission validation rather than new secret, PII, unsafe, or public-auth surface.
- `product_reviewer`: Phase 5 expansion/soak metrics are explicitly moved to P093; P086 product-value claims are adequately represented by service-flow conformance and rollout telemetry.
- `performance_reviewer`: no explicit latency/throughput target in P086; write-budget behavior is handled as data/rollout evidence.

## Contract Summary

P086 adds server-owned work continuation for code-writer implementation work. It must remain distinct from retry, output repair, and checkpoint rehydration. The proposal requires:

- MCP `agents.continue_work` for operator continuation and lead-directed automatic continuation.
- Live-handle continuation first; provider-session resurrection must exist as an explicit mode and fail closed when adapter support is absent.
- Strict eligibility checks: same run/stage/agent/session/worktree, continuation-capable role, no unresolved side effects, no release/publish/upload lane, policy limits, and mode-specific checks.
- Durable SQLite metadata, artifact pointers, canonical request fingerprints, idempotent replay/conflict handling, and no duplicate provider send after `prompt_sent`.
- Evidence readback from worktree/transcript/tool/test artifacts without depending on a strict output envelope.
- Read-only GraphQL/SwiftUI visibility with no in-app continue command.
- Lead-auto decision artifacts with server-side target, hash, safety, and policy validation.
- Recovery behavior that reaps orphan ACP subprocesses before stale truth reconciliation or future resurrection.
- Acceptance evidence through proposal gates and rollout fixtures for phases 1-4; P093 owns phase 5 soak/scale evidence.

Platform/product scope:

- Apple: macOS readback UI only; no governed app write command.
- Backend/service: Rust control-plane MCP API, worker, SQLite persistence, recovery, ACP live-session dispatch, GraphQL readback, rollout telemetry.
- Cross-stack scope: MCP command => SQLite admission => work queue/ACP worker => artifacts/metrics => GraphQL/Swift readback.

Primary implementation flows:

1. Operator calls `agents.continue_work` for a code-writer execution; server admits the request, enqueues `ProcessContinuation`, sends one live-session continuation prompt, and writes terminal evidence.
2. Duplicate operator or lead request with the same idempotency key and same canonical fingerprint replays the existing continuation; a different fingerprint rejects as `idempotency_conflict`.
3. Lead emits `lead_continuation_decision_v1`; engine validates target, hash, capability, safety, side effects, approvals, and policy limits before admitting through the same durable transaction.
4. Daemon/worker restart after `prompt_sent` refuses duplicate provider I/O, marks reconciliation, reads worktree/transcript evidence, and settles the existing continuation.
5. SwiftUI reads continuation history and metrics through GraphQL and renders passive readback only.

## Fidelity Inventory

Matches:

- Proposal scope excludes Phase 5 soak and scale evidence; implementation records P093 as the follow-up and the P086 API reference keeps Phase 5 out of closeout (`docs/reference/proposal-086-api-contracts.md:104-106`).
- `continuation_mode` is now canonical in MCP request schemas and server schema, with `mode` only as a deprecated compatibility alias (`docs/reference/p086/schemas/mcp/agents.continue_work.request.schema.json:43-51`, `control-plane/crates/mcp-server/src/tools/agents.rs:93-101`).
- MCP response is bounded admission output; terminal artifact/session fields are exposed by readback (`docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json:5-39`, `control-plane/crates/mcp-server/src/tools/agents.rs:1701-1739`).
- Lead-auto count limits are enforced transactionally and count terminal rows (`control-plane/crates/db/src/repos/agent_work_continuations.rs:921-990`).
- Reconciliation now reads transcript evidence or records explicit absence before settlement (`control-plane/crates/engine/src/executor.rs:5401-5459`, `control-plane/crates/engine/src/executor.rs:5647-5686`).

Divergences:

- The proposal's suggested main migration filename slot did not survive branch reality; implemented migration is `065_p086_agent_work_continuations.sql`, documented as equivalent because earlier slots were occupied (`docs/reference/proposal-086-api-contracts.md:84-87`).
- Provider-session resurrection is explicit but fail-closed for all current adapters. This matches the "unsupported mode fails closed" portion of P086, while adapter-specific attach/resume remains beyond P086 closeout until a provider advertises support (`docs/reference/proposal-086-api-contracts.md:88-104`).

Ambiguities / Evidence Gaps:

- No live macOS screenshot was captured during this audit. UI readiness is supported by Swift readback tests and the canonical gate, not visual runtime evidence.
- Metric events are durable, but the run-level summary currently aggregates through a fixed 500-event list. That is a rollout-quality risk before P093 scale evidence, not a P086 conformance blocker.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 15 |
| Out of Scope | 1 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Detailed REQ Audit

### REQ-001: Work continuation is a distinct operation

- Proposal source: sections 2-3, lines 52-128.
- Status: Implemented.
- Evidence types: proposal, code, migration, tests-run.
- Evidence references: `control-plane/crates/db/migrations/065_p086_agent_work_continuations.sql:11-85`; `control-plane/crates/db/src/work_item.rs` adds `ProcessContinuation`; `control-plane/crates/engine/src/executor.rs:6179-6204`.
- Mapping: Continuations persist as `agent_work_continuations`, execute as `ProcessContinuation`, and call ACP with `reuse_existing_session=true`; this is not ordinary retry/output repair.
- Note: Normal retry/checkpoint paths remain separate.

### REQ-002: Eligibility and fail-closed admission checks

- Proposal source: section 6, lines 205-244.
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references: `control-plane/crates/mcp-server/src/tools/agents.rs:1209-1284`; `control-plane/crates/engine/src/executor.rs:4067-4094`; `control-plane/crates/db/src/repos/agent_work_continuations.rs:801-990`.
- Mapping: Server validates allowed modes/triggers, principal class, continuation capability, live session, forbidden lanes, unresolved side effects, active rows, saturation, and lead-auto limits before provider I/O.
- Note: Unsupported provider-session resurrection rejects before admission.

### REQ-003: Durable continuation data model

- Proposal source: section 8.1, lines 335-380.
- Status: Implemented.
- Evidence types: migration, code.
- Evidence references: `control-plane/crates/db/migrations/065_p086_agent_work_continuations.sql:11-185`; `control-plane/crates/db/migrations/067_p086_continuation_metric_events.sql:7-26`.
- Mapping: Main continuation rows, ordered external side-effect ledger, supervised-worker heartbeat/process binding, and metric events are all durable SQLite structures.
- Note: Migration slot differs from the proposal suggestion but the schema contract is present.

### REQ-004: Canonical request fingerprint and idempotency semantics

- Proposal source: sections 8.2-8.3, lines 383-471.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/db/src/repos/agent_work_continuations.rs:815-880`; `control-plane/crates/mcp-server/src/tools/agents.rs:1742-1760`; `control-plane/crates/db/tests/proposal_086_continuation_lifecycle.rs:230-245`.
- Mapping: Admission checks scope/key/fingerprint inside the transaction, returns replay for same fingerprint, increments conflict evidence, and rejects changed payloads.
- Note: Same-key prompt-sent replay is handled by the worker claim/reconciliation path.

### REQ-005: No duplicate provider send after `prompt_sent`

- Proposal source: sections 8.3-8.4 and test list, lines 469-501 and 1203-1213.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs:5740-5754`; `control-plane/crates/engine/src/executor.rs:5902-5920`; `control-plane/crates/engine/src/executor.rs:6116-6156`.
- Mapping: Provider-send ledger row is inserted before `prompt_sent`; duplicate workers after prompt delivery mark reconciliation and do not call ACP again.
- Note: Reconciliation settles the existing continuation.

### REQ-006: Evidence artifacts and write-budget rules

- Proposal source: sections 8.8-10, lines 539-583.
- Status: Implemented.
- Evidence types: code, schema, migration, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs:4698-4910`; `control-plane/crates/engine/src/executor.rs:4910-5255`; artifact schemas under `docs/reference/p086/schemas/artifacts/`; `control-plane/crates/db/migrations/065_p086_agent_work_continuations.sql:53-64`.
- Mapping: Worker writes worktree readback, evidence bundle, response snapshot, result/no-progress report, and continuation report artifacts, then stores compact artifact IDs/fingerprints in SQLite.
- Note: Transcript contents are represented by artifact payload/sha metadata, not stream-chunk rows.

### REQ-007: Side-effect safety

- Proposal source: section 11, lines 587-605.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs:4082-4094`; `control-plane/crates/engine/src/executor.rs:5988-6127`; `control-plane/crates/db/tests/proposal_086_continuation_lifecycle.rs:120-149`.
- Mapping: Admission blocks unresolved P078 side effects and forbidden stages; worker records attach/runtime/worktree/provider-send/provider-cancel ledger rows in order.
- Note: Release/publish/upload lanes fail closed.

### REQ-008: MCP request/response contract

- Proposal source: section 12.1, lines 609-679.
- Status: Implemented.
- Evidence types: schema, code, tests-run.
- Evidence references: `docs/reference/p086/schemas/mcp/agents.continue_work.request.schema.json:1-80`; `docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json:1-79`; `control-plane/crates/mcp-server/src/tools/agents.rs:1041-1117`; `control-plane/crates/mcp-server/src/tools/agents.rs:1691-1814`.
- Mapping: Request schema uses canonical `continuation_mode`, bounded fields, and `additionalProperties=false`; response is admission-only with accepted/replay/rejected outcomes and bounded `error.data`.
- Note: Deprecated `mode` alias is accepted only as compatibility and must match if both fields are provided.

### REQ-009: Continuation status, candidates, and readback

- Proposal source: sections 12.2-13, lines 681-741.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/graphql-server/src/schema.rs:1160-1234`; `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:3054-3122`; `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:620-655`.
- Mapping: GraphQL exposes read-only continuation status/history/candidates/metrics; Swift decodes and presents readback; tests assert there is no `continueWork` mutation in read documents.
- Note: Runtime screenshot not captured.

### REQ-010: SwiftUI must not invoke continuation

- Proposal source: sections 3 and 13, lines 123-128 and 728-741.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:651-654`; `Chainworks Forge/Views/RunsHomeView.swift:2906-2955`.
- Mapping: UI renders passive latest status/mode/trigger/artifact/metric summary in a readback card; tests assert no continuation write document appears.
- Note: The audited UI is readback-only, not a command surface.

### REQ-011: Lead-directed automatic continuation

- Proposal source: sections 7.2 and 14, lines 297-331 and 745-793.
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs:3992-4242`; `docs/reference/p086/schemas/artifacts/lead_continuation_decision_v1.schema.json`; `control-plane/crates/mcp-server/src/tools/agents.rs:1237-1254`.
- Mapping: Engine scans lead artifacts, validates target/hash/instruction/capability/side effects/approvals/safety, admits through the same transaction, and enqueues `ProcessContinuation`.
- Note: Lead-auto remains live-handle only; resurrection by lead is not enabled.

### REQ-012: Automatic continuation policy limits

- Proposal source: section 7.3, lines 321-331.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/db/src/repos/agent_work_continuations.rs:921-990`; `control-plane/crates/mcp-server/src/tools/agents.rs:1776-1794`; `control-plane/crates/db/tests/proposal_086_continuation_lifecycle.rs:151-228`.
- Mapping: Atomic admission counts all `lead_auto` rows per agent and per stage, including terminal rows, and returns a typed `LeadAutoLimitExceeded` rejection.
- Note: This resolves the earlier count-limit gap.

### REQ-013: Canonical mode reset prompt and live-session reuse path

- Proposal source: section 15, lines 797-831.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs:6165-6204`; `scripts/test-gate.sh:7937-7971`.
- Mapping: Worker builds the P086 mode-reset continuation prompt, calls ACP `execute`, sets `reuse_existing_session=true`, passes the recorded `session_generation_id` / `provider_session_id`, and does not start a new ACP session for live continuation.
- Note: The proposal gate statically enforces prompt and live-reuse needles.

### REQ-014: Recovery, orphan process reaping, and cancellation

- Proposal source: section 7.1 and test list, lines 246-269 and 1196-1213.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/recovery.rs:80-238`; `control-plane/crates/engine/src/executor.rs:5804-5850`; `control-plane/crates/engine/src/executor.rs:5258-5313`; `control-plane/crates/engine/src/executor.rs:5401-5686`.
- Mapping: Recovery uses supervised provider process group identity checks and bounded TERM/KILL, records verified/unverified outcomes, and continuation reconciliation reads worktree/transcript evidence before terminal settlement.
- Note: Cancellation after provider send is preserved as cancellation evidence rather than overwritten by a late provider result.

### REQ-015: Provider-session resurrection fail-closed mode

- Proposal source: sections 2.4, 6, 12.1, and acceptance criteria, lines 86-101, 231-242, 630-635, 1225-1227.
- Status: Implemented.
- Evidence types: code, schema, docs-reference, tests-run.
- Evidence references: `control-plane/crates/mcp-server/src/tools/agents.rs:56-63`; `control-plane/crates/mcp-server/src/tools/agents.rs:1256-1284`; `docs/reference/proposal-086-api-contracts.md:88-104`.
- Mapping: `provider_session_resurrection` is a typed mode, and current unsupported adapters reject with `provider_session_resurrection_unsupported` while recording resurrection metrics. It does not fall back to fresh retry or checkpoint rehydration.
- Note: This covers P086 closeout for adapters without attach support.

### REQ-016: Supported provider-session resurrection attach/resume

- Proposal source: acceptance examples, lines 1193-1202.
- Status: Out of Scope.
- Evidence types: proposal, docs-reference.
- Evidence references: proposal line 11 excludes Phase 5 expansion; `docs/reference/proposal-086-api-contracts.md:104`.
- Mapping: No current adapter advertises provider-session resurrection attach/resume support in this implementation target. Per P086, unsupported adapters fail closed; provider-specific resurrection enablement is explicitly pending beyond P086 closeout.
- Note: This does not block overall conformance because no supported adapter slice exists in the audited implementation.

## Reviewer / Lens Scorecard

| Lens | Result | Top Risk | Confidence |
|---|---|---|---|
| Objective conformance | Pass | Provider-specific resurrection attach remains future gated | High |
| Rust architecture | Pass | Large executor surface, but continuation path is isolated by durable row/work-item model | Medium-High |
| Rust reliability | Pass | Restart/replay/cancel behavior is broad; covered by focused tests and gate | High |
| API contract | Pass | Canonical `continuation_mode` now fixed; alias compatibility must remain documented | High |
| Observability/rollout | Pass with minor risk | Metrics summary caps at 500 events | Medium |
| macOS UI | Pass | No live screenshot; readback covered by Swift tests | Medium |
| Readiness | Ready with Risks | OPS-001 should be handled before P093 scale gate | High |

## Routed Specialist Findings

### OPS-001: Run-level metric summary can undercount long histories

- Reviewer: `observability_rollout_reviewer`
- Severity: Minor
- Confidence: Medium
- Related proposal items: rollout metrics/readback, REQ-013, P093 handoff risk.
- Evidence types: code, telemetry.
- Evidence references: `control-plane/crates/db/src/repos/agent_work_continuations.rs:286-290`; `control-plane/crates/db/migrations/067_p086_continuation_metric_events.sql:7-26`.
- Why it matters: Every metric event is persisted durably, but the summary helper builds run metrics from `list_p086_continuation_metric_events_for_run(pool, run_id, 500)`. P086 closeout and current gates pass, but P093 scale/soak evidence could show misleading rollups if a run produces more than 500 metric events.
- Recommended action: Before P093 scale evidence, replace list-then-fold summary with SQL aggregation over all rows for the run or expose a `truncated=true` flag and total event count in readback.
- Acceptance criteria: A synthetic run with more than 500 P086 metric events reports exact totals, or readback explicitly marks truncation so operators do not treat partial summaries as complete.

No Critical or Major specialist findings remain.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Canonical P086 gate on audited tree/HEAD | Passed | `./scripts/test-gate.sh proposal-086` passed on this worktree at HEAD `9b79b0667ed9ea0c67659fe4f47e47a60118feab` |
| Rust continuation admission/lifecycle tests | Passed | 11 db tests, including lead-auto limits and prompt-sent replay |
| Rust engine P086 tests | Passed | 6 engine tests, including prompt contract and reconciliation |
| MCP/API contract tests | Passed | 33 mcp-server `tools::agents` tests |
| GraphQL readback tests | Passed | `proposal_086_continuation_readback` passed |
| Daemon live reuse integration test | Passed | `proposal_086_mcp_continuation_live_reuse` passed |
| Swift readback tests | Passed | 85 `Proposal031ThinGraphQLReadBoundaryTests` passed; result bundle `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-086-swift-readback-20260523-183806.xcresult` |
| Focused readback fixture gate | Passed | `./scripts/test-gate.sh p086-continuation-readback` |
| Focused negative fixtures gate | Passed | `./scripts/test-gate.sh p086-continuation-negative-fixtures`; all 16 fixtures valid |
| Focused operator report gate | Passed | `./scripts/test-gate.sh p086-continuation-operator-report` |
| UI runtime/screenshot | Not run | Not required for successful conformance; Swift readback tests cover no-write and presentation contract |
| Accessibility/localization/privacy/entitlements | No new blocker found | Read-only SwiftUI card; no new permissions or entitlement surface identified |
| Full repo regression | Not run | Canonical proposal gate was run and passed, satisfying P086 readiness evidence for this audit |

## Verification Log

Commands executed from `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b`:

1. `git status --short --branch` - captured dirty implementation target.
2. `git rev-parse HEAD && git branch --show-current` - captured HEAD and branch.
3. `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` - selected `R6` report path.
4. `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` - no prior proposal-review artifacts found.
5. Focused code/schema/doc reads with `rg` and `nl` for proposal commitments, MCP schemas, MCP implementation, DB admission, engine worker/recovery/reconciliation, GraphQL readback, Swift readback, tests, migrations, and test gates.
6. `./scripts/test-gate.sh proposal-086` - passed. Selected evidence included Rust continuation tests, MCP tests, GraphQL readback, daemon live reuse, and Swift P031 readback tests.
7. `./scripts/test-gate.sh p086-continuation-readback` - passed.
8. `./scripts/test-gate.sh p086-continuation-negative-fixtures` - passed with 16 valid fixtures.
9. `./scripts/test-gate.sh p086-continuation-operator-report` - passed.

## Final Verdict

Overall conformance: Implemented.

Overall implementation readiness: Ready with Risks.

The implementation satisfies the in-scope P086 phases 1-4 contract. The previously blocking gaps around canonical `continuation_mode`, bounded rejection response schema, lead-auto policy limits, and transcript-aware reconciliation are fixed and covered by same-tree gate evidence. The remaining risk is operational: the metric summary cap should be fixed or made explicit before P093 scale/soak evidence, but it does not block P086 closeout.

Recommended next actions:

1. Close out P086 into reference truth once maintainers accept this R6 audit.
2. Track OPS-001 under P093 or a narrow rollout-readback cleanup before scale/soak.
3. Keep provider-specific resurrection attach/resume disabled until an adapter exposes and proves that capability.
